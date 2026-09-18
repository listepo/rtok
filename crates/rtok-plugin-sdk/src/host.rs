//! What a plugin may ask the host to do.
//!
//! A plugin never sees the host's database. It sees [`Host`] — estimate, record, log, read my
//! own configuration — plus one narrow trait per area of stored state it actually uses:
//! [`Archive`], [`Notes`], [`ReadCache`], [`Ledger`], [`Symbols`]. That is the whole reason
//! this crate can be three dependencies deep while `rtok` carries SQLite, tree-sitter and a
//! web server.
//!
//! Nothing here is a general database API. A method exists because a plugin calls it; the
//! host is free to store the state any way it likes, and two of these traits (`Symbols`,
//! `ReadCache`) are caches whose contents may vanish between calls.
//!
//! Every method takes `&self` and may be called from any thread. Errors are returned, never
//! panicked: a plugin that cannot read the store still has to fail open.

use std::collections::HashSet;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::Measurement;

/// What kind of text is being measured. The host keeps a chars-per-token rate per class, so
/// the estimate stays within about ±15 % without a tokenizer and without a network call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Source code, shell output, diffs.
    Code,
    /// Natural-language prose, markdown.
    Prose,
    /// JSON / structured data (punctuation-heavy → fewer chars per token).
    Json,
    /// CJK scripts: roughly one token per character.
    Cjk,
}

/// One full-text search hit from [`Notes::search_notes`].
#[derive(Clone, Debug)]
pub struct NoteHit {
    /// Note id; pass it to [`Notes::get_note_body`] for the whole note.
    pub id: i32,
    /// The note's title.
    pub title: String,
    /// A short excerpt around the match — capped by the host, never the whole body.
    pub snippet: String,
}

/// The frozen decision for one archived tool result, from [`Archive::archive_decision`].
///
/// A tool result is shortened once. Every later turn replays the same pointer text, because
/// context that changes byte-for-byte between turns costs a cache miss on every one of them.
#[derive(Clone, Debug)]
pub struct ArchiveDecision {
    /// Handle for [`Archive::get_archive`] and for `rtok expand <id>`.
    pub archive_id: String,
    /// The exact replacement text shown in place of the result.
    pub pointer: String,
    /// Whether the user has already expanded this one.
    pub expanded: bool,
}

/// A same-session archive row whose sha256 matched a later payload (T65.1).
#[derive(Clone, Debug)]
pub struct ArchiveHit {
    /// Handle for [`Archive::get_archive`] and for `rtok expand <id>`.
    pub id: String,
    /// Measurement rows in this session after the original archive; 0 if none yet.
    pub turns: u64,
}

/// The host, as a plugin sees it.
///
/// This is the part every plugin gets: estimate what text will cost, record what you saved,
/// say what happened, and read your own configuration section. Everything else is a
/// capability trait.
pub trait Host: Send + Sync {
    /// The host session this dispatch belongs to. Every row a plugin writes is attributed
    /// to it; a plugin rarely needs it directly.
    fn session(&self) -> &str;

    /// Working directory of this dispatch, when the surface knows it.
    /// Hook events carry one; MCP and the CLI typically do not.
    fn cwd(&self) -> Option<&str> {
        None
    }

    /// Estimated token count for `text`. No tokenizer, no network — safe on the hot path.
    fn estimate(&self, text: &str, class: Class) -> u32;

    /// Persist a [`Measurement`]. This is the only path a saving has into the host's
    /// numbers: what is not recorded here did not happen.
    fn record(&self, m: &Measurement) -> Result<()>;

    /// Open a `calls` row for one unit of work on `surface`, returning its id.
    fn record_call(&self, surface: &str, kind: &str, name: Option<&str>) -> Result<i32>;

    /// Open a `plugin_run` row for `plugin`, nested under the current call when there is one.
    fn record_plugin_run(&self, surface: &str, plugin: &str) -> Result<i32>;

    /// Attribute `tokens` to a call — the before/after phases of a plugin's own work.
    fn record_tokens(
        &self,
        call_id: i32,
        plugin: Option<&str>,
        phase: &str,
        source: &str,
        tokens: i64,
    ) -> Result<()>;

    /// Append a log line. Never fails: a plugin cannot be made to care where logs go.
    fn log(&self, level: &str, source: &str, name: &str, message: &str);

