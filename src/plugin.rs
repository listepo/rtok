//! The plugin contract (plan §1, T0.4). Every token-reduction method implements [`Plugin`].
//!
//! The contract itself lives in the published `rtok-plugin-sdk` crate (D25) and is re-exported
//! here, so `rtok::plugin::*` keeps resolving and an out-of-tree plugin and an in-tree one
//! implement the same types. What stays here is the host side: [`Runtime`], which owns the config
//! and the store, and the [`Plugin`] trait until T23.3 puts the host behind capability traits.
//!
//! Rules (AGENTS.md): fail open, lossless by default, and a saving that is not a
//! [`Measurement`] row does not exist. Default method bodies do nothing, so a plugin
//! implements only the surfaces it declares in its [`Manifest`].

use std::collections::HashSet;

use anyhow::Result;

use crate::config::Config;
use crate::store::Store;
use crate::tokens;

pub use rtok_plugin_sdk::{
    Archive, ArchiveDecision, Capabilities, Class, Ctx, DashboardPage, Host, Injection, Ledger,
    Manifest, Measurement, NoteHit, Notes, Plugin, PostToolUse, PreCompact, PreToolDecision,
    PreToolUse, PromptSubmit, ReadCache, SessionStart, Surface, Symbols, ToolDef, ToolResultRef,
    ToolResults, WireRequest,
};

/// Everything a plugin may touch: config, the store, and the session id.
/// The archive store is added in T3.1.
pub struct Runtime {
    /// Merged configuration for this run.
    pub config: Config,
    /// The one SQLite file (D8).
    pub store: Store,
    /// Host session id; every measurement is attributed to it.
    pub session: String,
    /// The `calls` row this dispatch runs under (the API request in the proxy), when the
    /// surface has one. `record_call` / `record_plugin_run` nest their rows under it.
    pub call_id: Option<i32>,
    /// Resolved once from `[hook] host` (same shape as `proxy::ProxyState::new`); an unknown
    /// slug falls back to `other` (6) rather than leaving the session row unattributed.
    host_id: Option<i32>,
    /// Host process cwd, when the surface knows it (the hook surface sets this from the
    /// event's own `cwd`, T25.0). `project` is derived from it at write time.
    pub cwd: Option<String>,
}

impl Runtime {
    /// Open the store at `config.core.db_path`.
    pub fn open(config: Config, session: impl Into<String>) -> Result<Self> {
        let store = Store::open(&config.core.db_path)?;
        let host_id = store.host_id(&config.hook.host)?.or(Some(6));
        Ok(Self {
            config,
            store,
            session: session.into(),
            call_id: None,
            host_id,
            cwd: None,
        })
    }

    /// Default config + in-memory store, for tests and examples.
    pub fn in_memory(session: impl Into<String>) -> Result<Self> {
        let config = Config::default();
        let store = Store::open_in_memory()?;
        let host_id = store.host_id(&config.hook.host)?.or(Some(6));
        Ok(Self {
            config,
            store,
            session: session.into(),
            call_id: None,
            host_id,
            cwd: None,
        })
    }

    /// Estimated token count for `text` (±15 %, no tokenizer, no network).
    pub fn estimate(&self, text: &str, class: Class) -> u32 {
        tokens::estimate(text, class, &self.config.estimator)
    }

    /// Persist a measurement for this session (the only path for savings into the DB).
    pub fn record(&self, m: &Measurement) -> Result<()> {
        self.store.insert_measurement(&self.session, m)
    }

    pub fn record_call(&self, surface: &str, kind: &str, name: Option<&str>) -> Result<i32> {
        self.insert_call(surface, kind, None, name)
    }

    /// A `plugin_run` row for `plugin`, nested under [`Runtime::call_id`] when set.
    pub fn record_plugin_run(&self, surface: &str, plugin: &str) -> Result<i32> {
        self.insert_call(surface, "plugin_run", Some(plugin), None)
    }

    fn insert_call(
        &self,
        surface: &str,
        kind: &str,
        plugin: Option<&str>,
        name: Option<&str>,
    ) -> Result<i32> {
        let project = project_of(self.cwd.as_deref());
        self.store.upsert_session(
            &self.session,
            self.host_id,
            project.as_deref(),
            self.cwd.as_deref(),
            None,
        )?;
        let id =
            self.store
                .insert_call(&self.session, surface, kind, None, None, None, plugin, name)?;
        if let Some(parent) = self.call_id {
            self.store.set_call_parent(id, parent)?;
        }
        Ok(id)
    }

