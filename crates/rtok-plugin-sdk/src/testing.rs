//! A host you can run a plugin against without a database.
//!
//! Plugins are written to be tested, and a test needs a [`Ctx`](crate::Ctx), which needs something that
//! implements every capability trait. [`MemoryHost`] is that something: it keeps the
//! [`Measurement`]s a plugin records and the blobs it archives, in memory, and answers every
//! other capability empty.
//!
//! Empty is a legal answer, not a stub: a cache may be cold, a note may be absent, an index
//! may not have been built yet, and a plugin has to survive all three. If a plugin needs a
//! capability to actually hold state, implement [`Host`] and the traits it needs yourself —
//! there is nothing privileged about this one.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;

use crate::Measurement;
use crate::host::{
    Archive, ArchiveDecision, Class, Host, Ledger, NoteHit, Notes, ReadCache, Symbols,
};

/// An in-memory host for tests, examples and doctests.
///
/// ```
/// use rtok_plugin_sdk::testing::MemoryHost;
/// use rtok_plugin_sdk::{Class, Ctx, Measurement};
///
/// let host = MemoryHost::new();
/// let cx = Ctx::new(&host);
/// cx.record(&Measurement {
///     plugin: "demo",
///     kind: "raw",
///     before_bytes: 400,
///     after_bytes: 40,
///     est_before: cx.estimate(&"x".repeat(400), Class::Code),
///     est_after: cx.estimate(&"x".repeat(40), Class::Code),
///     ref_id: None,
///     call_id: None,
/// })
/// .unwrap();
/// assert_eq!(host.recorded().len(), 1);
/// ```
#[derive(Default)]
pub struct MemoryHost {
    config: Value,
    recorded: Mutex<Vec<Measurement>>,
    logged: Mutex<Vec<String>>,
    blobs: Mutex<HashMap<String, Vec<u8>>>,
    decisions: Mutex<HashMap<String, ArchiveDecision>>,
}

impl MemoryHost {
    /// A host with no configuration: every section a plugin asks for is its `Default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// A host whose configuration is `config`, shaped like the host's own file —
    /// `{"plugins": {"mine": {"enabled": true}}}` is what `plugin_config("mine")` reads.
    pub fn with_config(config: Value) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Every [`Measurement`] recorded so far, in order.
    pub fn recorded(&self) -> Vec<Measurement> {
        self.recorded.lock().unwrap().clone()
    }

    /// Every log line so far, formatted `level source/name: message`.
    pub fn logged(&self) -> Vec<String> {
        self.logged.lock().unwrap().clone()
    }
}

impl Host for MemoryHost {
    fn session(&self) -> &str {
        "memory"
    }

    /// The host's own rate table is calibrated per class; this one is the crude version of
    /// the same idea, so a plugin's arithmetic can be asserted without a tokenizer.
    fn estimate(&self, text: &str, class: Class) -> u32 {
        let per_token = match class {
            Class::Code => 3.5,
            Class::Prose => 4.0,
            Class::Json => 2.8,
            Class::Cjk => 1.0,
        };
        (text.chars().count() as f32 / per_token).ceil() as u32
    }

    fn record(&self, m: &Measurement) -> Result<()> {
        self.recorded.lock().unwrap().push(m.clone());
        Ok(())
    }

    fn record_call(&self, _surface: &str, _kind: &str, _name: Option<&str>) -> Result<i32> {
        Ok(0)
    }

    fn record_plugin_run(&self, _surface: &str, _plugin: &str) -> Result<i32> {
        Ok(0)
    }

    fn record_tokens(
        &self,
        _call_id: i32,
        _plugin: Option<&str>,
        _phase: &str,
        _source: &str,
        _tokens: i64,
    ) -> Result<()> {
        Ok(())
    }

    fn log(&self, level: &str, source: &str, name: &str, message: &str) {
        self.logged
            .lock()
            .unwrap()
            .push(format!("{level} {source}/{name}: {message}"));
    }

    fn config_json(&self, path: &str) -> Value {
        let mut node = &self.config;
        for key in path.split('.') {
            match node.get(key) {
                Some(next) => node = next,
                None => return Value::Null,
            }
        }
        node.clone()
    }
}

impl Archive for MemoryHost {
    fn put_archive(&self, body: &[u8]) -> Result<String> {
        // Content-addressed like the real one, but by length and prefix — an example does not
        // need a hash, it needs the same id for the same bytes.
        let id = format!(
            "mem{:x}{:x}",
            body.len(),
            body.first().copied().unwrap_or(0)
        );
        self.blobs.lock().unwrap().insert(id.clone(), body.to_vec());
        Ok(id)
    }

    fn get_archive(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.blobs.lock().unwrap().get(id).cloned())
    }

    fn archive_decision(&self, tool_use_id: &str) -> Result<Option<ArchiveDecision>> {
        Ok(self.decisions.lock().unwrap().get(tool_use_id).cloned())
    }

    fn put_archive_decision(
        &self,
        tool_use_id: &str,
        archive_id: &str,
        pointer: &str,
    ) -> Result<()> {
        self.decisions
            .lock()
            .unwrap()
            .entry(tool_use_id.to_string())
            .or_insert_with(|| ArchiveDecision {
                archive_id: archive_id.to_string(),
                pointer: pointer.to_string(),
                expanded: false,
            });
        Ok(())
    }

    fn mark_expanded(&self, archive_id: &str) -> Result<usize> {
        let mut d = self.decisions.lock().unwrap();
        let mut n = 0;
        for entry in d.values_mut().filter(|e| e.archive_id == archive_id) {
            entry.expanded = true;
            n += 1;
        }
        Ok(n)
    }
}