    /// One section of the host's configuration as JSON, by dotted path — `"plugins.read"`,
    /// `"proxy"`, `"estimator"` — or [`Value::Null`] when the host has no such section.
    ///
    /// A plugin reads its own settings through [`Ctx::plugin_config`]; this is the way out
    /// for the few that legitimately need a host-wide value, such as the proxy's mode.
    fn config_json(&self, path: &str) -> Value;

    /// The `calls` row this dispatch runs under, when the surface has one.
    fn call_id(&self) -> Option<i32> {
        None
    }

    /// Watcher debounce window: relative paths not yet re-indexed (T68.3).
    fn graph_watch_pending(&self) -> Vec<String> {
        Vec::new()
    }

    /// Publish the watcher's in-flight pending set for staleness banners (T68.3).
    fn publish_graph_watch_pending(&self, paths: &[String]) {
        let _ = paths;
    }
}

/// Everything a plugin may touch, in one object-safe bound. [`Ctx`] derefs to it, so a
/// plugin calls `cx.estimate(..)`, `cx.put_archive(..)`, `cx.symbol_defs(..)` and the rest
/// without naming a single trait.
pub trait Capabilities: Host + Archive + Notes + ReadCache + Ledger + Symbols {}

impl<T> Capabilities for T where T: Host + Archive + Notes + ReadCache + Ledger + Symbols + ?Sized {}

/// What a plugin is handed on every event: the host, and nothing else.
///
/// `Ctx` is a borrow, not a state — the host builds one per dispatch. It derefs to
/// [`Capabilities`], so every host method is reachable directly on it.
pub struct Ctx<'a> {
    host: &'a dyn Capabilities,
}

impl<'a> Ctx<'a> {
    /// Wrap a host for one dispatch.
    pub fn new(host: &'a dyn Capabilities) -> Self {
        Self { host }
    }

    /// This plugin's own `[plugins.<id>]` section, deserialized into `T`.
    ///
    /// Configuration is data, not a schema the host has to know: a plugin declares whatever
    /// struct it wants and the host hands over the matching section. A missing or unparsable
    /// section yields `T::default()` — a plugin that cannot read its own settings still runs.
    pub fn plugin_config<T: DeserializeOwned + Default>(&self, id: &str) -> T {
        section(self.host, &format!("plugins.{id}"))
    }

    /// A host-wide section by dotted path, deserialized into `T`; `T::default()` when the
    /// host has no such section. Reach for [`Ctx::plugin_config`] first.
    pub fn config<T: DeserializeOwned + Default>(&self, path: &str) -> T {
        section(self.host, path)
    }
}

/// One configuration section, or `T::default()` when the host has none under that path.
fn section<T: DeserializeOwned + Default>(host: &dyn Host, path: &str) -> T {
    serde_json::from_value(host.config_json(path)).unwrap_or_default()
}

impl<'a> std::ops::Deref for Ctx<'a> {
    type Target = dyn Capabilities + 'a;

    fn deref(&self) -> &(dyn Capabilities + 'a) {
        self.host
    }
}

/// Content-addressed blob storage — the other half of "lossless by default".
///
/// A plugin that shortens something puts the original here first and quotes the returned id,
/// so `rtok expand <id>` can always give it back.
pub trait Archive {
    /// Store `body` and return its handle. Storing the same bytes twice returns the same id.
    fn put_archive(&self, body: &[u8]) -> Result<String>;

    /// The bytes behind a handle, or `None` if the host no longer has them.
    fn get_archive(&self, id: &str) -> Result<Option<Vec<u8>>>;

    /// The payload's size in bytes, or `None` if the host no longer has it. Metadata only:
    /// hot paths (the guard deny) must not read a possibly-megabyte body. The default fails
    /// open (`None`), so hosts that only keep bodies in memory need no change.
    fn archive_size(&self, _id: &str) -> Result<Option<u64>> {
        Ok(None)
    }

    /// The decision already made for `tool_use_id`, if this result was shortened before.
    fn archive_decision(&self, tool_use_id: &str) -> Result<Option<ArchiveDecision>>;

    /// Persist the decision for `tool_use_id`. First writer wins: a pointer that is already
    /// recorded stays exactly as it is, because the model pays a cache miss for every byte
    /// of context that moves between turns.
    fn put_archive_decision(
        &self,
        tool_use_id: &str,
        archive_id: &str,
        pointer: &str,
    ) -> Result<()>;

    /// Mark an archived blob as expanded by the user; returns how many rows changed.
    fn mark_expanded(&self, archive_id: &str) -> Result<usize>;