    pub fn record_tokens(
        &self,
        call_id: i32,
        plugin: Option<&str>,
        phase: &str,
        source: &str,
        tokens: i64,
    ) -> Result<()> {
        self.store
            .insert_tokens(call_id, plugin, phase, source, tokens)
    }

    /// Never returns `Err` to a plugin (fail open). On DB error, append to `log_file`.
    pub fn log(&self, level: &str, source: &str, name: &str, message: &str) {
        if let Err(e) = self.store.insert_log(
            level,
            source,
            name,
            message,
            Some(&self.session),
            None,
            None,
        ) {
            let path = &self.config.core.log_file;
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                use std::io::Write;
                let _ = writeln!(f, "{level} {source}/{name}: {message} ({e})");
            }
        }
    }
}

/// The basename of the nearest ancestor of `cwd` holding `.git` (T25.0). The walk itself is
/// `config::layers::git_root` — one helper, and already the no-subprocess one the fail-open
/// hook path needs.
fn project_of(cwd: Option<&str>) -> Option<String> {
    let root = crate::config::layers::git_root(std::path::Path::new(cwd?))?;
    root.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// The host side of the contract (D25). `Runtime` *is* the host: every capability trait is
/// implemented here by delegating to the one store, and the session and the archive
/// directory come from the context rather than from the plugin's arguments.
///
/// A plugin sees only these traits, which is what lets `rtok-plugin-sdk` stay three
/// dependencies deep while this crate carries SQLite and tree-sitter.
impl Host for Runtime {
    fn session(&self) -> &str {
        &self.session
    }

    fn estimate(&self, text: &str, class: Class) -> u32 {
        Runtime::estimate(self, text, class)
    }

    fn record(&self, m: &Measurement) -> Result<()> {
        Runtime::record(self, m)
    }

    fn record_call(&self, surface: &str, kind: &str, name: Option<&str>) -> Result<i32> {
        Runtime::record_call(self, surface, kind, name)
    }

    fn record_plugin_run(&self, surface: &str, plugin: &str) -> Result<i32> {
        Runtime::record_plugin_run(self, surface, plugin)
    }

    fn record_tokens(
        &self,
        call_id: i32,
        plugin: Option<&str>,
        phase: &str,
        source: &str,
        tokens: i64,
    ) -> Result<()> {
        Runtime::record_tokens(self, call_id, plugin, phase, source, tokens)
    }

    fn log(&self, level: &str, source: &str, name: &str, message: &str) {
        Runtime::log(self, level, source, name, message);
    }

    /// One configuration section by dotted path. An unknown path is `Null`, which
    /// deserializes to the plugin's own defaults rather than to an error.
    fn config_json(&self, path: &str) -> serde_json::Value {
        let mut node = match serde_json::to_value(&self.config) {
            Ok(v) => v,
            Err(_) => return serde_json::Value::Null,
        };
        for key in path.split('.') {
            node = match node.get_mut(key) {
                Some(v) => v.take(),
                None => return serde_json::Value::Null,
            };
        }
        node
    }

    fn call_id(&self) -> Option<i32> {
        self.call_id
    }
}

impl Archive for Runtime {
    fn put_archive(&self, body: &[u8]) -> Result<String> {
        self.store
            .put_archive(&self.session, body, &self.config.core.archive_dir)
    }

    fn get_archive(&self, id: &str) -> Result<Option<Vec<u8>>> {
        self.store.get_archive(id)
    }

    fn archive_decision(&self, tool_use_id: &str) -> Result<Option<ArchiveDecision>> {
        self.store.archive_decision(tool_use_id)
    }

    fn put_archive_decision(
        &self,
        tool_use_id: &str,
        archive_id: &str,
        pointer: &str,
    ) -> Result<()> {
        self.store
            .put_archive_decision(tool_use_id, archive_id, &self.session, pointer)
    }

    fn mark_expanded(&self, archive_id: &str) -> Result<usize> {
        self.store.mark_expanded(archive_id)
    }
}

impl Notes for Runtime {
    fn insert_note(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<i32> {
        self.store.insert_note(project, kind, title, body)
    }

    fn latest_note(&self, kind: &str) -> Result<Option<String>> {
        self.store.latest_note(kind)
    }

    fn list_note_titles(&self, project: Option<&str>, limit: u32) -> Result<Vec<(i32, String)>> {
        self.store.list_note_titles(project, limit)
    }

    fn get_note_body(&self, id: i32) -> Result<Option<String>> {
        self.store.get_note_body(id)
    }

    fn search_notes(&self, query: &str, limit: u32) -> Result<Vec<NoteHit>> {
        self.store.search_notes(query, limit)
    }
}

impl ReadCache for Runtime {
    fn put_read_cache(&self, path: &str, sha256: &str, archive_id: Option<&str>) -> Result<()> {
        self.store
            .put_read_cache(&self.session, path, sha256, archive_id)
    }

    fn get_read_cache(&self, path: &str) -> Result<Option<(Option<String>, i64)>> {
        self.store.get_read_cache(&self.session, path)
    }

    fn clear_read_cache(&self, path: &str) -> Result<()> {
        self.store.clear_read_cache(&self.session, path)
    }
}

impl Ledger for Runtime {
    fn recent_hook_inputs(&self, limit: i64) -> Result<Vec<String>> {
        self.store.recent_hook_inputs(&self.session, limit)
    }

    fn calls_since(&self, ts: i64) -> Result<i64> {
        self.store.calls_since(&self.session, ts)
    }
}

impl Symbols for Runtime {
    fn symbol_count(&self, root: &str) -> Result<i64> {
        self.store.symbol_count(root)
    }

    fn symbol_stat(&self, root: &str, path: &str) -> Result<Option<(String, i64, i64)>> {
        self.store.symbol_stat(root, path)
    }

    fn touch_symbols(&self, root: &str, path: &str, mtime: i64, size: i64) -> Result<()> {
        self.store.touch_symbols(root, path, mtime, size)
    }

    fn replace_symbols(
        &self,
        root: &str,
        path: &str,
        file_sha: &str,
        stat: (i64, i64),
        rows: &[(String, String, i32, bool, i32, String)],
    ) -> Result<usize> {
        self.store.replace_symbols(root, path, file_sha, stat, rows)
    }

    fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize> {
        self.store.delete_symbols_missing(root, keep)
    }

    fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        self.store.mark_symbols_stale(abs_path)
    }

    fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        self.store.symbol_defs(root, name)
    }

    fn symbol_ref_groups(&self, root: &str, name: &str) -> Result<Vec<(String, String, i64, i32)>> {
        self.store.symbol_ref_groups(root, name)
    }

    fn symbol_impact(
        &self,
        root: &str,
        name: &str,
        depth: u32,
    ) -> Result<Vec<(u32, String, String)>> {
        self.store.symbol_impact(root, name, depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One contract, not a copy: `rtok::plugin::*` re-exports the published crate (D25), so
    /// these assignments only compile while both paths name the same type.
    #[test]
    fn re_exports_are_the_sdk_types() {
        let m = rtok_plugin_sdk::Manifest {
            id: "ext",
            surfaces: &[Surface::Mcp],
            default_on: true,
        };
        let m: Manifest = m;
        assert_eq!(m.id, "ext");
        let page: DashboardPage = rtok_plugin_sdk::DashboardPage::new("Ext", "external.", false);
        assert_eq!(page.title, "Ext");
    }

    /// D25's whole claim in one assignment: an out-of-tree plugin implements
    /// `rtok_plugin_sdk::Plugin` and the host accepts it, because there is only one trait.
    #[test]
    fn the_trait_is_the_published_one() {
        struct Ext;
        impl rtok_plugin_sdk::Plugin for Ext {
            fn manifest(&self) -> Manifest {
                Manifest {
                    id: "ext",
                    surfaces: &[Surface::Mcp],
                    default_on: false,
                }
            }
            fn dashboard_page(&self) -> DashboardPage {
                DashboardPage::new("Ext", "out of tree.", false)
            }
        }
        let p: &dyn Plugin = &Ext;
        assert_eq!(p.manifest().id, "ext");
    }

    #[test]
    fn log_survives_db_failure() {
        let cx = Runtime::in_memory("s").unwrap();
        cx.store.set_query_only().unwrap();
        cx.log("error", "plugin", "read", "boom");
    }
}
