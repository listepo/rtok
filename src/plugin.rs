//! The plugin contract (plan §1, T0.4). Every token-reduction method implements [`Plugin`].
//!
//! The contract itself lives in the published `rtok-plugin-sdk` crate (D25) and is re-exported
//! here, so `rtok::plugin::*` keeps resolving and an out-of-tree plugin and an in-tree one
//! implement the same types. What stays here is the host side: [`Ctx`], which owns the config
//! and the store, and the [`Plugin`] trait until T23.3 puts the host behind capability traits.
//!
//! Rules (AGENTS.md): fail open, lossless by default, and a saving that is not a
//! [`Measurement`] row does not exist. Default method bodies do nothing, so a plugin
//! implements only the surfaces it declares in its [`Manifest`].

use anyhow::Result;

use crate::config::Config;
use crate::proxy::wire::WireRequest;
use crate::store::Store;
use crate::tokens::{self, Class};

pub use rtok_plugin_sdk::{
    DashboardPage, Injection, Manifest, Measurement, PostToolUse, PreCompact, PreToolDecision,
    PreToolUse, PromptSubmit, SessionStart, Surface, ToolDef,
};

/// Everything a plugin may touch: config, the store, and the session id.
/// The archive store is added in T3.1.
pub struct Ctx {
    /// Merged configuration for this run.
    pub config: Config,
    /// The one SQLite file (D8).
    pub store: Store,
    /// Host session id; every measurement is attributed to it.
    pub session: String,
    /// The `calls` row this dispatch runs under (the API request in the proxy), when the
    /// surface has one. `record_call` / `record_plugin_run` nest their rows under it.
    pub call_id: Option<i32>,
}

impl Ctx {
    /// Open the store at `config.core.db_path`.
    pub fn open(config: Config, session: impl Into<String>) -> Result<Self> {
        let store = Store::open(&config.core.db_path)?;
        Ok(Self {
            config,
            store,
            session: session.into(),
            call_id: None,
        })
    }

    /// Default config + in-memory store, for tests and examples.
    pub fn in_memory(session: impl Into<String>) -> Result<Self> {
        Ok(Self {
            config: Config::default(),
            store: Store::open_in_memory()?,
            session: session.into(),
            call_id: None,
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

    /// A `plugin_run` row for `plugin`, nested under [`Ctx::call_id`] when set.
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
        self.store
            .upsert_session(&self.session, None, None, None, None)?;
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

/// One token-reduction method. Implement the surfaces your [`Manifest`] declares and leave
/// the rest to the no-op defaults. External crates implement this too and register through
/// [`Registry::from_plugins`](crate::plugins::Registry::from_plugins).
///
/// Two methods are required: [`Plugin::manifest`] says what the plugin is, and
/// [`Plugin::dashboard_page`] is the page every operator surface renders for it (D23). A
/// plugin that implements only the first does not compile:
///
/// ```compile_fail
/// use rtok::plugin::{Manifest, Plugin, Surface};
/// struct Half;
/// impl Plugin for Half {
///     fn manifest(&self) -> Manifest {
///         Manifest { id: "half", surfaces: &[Surface::Cli], default_on: false }
///     }
/// }
/// ```
pub trait Plugin: Send + Sync {
    /// Id, surfaces and default state. Called on every dispatch; keep it cheap.
    fn manifest(&self) -> Manifest;

    /// The page this plugin contributes to `rtok web` and `rtok tui` — the same one, rendered
    /// twice (D23). Required: nothing else knows what the plugin does well enough to write it.
    fn dashboard_page(&self) -> DashboardPage;

    /// May deny or rewrite the tool call. `None` = no opinion.
    fn pre_tool(&self, _ev: &PreToolUse, _cx: &Ctx) -> Option<PreToolDecision> {
        None
    }

    /// May only add `additionalContext`; tool results cannot be changed here.
    fn post_tool(&self, _ev: &PostToolUse, _cx: &Ctx) -> Option<String> {
        None
    }

    /// Text to offer at session start; the `inject` plugin decides what fits the budget.
    fn session_start(&self, _ev: &SessionStart, _cx: &Ctx) -> Option<Injection> {
        None
    }

    /// Text to offer with a user prompt; budgeted the same way as [`Plugin::session_start`].
    fn prompt_submit(&self, _ev: &PromptSubmit, _cx: &Ctx) -> Option<Injection> {
        None
    }

    /// Last chance to persist state before the transcript is compacted.
    fn pre_compact(&self, _ev: &PreCompact, _cx: &Ctx) {}

    /// Tools this plugin adds to `rtok mcp`.
    fn mcp_tools(&self) -> Vec<ToolDef> {
        Vec::new()
    }

    /// Rewrite the selected wire's normalised tool results; return one measurement per change.
    fn proxy_filter(&self, _req: &mut WireRequest<'_>, _cx: &Ctx) -> Vec<Measurement> {
        Vec::new()
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

    #[test]
    fn log_survives_db_failure() {
        let cx = Ctx::in_memory("s").unwrap();
        cx.store.set_query_only().unwrap();
        cx.log("error", "plugin", "read", "boom");
    }
}