    /// The archive row for `sha256` written in this session, if any. Default `Ok(None)`
    /// so a host that cannot look it up fails open (the caller prints the body).
    fn archive_in_session(&self, _sha256: &str) -> Result<Option<ArchiveHit>> {
        Ok(None)
    }
}

/// Durable notes the host can search — what a plugin remembers between sessions.
pub trait Notes {
    /// Upsert on `(project, kind, title)` and return the note id (T69.5 `remember:` path).
    fn upsert_note(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<i32>;

    /// Save a note and return its id. `project` scopes it; `None` means "not project-bound".
    fn insert_note(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<i32>;

    /// The body of the most recent note of `kind`.
    fn latest_note(&self, kind: &str) -> Result<Option<String>>;

    /// The `limit` most recent `(id, title)` pairs, newest first.
    fn list_note_titles(&self, project: Option<&str>, limit: u32) -> Result<Vec<(i32, String)>>;

    /// One note's body by id.
    fn get_note_body(&self, id: i32) -> Result<Option<String>>;

    /// Full-text search over notes. An empty query returns no hits rather than everything.
    fn search_notes(&self, query: &str, limit: u32) -> Result<Vec<NoteHit>>;
}

/// Per-session memory of what has already been read, so the same file is not sent twice.
///
/// This is a cache: entries may be evicted, and every method is scoped to the current
/// session by the host.
pub trait ReadCache {
    /// Remember that `path` was read, by content hash, optionally with the archived body.
    fn put_read_cache(&self, path: &str, sha256: &str, archive_id: Option<&str>) -> Result<()>;

    /// The `(archive_id, timestamp)` remembered for `path`, if any.
    fn get_read_cache(&self, path: &str) -> Result<Option<(Option<String>, i64)>>;

    /// Forget `path` and anything stored under it — call this when the file changes.
    fn clear_read_cache(&self, path: &str) -> Result<()>;
}

/// A read-only look back at this session's own traffic, for plugins that need a window
/// rather than a single event.
pub trait Ledger {
    /// The raw JSON of the last `limit` hook inputs in this session, newest first.
    fn recent_hook_inputs(&self, limit: i64) -> Result<Vec<String>>;

    /// How many calls this session has made since the unix timestamp `ts`.
    fn calls_since(&self, ts: i64) -> Result<i64>;

    /// The newest `ref_id` on a measurement row for this session (T69.5 dedup).
    fn last_measurement_ref(&self, plugin: &str, kind: &str) -> Result<Option<String>>;
}

/// Symbol rows for one indexed file.
pub type SymbolFileRows = Vec<(String, String, i32, bool, i32, String)>;
/// Batched cold-index writes: `(path, sha, stat, rows)` per file.
pub type SymbolFileBatch = Vec<(String, String, (i64, i64), SymbolFileRows)>;

/// The symbol index behind `symbol` / `callers` / `impact`: definitions and references
/// extracted from source, keyed by repository root and relative path.
///
/// This is a cache the host rebuilds from the files themselves, so a stale or empty index is
/// a correctness-neutral miss, never an error to propagate.
pub trait Symbols {
    /// How many symbols are indexed under `root`. Zero means "not indexed yet".
    fn symbol_count(&self, root: &str) -> Result<i64>;

    /// The recorded `(sha, mtime, size)` for one file, if it has been indexed.
    fn symbol_stat(&self, root: &str, path: &str) -> Result<Option<(String, i64, i64)>>;

    /// Every indexed file's `(sha, mtime, size)` under `root` (T35.3).
    fn symbol_stats(
        &self,
        root: &str,
    ) -> Result<std::collections::HashMap<String, (String, i64, i64)>>;

    /// Update a file's mtime and size without re-parsing it — the content is unchanged.
    fn touch_symbols(&self, root: &str, path: &str, mtime: i64, size: i64) -> Result<()>;

    /// Replace every symbol of one file in a single transaction. `rows` are
    /// `(name, kind, line, is_def, end_line, text)`; returns how many were written.
    fn replace_symbols(
        &self,
        root: &str,
        path: &str,
        file_sha: &str,
        stat: (i64, i64),
        rows: &[(String, String, i32, bool, i32, String)],
    ) -> Result<usize>;

    /// Replace many files in one transaction (T35.3 cold index).
    fn replace_symbol_files(&self, root: &str, files: &SymbolFileBatch) -> Result<usize>;

    /// Drop every indexed file under `root` that is not in `keep`; returns how many went.
    fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize>;

    /// Drop what was indexed for one absolute path — the file changed under the index.
    fn mark_symbols_stale(&self, abs_path: &str) -> Result<()>;

    /// Distinct indexed files under `root` (T68.3 `graph status`).
    fn symbol_file_count(&self, root: &str) -> Result<i64> {
        let _ = root;
        Ok(0)
    }

    /// Pending re-index paths for `root` (T68.3).
    fn symbol_pending(&self, root: &str, root_path: &std::path::Path) -> Result<Vec<String>> {
        let _ = (root, root_path);
        Ok(Vec::new())
    }

    /// Unix seconds of the last successful index for `root` (T68.3).
    fn symbol_indexed_at(&self, root: &str) -> Result<Option<i64>> {
        let _ = root;
        Ok(None)
    }

    /// Record the last successful index time for `root` (T68.3).
    fn touch_symbol_indexed_at(&self, root: &str, ts: i64) -> Result<()> {
        let _ = (root, ts);
        Ok(())
    }

    /// Extractor fingerprint stored for `root`, if any (T35.5).
    fn extractor_fingerprint(&self, root: &str) -> Result<Option<String>> {
        let _ = root;
        Ok(None)
    }

    /// Record the extractor fingerprint for `root` after a cold index (T35.5).
    fn set_extractor_fingerprint(&self, root: &str, fp: &str) -> Result<()> {
        let _ = (root, fp);
        Ok(())
    }

    /// Definitions of `name`: `(path, kind, line, end_line)`.
    fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32, i32)>>;

    /// References to `name` grouped by location: `(path, kind, count, line)`.
    fn symbol_ref_groups(&self, root: &str, name: &str) -> Result<Vec<(String, String, i64, i32)>>;

    /// Callees per definition of `name`: `(def_path, def_line, callee, first_line)` (T68.2).
    fn symbol_callees(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, i32, String, i32)>> {
        let _ = (root, name);
        Ok(Vec::new())
    }

    /// What `name` reaches within `depth` hops: `(depth, path, name)`.
    fn symbol_impact(
        &self,
        root: &str,
        name: &str,
        depth: u32,
    ) -> Result<Vec<(u32, String, String)>>;

    /// Definitions with no same-name reference under `root`: `(path, name, kind, line)`
    /// (T52.4). Name-based, like `callers`: a shared name keeps every same-named
    /// definition live. Callers filter pub, trait impls, tests and macros from these.
    fn symbol_dead_candidates(&self, root: &str) -> Result<Vec<(String, String, String, i32)>> {
        let _ = root;
        Ok(Vec::new())
    }

    /// Distinct definition names starting with `prefix`, best `limit` by reference
    /// count, ties by name (T68.1 `explore` prefix resolution).
    fn symbol_name_prefix(&self, root: &str, prefix: &str, limit: i64) -> Result<Vec<String>> {
        let _ = (root, prefix, limit);
        Ok(Vec::new())
    }

    /// Call chains `from → … → to` walked in the caller direction within `depth`
    /// hops, shortest first (T68.1 `explore`; T68.4 `impact --to` reuses it).
    fn symbol_paths(&self, root: &str, from: &str, to: &str, depth: u32) -> Result<Vec<String>> {
        let _ = (root, from, to, depth);
        Ok(Vec::new())
    }

    /// T68.6: import rows of `path` as `(name, line)`.
    fn symbol_imports(&self, root: &str, path: &str) -> Result<Vec<(String, i32)>> {
        let _ = (root, path);
        Ok(Vec::new())
    }

    /// T68.6: files that import `module` as `(path, line)`.
    fn symbol_importers(&self, root: &str, module: &str) -> Result<Vec<(String, i32)>> {
        let _ = (root, module);
        Ok(Vec::new())
    }

    /// T68.6: definitions in files that import `name` (the extra impact hop).
    fn symbol_import_follow(&self, root: &str, name: &str) -> Result<Vec<(String, String)>> {
        let _ = (root, name);
        Ok(Vec::new())
    }

    /// T52.3: names ranked by reference count with one def site:
    /// `(name, refs, path, line)`, `ORDER BY refs DESC, name ASC`.
    fn symbol_top_refs(&self, root: &str, limit: i64) -> Result<Vec<(String, i64, String, i32)>> {
        let _ = (root, limit);
        Ok(Vec::new())
    }
}

// The tests for what a host owes a plugin live beside `testing::MemoryHost`, the host this
// crate ships for exactly that purpose (`src/testing.rs`).