impl Notes for MemoryHost {
    fn insert_note(
        &self,
        _project: Option<&str>,
        _kind: &str,
        _title: &str,
        _body: &str,
    ) -> Result<i32> {
        Ok(0)
    }

    fn latest_note(&self, _kind: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn list_note_titles(&self, _project: Option<&str>, _limit: u32) -> Result<Vec<(i32, String)>> {
        Ok(Vec::new())
    }

    fn get_note_body(&self, _id: i32) -> Result<Option<String>> {
        Ok(None)
    }

    fn search_notes(&self, _query: &str, _limit: u32) -> Result<Vec<NoteHit>> {
        Ok(Vec::new())
    }
}

impl ReadCache for MemoryHost {
    fn put_read_cache(&self, _path: &str, _sha256: &str, _archive_id: Option<&str>) -> Result<()> {
        Ok(())
    }

    fn get_read_cache(&self, _path: &str) -> Result<Option<(Option<String>, i64)>> {
        Ok(None)
    }

    fn clear_read_cache(&self, _path: &str) -> Result<()> {
        Ok(())
    }
}

impl Ledger for MemoryHost {
    fn recent_hook_inputs(&self, _limit: i64) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn calls_since(&self, _ts: i64) -> Result<i64> {
        Ok(0)
    }
}

impl Symbols for MemoryHost {
    fn symbol_count(&self, _root: &str) -> Result<i64> {
        Ok(0)
    }

    fn symbol_stat(&self, _root: &str, _path: &str) -> Result<Option<(String, i64, i64)>> {
        Ok(None)
    }

    fn touch_symbols(&self, _root: &str, _path: &str, _mtime: i64, _size: i64) -> Result<()> {
        Ok(())
    }

    fn replace_symbols(
        &self,
        _root: &str,
        _path: &str,
        _file_sha: &str,
        _stat: (i64, i64),
        _rows: &[(String, String, i32, bool, i32, String)],
    ) -> Result<usize> {
        Ok(0)
    }

    fn delete_symbols_missing(&self, _root: &str, _keep: &HashSet<String>) -> Result<usize> {
        Ok(0)
    }

    fn mark_symbols_stale(&self, _abs_path: &str) -> Result<()> {
        Ok(())
    }

    fn symbol_defs(&self, _root: &str, _name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        Ok(Vec::new())
    }

    fn symbol_ref_groups(
        &self,
        _root: &str,
        _name: &str,
    ) -> Result<Vec<(String, String, i64, i32)>> {
        Ok(Vec::new())
    }

    fn symbol_impact(
        &self,
        _root: &str,
        _name: &str,
        _depth: u32,
    ) -> Result<Vec<(u32, String, String)>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ctx;
    use serde_json::json;

    #[derive(serde::Deserialize, Default, PartialEq, Debug)]
    struct Cfg {
        #[serde(default)]
        enabled: bool,
    }

    /// The dotted path a plugin reads its own section by has to reach into nested config,
    /// and a section the host does not have is the plugin's `Default`, not an error.
    #[test]
    fn config_is_read_by_dotted_path_and_missing_is_default() {
        let host = MemoryHost::with_config(json!({"plugins": {"mine": {"enabled": true}}}));
        let cx = Ctx::new(&host);
        assert_eq!(cx.plugin_config::<Cfg>("mine"), Cfg { enabled: true });
        assert_eq!(cx.plugin_config::<Cfg>("absent"), Cfg::default());
    }

    /// Lossless means the id a plugin quotes gives the bytes back, and that the same bytes
    /// archive to the same id — a result shortened twice must not grow two archives.
    #[test]
    fn an_archived_blob_comes_back_under_a_stable_id() {
        let host = MemoryHost::new();
        let id = host.put_archive(b"the original output").unwrap();
        assert_eq!(id, host.put_archive(b"the original output").unwrap());
        assert_eq!(
            host.get_archive(&id).unwrap().as_deref(),
            Some(&b"the original output"[..])
        );
        assert_eq!(host.get_archive("mem0").unwrap(), None);
    }

    /// First writer wins: the pointer text is frozen so every later turn replays the same
    /// bytes and the prompt cache survives.
    #[test]
    fn an_archive_decision_is_frozen_by_its_first_writer() {
        let host = MemoryHost::new();
        host.put_archive_decision("t1", "mem1", "[archived mem1]")
            .unwrap();
        host.put_archive_decision("t1", "mem2", "[archived mem2]")
            .unwrap();
        let d = host.archive_decision("t1").unwrap().unwrap();
        assert_eq!(d.pointer, "[archived mem1]");
        assert!(!d.expanded);
        assert_eq!(host.mark_expanded("mem1").unwrap(), 1);
        assert!(host.archive_decision("t1").unwrap().unwrap().expanded);
    }
}
