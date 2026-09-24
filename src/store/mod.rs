//! One SQLite file (plan T0.3, T13.1, decision D8): WAL mode, FTS5, migrations keyed by filename.

pub mod embed;
pub mod models;
pub mod otel;
pub mod schema;
// T163: shared Diesel extension for SQL the DSL cannot express (recursive CTEs, FTS5).
// Symbol index (graph plugin) — SQLite only (D18 loser deleted; P39: Ladybug/Grafeo removed).
mod symbols;

use std::collections::{BTreeMap, HashMap};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Bool, Double, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::plugin::Measurement;
// The two row shapes a plugin sees are the contract's (D25); the diesel rows below feed them.
pub use crate::plugin::{ArchiveDecision, NoteHit};

// `models` (the schema::models table) is not imported bare: it collides with this file's
// own `pub mod models` of Diesel row structs, so upsert_model qualifies it as `schema::models`.
use schema::{
    archive, archive_decisions, call_io, calls, hosts, kv, logs, measurements, notes, providers,
    read_cache, sessions, tokens, usage,
};

/// `COALESCE(x, y)` for an upsert `DO UPDATE SET` (T163.5): not one of Diesel's built-in
/// functions, so declared here rather than dropping to raw SQL.
#[diesel::declare_sql_function]
extern "SQL" {
    fn coalesce<T: diesel::sql_types::SqlType + diesel::sql_types::SingleValue>(
        x: diesel::sql_types::Nullable<T>,
        y: diesel::sql_types::Nullable<T>,
    ) -> diesel::sql_types::Nullable<T>;
}

// SQLite `length()`: character count of a TEXT value (not byte count) — matches what the
// raw SQL it replaces computed, so `memory_note_aggs`'s `body_bytes` stays unchanged.
diesel::define_sql_function!(fn length(x: Text) -> BigInt);
// `SUM` declared to return `Nullable<BigInt>`: Diesel's generic `sum()` widens every
// integer sum to `Nullable<Numeric>` (ANSI's overflow-safe rule, via `Foldable`), which
// this crate has no `bigdecimal` support to deserialize. SQLite has no separate NUMERIC
// storage class — an integer sum is still an integer — so a `BigInt` result is exact.
diesel::define_sql_function! {
    #[aggregate]
    #[sql_name = "SUM"]
    fn sum_bigint(x: BigInt) -> Nullable<BigInt>;
}

/// `substr(text, start, length)` for a byte-prefix compare (T163.6): `read_cache.path` can
/// itself hold `%`/`_`, so `LIKE` cannot express "starts with" — not one of Diesel's built-ins.
#[diesel::declare_sql_function]
extern "SQL" {
    fn substr(
        x: diesel::sql_types::Text,
        start: diesel::sql_types::Integer,
        length: diesel::sql_types::Integer,
    ) -> diesel::sql_types::Text;
}

/// Embedded migrations, applied in order, each exactly once.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001.sql",
        include_str!("../../migrations/0001_schema_v1/up.sql"),
    ),
    (
        "0002.sql",
        include_str!("../../migrations/0002_schema_v2/up.sql"),
    ),
    (
        "0003.sql",
        include_str!("../../migrations/0003_symbol_index/up.sql"),
    ),
    (
        "0004.sql",
        include_str!("../../migrations/0004_archive_decisions/up.sql"),
    ),
    (
        "0005.sql",
        include_str!("../../migrations/0005_usage_api/up.sql"),
    ),
    (
        "0006.sql",
        include_str!("../../migrations/0006_symbols_root/up.sql"),
    ),
    (
        "0007.sql",
        include_str!("../../migrations/0007_symbols_freshness/up.sql"),
    ),
    (
        "0008.sql",
        include_str!("../../migrations/0008_call_edges/up.sql"),
    ),
    (
        "0009.sql",
        include_str!("../../migrations/0009_otel_export/up.sql"),
    ),
    (
        "0010.sql",
        include_str!("../../migrations/0010_seed_pi_host/up.sql"),
    ),
    (
        "0011.sql",
        include_str!("../../migrations/0011_extractor/up.sql"),
    ),
    (
        "0012.sql",
        include_str!("../../migrations/0012_note_embeddings/up.sql"),
    ),
    (
        "0013.sql",
        include_str!("../../migrations/0013_call_id_indexes/up.sql"),
    ),
    (
        "0014.sql",
        include_str!("../../migrations/0014_archive_decisions_pk/up.sql"),
    ),
    (
        "0015.sql",
        include_str!("../../migrations/0015_notes_lifecycle/up.sql"),
    ),
    (
        "0016.sql",
        include_str!("../../migrations/0016_symbol_stale/up.sql"),
    ),
    (
        "0017.sql",
        include_str!("../../migrations/0017_notes_recall/up.sql"),
    ),
    (
        "0018.sql",
        include_str!("../../migrations/0018_kv_guard/up.sql"),
    ),
    (
        "0019.sql",
        include_str!("../../migrations/0019_archive_agent_context/up.sql"),
    ),
    (
        "0020.sql",
        include_str!("../../migrations/0020_notes_topic_unique/up.sql"),
    ),
    (
        "0021.sql",
        include_str!("../../migrations/0021_measurements_once/up.sql"),
    ),
    (
        "0022.sql",
        include_str!("../../migrations/0022_call_io_raw_bodies/up.sql"),
    ),
    (
        "0023.sql",
        include_str!("../../migrations/0023_measurements_session_ts/up.sql"),
    ),
];

pub struct Store {
    conn: Mutex<SqliteConnection>,
    wait: LockWait,
}

/// How long one connection waits on another process's lock (T178).
#[derive(Debug, Clone, Copy)]
pub struct LockWait {
    /// `busy_timeout` for every statement.
    pub busy: std::time::Duration,
    /// Fresh connections tried when the WAL switch returns "database is locked".
    pub attempts: u32,
    /// `busy_timeout` while migrations run.
    pub migrate: std::time::Duration,
}

impl LockWait {
    /// Every surface but the hook: 1 s per statement, 10 connects, 30 s for a migration run.
    pub const STEADY: Self = Self {
        busy: std::time::Duration::from_secs(1),
        attempts: 10,
        migrate: std::time::Duration::from_secs(30),
    };
}

/// A statement gave up on another process's lock after its `busy_timeout`.
pub fn is_locked(e: &anyhow::Error) -> bool {
    format!("{e:#}").contains("database is locked")
}

fn set_busy(conn: &mut SqliteConnection, busy: std::time::Duration) -> Result<()> {
    Ok(conn.batch_execute(&format!("PRAGMA busy_timeout = {};", busy.as_millis()))?)
}

/// One row of [`Store::sessions_by_cwd`].
#[derive(Debug, Clone)]
pub struct SessionSeen {
    pub id: String,
    pub host: Option<String>,
    pub cwd: String,
    /// Newest `calls.ts` of the session, else `started_at`.
    pub last_seen: i64,
    pub ended_at: Option<i64>,
}

/// Turn arbitrary user text into an FTS5 MATCH phrase query: every blank-separated token is
/// quoted, so `*`, `(`, `-`, `AND` and `"` are searched for as characters instead of being
/// read as FTS5 syntax. `None` when there is no token left to search for.
pub(crate) fn fts_phrase_query(query: &str) -> Option<String> {
    let quoted: Vec<String> = query
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect();
    (!quoted.is_empty()).then(|| quoted.join(" "))
}

/// The four required `notes` columns as one `.values(...)` tuple — shared by
/// [`Store::insert_note`] and [`Store::insert_note_if_absent`] (T209), which differ only
/// in the `INSERT` variant and what an ignored conflict means for the caller.
#[allow(clippy::type_complexity)]
fn note_values<'a>(
    project: Option<&'a str>,
    kind: &'a str,
    title: &'a str,
    body: &'a str,
) -> (
    diesel::dsl::Eq<notes::project, Option<&'a str>>,
    diesel::dsl::Eq<notes::kind, &'a str>,
    diesel::dsl::Eq<notes::title, &'a str>,
    diesel::dsl::Eq<notes::body, &'a str>,
) {
    (
        notes::project.eq(project),
        notes::kind.eq(kind),
        notes::title.eq(title),
        notes::body.eq(body),
    )
}

#[cfg(test)]
thread_local! {
    /// Test-only tally of fresh SQLite connections on this thread (T203): PreCompact,
    /// SessionEnd and SessionStart used to open a second or third `Store` beside the one the
    /// hook `Runtime` already holds; the count lets a test assert one open per hook run.
    pub(crate) static OPEN_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Store {
    /// Open (creating directories and the file as needed) and migrate.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, LockWait::STEADY)
    }

    /// [`Store::open`] with its own bound on waiting for other processes' locks.
    pub fn open_with(path: &Path, wait: LockWait) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let url = path.to_str().context("db path is not UTF-8")?;
        // A first run races: hooks, the MCP server, the proxy and `otel flush` all open
        // the same fresh file, and the `journal_mode = WAL` switch can return
        // "database is locked" straight away — SQLite does not always run the busy
        // handler for a journal-mode change. Retry with a fresh connection instead of
        // failing the open.
        let attempts = wait.attempts.max(1);
        for attempt in 0..attempts {
            match Self::connect(url, wait) {
                Ok(store) => return Ok(store),
                Err(e) if is_locked(&e) && attempt + 1 < attempts => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => return Err(e.context(path.display().to_string())),
            }
        }
        unreachable!("open: the retry loop always returns")
    }

    fn connect(url: &str, wait: LockWait) -> Result<Self> {
        let mut conn = SqliteConnection::establish(url)?;
        // Hooks, the MCP server, the proxy and the detached `otel flush` child all write this one
        // file. SQLite's default busy timeout is 0, so a second writer failed at once with
        // "database is locked" instead of waiting the few ms the first one holds the lock. First,
        // so switching to WAL waits too; `wait.busy` bounds each statement's wait.
        set_busy(&mut conn, wait.busy)?;
        conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        Self::init(conn, wait)
    }

    /// Fresh in-memory store for tests and examples.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(SqliteConnection::establish(":memory:")?, LockWait::STEADY)
    }

    fn init(mut conn: SqliteConnection, wait: LockWait) -> Result<Self> {
        #[cfg(test)]
        OPEN_COUNT.with(|n| n.set(n.get() + 1));
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
            wait,
        };
        store.migrate()?;
        Ok(store)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, SqliteConnection>> {
        Ok(self.conn.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Apply pending migrations; returns how many ran. Idempotent.
    ///
    /// Each migration's SQL and its `schema_migrations` row commit together: several are
    /// `ALTER TABLE … ADD COLUMN`, so a run interrupted between the two used to leave a
    /// column added with no version row, and every later `Store::open` failed on
    /// `duplicate column name` — a store nothing could repair but deletion.
    ///
    /// The check and the apply share one `BEGIN EXCLUSIVE` for the same reason across
    /// processes: hooks, the MCP server and the proxy all open this file, and on a fresh
    /// store two of them landed in the gap between the `SELECT` and the `ALTER`, so the
    /// loser died on `duplicate column name`. The read-only pre-check keeps the settled
    /// case — every open after the first — off the write lock.
    pub fn migrate(&self) -> Result<usize> {
        let mut conn = self.lock()?;
        conn.batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch()))",
        )?;
        let done: Vec<Count> =
            sql_query("SELECT COUNT(*) AS n FROM schema_migrations").load(&mut *conn)?;
        if done.first().map(|r| r.n).unwrap_or(0) >= MIGRATIONS.len() as i64 {
            return Ok(0);
        }
        // Only a fresh or upgraded store reaches here, and everything else opening it in the
        // same moment queues behind this one transaction. `open`'s 1 s is the steady-state
        // bound; one migration run plus that queue outlives it, and the losers came back
        // "database is locked". Restored below, so the bound still holds after. The hook
        // keeps its few ms here too and fails open instead (T178).
        set_busy(&mut conn, self.wait.migrate)?;
        let applied = conn.exclusive_transaction::<_, anyhow::Error, _>(|conn| {
            let mut applied = 0;
            for (name, sql) in MIGRATIONS {
                let rows: Vec<Count> =
                    sql_query("SELECT COUNT(*) AS n FROM schema_migrations WHERE name = ?")
                        .bind::<Text, _>(*name)
                        .load(&mut *conn)?;
                if rows.first().map(|r| r.n).unwrap_or(0) > 0 {
                    continue;
                }
                conn.batch_execute(sql)
                    .with_context(|| format!("migration {name}"))?;
                sql_query("INSERT OR IGNORE INTO schema_migrations (name) VALUES (?)")
                    .bind::<Text, _>(*name)
                    .execute(conn)?;
                applied += 1;
            }
            Ok(applied)
        });
        set_busy(&mut conn, self.wait.busy)?;
        applied
    }

    /// One `measurements` row. Prefer `Runtime::record`, which supplies the session.
    pub fn insert_measurement(&self, session: &str, m: &Measurement) -> Result<()> {
        self.insert_measurement_once(session, m, None)
    }

    /// [`Store::insert_measurement`] for one delivery of a call: a row whose `once` key (the
    /// call, e.g. `PreToolUse:<tool_use_id>`) plus plugin, kind and ref is already stored is
    /// dropped, so a call delivered twice counts once (T245).
    pub fn insert_measurement_once(
        &self,
        session: &str,
        m: &Measurement,
        once: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        insert_measurement_conn(&mut conn, session, m, once)
    }

    /// Count `measurements` for one plugin. Used by `examples/hello_plugin.rs`.
    pub fn measurement_count(&self, plugin: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(measurements::table
            .filter(measurements::plugin.eq(plugin))
            .count()
            .get_result(&mut *conn)?)
    }

    pub fn upsert_session(
        &self,
        id: &str,
        host_id: Option<i32>,
        project: Option<&str>,
        cwd: Option<&str>,
        source: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        // COALESCE keeps a non-NULL value: a later writer that does not know the
        // attribution (proxy/mcp pass None for project/cwd; Runtime::insert_call
        // passes None for source) must not wipe what an earlier hook already set.
        diesel::insert_into(sessions::table)
            .values((
                sessions::id.eq(id),
                sessions::host_id.eq(host_id),
                sessions::project.eq(project),
                sessions::cwd.eq(cwd),
                sessions::source.eq(source),
            ))
            .on_conflict(sessions::id)
            .do_update()
            .set((
                sessions::host_id.eq(coalesce(
                    diesel::upsert::excluded(sessions::host_id),
                    sessions::host_id,
                )),
                sessions::project.eq(coalesce(
                    diesel::upsert::excluded(sessions::project),
                    sessions::project,
                )),
                sessions::cwd.eq(coalesce(
                    diesel::upsert::excluded(sessions::cwd),
                    sessions::cwd,
                )),
                sessions::source.eq(coalesce(
                    diesel::upsert::excluded(sessions::source),
                    sessions::source,
                )),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Upsert provider + model; returns `(provider_id, model_id)`. Proxy ground truth:
    /// the request `model` must resolve to a `models` row (plan T5.1 Check).
    /// T208: `BEGIN IMMEDIATE` — the select-then-insert-then-select below would otherwise
    /// start as a read and race another writer's upgrade to the same rows ("database is
    /// locked" even inside `wait.busy`); an immediate transaction takes the write lock up
    /// front and serializes instead.
    pub fn upsert_model(&self, provider_slug: &str, model_slug: &str) -> Result<(i32, i32)> {
        let mut conn = self.lock()?;
        conn.immediate_transaction(|conn| -> Result<(i32, i32)> {
            diesel::insert_or_ignore_into(providers::table)
                .values((
                    providers::slug.eq(provider_slug),
                    providers::name.eq(provider_slug),
                ))
                .execute(&mut *conn)?;
            let provider_id: i32 = providers::table
                .filter(providers::slug.eq(provider_slug))
                .select(providers::id)
                .first(&mut *conn)
                .optional()?
                .context("provider")?;
            diesel::insert_or_ignore_into(schema::models::table)
                .values((
                    schema::models::provider_id.eq(provider_id),
                    schema::models::slug.eq(model_slug),
                ))
                .execute(&mut *conn)?;
            let model_id: i32 = schema::models::table
                .filter(schema::models::provider_id.eq(provider_id))
                .filter(schema::models::slug.eq(model_slug))
                .select(schema::models::id)
                .first(&mut *conn)
                .optional()?
                .context("model")?;
            Ok((provider_id, model_id))
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_call(
        &self,
        session_id: &str,
        surface: &str,
        kind: &str,
        host_id: Option<i32>,
        provider_id: Option<i32>,
        model_id: Option<i32>,
        plugin: Option<&str>,
        name: Option<&str>,
    ) -> Result<i32> {
        let mut conn = self.lock()?;
        Ok(diesel::insert_into(calls::table)
            .values((
                calls::session_id.eq(session_id),
                calls::surface.eq(surface),
                calls::kind.eq(kind),
                calls::host_id.eq(host_id),
                calls::provider_id.eq(provider_id),
                calls::model_id.eq(model_id),
                calls::plugin.eq(plugin),
                calls::name.eq(name),
            ))
            .returning(calls::id)
            .get_result(&mut *conn)?)
    }

    /// Nest a `plugin_run` row under the hook, MCP call or API request it ran in.
    pub fn set_call_parent(&self, id: i32, parent: i32) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(calls::table.filter(calls::id.eq(id)))
            .set(calls::parent_id.eq(parent))
            .execute(&mut *conn)?;
        Ok(())
    }

    pub fn set_call_ms(&self, id: i32, ms: f64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(calls::table.filter(calls::id.eq(id)))
            .set(calls::ms.eq(ms))
            .execute(&mut *conn)?;
        Ok(())
    }

    pub fn count_kind(&self, kind: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(calls::table
            .filter(calls::kind.eq(kind))
            .count()
            .get_result(&mut *conn)?)
    }

    pub fn call_ids_of_kind(&self, kind: &str) -> Result<Vec<i32>> {
        let mut conn = self.lock()?;
        Ok(calls::table
            .filter(calls::kind.eq(kind))
            .select(calls::id)
            .load(&mut *conn)?)
    }

    pub fn call_io_archives(&self, call_id: i32) -> Result<(Option<String>, Option<String>)> {
        let mut conn = self.lock()?;
        Ok(call_io::table
            .filter(call_io::call_id.eq(call_id))
            .select((call_io::request_archive, call_io::response_archive))
            .first::<(Option<String>, Option<String>)>(&mut *conn)
            .optional()?
            .unwrap_or((None, None)))
    }

    /// T201: the sha256 columns beside [`Self::call_io_archives`] — `NULL` for a body
    /// `spill` never archived (over `inline_cap` with `archive_dir = None`, the hook path),
    /// so a caller can tell "never hashed" from "hashed and inlined".
    #[cfg(test)]
    pub fn call_io_shas(&self, call_id: i32) -> Result<(Option<String>, Option<String>)> {
        let mut conn = self.lock()?;
        Ok(call_io::table
            .filter(call_io::call_id.eq(call_id))
            .select((call_io::request_sha256, call_io::response_sha256))
            .first::<(Option<String>, Option<String>)>(&mut *conn)
            .optional()?
            .unwrap_or((None, None)))
    }

    /// Archive ids a Calls row can expand (T60.4): spilled `call_io` body first,
    /// else a `measurements.ref_id` on that call. One pair of queries for the
    /// page, so the snapshot does not N+1 on a tick.
    pub fn archive_ref_ids(
        &self,
        call_ids: &[i32],
    ) -> Result<std::collections::BTreeMap<i32, String>> {
        let mut out = std::collections::BTreeMap::new();
        if call_ids.is_empty() {
            return Ok(out);
        }
        let mut conn = self.lock()?;
        let io: Vec<(i32, Option<String>, Option<String>)> = call_io::table
            .filter(call_io::call_id.eq_any(call_ids.iter().copied()))
            .select((
                call_io::call_id,
                call_io::request_archive,
                call_io::response_archive,
            ))
            .load(&mut *conn)?;
        for (call_id, request_archive, response_archive) in io {
            if let Some(id) = response_archive.or(request_archive) {
                out.insert(call_id, id);
            }
        }
        let ms: Vec<(Option<i32>, Option<String>)> = measurements::table
            .filter(measurements::call_id.eq_any(call_ids.iter().copied()))
            .filter(measurements::ref_id.is_not_null())
            .select((measurements::call_id, measurements::ref_id))
            .load(&mut *conn)?;
        for (call_id, ref_id) in ms {
            if let (Some(call_id), Some(ref_id)) = (call_id, ref_id) {
                out.entry(call_id).or_insert(ref_id);
            }
        }
        Ok(out)
    }

    pub fn token_phases(&self, call_id: i32) -> Result<Vec<String>> {
        let mut conn = self.lock()?;
        Ok(tokens::table
            .filter(tokens::call_id.eq(call_id))
            .select(tokens::phase)
            .load(&mut *conn)?)
    }

    pub fn host_id(&self, slug: &str) -> Result<Option<i32>> {
        let mut conn = self.lock()?;
        Ok(hosts::table
            .filter(hosts::slug.eq(slug))
            .select(hosts::id)
            .first(&mut *conn)
            .optional()?)
    }

    /// Test-only: one session's `(host_id slug, project, cwd)` — T25.0's Check reads the row
    /// a hook run left rather than re-deriving it from `upsert_session`'s arguments.
    #[cfg(test)]
    pub fn session_row(&self, id: &str) -> Result<Option<SessionRow>> {
        let mut conn = self.lock()?;
        sessions::table
            .left_join(hosts::table)
            .filter(sessions::id.eq(id))
            .select((hosts::slug.nullable(), sessions::project, sessions::cwd))
            .first::<SessionRow>(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// Every session with a `cwd`, newest activity first (T154): the worktree ownership
    /// ledger is this projection of what the hooks already write — no second writer.
    /// `last_seen` is the newest `calls` row, or `started_at` before the first one lands.
    pub fn sessions_by_cwd(&self) -> Result<Vec<SessionSeen>> {
        let mut conn = self.lock()?;
        // Two builder queries joined in memory: newest call per session, then the sessions
        // with their host — the query builder has no COALESCE over a grouped join.
        let last_call: HashMap<String, i64> = calls::table
            .group_by(calls::session_id)
            .select((calls::session_id, diesel::dsl::max(calls::ts)))
            .load::<(String, Option<i64>)>(&mut *conn)?
            .into_iter()
            .filter_map(|(id, ts)| Some((id, ts?)))
            .collect();
        let mut rows: Vec<SessionSeen> = sessions::table
            .left_join(hosts::table)
            .filter(sessions::cwd.is_not_null())
            .select((
                sessions::id,
                hosts::slug.nullable(),
                sessions::cwd.assume_not_null(),
                sessions::started_at,
                sessions::ended_at,
            ))
            .load::<(String, Option<String>, String, i64, Option<i64>)>(&mut *conn)?
            .into_iter()
            .map(|(id, host, cwd, started_at, ended_at)| SessionSeen {
                last_seen: last_call.get(&id).copied().unwrap_or(started_at),
                id,
                host,
                cwd,
                ended_at,
            })
            .collect();
        rows.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.id.cmp(&b.id)));
        Ok(rows)
    }

    pub fn count_call_io(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(call_io::table.count().get_result(&mut *conn)?)
    }

    pub fn count_tokens(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(tokens::table.count().get_result(&mut *conn)?)
    }

    /// Whole-`measurements`-ledger row count (T207): `report_window`'s "measurements"
    /// figure, replacing a per-catalogue-plugin `list_measurements` loop that missed
    /// out-of-tree/WASM plugins and cost an N+1.
    pub fn count_measurements(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(measurements::table.count().get_result(&mut *conn)?)
    }

    /// Whole-`usage`-ledger row count (T207): `report_window`'s "usage" figure,
    /// replacing a per-session `usage_rows` loop (an N+1 for no reason — the loop never
    /// used anything but the row count).
    pub fn count_usage(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(usage::table.count().get_result(&mut *conn)?)
    }

    #[cfg(test)]
    pub fn count_calls(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(calls::table.count().get_result(&mut *conn)?)
    }

    #[cfg(test)]
    pub fn set_call_ts(&self, call_id: i32, ts: i64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(calls::table.filter(calls::id.eq(call_id)))
            .set(calls::ts.eq(ts))
            .execute(&mut *conn)?;
        Ok(())
    }

    fn call_session(&self, call_id: i32) -> Result<String> {
        let mut conn = self.lock()?;
        calls::table
            .filter(calls::id.eq(call_id))
            .select(calls::session_id)
            .first(&mut *conn)
            .with_context(|| format!("call {call_id} has no session"))
    }

    /// T208: the up-to-two payload files are written before the transaction (SQLite cannot
    /// hold them), then their `archive` rows and the `call_io` row commit together. A failed
    /// insert rolls back the rows and removes only the files this call created — a body
    /// that repeats an existing sha is left alone, since another row may still reference it.
    pub fn insert_call_io(
        &self,
        call_id: i32,
        request: Option<&[u8]>,
        response: Option<&[u8]>,
        inline_cap: usize,
        archive_dir: Option<&Path>,
    ) -> Result<()> {
        let session = self.call_session(call_id)?;
        let (req_json, req_arch, req_bytes, req_sha, req_path, req_created, req_raw) =
            self.spill(request, inline_cap, archive_dir)?;
        let (res_json, res_arch, res_bytes, res_sha, res_path, res_created, res_raw) =
            self.spill(response, inline_cap, archive_dir)?;
        let mut created_files = Vec::new();
        if req_created {
            created_files.extend(req_path.clone());
        }
        if res_created {
            created_files.extend(res_path.clone());
        }
        let mut conn = self.lock()?;
        let result = conn.immediate_transaction(|conn| -> Result<()> {
            if let (Some(sha), Some(path)) = (&req_arch, &req_path) {
                insert_archive_row_conn(&mut *conn, sha, &session, req_bytes, path, None)?;
            }
            if let (Some(sha), Some(path)) = (&res_arch, &res_path) {
                insert_archive_row_conn(&mut *conn, sha, &session, res_bytes, path, None)?;
            }
            diesel::insert_into(call_io::table)
                .values((
                    call_io::call_id.eq(call_id),
                    call_io::request_bytes.eq(req_bytes),
                    call_io::response_bytes.eq(res_bytes),
                    call_io::request_sha256.eq(req_sha.as_deref()),
                    call_io::response_sha256.eq(res_sha.as_deref()),
                    call_io::request_json.eq(req_json.as_deref()),
                    call_io::response_json.eq(res_json.as_deref()),
                    call_io::request_archive.eq(req_arch.as_deref()),
                    call_io::response_archive.eq(res_arch.as_deref()),
                    call_io::request_raw.eq(req_raw.as_deref()),
                    call_io::response_raw.eq(res_raw.as_deref()),
                ))
                .execute(&mut *conn)?;
            Ok(())
        });
        if result.is_err() {
            for p in &created_files {
                // Another writer may have committed a row for the same sha after our write.
                let sha = p.file_name().map(|f| f.to_string_lossy().into_owned());
                let referenced = archive::table
                    .filter(archive::id.eq(sha.unwrap_or_default()))
                    .count()
                    .get_result::<i64>(&mut *conn)
                    .map_or(true, |n| n > 0);
                if !referenced {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        result
    }

    /// Stages a body for `insert_call_io`: over `cap` it is written to `archive_dir` right
    /// away (files live outside SQLite), but its `archive` row is left to the caller's
    /// transaction. The last two fields are `Some`/`true` only when this call's write
    /// created the file, so a failed transaction knows which files are safe to remove.
    fn spill(&self, body: Option<&[u8]>, cap: usize, archive_dir: Option<&Path>) -> Result<Spill> {
        let Some(body) = body else {
            return Ok((None, None, 0, None, None, false, None));
        };
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        if body.len() <= cap {
            let (text, sha, raw) = inline_body(body);
            return Ok((Some(text), None, n, Some(sha), None, false, raw));
        }
        // Over cap: metadata always. Archive only when a directory is supplied (never on
        // hook) — T201: without one, the sha is never written or expanded from anywhere,
        // so the hash itself is skipped too rather than paying a full pass over a body the
        // hook path can only ever throw away. The archive file already holds the exact
        // bytes, so no `raw` column is needed here.
        let Some(dir) = archive_dir else {
            return Ok((None, None, n, None, None, false, None));
        };
        let sha = hex_sha256(body);
        let (path, created) = write_archive_file(dir, &sha, body)?;
        Ok((
            None,
            Some(sha.clone()),
            n,
            Some(sha),
            Some(path),
            created,
            None,
        ))
    }

    /// Write `body` to `dir/<sha256>` and upsert the `archive` row. Returns the id.
    pub fn put_archive(&self, session: &str, body: &[u8], dir: &Path) -> Result<String> {
        let sha = hex_sha256(body);
        self.write_archive(session, body, &sha, dir, None)?;
        Ok(sha)
    }

    /// [`Self::put_archive`], tagged with the context window that wrote it (T127): a
    /// sub-agent's `agent_id`, or `None` for the main window. The first (session, context)
    /// pair to archive a given body owns the row — the same "other writers never dedup
    /// content they did not archive themselves" tradeoff [`Self::archive_in_session`] already
    /// makes across sessions, now also made across contexts within one session.
    pub fn put_archive_for(
        &self,
        session: &str,
        body: &[u8],
        dir: &Path,
        agent_id: Option<&str>,
    ) -> Result<String> {
        let sha = hex_sha256(body);
        self.write_archive(session, body, &sha, dir, agent_id)?;
        Ok(sha)
    }

    /// T65.1: one PK lookup on `archive.id` (= sha256) scoped to `session` and, since T127,
    /// to `agent_id` — the sub-agent's context window, or `None` for the main one. A body
    /// the row's own writer never saw in its context returns no hit, so the caller prints
    /// the body it actually has rather than a pointer to bytes it never received.
    /// `turns` is later `measurements` in that session (a proxy for "N turns ago"); 0 if none.
    pub fn archive_in_session(
        &self,
        session: &str,
        sha: &str,
        agent_id: Option<&str>,
    ) -> Result<Option<(String, u64)>> {
        let mut conn = self.lock()?;
        // Correlated subquery: turns is later measurements in archive's own session, as a
        // scalar column on the archive row — `.single_value()` keeps it one query.
        let turns = measurements::table
            .filter(measurements::session.eq(archive::session))
            .filter(measurements::ts.gt(archive::ts))
            .count()
            .single_value();
        let query = archive::table
            .filter(archive::id.eq(sha))
            .filter(archive::session.eq(session))
            .select((archive::id, turns))
            .into_boxed();
        // SQL `= NULL` never matches, so the "main window" side needs `IS NULL` instead.
        let query = match agent_id {
            Some(a) => query.filter(archive::agent_id.eq(a.to_owned())),
            None => query.filter(archive::agent_id.is_null()),
        };
        let row: Option<(String, Option<i64>)> = query.first(&mut *conn).optional()?;
        Ok(row.map(|(id, turns)| (id, turns.unwrap_or(0).max(0) as u64)))
    }

    /// The one archive write behind [`Self::put_archive`]: the body under its sha256 in
    /// `dir`, then one row per distinct body (the same body twice — T5.3 repeat requests —
    /// is one row). `tool` stays NULL: neither caller knows which plugin archived, and the
    /// column used to say `cmd` for every plugin. `call_io` spills stage the file the same
    /// way ([`write_archive_file`]) but insert the row inside their own transaction (T208).
    fn write_archive(
        &self,
        session: &str,
        body: &[u8],
        sha: &str,
        dir: &Path,
        agent_id: Option<&str>,
    ) -> Result<()> {
        let (path, created) = write_archive_file(dir, sha, body)?;
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        let mut conn = self.lock()?;
        let result = insert_archive_row_conn(&mut conn, sha, session, n, &path, agent_id);
        if result.is_err() && created {
            let _ = std::fs::remove_file(&path);
        }
        result
    }

    /// T5.3: the persisted decision for this `tool_use_id`, scoped to `session`.
    ///
    /// Scoping is what keeps one session's pointer out of another's context: a decision is
    /// an `(archive id, pointer)` pair cut from the payload it replaced, so replaying a
    /// foreign one overwrites a live tool result with unrelated head/tail lines — and the
    /// payload it overwrote was never archived, so D4 has nothing to expand.
    pub fn archive_decision(
        &self,
        session: &str,
        tool_use_id: &str,
    ) -> Result<Option<ArchiveDecision>> {
        let mut conn = self.lock()?;
        let row: Option<(String, String, bool)> = archive_decisions::table
            .filter(archive_decisions::session.eq(session))
            .filter(archive_decisions::tool_use_id.eq(tool_use_id))
            .select((
                archive_decisions::archive_id,
                archive_decisions::pointer,
                archive_decisions::expanded_ts.is_not_null(),
            ))
            .first(&mut *conn)
            .optional()?;
        Ok(row.map(|(archive_id, pointer, expanded)| ArchiveDecision {
            archive_id,
            pointer,
            expanded,
        }))
    }

    /// T5.3: persist a decision. First writer wins — the pointer must never change.
    pub fn put_archive_decision(
        &self,
        tool_use_id: &str,
        archive_id: &str,
        session: &str,
        pointer: &str,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_or_ignore_into(archive_decisions::table)
            .values((
                archive_decisions::tool_use_id.eq(tool_use_id),
                archive_decisions::archive_id.eq(archive_id),
                archive_decisions::session.eq(session),
                archive_decisions::pointer.eq(pointer),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// This session's archived tool results still in the live window, newest first (T58.2).
    pub fn session_live_archives(&self, session: &str) -> Result<Vec<(String, String, i64)>> {
        let mut conn = self.lock()?;
        let rows: Vec<(String, Option<String>, i64)> = archive_decisions::table
            .inner_join(archive::table)
            .filter(archive_decisions::session.eq(session))
            .order((archive::ts.desc(), archive::id.desc()))
            .select((archive::id, archive::tool, archive::bytes))
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(id, tool, bytes)| {
                let tool = tool.filter(|t| !t.is_empty()).unwrap_or_else(|| "-".into());
                (id, tool, bytes)
            })
            .collect())
    }

    /// Any pointer text for one archive id (T36.2: attribute expand rows to toon vs archive;
    /// T55.11: the expander — CLI session `expand`, MCP `mcp-<pid>` — never shares a session
    /// with the proxy that wrote the decision, so the lookup is by archive id alone).
    pub fn live_zone_pointer(&self, archive_id: &str) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        archive_decisions::table
            .filter(archive_decisions::archive_id.eq(archive_id))
            .order((archive_decisions::tool_use_id, archive_decisions::session))
            .select(archive_decisions::pointer)
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// T5.4/T55.11: an `expand <id>` freezes every decision pointing at that archive id.
    /// The expander's session is never the writer's, so the freeze is keyed by archive id
    /// alone: every session following that pointer starts receiving the original from its
    /// next request (more tokens; never wrong bytes — the archive holds the exact payload).
    /// Returns how many decisions changed (0 = nothing pointed at the id).
    ///
    /// Prefer [`Self::mark_expanded_recorded`] when the freeze and its expand `Measurement`
    /// must commit together.
    pub fn mark_expanded(&self, archive_id: &str) -> Result<usize> {
        let mut conn = self.lock()?;
        mark_expanded_conn(&mut conn, archive_id)
    }

    /// T208: freeze the decision and insert its expand `Measurement` in one transaction — a
    /// crash or a failed insert (e.g. an overflowing `Measurement` field) rolls back the
    /// freeze too, instead of leaving the decision expanded with no ledger row and
    /// `report_expand.cost` under-counted. `m` is only recorded when something was actually
    /// frozen, matching [`Self::mark_expanded`]'s callers. Returns the freeze count.
    pub fn mark_expanded_recorded(
        &self,
        session: &str,
        archive_id: &str,
        m: &Measurement,
    ) -> Result<usize> {
        let mut conn = self.lock()?;
        conn.immediate_transaction(|conn| -> Result<usize> {
            let n = mark_expanded_conn(&mut *conn, archive_id)?;
            if n > 0 {
                insert_measurement_conn(&mut *conn, session, m, None)?;
            }
            Ok(n)
        })
    }

    /// `(decisions, expanded)` — the expand rate is the archive plugin's honesty metric (T5.4).
    pub fn archive_decision_counts(&self) -> Result<(i64, i64)> {
        let mut conn = self.lock()?;
        let total: i64 = archive_decisions::table.count().get_result(&mut *conn)?;
        let expanded: i64 = archive_decisions::table
            .filter(archive_decisions::expanded_ts.is_not_null())
            .count()
            .get_result(&mut *conn)?;
        Ok((total, expanded))
    }

    /// The exact request bytes recorded for a call: `call_io.request_raw` when present (T211
    /// — an inline body that was not valid UTF-8), else inline `request_json` (valid UTF-8,
    /// so its bytes already are the wire bytes), else the archive. A row written before T211
    /// has no `request_raw` and falls back to the lossy `request_json` text, lazily.
    pub fn call_io_request(&self, call_id: i32) -> Result<Option<Vec<u8>>> {
        // `(request_json, request_raw, request_archive)`.
        type RequestRow = (Option<String>, Option<Vec<u8>>, Option<String>);
        let row: Option<RequestRow> = {
            let mut conn = self.lock()?;
            call_io::table
                .find(call_id)
                .select((
                    call_io::request_json,
                    call_io::request_raw,
                    call_io::request_archive,
                ))
                .first(&mut *conn)
                .optional()?
        };
        match row {
            Some((_, Some(raw), _)) => Ok(Some(raw)),
            Some((Some(json), None, _)) => Ok(Some(json.into_bytes())),
            Some((None, None, Some(id))) => self.get_archive(&id, None),
            _ => Ok(None),
        }
    }

    /// Path and bytes for `rtok expand <id>`. `None` if the id is unknown
    /// or the payload file is gone. `dir` is the live `[core] archive_dir`;
    /// the stored path is only a fallback for rows written under an old dir.
    pub fn get_archive(&self, id: &str, dir: Option<&Path>) -> Result<Option<Vec<u8>>> {
        let mut conn = self.lock()?;
        let stored: Option<String> = archive::table
            .find(id)
            .select(archive::path)
            .first(&mut *conn)
            .optional()?;
        drop(conn);
        let Some(stored) = stored else {
            return Ok(None);
        };
        let mut paths = Vec::new();
        if let Some(d) = dir {
            paths.push(d.join(id));
        }
        let stored = PathBuf::from(stored);
        if !paths.iter().any(|p| p == &stored) {
            paths.push(stored);
        }
        for p in paths {
            match std::fs::read(&p) {
                Ok(b) => return Ok(Some(b)),
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(e) => return Err(e).with_context(|| p.display().to_string()),
            }
        }
        Ok(None)
    }

    /// Size-only variant of [`Self::get_archive`] for hot paths (T55.16: the guard deny):
    /// the `archive` row's `bytes` when the payload file still exists (`dir/<id>`, else
    /// the stored path), `None` otherwise. Never reads the body.
    pub fn archive_size(&self, id: &str, dir: Option<&Path>) -> Result<Option<u64>> {
        let mut conn = self.lock()?;
        let row: Option<(i64, String)> = archive::table
            .find(id)
            .select((archive::bytes, archive::path))
            .first(&mut *conn)
            .optional()?;
        drop(conn);
        let Some((bytes, stored)) = row else {
            return Ok(None);
        };
        let mut paths = Vec::new();
        if let Some(d) = dir {
            paths.push(d.join(id));
        }
        let stored_path = PathBuf::from(stored);
        if !paths.contains(&stored_path) {
            paths.push(stored_path);
        }
        Ok(paths
            .iter()
            .find(|p| std::fs::metadata(p).is_ok())
            .map(|_| bytes.max(0) as u64))
    }

    /// Insert a note (T2.5 checkpoints, later memory).
    pub fn insert_note(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<i32> {
        let mut conn = self.lock()?;
        diesel::insert_into(notes::table)
            .values(note_values(project, kind, title, body))
            .returning(notes::id)
            .get_result(&mut *conn)
            .map_err(Into::into)
    }

    /// Insert only if `(project, kind, title)` is still free; `None` when a row already
    /// holds that topic key (T209: `memory import` must never let an older export
    /// overwrite a newer local body — unlike [`Store::upsert_note`], this never touches
    /// an existing row). `INSERT OR IGNORE` names no conflict target, so it works against
    /// the `notes_topic` expression index without the raw SQL `upsert_note` needs;
    /// `RETURNING` yields no row when the insert was ignored, which `.optional()` reads
    /// as the "already there" case.
    pub fn insert_note_if_absent(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<Option<i32>> {
        let mut conn = self.lock()?;
        diesel::insert_or_ignore_into(notes::table)
            .values(note_values(project, kind, title, body))
            .returning(notes::id)
            .get_result(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// One row per `(project, kind, title)` — the title is the topic key (T66.1). An
    /// existing row gets the new body and a fresh `ts`; returns `(id, updated)`.
    ///
    /// T209: this used to be a SELECT for the existing id followed by an UPDATE or
    /// INSERT, with the mutex dropped before the INSERT — two writers (hooks, MCP, proxy
    /// and `otel flush` are separate processes) racing the same topic key could both
    /// insert. The write below is one atomic `INSERT … ON CONFLICT … DO UPDATE`, backed
    /// by the `notes_topic` UNIQUE index (migration 0020), so a race resolves inside
    /// SQLite instead of in this gap.
    ///
    /// `notes_topic` indexes `COALESCE(project, '')` rather than `project`: SQLite treats
    /// NULL as a distinct value in a UNIQUE index, so a plain `(project, kind, title)`
    /// index would not stop two NULL-project ("no project") notes from duplicating. Diesel's
    /// `on_conflict` can only target a column tuple, not an expression index, so this one
    /// statement is raw SQL — the DSL cannot express an expression conflict target.
    pub fn upsert_note(
        &self,
        project: Option<&str>,
        kind: &str,
        title: &str,
        body: &str,
    ) -> Result<(i32, bool)> {
        let mut conn = self.lock()?;
        // Informational only (callers report "created" vs "updated"): read before the
        // atomic write below, so a true concurrent race can make it stale without ever
        // producing a duplicate row — the UNIQUE index and the single statement own that.
        let mut existed_q = notes::table
            .filter(notes::kind.eq(kind))
            .filter(notes::title.eq(title))
            .select(notes::id)
            .into_boxed();
        existed_q = match project {
            Some(p) => existed_q.filter(notes::project.eq(p)),
            None => existed_q.filter(notes::project.is_null()),
        };
        let updated = existed_q.first::<i32>(&mut *conn).optional()?.is_some();

        let id = sql_query(
            "INSERT INTO notes (project, kind, title, body) VALUES (?, ?, ?, ?)
             ON CONFLICT (COALESCE(project, ''), kind, title) DO UPDATE SET
                 body = excluded.body,
                 ts = unixepoch(),
                 -- An explicit re-save revives the topic: a retired tombstone does not
                 -- outlive the human/agent writing the note again (T69.1).
                 retired = NULL,
                 superseded_by = NULL
             RETURNING id",
        )
        .bind::<Nullable<Text>, _>(project)
        .bind::<Text, _>(kind)
        .bind::<Text, _>(title)
        .bind::<Text, _>(body)
        .get_result::<UpsertedId>(&mut *conn)?
        .id;
        Ok((id, updated))
    }

    /// Every note but the session-local `checkpoint:*` / `session:*` rows, id order
    /// (`memory export`, T66.2 / T71.2): `(project, kind, title, body)`.
    #[allow(clippy::type_complexity)]
    pub fn list_notes(
        &self,
        project: Option<&str>,
    ) -> Result<Vec<(Option<String>, String, String, String)>> {
        let mut conn = self.lock()?;
        let mut q = notes::table
            .filter(notes::kind.not_like("checkpoint:%"))
            .filter(notes::kind.not_like("session:%"))
            .order(notes::id.asc())
            .select((notes::project, notes::kind, notes::title, notes::body))
            .into_boxed();
        if let Some(p) = project {
            q = q.filter(notes::project.eq(p));
        }
        q.load(&mut *conn).map_err(Into::into)
    }

    pub fn latest_note_for_project(
        &self,
        project: Option<&str>,
        kind_prefix: &str,
    ) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        let mut q = notes::table
            .filter(notes::kind.like(format!("{kind_prefix}%")))
            .order(notes::id.desc())
            .select(notes::body)
            .into_boxed();
        if let Some(p) = project {
            q = q.filter(notes::project.eq(p));
        } else {
            q = q.filter(notes::project.is_null());
        }
        q.first(&mut *conn).optional().map_err(Into::into)
    }

    /// Newest note body for `kind`, if any.
    pub fn latest_note(&self, kind: &str) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        notes::table
            .filter(notes::kind.eq(kind))
            .order(notes::id.desc())
            .select(notes::body)
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// Session ids that already have a compaction (`checkpoint:<id>`) or handoff
    /// (`session:<id>`) note. Suffixes only; a session with both kinds is one id.
    pub fn checkpoint_session_ids(&self) -> Result<Vec<String>> {
        let mut conn = self.lock()?;
        let kinds: Vec<String> = notes::table
            .filter(
                notes::kind
                    .like("checkpoint:%")
                    .or(notes::kind.like("session:%")),
            )
            .select(notes::kind)
            .load(&mut *conn)?;
        Ok(kinds
            .into_iter()
            .filter_map(|k| {
                k.strip_prefix("checkpoint:")
                    .or_else(|| k.strip_prefix("session:"))
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
            })
            .collect())
    }

    /// Newest `session:*` note body for `project` (`None` = unbound), id order.
    pub fn latest_session_note(&self, project: Option<&str>) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        let mut q = notes::table
            .filter(notes::kind.like("session:%"))
            .order(notes::id.desc())
            .select(notes::body)
            .into_boxed();
        q = match project {
            Some(p) => q.filter(notes::project.eq(p)),
            None => q.filter(notes::project.is_null()),
        };
        q.first(&mut *conn).optional().map_err(Into::into)
    }

    /// Remember a Read/Bash result so `guard` can deny the duplicate (T2.6).
    /// Newest note titles for SessionStart recall (T6.2). Never bodies. Retired notes
    /// never recall; pinned ones lead (then newest-first) and both orders are id-stable.
    pub fn list_note_titles(
        &self,
        project: Option<&str>,
        limit: u32,
    ) -> Result<Vec<(i32, String)>> {
        let mut conn = self.lock()?;
        let lim = i64::from(limit.max(1));
        let mut q = notes::table
            .filter(notes::retired.is_null())
            .filter(notes::kind.not_like("checkpoint:%"))
            .filter(notes::kind.not_like("session:%"))
            .order((notes::pinned.desc(), notes::id.desc()))
            .limit(lim)
            .select((notes::id, notes::title))
            .into_boxed();
        if let Some(p) = project {
            q = q.filter(notes::project.eq(p));
        }
        q.load(&mut *conn).map_err(Into::into)
    }

    /// One note's lifecycle row (T69.1): kind/project for a revise, the retired line and
    /// pinned flag for `mem_get` / `mem_update`.
    pub fn note_row(&self, id: i32) -> Result<Option<NoteRow>> {
        let mut conn = self.lock()?;
        notes::table
            .find(id)
            .select((
                notes::id,
                notes::project,
                notes::kind,
                notes::title,
                notes::body,
                notes::retired,
                notes::superseded_by,
                notes::pinned,
            ))
            .first::<NoteRow>(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// Retire `id` — a tombstone, not a delete (D4): recall and search skip the note,
    /// `mem_get` keeps returning the body with a `retired` prefix. `superseded_by` names
    /// the replacement note. Returns `false` for an unknown id.
    pub fn retire_note(&self, id: i32, superseded_by: Option<i32>) -> Result<bool> {
        let mut conn = self.lock()?;
        let n = diesel::update(notes::table.find(id))
            .set((
                notes::retired.eq(diesel::dsl::sql::<Nullable<BigInt>>("unixepoch()")),
                notes::superseded_by.eq(superseded_by),
            ))
            .execute(&mut *conn)?;
        Ok(n == 1)
    }

    /// Last-written `rtok memory sync` block digest (T69.6 hand-edit guard).
    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        kv::table
            .filter(kv::key.eq(key))
            .select(kv::value)
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(kv::table)
            .values((kv::key.eq(key), kv::value.eq(value)))
            .on_conflict(kv::key)
            .do_update()
            .set(kv::value.eq(value))
            .execute(&mut *conn)?;
        Ok(())
    }

    pub fn kv_delete(&self, key: &str) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::delete(kv::table.filter(kv::key.eq(key))).execute(&mut *conn)?;
        Ok(())
    }

    /// Pin or unpin `id`; pinned notes lead recall (T69.1). Returns `false` for an
    /// unknown id.
    pub fn set_note_pinned(&self, id: i32, pinned: bool) -> Result<bool> {
        let mut conn = self.lock()?;
        let n = diesel::update(notes::table.find(id))
            .set(notes::pinned.eq(i32::from(pinned)))
            .execute(&mut *conn)?;
        Ok(n == 1)
    }

    /// All note bodies (for import dedupe, T6.3).
    pub fn note_bodies(&self) -> Result<Vec<String>> {
        let mut conn = self.lock()?;
        notes::table
            .select(notes::body)
            .load(&mut *conn)
            .map_err(Into::into)
    }

    /// Full note body by row id.
    pub fn get_note_body(&self, id: i32) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        notes::table
            .find(id)
            .select(notes::body)
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// FTS5 search, BM25 order, 120-char snippets.
    ///
    /// The query is user text (MCP `mem_search`), and FTS5 reads bare `*`, `(`, `-`, `AND`
    /// and friends as query syntax: `read(` was a syntax error, not an empty result. Every
    /// token is quoted into a phrase, so the words are searched for literally; a query with
    /// nothing quotable left returns no hits instead of an error.
    pub fn search_notes(&self, query: &str, limit: u32) -> Result<Vec<NoteHit>> {
        let Some(q) = fts_phrase_query(query) else {
            return Ok(Vec::new());
        };
        let mut conn = self.lock()?;
        let hits = sql_query(
            "SELECT n.id AS id, n.title AS title, substr(n.body, 1, 120) AS snippet
             FROM notes_fts f JOIN notes n ON n.id = f.rowid
             WHERE notes_fts MATCH ? AND n.retired IS NULL
             ORDER BY bm25(notes_fts)
             LIMIT ?",
        )
        .bind::<Text, _>(q)
        .bind::<Integer, _>(i32::try_from(limit).unwrap_or(5))
        .load::<NoteHitRow>(&mut *conn)?
        .into_iter()
        .map(|r| NoteHit {
            id: r.id,
            title: r.title,
            snippet: r.snippet,
        })
        .collect::<Vec<_>>();
        Ok(hits)
    }

    pub fn put_read_cache(
        &self,
        session: &str,
        path: &str,
        sha256: &str,
        archive_id: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        // `unixepoch()` has no typed-DSL form; bind Rust's now.
        let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
        diesel::insert_into(read_cache::table)
            .values((
                read_cache::session.eq(session),
                read_cache::path.eq(path),
                read_cache::sha256.eq(sha256),
                read_cache::archive_id.eq(archive_id),
            ))
            .on_conflict((read_cache::session, read_cache::path))
            .do_update()
            .set((
                read_cache::sha256.eq(sha256),
                read_cache::ts.eq(now),
                read_cache::archive_id.eq(archive_id),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// `(archive_id, ts)` for a prior Read/Bash in this session.
    pub fn get_read_cache(
        &self,
        session: &str,
        path: &str,
    ) -> Result<Option<(Option<String>, i64)>> {
        let mut conn = self.lock()?;
        read_cache::table
            .find((session, path))
            .select((read_cache::archive_id, read_cache::ts))
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    /// Drop cache rows for `path` and `path\t…` mode/range keys (T4.4). A prefix compare, not
    /// `LIKE`: `%` and `_` are ordinary file-name characters, and `LIKE` ignores ASCII case.
    pub fn clear_read_cache(&self, session: &str, path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let keyed = format!("{path}\t");
        let keyed_len = i32::try_from(keyed.chars().count()).unwrap_or(i32::MAX);
        diesel::delete(
            read_cache::table
                .filter(read_cache::session.eq(session))
                .filter(
                    read_cache::path
                        .eq(path)
                        .or(substr(read_cache::path, 1, keyed_len).eq(keyed)),
                ),
        )
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Request bodies of the newest `limit` hook calls in this session (T4.6
    /// edit window: a PreToolUse(Read) checks them for a recent Edit|Write).
    /// The live call's own row has no `call_io` yet, so it never matches itself.
    /// The newest `limit` hook bodies for this session, as JSON where the body was kept.
    ///
    /// A row whose body exceeded `core.call_io_inline_bytes` comes back as `""` rather than
    /// being left out: the caller (`read`'s edit window) must see that *something* happened
    /// it cannot read and fail open, instead of concluding no edit happened. Dropping these
    /// rows is what made a 70 KiB `Write` followed by a native `Read` of the same file end in
    /// a deny.
    pub fn recent_hook_inputs(&self, session: &str, limit: i64) -> Result<Vec<String>> {
        let mut conn = self.lock()?;
        let rows: Vec<Option<String>> = calls::table
            .inner_join(call_io::table)
            .filter(calls::session_id.eq(session))
            .filter(calls::kind.eq("hook"))
            .order(calls::id.desc())
            .limit(limit)
            .select(call_io::request_json)
            .load(&mut *conn)?;
        Ok(rows.into_iter().map(Option::unwrap_or_default).collect())
    }

    /// Like [`Store::recent_hook_inputs`], but filtered to rows whose `hook_event_name` is
    /// `event` (T202). `record_call` (`src/hooks/mod.rs`) already stores that name in
    /// `calls.name`, so the filter is a `WHERE` on an existing column, not a JSON re-scan:
    /// callers that only care about `PostToolUse` or `PreToolUse` rows no longer fetch and
    /// parse every other event type to find them.
    pub fn recent_hook_inputs_for_event(
        &self,
        session: &str,
        event: &str,
        limit: i64,
    ) -> Result<Vec<String>> {
        let mut conn = self.lock()?;
        let rows: Vec<Option<String>> = calls::table
            .inner_join(call_io::table)
            .filter(calls::session_id.eq(session))
            .filter(calls::kind.eq("hook"))
            .filter(calls::name.eq(event))
            .order(calls::id.desc())
            .limit(limit)
            .select(call_io::request_json)
            .load(&mut *conn)?;
        Ok(rows.into_iter().map(Option::unwrap_or_default).collect())
    }

    /// Hook/call rows in this session at or after `ts` (window for `guard`).
    pub fn calls_since(&self, session: &str, ts: i64) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(calls::table
            .filter(calls::session_id.eq(session))
            .filter(calls::ts.ge(ts))
            .count()
            .get_result(&mut *conn)?)
    }

    /// Measurement rows for `rtok stats --plugin <id>` (T3.6).
    pub fn list_measurements(&self, plugin: &str) -> Result<Vec<MeasRow>> {
        let mut conn = self.lock()?;
        measurements::table
            .filter(measurements::plugin.eq(plugin))
            .order(measurements::id.asc())
            .select((
                measurements::kind,
                measurements::before_bytes,
                measurements::after_bytes,
                measurements::est_before,
                measurements::est_after,
                measurements::ref_id,
            ))
            .load::<MeasRow>(&mut *conn)
            .map_err(Into::into)
    }

    /// Rows and est_before/est_after summed per `(plugin, kind)`, across every plugin the
    /// ledger has ever seen — not just the catalogue (T207). The one aggregate
    /// `report_window`, `report_savings`, `otel_saved_totals` and both `plugin_stats`
    /// read instead of each hand-rolling its own sum (or, for `otel_saved_totals`,
    /// dropping to raw SQL); callers decide what an `expand` group means (a cost, not a
    /// saving — `ReportSavings::saved`'s contract), this is just the read.
    pub fn measurement_totals(&self) -> Result<Vec<MeasurementTotal>> {
        use diesel::dsl::{count_star, sum};
        let mut conn = self.lock()?;
        let rows = measurements::table
            .group_by((measurements::plugin, measurements::kind))
            .select((
                measurements::plugin,
                measurements::kind,
                count_star(),
                sum(measurements::est_before),
                sum(measurements::est_after),
            ))
            .order((measurements::plugin, measurements::kind))
            .load::<(String, String, i64, Option<i64>, Option<i64>)>(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(
                |(plugin, kind, rows, est_before, est_after)| MeasurementTotal {
                    plugin,
                    kind,
                    rows,
                    est_before: est_before.unwrap_or(0),
                    est_after: est_after.unwrap_or(0),
                },
            )
            .collect())
    }

    /// Per `(project, kind)` note counts for `memory status` (T69.4).
    pub fn memory_note_aggs(&self, project: Option<&str>) -> Result<Vec<MemoryNoteKindAgg>> {
        use diesel::IntoSql;
        use diesel::dsl::{case_when, max, min};
        type Row = (
            Option<String>,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        );
        let mut conn = self.lock()?;
        // One statement for both `project` cases: `project.is_none()` is bound as a SQL
        // boolean literal and OR'd ahead of the equality check, so a `None` short-circuits
        // the whole condition to true (no project filter) while `Some(p)` falls through to
        // `notes::project.eq(p)` — the same filter the two-statement version used per branch.
        let no_project_filter = project.is_none().into_sql::<Bool>().nullable();
        let rows: Vec<Row> = notes::table
            .filter(notes::kind.not_like("checkpoint%"))
            .filter(no_project_filter.or(notes::project.eq(project.unwrap_or_default())))
            .group_by((notes::project, notes::kind))
            .select((
                notes::project,
                notes::kind,
                sum_bigint(
                    case_when::<_, _, BigInt>(notes::retired.is_null(), 1i64).otherwise(0i64),
                ),
                sum_bigint(
                    case_when::<_, _, BigInt>(
                        notes::retired.is_null().and(notes::pinned.ne(0)),
                        1i64,
                    )
                    .otherwise(0i64),
                ),
                sum_bigint(
                    case_when::<_, _, BigInt>(notes::retired.is_not_null(), 1i64).otherwise(0i64),
                ),
                sum_bigint(length(notes::body)),
                min(notes::ts),
                max(notes::ts),
            ))
            .order((notes::project, notes::kind))
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(
                |(project, kind, live, pinned, retired, body_bytes, oldest_ts, newest_ts)| {
                    MemoryNoteKindAgg {
                        project,
                        kind,
                        live: u64::try_from(live.unwrap_or(0)).unwrap_or(0),
                        pinned: u64::try_from(pinned.unwrap_or(0)).unwrap_or(0),
                        retired: u64::try_from(retired.unwrap_or(0)).unwrap_or(0),
                        body_bytes: body_bytes.unwrap_or(0),
                        oldest_ts: oldest_ts.unwrap_or(0),
                        newest_ts: newest_ts.unwrap_or(0),
                    }
                },
            )
            .collect())
    }

    /// SessionStart recall measurements in a time window (T69.4).
    pub fn memory_recall_totals(&self, since_unix: i64) -> Result<(u64, i64, i64)> {
        use diesel::dsl::count_star;
        let mut conn = self.lock()?;
        let (recalls, stood_for_bytes, injected_bytes): (i64, Option<i64>, Option<i64>) =
            measurements::table
                .filter(measurements::plugin.eq("memory"))
                .filter(measurements::kind.eq("recall"))
                .filter(measurements::ts.ge(since_unix))
                .select((
                    count_star(),
                    sum_bigint(measurements::before_bytes),
                    sum_bigint(measurements::after_bytes),
                ))
                .first(&mut *conn)?;
        Ok((
            u64::try_from(recalls).unwrap_or(0),
            stood_for_bytes.unwrap_or(0),
            injected_bytes.unwrap_or(0),
        ))
    }

    /// MCP `mem_search` / `mem_get` calls in a time window (T69.4).
    pub fn memory_mcp_calls(&self, since_unix: i64) -> Result<(u64, u64)> {
        use diesel::dsl::case_when;
        let mut conn = self.lock()?;
        let (mem_search, mem_get): (Option<i64>, Option<i64>) = calls::table
            .filter(calls::plugin.eq("memory"))
            .filter(calls::surface.eq("mcp"))
            .filter(calls::kind.eq("mcp_call"))
            .filter(calls::ts.ge(since_unix))
            .select((
                sum_bigint(
                    case_when::<_, _, BigInt>(calls::name.eq("mem_search"), 1i64).otherwise(0i64),
                ),
                sum_bigint(
                    case_when::<_, _, BigInt>(calls::name.eq("mem_get"), 1i64).otherwise(0i64),
                ),
            ))
            .first(&mut *conn)?;
        Ok((
            u64::try_from(mem_search.unwrap_or(0)).unwrap_or(0),
            u64::try_from(mem_get.unwrap_or(0)).unwrap_or(0),
        ))
    }

    /// Last `ref_id` on a measurement row for one session (T69.5 prompt_recall dedup).
    pub fn last_measurement_ref(
        &self,
        session: &str,
        plugin: &str,
        kind: &str,
    ) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        let ref_id: Option<Option<String>> = measurements::table
            .filter(measurements::session.eq(session))
            .filter(measurements::plugin.eq(plugin))
            .filter(measurements::kind.eq(kind))
            .order(measurements::id.desc())
            .select(measurements::ref_id)
            .first(&mut *conn)
            .optional()?;
        Ok(ref_id.flatten())
    }

    pub fn insert_tokens(
        &self,
        call_id: i32,
        plugin: Option<&str>,
        phase: &str,
        source: &str,
        n_tokens: i64,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(tokens::table)
            .values((
                tokens::call_id.eq(call_id),
                tokens::plugin.eq(plugin),
                tokens::phase.eq(phase),
                tokens::source.eq(source),
                tokens::n_tokens.eq(n_tokens),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Proxy ground truth (plan T5.1): one `usage` row per API request.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_usage(
        &self,
        session: &str,
        model: Option<&str>,
        api: &str,
        input: i64,
        cache_create: i64,
        cache_read: i64,
        output: i64,
        call_id: i32,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(usage::table)
            .values((
                usage::session.eq(session),
                usage::model.eq(model),
                usage::api.eq(api),
                usage::input.eq(input),
                usage::cache_create.eq(cache_create),
                usage::cache_read.eq(cache_read),
                usage::output.eq(output),
                usage::call_id.eq(call_id),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Test-only: one proxy `usage` turn — the session row, a bare `api_request` call
    /// and the `usage` row. Tests that need request bodies (cache-bust causes) still
    /// write their own `call_io`.
    #[cfg(test)]
    pub fn insert_proxy_turn(
        &self,
        session: &str,
        input: i64,
        cache_create: i64,
        cache_read: i64,
        output: i64,
    ) -> Result<()> {
        self.upsert_session(session, None, None, None, Some("proxy"))?;
        let id = self.insert_call(
            session,
            "proxy",
            "api_request",
            None,
            None,
            None,
            None,
            Some("/v1/messages"),
        )?;
        self.insert_usage(
            session,
            Some("m"),
            "anthropic",
            input,
            cache_create,
            cache_read,
            output,
            id,
        )
    }

    /// Provider counters for an api_request (plan T5.1): one `tokens` row,
    /// `phase = 'after'`, `source = 'provider'`, carrying the four counters. `total` comes
    /// from the wire ([`rtok_plugin_sdk`-side `Wire::provider_total`]): Anthropic's counters
    /// are disjoint and sum, OpenAI's `input` already contains the cached slice.
    pub fn insert_provider_tokens(
        &self,
        call_id: i32,
        total: i64,
        input: i64,
        cache_create: i64,
        cache_read: i64,
        output: i64,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(tokens::table)
            .values((
                tokens::call_id.eq(call_id),
                tokens::phase.eq("after"),
                tokens::source.eq("provider"),
                tokens::n_tokens.eq(total),
                tokens::input.eq(input),
                tokens::output.eq(output),
                tokens::cache_create.eq(cache_create),
                tokens::cache_read.eq(cache_read),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Sessions that have usage rows, oldest first (`rtok stats --cache`, T5.5).
    pub fn usage_sessions(&self) -> Result<Vec<String>> {
        use diesel::dsl::min;
        let mut conn = self.lock()?;
        usage::table
            .group_by(usage::session)
            .select(usage::session)
            .order((min(usage::ts), min(usage::id)))
            .load::<String>(&mut *conn)
            .map_err(Into::into)
    }

    /// Usage rows for one session, newest first (proxy Check, later `stats`).
    pub fn usage_rows(&self, session: &str) -> Result<Vec<UsageRow>> {
        let mut conn = self.lock()?;
        let rows = usage::table
            .filter(usage::session.eq(session))
            .order((usage::ts.desc(), usage::id.desc()))
            .select((
                usage::session,
                usage::model,
                usage::api,
                usage::input,
                usage::cache_create,
                usage::cache_read,
                usage::output,
                usage::call_id,
            ))
            .load::<(
                String,
                Option<String>,
                String,
                i64,
                i64,
                i64,
                i64,
                Option<i32>,
            )>(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(
                |(session, model, api, input, cache_create, cache_read, output, call_id)| {
                    UsageRow {
                        session,
                        model,
                        api,
                        input,
                        cache_create,
                        cache_read,
                        output,
                        call_id: call_id.map(i64::from),
                    }
                },
            )
            .collect())
    }

    /// The dashboard Overview's CTT and its last `turns` per-turn contexts, in the order of
    /// [`Self::usage_sessions`] then [`Self::usage_rows`] reversed (T15.3). Two reads: the
    /// Overview used to load every usage row, one query per session, on each 2 s tick.
    pub fn usage_ctt(&self, turns: i64) -> Result<(i64, Vec<i64>)> {
        let mut conn = self.lock()?;
        let ctt: Vec<Count> = sql_query(
            "SELECT COALESCE(SUM(ctx * (total - rn)), 0) AS n FROM (
                SELECT input + cache_create + cache_read AS ctx,
                       COUNT(*) OVER (PARTITION BY session) AS total,
                       ROW_NUMBER() OVER (PARTITION BY session ORDER BY ts, id) AS rn
                FROM usage)",
        )
        .load(&mut *conn)?;
        let mut tail: Vec<i64> = sql_query(
            "SELECT u.input + u.cache_create + u.cache_read AS n
             FROM usage u
             JOIN (SELECT session, MIN(ts) AS first_ts, MIN(id) AS first_id
                   FROM usage GROUP BY session) f ON f.session = u.session
             ORDER BY f.first_ts DESC, f.first_id DESC, u.ts DESC, u.id DESC
             LIMIT ?",
        )
        .bind::<BigInt, _>(turns)
        .load::<Count>(&mut *conn)?
        .into_iter()
        .map(|c| c.n)
        .collect();
        tail.reverse();
        Ok((ctt.first().map_or(0, |c| c.n), tail))
    }

    pub fn usage_by_api(&self) -> Result<Vec<ApiUsage>> {
        let mut conn = self.lock()?;
        let rows = usage::table
            .group_by(usage::api)
            .select((
                usage::api,
                sum_bigint(usage::input),
                sum_bigint(usage::cache_create),
                sum_bigint(usage::cache_read),
                sum_bigint(usage::output),
            ))
            .order(usage::api)
            .load::<(String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(api, input, cache_create, cache_read, output)| ApiUsage {
                api,
                input: input.unwrap_or(0),
                cache_create: cache_create.unwrap_or(0),
                cache_read: cache_read.unwrap_or(0),
                output: output.unwrap_or(0),
            })
            .collect())
    }

    /// Usage totals grouped by model (`rtok stats --price`, T49.1). One statement,
    /// like [`Self::usage_by_api`]: `NULL` models already group into one bucket under
    /// plain `GROUP BY model` (grouping treats every `NULL` as equal), so the SQL side
    /// only needs the raw column. The old raw SQL grouped by `COALESCE(model, 'unknown')`,
    /// so a `NULL`-model group and a row whose model is literally `"unknown"` merged into
    /// one row — replicated here by folding the `NULL → "unknown"` rows into a
    /// `BTreeMap<String, ModelUsage>` keyed by the display name, which also gives the sort
    /// (Diesel cannot validate a `CASE` as the same grouped expression once it also appears
    /// inside `ORDER BY`/`SELECT` — a Diesel 2.3.13 `GROUP BY`-over-computed-expression gap,
    /// not a raw-SQL fallback).
    pub fn usage_by_model(&self) -> Result<Vec<ModelUsage>> {
        type Row = (
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        );
        let mut conn = self.lock()?;
        let rows: Vec<Row> = usage::table
            .group_by(usage::model)
            .select((
                usage::model,
                sum_bigint(usage::input),
                sum_bigint(usage::cache_create),
                sum_bigint(usage::cache_read),
                sum_bigint(usage::output),
            ))
            .load(&mut *conn)?;
        let mut by_model: BTreeMap<String, ModelUsage> = BTreeMap::new();
        for (model, input, cache_create, cache_read, output) in rows {
            let model = model.unwrap_or_else(|| "unknown".to_string());
            let entry = by_model.entry(model.clone()).or_insert(ModelUsage {
                model,
                input: 0,
                cache_create: 0,
                cache_read: 0,
                output: 0,
            });
            entry.input += input.unwrap_or(0);
            entry.cache_create += cache_create.unwrap_or(0);
            entry.cache_read += cache_read.unwrap_or(0);
            entry.output += output.unwrap_or(0);
        }
        Ok(by_model.into_values().collect())
    }

    /// One row per session the store knows (T25.1, D27): the single read behind the
    /// Sessions page and `rtok agent sessions`. One statement — `usage` (by `session`)
    /// and `calls` (by `session_id`) are pre-aggregated per session because joining
    /// both flat to `sessions` would fan every `usage` row out across every `calls`
    /// row and multiply the token sums. `since` windows on `sessions.started_at`
    /// (unix seconds; 0 = every session) while the totals stay whole-session — a
    /// session's cost is what it spent, not what it spent after an arbitrary line.
    /// Newest first; `ended_at IS NULL` is "live". A session with no `usage` rows yet
    /// still appears, zeroed, with `last_activity = started_at`.
    pub fn session_totals(&self, since: i64) -> Result<Vec<SessionTotals>> {
        self.recent_session_totals(since, -1)
    }

    /// [`Store::session_totals`] capped at the newest `limit` sessions in SQL, so the Sessions
    /// page does not load every session to show a screenful. A negative `limit` is no cap.
    pub fn recent_session_totals(&self, since: i64, limit: i64) -> Result<Vec<SessionTotals>> {
        let mut conn = self.lock()?;
        // tot: the four sums per session. last_u: the newest `usage` row's api and
        // model — what the session is spending on now. act: last activity as MAX(ts)
        // over both tables (no `[agents] idle_secs`; T25.0's clause was not built).
        // prov: the newest provider-bearing `calls` row's `providers.slug`.
        sql_query(
            "WITH tot AS (
                 SELECT session AS sid, SUM(input) AS input, SUM(cache_create) AS cache_create,
                        SUM(cache_read) AS cache_read, SUM(output) AS output
                 FROM usage GROUP BY session
             ),
             last_u AS (
                 SELECT session AS sid, api, model FROM usage
                 WHERE id IN (SELECT MAX(id) FROM usage GROUP BY session)
             ),
             act AS (
                 SELECT sid, MAX(ts) AS ts FROM (
                     SELECT session AS sid, ts FROM usage
                     UNION ALL
                     SELECT session_id AS sid, ts FROM calls
                 ) GROUP BY sid
             ),
             prov AS (
                 SELECT c.session_id AS sid, p.slug AS provider
                 FROM calls c JOIN providers p ON p.id = c.provider_id
                 WHERE c.id IN (SELECT MAX(id) FROM calls
                                WHERE provider_id IS NOT NULL GROUP BY session_id)
             )
             SELECT s.id AS id, h.slug AS host, s.project AS project,
                    prov.provider AS provider, last_u.api AS api, last_u.model AS model,
                    COALESCE(tot.input, 0) AS input,
                    COALESCE(tot.cache_create, 0) AS cache_create,
                    COALESCE(tot.cache_read, 0) AS cache_read,
                    COALESCE(tot.output, 0) AS output,
                    s.started_at AS started_at,
                    COALESCE(act.ts, s.started_at) AS last_activity,
                    s.ended_at AS ended_at
             FROM sessions s
             LEFT JOIN hosts h ON h.id = s.host_id
             LEFT JOIN tot ON tot.sid = s.id
             LEFT JOIN last_u ON last_u.sid = s.id
             LEFT JOIN act ON act.sid = s.id
             LEFT JOIN prov ON prov.sid = s.id
             WHERE s.started_at >= ?
             ORDER BY s.started_at DESC, s.id
             LIMIT ?",
        )
        .bind::<BigInt, _>(since)
        .bind::<BigInt, _>(limit)
        .load::<SessionTotals>(&mut *conn)
        .map_err(Into::into)
    }

    /// The Calls page's one read (T15.5, D27): the newest `limit` `calls` rows, newest
    /// first, each with the slugs its ids point at and — when the call recorded one —
    /// its newest `usage` row linked (the same linkage [`Store::call_detail`] serves the
    /// otel span). One statement, so no renderer can re-derive a field differently.
    pub fn recent_calls(&self, limit: i64) -> Result<Vec<CallRow>> {
        let mut conn = self.lock()?;
        sql_query(
            "SELECT c.id AS id, c.ts AS ts, c.session_id AS session, c.surface AS surface,
                    c.kind AS kind, c.plugin AS plugin, c.name AS name,
                    c.parent_id AS parent_id, c.ms AS ms, c.ok AS ok, c.error AS error,
                    h.slug AS host, p.slug AS provider, m.slug AS model,
                    u.api AS api, u.input AS input, u.cache_create AS cache_create,
                    u.cache_read AS cache_read, u.output AS output
             FROM calls c
             LEFT JOIN hosts h ON h.id = c.host_id
             LEFT JOIN providers p ON p.id = c.provider_id
             LEFT JOIN models m ON m.id = c.model_id
             LEFT JOIN usage u ON u.call_id = c.id
                  AND u.id = (SELECT MAX(id) FROM usage WHERE call_id = c.id)
             ORDER BY c.id DESC
             LIMIT ?",
        )
        .bind::<BigInt, _>(limit)
        .load::<CallRow>(&mut *conn)
        .map_err(Into::into)
    }

    /// `models.slug` recorded on a call — the proxy Check asserts it equals the request `model`.
    pub fn model_slug_of_call(&self, call_id: i32) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        calls::table
            .inner_join(schema::models::table)
            .filter(calls::id.eq(call_id))
            .select(schema::models::slug)
            .first(&mut *conn)
            .optional()
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_log(
        &self,
        level: &str,
        source: &str,
        name: &str,
        message: &str,
        session: Option<&str>,
        call_id: Option<i32>,
        plugin: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(logs::table)
            .values((
                logs::level.eq(level),
                logs::source.eq(source),
                logs::name.eq(name),
                logs::message.eq(message),
                logs::session.eq(session),
                logs::call_id.eq(call_id),
                logs::plugin.eq(plugin),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Drop `calls` older than `days` with the rows that only describe them (`logs`, `tokens`,
    /// `call_io`). Ledger rows that point at a dropped call — `usage`, `measurements`, a newer
    /// child call — are kept and detached: a saving is not deleted with its call, and without the
    /// detach `foreign_keys = ON` refused the delete after the first three had already committed.
    /// One transaction and one cutoff, so it is all of it or none of it.
    /// The archive paths `run_retention` would delete for `core.retain_calls_days`, still on
    /// disk — read-only, nothing is removed (T182 `agents junk clear` dry run).
    pub fn archives_pending_retention(&self, retain_calls_days: u32) -> Result<Vec<PathBuf>> {
        let days = i64::from(retain_calls_days);
        if days <= 0 {
            return Ok(Vec::new());
        }
        let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let mut conn = self.lock()?;
        Ok(doomed_archives(&mut conn, cutoff)?
            .into_iter()
            .map(|a| PathBuf::from(a.path))
            .collect())
    }

    pub fn purge_calls_older_than(&self, days: i64) -> Result<usize> {
        if days <= 0 {
            return Ok(0);
        }
        let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let old = "(SELECT id FROM calls WHERE ts < ?1)";
        let mut conn = self.lock()?;
        // T75: every surface opens this one file, and a purge starting while another
        // process held the write lock came back "database is locked" — the deferred
        // read-then-write transaction could lose instantly (a snapshot upgrade skips
        // the busy handler) or after the steady 1 s, and `mcp`/`proxy` died on it at
        // session start. Same contract as `migrate`: take the writer lock up front
        // under the maintenance window, and restore the hook's 1 s bound after,
        // whatever happened inside.
        conn.batch_execute("PRAGMA busy_timeout = 30000;")?;
        let purged = conn.exclusive_transaction::<_, anyhow::Error, _>(|c| {
            let doomed = doomed_archives(c, cutoff)?;
            for sql in [
                format!("DELETE FROM logs WHERE ts < ?1 OR call_id IN {old}"),
                format!("DELETE FROM tokens WHERE ts < ?1 OR call_id IN {old}"),
                format!("DELETE FROM call_io WHERE call_id IN {old}"),
                format!("UPDATE usage SET call_id = NULL WHERE call_id IN {old}"),
                format!("UPDATE measurements SET call_id = NULL WHERE call_id IN {old}"),
                format!("UPDATE calls SET parent_id = NULL WHERE parent_id IN {old}"),
            ] {
                sql_query(sql).bind::<BigInt, _>(cutoff).execute(c)?;
            }
            for arch in &doomed {
                sql_query("DELETE FROM archive_decisions WHERE archive_id = ?1")
                    .bind::<Text, _>(&arch.id)
                    .execute(c)?;
                sql_query("UPDATE read_cache SET archive_id = NULL WHERE archive_id = ?1")
                    .bind::<Text, _>(&arch.id)
                    .execute(c)?;
                sql_query("DELETE FROM archive WHERE id = ?1")
                    .bind::<Text, _>(&arch.id)
                    .execute(c)?;
            }
            let n = sql_query("DELETE FROM calls WHERE ts < ?1")
                .bind::<BigInt, _>(cutoff)
                .execute(c)?;
            Ok((
                n,
                doomed
                    .into_iter()
                    .map(|a| PathBuf::from(a.path))
                    .collect::<Vec<_>>(),
            ))
        });
        set_busy(&mut conn, self.wait.busy)?;
        let paths = purged?;
        for path in paths.1 {
            let _ = std::fs::remove_file(path);
        }
        Ok(paths.0)
    }

    /// Apply `core.retain_calls_days` (0 = keep forever). Proxy and MCP call this once at session
    /// start.
    pub fn run_retention(&self, retain_calls_days: u32) -> Result<usize> {
        self.purge_calls_older_than(i64::from(retain_calls_days))
    }

    #[cfg(test)]
    pub fn set_query_only(&self) -> Result<()> {
        self.lock()?.batch_execute("PRAGMA query_only = ON;")?;
        Ok(())
    }
}

#[derive(QueryableByName)]
struct ArchPath {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    path: String,
}

/// Archive rows whose only `call_io` references are all older than `cutoff` and that carry no
/// decision or read-cache row — shared by [`Store::purge_calls_older_than`] (which deletes them)
/// and [`Store::archives_pending_retention`] (which only previews the same set, T182).
fn doomed_archives(c: &mut SqliteConnection, cutoff: i64) -> Result<Vec<ArchPath>> {
    let old = "(SELECT id FROM calls WHERE ts < ?1)";
    Ok(sql_query(format!(
        "SELECT DISTINCT a.id, a.path FROM archive a
             WHERE a.id IN (
               SELECT request_archive FROM call_io
               WHERE call_id IN {old} AND request_archive IS NOT NULL
               UNION
               SELECT response_archive FROM call_io
               WHERE call_id IN {old} AND response_archive IS NOT NULL
             )
             AND NOT EXISTS (
               SELECT 1 FROM call_io c
               WHERE c.call_id NOT IN {old}
               AND (c.request_archive = a.id OR c.response_archive = a.id)
             )
             AND NOT EXISTS (
               SELECT 1 FROM archive_decisions d WHERE d.archive_id = a.id
             )
             AND NOT EXISTS (
               SELECT 1 FROM read_cache r WHERE r.archive_id = a.id
             )"
    ))
    .bind::<BigInt, _>(cutoff)
    .load(c)?)
}

/// `(inline json, archive sha, byte count, content sha, archive file path, file created by
/// this call, raw bytes)`. The archive fields are `Some`/`true` together only when the body
/// was spilled to disk — see [`Store::spill`] (T208). `raw` is `Some` only for an inline body
/// that is not valid UTF-8 (T211) — see [`inline_body`].
type Spill = (
    Option<String>,
    Option<String>,
    i64,
    Option<String>,
    Option<PathBuf>,
    bool,
    Option<Vec<u8>>,
);

/// `(host slug, project, cwd)` — [`Store::session_row`].
#[cfg(test)]
type SessionRow = (Option<String>, Option<String>, Option<String>);

/// Text stored inline for display, the sha256 of `body`'s raw bytes, and — only when `body`
/// is not valid UTF-8 — the exact bytes to keep alongside the lossy text (T211). Valid UTF-8
/// already round-trips losslessly through the `TEXT` column (its bytes are `body`'s bytes),
/// so the `BLOB` column is written only for the lossy case, avoiding doubled storage for the
/// common path. The sha is always over `body` itself, so it matches the old behavior for
/// valid UTF-8 and now verifies the true wire bytes for invalid UTF-8 too.
fn inline_body(body: &[u8]) -> (String, String, Option<Vec<u8>>) {
    let sha = hex_sha256(body);
    match std::str::from_utf8(body) {
        Ok(s) => (s.to_owned(), sha, None),
        Err(_) => (
            String::from_utf8_lossy(body).into_owned(),
            sha,
            Some(body.to_vec()),
        ),
    }
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Write `body` to `dir/<sha>`, content-addressed so the same body written twice is the same
/// file (T208: `insert_call_io`'s two spills can share a body with an earlier archive row).
/// The bool says whether this call created the file (`false` when it already held this exact
/// content) — only a file this call created is safe to remove if the caller's insert fails.
fn write_archive_file(dir: &Path, sha: &str, body: &[u8]) -> Result<(PathBuf, bool)> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(sha);
    let created = !path.exists();
    std::fs::write(&path, body)?;
    Ok((path, created))
}

/// The `archive` row behind a file [`write_archive_file`] already wrote. `on_conflict …
/// do_nothing`: the same body archived twice (T5.3 repeat requests) is one row.
fn insert_archive_row_conn(
    conn: &mut SqliteConnection,
    sha: &str,
    session: &str,
    bytes: i64,
    path: &Path,
    agent_id: Option<&str>,
) -> Result<()> {
    diesel::insert_into(archive::table)
        .values((
            archive::id.eq(sha),
            archive::session.eq(session),
            archive::bytes.eq(bytes),
            archive::path.eq(path.to_string_lossy().as_ref()),
            archive::sha256.eq(sha),
            archive::agent_id.eq(agent_id),
        ))
        .on_conflict(archive::id)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

/// The `archive_decisions.expanded_ts` update behind [`Store::mark_expanded`] and
/// [`Store::mark_expanded_recorded`].
fn mark_expanded_conn(conn: &mut SqliteConnection, archive_id: &str) -> Result<usize> {
    // `unixepoch()` has no typed-DSL form; bind Rust's now.
    let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
    Ok(diesel::update(
        archive_decisions::table
            .filter(archive_decisions::archive_id.eq(archive_id))
            .filter(archive_decisions::expanded_ts.is_null()),
    )
    .set(archive_decisions::expanded_ts.eq(now))
    .execute(conn)?)
}

/// The `measurements` insert behind [`Store::insert_measurement`] and
/// [`Store::mark_expanded_recorded`].
fn insert_measurement_conn(
    conn: &mut SqliteConnection,
    session: &str,
    m: &Measurement,
    once: Option<&str>,
) -> Result<()> {
    let once_key = once.map(|o| {
        let r = m.ref_id.as_deref().unwrap_or("");
        format!("{o}|{}|{}|{r}", m.plugin, m.kind)
    });
    let before_bytes = i64::try_from(m.before_bytes).context("measurement before_bytes")?;
    let after_bytes = i64::try_from(m.after_bytes).context("measurement after_bytes")?;
    let est_before = i32::try_from(m.est_before).context("measurement est_before")?;
    let est_after = i32::try_from(m.est_after).context("measurement est_after")?;
    diesel::insert_into(measurements::table)
        .values((
            measurements::session.eq(session),
            measurements::plugin.eq(m.plugin),
            measurements::kind.eq(m.kind),
            measurements::before_bytes.eq(before_bytes),
            measurements::after_bytes.eq(after_bytes),
            measurements::est_before.eq(est_before),
            measurements::est_after.eq(est_after),
            measurements::ref_id.eq(m.ref_id.as_deref()),
            measurements::call_id.eq(m.call_id),
            measurements::once_key.eq(once_key),
        ))
        .on_conflict(measurements::once_key)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

/// [`Store::upsert_note`]'s `RETURNING id` row.
#[derive(QueryableByName)]
struct UpsertedId {
    #[diesel(sql_type = Integer)]
    id: i32,
}

/// FTS5 search hit (T6.1). The shape is the published contract's (D25); this is only the
/// row diesel loads it into.
#[derive(Debug, QueryableByName)]
struct NoteHitRow {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    snippet: String,
}

/// One `notes` row's lifecycle-relevant fields (T69.1): revise needs `kind`/`project`,
/// `mem_get` prefixes retired rows, recall orders by `pinned`. Field order matches
/// [`Store::note_row`]'s select.
#[derive(Debug, Clone, Queryable)]
pub struct NoteRow {
    pub id: i32,
    pub project: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub retired: Option<i64>,
    pub superseded_by: Option<i32>,
    pub pinned: i32,
}

impl NoteRow {
    pub fn is_pinned(&self) -> bool {
        self.pinned != 0
    }
}

/// One `(project, kind)` row from [`Store::memory_note_aggs`].
#[derive(Debug, Clone)]
pub struct MemoryNoteKindAgg {
    pub project: Option<String>,
    pub kind: String,
    pub live: u64,
    pub pinned: u64,
    pub retired: u64,
    pub body_bytes: i64,
    pub oldest_ts: i64,
    pub newest_ts: i64,
}

/// One `measurements` row for `stats --plugin`.
#[derive(Debug, Queryable)]
pub struct MeasRow {
    pub kind: String,
    pub before_bytes: i64,
    pub after_bytes: i64,
    pub est_before: i32,
    pub est_after: i32,
    pub ref_id: Option<String>,
}

/// One `(plugin, kind)` group from [`Store::measurement_totals`] (T207).
#[derive(Debug, Clone)]
pub struct MeasurementTotal {
    pub plugin: String,
    pub kind: String,
    pub rows: i64,
    pub est_before: i64,
    pub est_after: i64,
}

/// Aggregated usage totals grouped by API (T11.6).
#[derive(Debug, Clone)]
pub struct ApiUsage {
    pub api: String,
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
}

/// Aggregated usage totals grouped by model (`rtok stats --price`, T49.1).
/// A `NULL` model (older rows) reads back as `"unknown"`.
#[derive(Debug, Clone)]
pub struct ModelUsage {
    pub model: String,
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
}

/// One `usage` row (proxy ground truth, T5.1).
#[derive(Debug)]
pub struct UsageRow {
    pub session: String,
    pub model: Option<String>,
    pub api: String,
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
    pub call_id: Option<i64>,
}

/// One session's totals ([`Store::session_totals`], T25.1) — the rendering input of the
/// Sessions page and `rtok agent sessions` (T25.2). Everything below is one query's
/// output, so no renderer can re-derive a number differently (D27): tokens are whole-
/// session sums of `usage`, `last_activity` is the MAX ts over the session's `usage`
/// and `calls` rows (falling back to `started_at` when there are none), and `ended_at`
/// `None` means live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, QueryableByName)]
pub struct SessionTotals {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub host: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub project: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub provider: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub api: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub model: Option<String>,
    #[diesel(sql_type = BigInt)]
    pub input: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_create: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_read: i64,
    #[diesel(sql_type = BigInt)]
    pub output: i64,
    #[diesel(sql_type = BigInt)]
    pub started_at: i64,
    #[diesel(sql_type = BigInt)]
    pub last_activity: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub ended_at: Option<i64>,
}

/// One `calls` row as the Calls page serves it ([`Store::recent_calls`], T15.5): the
/// ledger's own columns plus the slugs its ids point at and — when the call is one
/// that recorded usage — the newest `usage` row linked to it. One query's output, so
/// no renderer can re-derive a field differently (D27); `api` `None` means no usage
/// row is linked (a hook, MCP call or plugin run carries none).
#[derive(Debug, Clone, PartialEq, Serialize, QueryableByName)]
pub struct CallRow {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = BigInt)]
    pub ts: i64,
    #[diesel(sql_type = Text)]
    pub session: String,
    #[diesel(sql_type = Text)]
    pub surface: String,
    #[diesel(sql_type = Text)]
    pub kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub plugin: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub name: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    pub parent_id: Option<i32>,
    #[diesel(sql_type = Nullable<Double>)]
    pub ms: Option<f64>,
    #[diesel(sql_type = Integer)]
    pub ok: i32,
    #[diesel(sql_type = Nullable<Text>)]
    pub error: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub host: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub provider: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub model: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub api: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub input: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub cache_create: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub cache_read: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub output: Option<i64>,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::config::Config;
    use schema::notes;

    #[derive(QueryableByName)]
    struct Title {
        #[diesel(sql_type = Text)]
        title: String,
    }

    #[derive(QueryableByName)]
    struct Journal {
        #[diesel(sql_type = Text)]
        journal_mode: String,
    }

    /// FTS5 reads `*`, `(`, `-` and bare operators as syntax, so `mem_search "read("` used to
    /// raise a SQL error instead of returning no hits. User text is quoted now.
    #[test]
    fn note_search_treats_query_text_literally() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_note(None, "note", "parser", "call read( on the file")
            .unwrap();
        for query in ["read(", "*", "-", "AND", "\"quoted\"", "read()"] {
            let hits = store.search_notes(query, 5).unwrap();
            assert!(hits.len() <= 1, "{query:?} → {hits:?}");
        }
        assert_eq!(
            store.search_notes("read(", 5).unwrap().len(),
            1,
            "literal hit"
        );
        assert!(store.search_notes("   ", 5).unwrap().is_empty());
    }

    #[test]
    fn migration_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.migrate().unwrap(),
            0,
            "init already applied everything"
        );
        let mut conn = store.lock().unwrap();
        let rows: Vec<Count> = sql_query(
            "SELECT count(*) AS n FROM sqlite_master WHERE type = 'table' AND name IN
             ('events','measurements','archive','read_cache','notes','usage')",
        )
        .load(&mut *conn)
        .unwrap();
        assert_eq!(rows[0].n, 6);
    }

    /// Every surface opens the same file, so a fresh store is migrated by whichever of them
    /// starts first — and the others start at the same moment. Each `Store::open` is its own
    /// connection, so these threads race exactly as separate processes do: before the
    /// exclusive transaction, the losers failed on `duplicate column name: mtime` (0007).
    #[test]
    fn concurrent_opens_of_a_fresh_store_all_migrate() {
        let dir = std::env::temp_dir().join(format!("rtok-mig-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("rtok.db");
        let errs: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|_| s.spawn(|| Store::open(&path).map(|_| ()).map_err(|e| format!("{e:#}"))))
                .collect();
            handles
                .into_iter()
                .filter_map(|h| h.join().unwrap().err())
                .collect()
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert!(errs.is_empty(), "{errs:?}");
    }

    /// T75: the startup purge (`mcp`/`proxy` session start) must queue behind another
    /// process's write transaction instead of dying on "database is locked" — the
    /// deferred read-then-write transaction could lose instantly (SQLITE_BUSY_SNAPSHOT
    /// skips the busy handler) or after the steady 1 s.
    #[test]
    fn purge_waits_out_a_concurrent_writer() {
        let dir = std::env::temp_dir().join(format!("rtok-purge-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("rtok.db");
        let store = Store::open(&db).unwrap();
        store.upsert_session("s", None, None, None, None).unwrap();
        let call = store
            .insert_call("s", "mcp", "mcp_call", None, None, None, None, None)
            .unwrap();
        store.set_call_ts(call, 1).unwrap(); // older than any cutoff
        drop(store);
        // A second connection holds the WAL writer lock for 1.2 s — past the steady
        // 1 s busy timeout the purge used to die on.
        let (held, held_ack) = std::sync::mpsc::channel();
        let url = db.to_str().unwrap().to_string();
        let holder = std::thread::spawn(move || {
            let mut conn = SqliteConnection::establish(&url).unwrap();
            conn.batch_execute("PRAGMA busy_timeout = 1000; PRAGMA journal_mode = WAL;")
                .unwrap();
            conn.batch_execute("BEGIN IMMEDIATE;").unwrap();
            held.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1200));
            conn.batch_execute("COMMIT;").unwrap();
        });
        held_ack.recv().unwrap();
        let store = Store::open(&db).unwrap();
        let purged = store
            .run_retention(30)
            .expect("purge queues behind the writer");
        assert_eq!(purged, 1, "the old call is gone once the lock is released");
        assert_eq!(store.count_calls().unwrap(), 0);
        holder.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A fresh on-disk db with every migration before `tag` applied — the fixture a
    /// test seeding a previous-schema quirk (0015, 0020, …) builds on before it seeds a
    /// row and calls `Store::open` to run the one migration under test.
    fn db_before_migration(tag: &str) -> (PathBuf, SqliteConnection) {
        let dir = std::env::temp_dir().join(format!("rtok-mig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("rtok.db");
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.batch_execute("PRAGMA busy_timeout = 1000; PRAGMA journal_mode = WAL;")
            .unwrap();
        conn.batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch()))",
        )
        .unwrap();
        let before = format!("{tag}.sql");
        for (name, sql) in MIGRATIONS.iter().take_while(|(n, _)| *n < before.as_str()) {
            conn.batch_execute(sql).unwrap();
            sql_query("INSERT OR IGNORE INTO schema_migrations (name) VALUES (?)")
                .bind::<Text, _>(*name)
                .execute(&mut conn)
                .unwrap();
        }
        (dir, conn)
    }

    /// T69.1: a `rtok.db` of the previous schema (0001–0014, one note) migrates in place —
    /// 0015 adds the lifecycle columns with live defaults and the note survives retiring.
    #[test]
    fn migration_0015_adds_lifecycle_columns_to_a_previous_schema_db() {
        let (dir, mut conn) = db_before_migration("0015");
        let db = dir.join("rtok.db");
        sql_query(
            "INSERT INTO notes (ts, kind, title, body)
             VALUES (1, 'note', 'old', 'before the lifecycle')",
        )
        .execute(&mut conn)
        .unwrap();
        drop(conn);
        let store = Store::open(&db).unwrap();
        let row = store.note_row(1).unwrap().unwrap();
        assert_eq!(
            (
                row.title.as_str(),
                row.retired,
                row.superseded_by,
                row.pinned
            ),
            ("old", None, None, 0)
        );
        assert!(store.retire_note(1, None).unwrap());
        assert!(
            store.search_notes("before", 5).unwrap().is_empty(),
            "retired note does not search"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T209: a `rtok.db` already holding duplicate `(project, kind, title)` notes — the
    /// select-then-insert race migration 0020 closes — migrates by keeping only the
    /// newest row per topic key, the same "highest id" tie-break `upsert_note` used
    /// before the fix. Covers a NULL-project key and a project-scoped key.
    #[test]
    fn migration_0020_drops_pre_existing_duplicate_notes() {
        let (dir, mut conn) = db_before_migration("0020");
        let db = dir.join("rtok.db");
        conn.batch_execute(
            "INSERT INTO notes (id, ts, project, kind, title, body) VALUES
             (1, 1, NULL,   'note', 'dup', 'stale'),
             (2, 2, NULL,   'note', 'dup', 'fresh'),
             (3, 1, 'rtok', 'note', 'dup', 'stale'),
             (4, 2, 'rtok', 'note', 'dup', 'fresh')",
        )
        .unwrap();
        drop(conn);
        let store = Store::open(&db).unwrap();
        let mut conn = store.lock().unwrap();
        let rows: Vec<(i32, String)> = notes::table
            .order(notes::id.asc())
            .select((notes::id, notes::body))
            .load(&mut *conn)
            .unwrap();
        assert_eq!(
            rows,
            vec![(2, "fresh".to_string()), (4, "fresh".to_string())],
            "kept only the newest row per (project, kind, title)"
        );
        let err = diesel::insert_into(notes::table)
            .values((
                notes::project.eq(None::<&str>),
                notes::kind.eq("note"),
                notes::title.eq("dup"),
                notes::body.eq("third"),
            ))
            .execute(&mut *conn)
            .unwrap_err();
        assert!(
            format!("{err}").contains("UNIQUE"),
            "the index rejects a fresh duplicate too: {err}"
        );
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T245: rows from before migration 0021 (identical ones included) keep a NULL
    /// `once_key` and survive; afterwards a second delivery of one keyed call adds nothing.
    #[test]
    fn migration_0021_keeps_old_rows_and_records_a_keyed_call_once() {
        let (dir, mut conn) = db_before_migration("0021");
        let db = dir.join("rtok.db");
        conn.batch_execute(
            "INSERT INTO measurements (ts, session, plugin, kind, before_bytes, after_bytes,
             est_before, est_after) VALUES (1, 's', 'read', 'delta', 9, 1, 3, 1),
             (1, 's', 'read', 'delta', 9, 1, 3, 1)",
        )
        .unwrap();
        drop(conn);
        let store = Store::open(&db).unwrap();
        let m = Measurement {
            plugin: "read",
            kind: "delta",
            before_bytes: 9,
            after_bytes: 1,
            est_before: 3,
            est_after: 1,
            ref_id: None,
            call_id: None,
        };
        for _ in 0..2 {
            store
                .insert_measurement_once("s", &m, Some("PreToolUse:t1"))
                .unwrap();
        }
        store.insert_measurement("s", &m).unwrap();
        assert_eq!(store.count_measurements().unwrap(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T209: two connections (hooks/MCP/proxy/`otel flush` are separate processes) racing
    /// the same topic key must not land two rows — the UNIQUE index (migration 0020)
    /// makes the atomic `INSERT … ON CONFLICT … DO UPDATE` resolve the race inside
    /// SQLite instead of the old select-then-insert gap.
    #[test]
    fn concurrent_upsert_note_yields_one_row() {
        let dir = std::env::temp_dir().join(format!("rtok-note-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("rtok.db");
        let a = Store::open(&db).unwrap();
        let b = Store::open(&db).unwrap();
        std::thread::scope(|s| {
            for store in [&a, &b] {
                s.spawn(move || {
                    for i in 0..50 {
                        store
                            .upsert_note(Some("rtok"), "note", "topic", &format!("body {i}"))
                            .unwrap();
                    }
                });
            }
        });
        let mut conn = a.lock().unwrap();
        let rows: Vec<i32> = notes::table
            .filter(notes::project.eq("rtok"))
            .filter(notes::kind.eq("note"))
            .filter(notes::title.eq("topic"))
            .select(notes::id)
            .load(&mut *conn)
            .unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fts5_match_finds_inserted_note() {
        let store = Store::open_in_memory().unwrap();
        {
            let mut conn = store.lock().unwrap();
            diesel::insert_into(notes::table)
                .values((
                    notes::project.eq("rtok"),
                    notes::kind.eq("decision"),
                    notes::title.eq("WAL mode"),
                    notes::body.eq("use sqlite wal journal"),
                ))
                .execute(&mut *conn)
                .unwrap();
            let rows: Vec<Title> = sql_query(
                "SELECT n.title FROM notes_fts f JOIN notes n ON n.id = f.rowid WHERE notes_fts MATCH 'journal'",
            )
            .load(&mut *conn)
            .unwrap();
            assert_eq!(rows[0].title, "WAL mode");
        }
    }

    #[test]
    fn open_on_disk_uses_wal() {
        let dir = std::env::temp_dir().join(format!("rtok-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir.join("rtok.db")).unwrap();
        let mode = {
            let mut conn = store.lock().unwrap();
            let rows: Vec<Journal> = sql_query("PRAGMA journal_mode").load(&mut *conn).unwrap();
            rows[0].journal_mode.clone()
        };
        assert_eq!(mode, "wal");
        drop(store);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn schema_0002_seeds_hosts_and_rejects_bad_fk() {
        use schema::calls;
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.migrate().unwrap(), 0);
        let mut conn = store.lock().unwrap();
        let tables: Vec<Count> = sql_query(
            "SELECT count(*) AS n FROM sqlite_master WHERE type = 'table' AND name IN
             ('hosts','providers','models','sessions','calls','call_io','tokens','logs')",
        )
        .load(&mut *conn)
        .unwrap();
        assert_eq!(tables[0].n, 8);
        let hosts: Vec<Count> = sql_query("SELECT count(*) AS n FROM hosts")
            .load(&mut *conn)
            .unwrap();
        // 0002.sql seeds 6; 0010.sql (T25.0) adds `pi`, the slug `rtok agent setup` installs
        // but the original list never had.
        assert_eq!(hosts[0].n, 7);
        sql_query("INSERT INTO sessions (id) VALUES ('s1')")
            .execute(&mut *conn)
            .unwrap();
        let err = diesel::insert_into(calls::table)
            .values((
                calls::session_id.eq("s1"),
                calls::host_id.eq(999),
                calls::surface.eq("cli"),
                calls::kind.eq("cli"),
            ))
            .execute(&mut *conn);
        assert!(err.is_err(), "bad host_id must fail FK");
    }

    /// A clean `dir` plus an in-memory store holding one `mcp_call` row in session `s1`: the
    /// call `insert_call_io` tests attach request/response bodies to.
    fn io_fixture(dir: &Path) -> (Store, i32) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s1", Some(1), None, None, None)
            .unwrap();
        let id = store
            .insert_call(
                "s1",
                "mcp",
                "mcp_call",
                Some(1),
                None,
                None,
                Some("read"),
                Some("read"),
            )
            .unwrap();
        (store, id)
    }

    #[test]
    fn write_api_round_trip_and_spill() {
        let dir = std::env::temp_dir().join(format!("rtok-io-{}", std::process::id()));
        let (store, id) = io_fixture(&dir);
        store
            .insert_call_io(
                id,
                Some(br#"{"a":1}"#),
                Some(br#"{"ok":true}"#),
                65536,
                Some(&dir),
            )
            .unwrap();
        store
            .insert_tokens(id, Some("read"), "before", "estimate", 10)
            .unwrap();
        store
            .insert_tokens(id, Some("read"), "after", "estimate", 4)
            .unwrap();
        store
            .insert_tokens(id, Some("read"), "mcp", "mcp", 12)
            .unwrap();
        store
            .insert_log(
                "info",
                "plugin",
                "read",
                "ok",
                Some("s1"),
                Some(id),
                Some("read"),
            )
            .unwrap();
        let mut conn = store.lock().unwrap();
        let n: i64 = tokens::table
            .filter(tokens::call_id.eq(id))
            .count()
            .get_result(&mut *conn)
            .unwrap();
        assert_eq!(n, 3);
        let logs_n: i64 = logs::table
            .filter(logs::source.eq("plugin"))
            .filter(logs::call_id.eq(id))
            .count()
            .get_result(&mut *conn)
            .unwrap();
        assert_eq!(logs_n, 1);
        drop(conn);

        let big = vec![b'x'; 70 * 1024];
        let id2 = store
            .insert_call(
                "s1",
                "mcp",
                "mcp_call",
                Some(1),
                None,
                None,
                Some("read"),
                Some("big"),
            )
            .unwrap();
        store
            .insert_call_io(id2, Some(&big), None, 64 * 1024, Some(&dir))
            .unwrap();
        let mut conn = store.lock().unwrap();
        let (request_json, request_archive): (Option<String>, Option<String>) = call_io::table
            .filter(call_io::call_id.eq(id2))
            .select((call_io::request_json, call_io::request_archive))
            .first(&mut *conn)
            .unwrap();
        assert!(request_json.is_none());
        assert!(request_archive.is_some());
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// T208: `insert_call_io`'s archive-row inserts and its final `call_io` insert commit
    /// together. A pre-existing `call_io` row for the same call (its `call_id` is a
    /// `PRIMARY KEY`) makes the real call's final insert fail after its archive row already
    /// went in inside the same transaction — the rollback must leave no `archive` row and no
    /// payload file behind.
    #[test]
    fn insert_call_io_failure_leaves_no_orphan_archive() {
        let dir = std::env::temp_dir().join(format!("rtok-io-orphan-{}", std::process::id()));
        let (store, id) = io_fixture(&dir);
        {
            let mut conn = store.lock().unwrap();
            diesel::insert_into(call_io::table)
                .values(call_io::call_id.eq(id))
                .execute(&mut *conn)
                .unwrap();
        }
        let big = vec![b'x'; 70 * 1024];
        let err = store
            .insert_call_io(id, Some(&big), None, 64 * 1024, Some(&dir))
            .unwrap_err();
        assert!(err.to_string().contains("UNIQUE"), "{err}");
        let mut conn = store.lock().unwrap();
        let archive_n: i64 = archive::table.count().get_result(&mut *conn).unwrap();
        assert_eq!(
            archive_n, 0,
            "a failed call_io insert must not strand an archive row"
        );
        drop(conn);
        let sha = hex_sha256(&big);
        assert!(
            !dir.join(&sha).exists(),
            "the orphan payload file must be removed on rollback"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A later upsert that omits attribution must not wipe what an earlier one set —
    /// hook → proxy (same session id) used to NULL out project/cwd; Runtime → proxy
    /// used to NULL out source.
    #[test]
    fn upsert_session_keeps_non_null_attribution() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().expect("seeded");
        store
            .upsert_session("s1", Some(claude), Some("rtok"), Some("/tmp/rtok"), None)
            .unwrap();
        store
            .upsert_session("s1", Some(claude), None, None, Some("proxy"))
            .unwrap();
        let (slug, project, cwd) = store.session_row("s1").unwrap().unwrap();
        assert_eq!(slug.as_deref(), Some("claude"));
        assert_eq!(
            project.as_deref(),
            Some("rtok"),
            "project survived a None upsert"
        );
        assert_eq!(
            cwd.as_deref(),
            Some("/tmp/rtok"),
            "cwd survived a None upsert"
        );
        let mut conn = store.lock().unwrap();
        let source: Option<String> = sessions::table
            .filter(sessions::id.eq("s1"))
            .select(sessions::source)
            .first(&mut *conn)
            .unwrap();
        assert_eq!(source.as_deref(), Some("proxy"));
        // And a Runtime-shaped upsert (source None) must keep the proxy source.
        drop(conn);
        store
            .upsert_session("s1", Some(claude), None, None, None)
            .unwrap();
        let mut conn = store.lock().unwrap();
        let source: Option<String> = sessions::table
            .filter(sessions::id.eq("s1"))
            .select(sessions::source)
            .first(&mut *conn)
            .unwrap();
        assert_eq!(
            source.as_deref(),
            Some("proxy"),
            "source survived a None upsert"
        );
    }

    #[test]
    fn two_apis_are_two_stats_rows() {
        let store = Store::open_in_memory().unwrap();
        crate::testutil::seed_two_apis(&store);
        assert_eq!(store.usage_by_api().unwrap().len(), 2);
    }

    /// The old raw SQL grouped by `COALESCE(model, 'unknown')`, so a `NULL`-model row and
    /// a row whose model is literally `"unknown"` summed into a single bucket. The typed
    /// DSL groups by the raw column, so `usage_by_model` must fold those two groups back
    /// together in Rust to keep that behaviour.
    #[test]
    fn usage_by_model_merges_null_and_literal_unknown() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s1", None, None, None, Some("proxy"))
            .unwrap();
        let call = store
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        store
            .insert_usage("s1", None, "anthropic", 10, 1, 2, 3, call)
            .unwrap();
        store
            .insert_usage("s1", Some("unknown"), "anthropic", 5, 0, 1, 2, call)
            .unwrap();
        let rows = store.usage_by_model().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].model, "unknown");
        assert_eq!(rows[0].input, 15);
        assert_eq!(rows[0].cache_create, 1);
        assert_eq!(rows[0].cache_read, 3);
        assert_eq!(rows[0].output, 5);
    }

    /// T25.1's Check: three sessions across the two hosts the migrations seed
    /// (`claude` from 0002, `pi` from 0010), one ended, one with no usage yet —
    /// `session_totals` sums each one's tokens exactly, newest first, and `since`
    /// windows on `started_at` without cutting the totals.
    #[test]
    fn session_totals_sums_each_session_exactly() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().expect("0002 seeds claude");
        let pi = store.host_id("pi").unwrap().expect("0010 seeds pi");
        store
            .upsert_session(
                "a",
                Some(claude),
                Some("rtok"),
                Some("/w/rtok"),
                Some("proxy"),
            )
            .unwrap();
        store
            .upsert_session("b", Some(pi), Some("rtok"), None, None)
            .unwrap();
        store
            .upsert_session("c", Some(pi), None, None, None)
            .unwrap();
        let (pid, mid) = store.upsert_model("anthropic", "claude-x").unwrap();
        let call_a = store
            .insert_call(
                "a",
                "proxy",
                "api_request",
                Some(claude),
                Some(pid),
                Some(mid),
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        let call_b = store
            .insert_call(
                "b",
                "proxy",
                "api_request",
                Some(pi),
                None,
                None,
                None,
                Some("/v1/chat/completions"),
            )
            .unwrap();
        store
            .insert_usage("a", Some("claude-x"), "anthropic", 10, 1, 2, 3, call_a)
            .unwrap();
        store
            .insert_usage("a", Some("claude-x"), "anthropic", 20, 0, 5, 4, call_a)
            .unwrap();
        store
            .insert_usage("b", Some("gpt-x"), "openai_chat", 7, 2, 0, 1, call_b)
            .unwrap();
        store.end_session("b", 2500).unwrap();
        // The write path stamps `unixepoch()`; pin the timeline so last-activity and
        // the `since` window are exact. The in-memory DB is fresh, so rowids are the
        // insert order: usage 1–2 belong to "a", 3 to "b".
        {
            let mut conn = store.lock().unwrap();
            for (id, started) in [("a", 1000i64), ("b", 2000), ("c", 3000)] {
                diesel::update(sessions::table.filter(sessions::id.eq(id)))
                    .set(sessions::started_at.eq(started))
                    .execute(&mut *conn)
                    .unwrap();
            }
            for (id, ts) in [(1i32, 1100i64), (2, 1200), (3, 2100)] {
                diesel::update(usage::table.filter(usage::id.eq(id)))
                    .set(usage::ts.eq(ts))
                    .execute(&mut *conn)
                    .unwrap();
            }
            for (id, ts) in [(call_a, 1500i64), (call_b, 2100)] {
                diesel::update(calls::table.filter(calls::id.eq(id)))
                    .set(calls::ts.eq(ts))
                    .execute(&mut *conn)
                    .unwrap();
            }
        }
        // Newest first. "a": two usage rows summed, last activity from the newer call
        // (ts 1500 > 1200), live. "b": ended, one row. "c": no usage and no calls, so
        // zeroed with last activity = started_at.
        #[allow(clippy::too_many_arguments)]
        fn expect(
            id: &str,
            host: Option<&str>,
            project: Option<&str>,
            provider: Option<&str>,
            api: Option<&str>,
            model: Option<&str>,
            tokens: (i64, i64, i64, i64),
            started_at: i64,
            last_activity: i64,
            ended_at: Option<i64>,
        ) -> SessionTotals {
            SessionTotals {
                id: id.into(),
                host: host.map(String::from),
                project: project.map(String::from),
                provider: provider.map(String::from),
                api: api.map(String::from),
                model: model.map(String::from),
                input: tokens.0,
                cache_create: tokens.1,
                cache_read: tokens.2,
                output: tokens.3,
                started_at,
                last_activity,
                ended_at,
            }
        }
        assert_eq!(
            store.session_totals(0).unwrap(),
            vec![
                expect(
                    "c",
                    Some("pi"),
                    None,
                    None,
                    None,
                    None,
                    (0, 0, 0, 0),
                    3000,
                    3000,
                    None
                ),
                expect(
                    "b",
                    Some("pi"),
                    Some("rtok"),
                    None,
                    Some("openai_chat"),
                    Some("gpt-x"),
                    (7, 2, 0, 1),
                    2000,
                    2100,
                    Some(2500)
                ),
                expect(
                    "a",
                    Some("claude"),
                    Some("rtok"),
                    Some("anthropic"),
                    Some("anthropic"),
                    Some("claude-x"),
                    (30, 1, 7, 7),
                    1000,
                    1500,
                    None
                ),
            ]
        );
        // `since` floors `started_at`; the totals it returns stay whole-session.
        let ids: Vec<String> = store
            .session_totals(2000)
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["c", "b"]);
        let only_c = store.session_totals(3000).unwrap();
        assert_eq!(only_c.len(), 1);
        assert_eq!(only_c[0].id, "c");
    }

    /// T15.5's Check: `recent_calls` is the Calls page's one read — newest first,
    /// bounded, the three slugs joined, and the newest `usage` row linked when the call
    /// has one (the same row `call_detail` serves the otel span).
    #[test]
    fn recent_calls_is_newest_first_bounded_and_linked() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().expect("0002 seeds claude");
        store
            .upsert_session("s", Some(claude), None, None, Some("proxy"))
            .unwrap();
        let (pid, mid) = store.upsert_model("anthropic", "claude-x").unwrap();
        let hook = store
            .insert_call("s", "hook", "hook", None, None, None, None, Some("Stop"))
            .unwrap();
        let run = store
            .insert_call(
                "s",
                "hook",
                "plugin_run",
                None,
                None,
                None,
                Some("cmd"),
                None,
            )
            .unwrap();
        store.set_call_parent(run, hook).unwrap();
        let api = store
            .insert_call(
                "s",
                "proxy",
                "api_request",
                Some(claude),
                Some(pid),
                Some(mid),
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        store.set_call_ms(api, 12.5).unwrap();
        store
            .insert_usage("s", Some("claude-x"), "anthropic", 10, 1, 2, 3, api)
            .unwrap();
        // A second usage row on the same call: the linkage is the newest, not the sum.
        store
            .insert_usage("s", Some("claude-x"), "anthropic", 20, 0, 5, 4, api)
            .unwrap();

        let rows = store.recent_calls(10).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![api, run, hook],
            "newest first"
        );
        let top = &rows[0];
        assert_eq!(top.api.as_deref(), Some("anthropic"));
        assert_eq!(
            (top.input, top.cache_create, top.cache_read, top.output),
            (Some(20), Some(0), Some(5), Some(4))
        );
        assert_eq!(top.ms, Some(12.5));
        assert_eq!(top.host.as_deref(), Some("claude"));
        assert_eq!(top.provider.as_deref(), Some("anthropic"));
        assert_eq!(top.model.as_deref(), Some("claude-x"));
        assert_eq!(top.parent_id, None);
        let nested = &rows[1];
        assert_eq!(nested.parent_id, Some(hook), "the plugin run nests");
        assert!(nested.api.is_none(), "a plugin run carries no usage");
        assert!(rows[2].api.is_none(), "a hook call carries no usage");

        // The bound: only the newest survive it.
        let bounded = store.recent_calls(1).unwrap();
        assert_eq!(bounded.len(), 1);
        assert_eq!(bounded[0].id, api);
    }

    /// `%`, `_` and case are literal in the prefix compare — `LIKE` treated them as a pattern
    /// and dropped cached reads of other files.
    #[test]
    fn clear_read_cache_drops_only_that_path_and_its_mode_keys() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap();
        for s in ["s", "other"] {
            store
                .upsert_session(s, claude, None, None, Some("hook"))
                .unwrap();
        }
        let gone = ["/a/x_1.rs", "/a/x_1.rs\tmap", "/a/x_1.rs\tlines\t1-9"];
        let kept = [
            "/a/xa1.rs\tmap",
            "/a/X_1.RS\tmap",
            "/a/x_1.rsx",
            "/a/%\tmap",
        ];
        for p in gone.iter().chain(&kept) {
            store.put_read_cache("s", p, "h", None).unwrap();
        }
        store
            .put_read_cache("other", "/a/x_1.rs", "h", None)
            .unwrap();
        store.clear_read_cache("s", "/a/x_1.rs").unwrap();
        store.clear_read_cache("s", "/a/%").unwrap();
        let cached = |s: &str, p: &str| store.get_read_cache(s, p).unwrap().is_some();
        for p in gone {
            assert!(!cached("s", p), "{p:?} should be cleared");
        }
        for p in &kept[..3] {
            assert!(cached("s", p), "{p:?} is another file");
        }
        assert!(
            !cached("s", "/a/%\tmap"),
            "a literal `%` path clears its own keys"
        );
        assert!(cached("other", "/a/x_1.rs"), "other sessions keep theirs");
    }

    /// An old call's own rows go; the ledger rows that point at it stay, detached — and the
    /// foreign keys never refuse the delete.
    #[test]
    fn purge_drops_old_calls_and_detaches_their_ledger_rows() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap();
        store
            .upsert_session("s", claude, None, None, Some("proxy"))
            .unwrap();
        let call = |kind| {
            store
                .insert_call("s", "proxy", kind, None, None, None, None, None)
                .unwrap()
        };
        let (old, child, fresh) = (call("api_request"), call("plugin_run"), call("hook"));
        store.set_call_parent(child, old).unwrap();
        store
            .insert_usage("s", None, "anthropic", 1, 0, 0, 1, old)
            .unwrap();
        let m = Measurement {
            plugin: "cmd",
            kind: "rule",
            before_bytes: 10,
            after_bytes: 5,
            est_before: 3,
            est_after: 1,
            ref_id: None,
            call_id: Some(old),
        };
        store.insert_measurement("s", &m).unwrap();
        store.insert_provider_tokens(old, 2, 1, 0, 0, 1).unwrap();
        store
            .insert_call_io(old, Some(b"{}"), None, 1024, None)
            .unwrap();
        store
            .insert_log("info", "proxy", "x", "m", Some("s"), Some(old), None)
            .unwrap();
        let n = |sql: &str| -> i64 {
            let mut conn = store.lock().unwrap();
            sql_query(sql).load::<Count>(&mut *conn).unwrap()[0].n
        };
        {
            let mut conn = store.lock().unwrap();
            sql_query("UPDATE calls SET ts = 0 WHERE id = ?")
                .bind::<Integer, _>(old)
                .execute(&mut *conn)
                .unwrap();
        }

        assert_eq!(store.purge_calls_older_than(1).unwrap(), 1);
        let ids = |sql: &str| n(&format!("SELECT count(*) AS n FROM calls WHERE {sql}"));
        assert_eq!(ids(&format!("id IN ({child}, {fresh})")), 2);
        assert_eq!(ids("parent_id IS NOT NULL"), 0, "the child is detached");
        for t in ["usage", "measurements"] {
            let sql = format!("SELECT count(*) AS n FROM {t} WHERE call_id IS NULL");
            assert_eq!(n(&sql), 1, "{t} row kept, detached");
        }
        for t in ["tokens", "call_io", "logs"] {
            assert_eq!(n(&format!("SELECT count(*) AS n FROM {t}")), 0, "{t}");
        }
        assert_eq!(
            store.purge_calls_older_than(0).unwrap(),
            0,
            "days <= 0 is a no-op"
        );
    }

    /// One call whose 70 KiB body spills to archive, backdated past `retain_calls_days = 1` —
    /// shared by `run_retention_purges_old_call_and_archive` and
    /// `archives_pending_retention_previews_without_deleting` (T182), which exercise the same
    /// `doomed_archives` set through the deleting and the previewing entry point.
    fn seed_one_spilled_call(dir: &Path) -> (Config, Store, PathBuf) {
        let mut cfg = Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.core.archive_dir = dir.join("archive");
        cfg.core.retain_calls_days = 1;
        let store = Store::open(&cfg.core.db_path).unwrap();
        store
            .upsert_session("sess", Some(1), None, None, Some("proxy"))
            .unwrap();
        let call = store
            .insert_call(
                "sess",
                "proxy",
                "api_request",
                Some(1),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let body = vec![b'x'; 70 * 1024];
        store
            .insert_call_io(
                call,
                Some(&body),
                None,
                64 * 1024,
                Some(&cfg.core.archive_dir),
            )
            .unwrap();
        store.set_call_ts(call, 0).unwrap();
        let arch_path = cfg.core.archive_dir.join(hex_sha256(&body));
        (cfg, store, arch_path)
    }

    #[rstest]
    fn run_retention_purges_old_call_and_archive() {
        let dir = std::env::temp_dir().join(format!("rtok-retain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (cfg, store, arch_path) = seed_one_spilled_call(&dir);
        assert!(arch_path.is_file());
        assert_eq!(store.count_calls().unwrap(), 1);

        assert_eq!(store.run_retention(cfg.core.retain_calls_days).unwrap(), 1);
        assert_eq!(store.count_calls().unwrap(), 0);
        assert!(!arch_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `agents junk clear`'s dry run (T182): the same set `run_retention` would delete,
    /// named without touching the database or the file.
    #[test]
    fn archives_pending_retention_previews_without_deleting() {
        let dir = std::env::temp_dir().join(format!("rtok-t182-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (cfg, store, arch_path) = seed_one_spilled_call(&dir);

        let preview = store
            .archives_pending_retention(cfg.core.retain_calls_days)
            .unwrap();
        assert_eq!(preview, vec![arch_path.clone()]);
        assert!(arch_path.is_file(), "preview must not delete anything");
        assert_eq!(store.count_calls().unwrap(), 1);

        assert_eq!(
            store.archives_pending_retention(0).unwrap(),
            Vec::<PathBuf>::new(),
            "0 = keep forever"
        );

        assert_eq!(store.run_retention(cfg.core.retain_calls_days).unwrap(), 1);
        assert!(!arch_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T45.3: the same `tool_use_id` in two sessions is two decisions (composite key, 0014).
    #[test]
    fn archive_decision_repeated_id_persists_per_session() {
        let dir = std::env::temp_dir().join(format!("rtok-t453-id-{}", std::process::id()));
        let store = Store::open_in_memory().unwrap();
        let id = store.put_archive("a", b"body", &dir).unwrap();
        store
            .put_archive_decision("tu-1", &id, "a", "ptr-a")
            .unwrap();
        store
            .put_archive_decision("tu-1", &id, "b", "ptr-b")
            .unwrap();
        let a = store
            .archive_decision("a", "tu-1")
            .unwrap()
            .expect("session a");
        let b = store
            .archive_decision("b", "tu-1")
            .unwrap()
            .expect("session b");
        assert_eq!((a.pointer.as_str(), b.pointer.as_str()), ("ptr-a", "ptr-b"));
        assert_eq!(
            store.live_zone_pointer(&id).unwrap().as_deref(),
            Some("ptr-a"),
            "deterministic by (tool_use_id, session)"
        );
        let other = store.put_archive("c", b"none", &dir).unwrap();
        assert_eq!(store.live_zone_pointer(&other).unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn archive_in_session_hits_only_the_writer_session() {
        let dir = std::env::temp_dir().join(format!("rtok-t651-sess-{}", std::process::id()));
        let store = Store::open_in_memory().unwrap();
        let sha = store.put_archive("a", b"same-bytes", &dir).unwrap();
        let hit = store
            .archive_in_session("a", &sha, None)
            .unwrap()
            .expect("writer");
        assert_eq!(hit.0, sha);
        assert_eq!(store.archive_in_session("b", &sha, None).unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T127: a sub-agent's context never dedups on a body only its parent (or a sibling)
    /// wrote — the pointer would name bytes that context never saw — but the writer's own
    /// context still gets the pointer on its own repeat.
    #[test]
    fn archive_in_session_scopes_by_context_within_one_session() {
        let dir = std::env::temp_dir().join(format!("rtok-t127-ctx-{}", std::process::id()));
        let store = Store::open_in_memory().unwrap();
        let sha = store
            .put_archive_for("s", b"same-bytes", &dir, Some("agent-a"))
            .unwrap();
        assert_eq!(
            store
                .archive_in_session("s", &sha, Some("agent-b"))
                .unwrap(),
            None,
            "context B never saw what context A archived"
        );
        assert_eq!(store.archive_in_session("s", &sha, None).unwrap(), None);
        let hit = store
            .archive_in_session("s", &sha, Some("agent-a"))
            .unwrap()
            .expect("agent-a still hits its own row");
        assert_eq!(hit.0, sha);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T210: `measurements` is never pruned (`purge_calls_older_than` keeps it forever), and
    /// `archive_in_session`'s correlated subquery scans it by `(session, ts)` on every dedup
    /// hit. `EXPLAIN QUERY PLAN` on the query's real shape (mirroring the Diesel-generated
    /// SQL: `archive` filtered by `id`/`session`/`agent_id`, joined to the correlated
    /// `COUNT(*) FROM measurements WHERE session = archive.session AND ts > archive.ts`)
    /// must show the subquery using `measurements_session_ts`, never a full table scan.
    #[test]
    fn archive_in_session_query_plan_uses_the_session_ts_index() {
        let store = Store::open_in_memory().unwrap();
        #[derive(QueryableByName)]
        struct PlanRow {
            #[diesel(sql_type = Text)]
            detail: String,
        }
        let mut conn = store.lock().unwrap();
        // Raw SQL: Diesel has no `EXPLAIN QUERY PLAN`, so the query is restated by hand.
        let rows: Vec<PlanRow> = sql_query(
            "EXPLAIN QUERY PLAN SELECT archive.id, \
             (SELECT COUNT(*) FROM measurements \
              WHERE measurements.session = archive.session AND measurements.ts > archive.ts) \
             FROM archive \
             WHERE archive.id = 'x' AND archive.session = 's' AND archive.agent_id IS NULL",
        )
        .load(&mut *conn)
        .unwrap();
        let plan = rows
            .iter()
            .map(|r| r.detail.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            plan.contains("USING COVERING INDEX measurements_session_ts")
                || plan.contains("USING INDEX measurements_session_ts"),
            "expected the (session, ts) index on measurements, got: {plan}"
        );
        assert!(
            !plan.contains("SCAN measurements"),
            "measurements scanned: {plan}"
        );
    }

    /// T210: with 100k unrelated `measurements` rows ahead of it, one `archive_in_session`
    /// lookup must stay a `(session, ts)` index search, not a linear scan — the query-plan
    /// test above is the hard check; this is a generous, non-flaky wall-clock guard against
    /// a regression that keeps the plan right but still degrades in practice.
    #[test]
    fn archive_in_session_stays_fast_with_100k_measurements() {
        let store = Store::open_in_memory().unwrap();
        let dir = std::env::temp_dir().join(format!("rtok-t210-perf-{}", std::process::id()));
        let sha = store.put_archive("s-target", b"needle", &dir).unwrap();

        {
            let mut conn = store.lock().unwrap();
            conn.transaction::<_, anyhow::Error, _>(|conn| {
                // 1000 rows × 8 binds per statement stays under SQLite's 32766-variable cap.
                for chunk in (0..100_000i64).collect::<Vec<_>>().chunks(1000) {
                    let rows: Vec<_> = chunk
                        .iter()
                        .map(|&i| {
                            // Mostly other sessions, so a scan would pay for rows the index skips.
                            let session = if i % 7 == 0 { "s-target" } else { "s-other" };
                            (
                                measurements::ts.eq(i),
                                measurements::session.eq(session),
                                measurements::plugin.eq("cmd"),
                                measurements::kind.eq("rule"),
                                measurements::before_bytes.eq(1i64),
                                measurements::after_bytes.eq(1i64),
                                measurements::est_before.eq(1),
                                measurements::est_after.eq(1),
                            )
                        })
                        .collect();
                    diesel::insert_into(measurements::table)
                        .values(&rows)
                        .execute(conn)?;
                }
                Ok(())
            })
            .unwrap();
        }

        let start = std::time::Instant::now();
        let hit = store
            .archive_in_session("s-target", &sha, None)
            .unwrap()
            .expect("writer session still hits its own row");
        let elapsed = start.elapsed();
        assert_eq!(hit.0, sha);
        assert!(
            elapsed.as_millis() < 200,
            "archive_in_session took {elapsed:?} against 100k measurements rows \
             (index-backed lookup expected)"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T55.11: the expander's session is never the writer's, so one `expand` freezes
    /// every session's decision pointing at that archive id; a second expand is a no-op.
    #[test]
    fn expand_freezes_every_session_pointing_at_the_archive() {
        let dir = std::env::temp_dir().join(format!("rtok-t453-exp-{}", std::process::id()));
        let store = Store::open_in_memory().unwrap();
        let id = store.put_archive("a", b"body", &dir).unwrap();
        store.put_archive_decision("tu-1", &id, "a", "p").unwrap();
        store.put_archive_decision("tu-1", &id, "b", "p").unwrap();
        assert_eq!(store.mark_expanded(&id).unwrap(), 2);
        assert_eq!(store.mark_expanded(&id).unwrap(), 0, "already frozen");
        assert!(
            store
                .archive_decision("a", "tu-1")
                .unwrap()
                .unwrap()
                .expanded
        );
        assert!(
            store
                .archive_decision("b", "tu-1")
                .unwrap()
                .unwrap()
                .expanded
        );
        assert_eq!(store.archive_decision_counts().unwrap(), (2, 2));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T208: `mark_expanded_recorded` freezes the decision and inserts its `Measurement` in
    /// one transaction. A `before_bytes` that overflows `i64` makes the measurement insert
    /// fail before it issues any SQL — the already-applied freeze inside the same
    /// transaction must roll back with it, so the decision stays unexpanded and no
    /// `measurements` row is stranded.
    #[test]
    fn mark_and_record_are_atomic() {
        let dir = std::env::temp_dir().join(format!("rtok-t208-mark-{}", std::process::id()));
        let store = Store::open_in_memory().unwrap();
        let id = store.put_archive("a", b"body", &dir).unwrap();
        store.put_archive_decision("tu-1", &id, "a", "p").unwrap();
        let bad = Measurement {
            plugin: "archive",
            kind: "expand",
            before_bytes: u64::MAX,
            after_bytes: 4,
            est_before: 0,
            est_after: 1,
            ref_id: Some(id.clone()),
            call_id: None,
        };
        let err = store.mark_expanded_recorded("a", &id, &bad).unwrap_err();
        assert!(err.to_string().contains("before_bytes"), "{err}");
        assert!(
            !store
                .archive_decision("a", "tu-1")
                .unwrap()
                .unwrap()
                .expanded,
            "a rolled-back measurement insert must roll back the freeze too"
        );
        assert_eq!(store.measurement_count("archive").unwrap(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[rstest]
    fn retention_keeps_plugin_archives_without_call_io() {
        let dir = std::env::temp_dir().join(format!("rtok-retain-plugin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.core.archive_dir = dir.join("archive");
        cfg.core.retain_calls_days = 1;
        let store = Store::open(&cfg.core.db_path).unwrap();
        store
            .upsert_session("sess", Some(1), None, None, Some("mcp"))
            .unwrap();
        let body_decision = b"archive with decision";
        let arch_id = store
            .put_archive("sess", body_decision, &cfg.core.archive_dir)
            .unwrap();
        store
            .put_archive_decision("tu-1", &arch_id, "sess", "pointer")
            .unwrap();
        store.mark_expanded(&arch_id).unwrap();
        let body_read = b"read/cmd style archive";
        let read_arch_id = store
            .put_archive("sess", body_read, &cfg.core.archive_dir)
            .unwrap();

        assert_eq!(store.run_retention(cfg.core.retain_calls_days).unwrap(), 0);
        assert_eq!(store.archive_decision_counts().unwrap(), (1, 1));
        assert_eq!(
            store
                .get_archive(&arch_id, Some(&cfg.core.archive_dir))
                .unwrap(),
            Some(body_decision.to_vec())
        );
        assert_eq!(
            store
                .get_archive(&read_arch_id, Some(&cfg.core.archive_dir))
                .unwrap(),
            Some(body_read.to_vec())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[rstest]
    fn spill_archive_carries_session() {
        let dir = std::env::temp_dir().join(format!("rtok-arch-sess-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("sess-a", Some(1), None, None, None)
            .unwrap();
        let call_id = store
            .insert_call(
                "sess-a",
                "proxy",
                "api_request",
                Some(1),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let body = vec![b'x'; 70 * 1024];
        store
            .insert_call_io(call_id, Some(&body), None, 64 * 1024, Some(&dir))
            .unwrap();
        let mut conn = store.lock().unwrap();
        let session: String = archive::table
            .select(archive::session)
            .first(&mut *conn)
            .unwrap();
        assert_eq!(session, "sess-a");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// T201: the hook path (`archive_dir = None`) never writes or expands a spilled body,
    /// so `spill` must not pay a sha256 pass over it either — both sha columns land NULL,
    /// same as `request_archive`/`response_archive`.
    #[rstest]
    fn spill_over_cap_without_archive_dir_skips_hashing() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s", Some(1), None, None, None)
            .unwrap();
        let call_id = store
            .insert_call("s", "hook", "hook", Some(1), None, None, None, None)
            .unwrap();
        let big = vec![b'x'; 70 * 1024];
        store
            .insert_call_io(call_id, Some(&big), Some(&big), 64 * 1024, None)
            .unwrap();
        let mut conn = store.lock().unwrap();
        let row: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = call_io::table
            .filter(call_io::call_id.eq(call_id))
            .select((
                call_io::request_sha256,
                call_io::response_sha256,
                call_io::request_archive,
                call_io::response_archive,
            ))
            .first(&mut *conn)
            .unwrap();
        assert_eq!(
            row,
            (None, None, None, None),
            "over cap + no archive_dir must skip hashing, not just archiving"
        );
    }

    #[test]
    fn hex_sha256_matches_fips_180_2_abc_vector() {
        // sha2 0.11 no longer impls LowerHex on the digest; encode bytes ourselves.
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[rstest]
    fn inline_sha256_matches_stored_text() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s", Some(1), None, None, None)
            .unwrap();
        let call_id = store
            .insert_call("s", "mcp", "mcp_call", Some(1), None, None, None, None)
            .unwrap();
        store
            .insert_call_io(call_id, Some(b"plain"), None, 1 << 20, None)
            .unwrap();
        {
            let mut conn = store.lock().unwrap();
            let (request_json, request_sha256): (Option<String>, Option<String>) = call_io::table
                .filter(call_io::call_id.eq(call_id))
                .select((call_io::request_json, call_io::request_sha256))
                .first(&mut *conn)
                .unwrap();
            let text = request_json.unwrap();
            assert_eq!(text, "plain");
            assert_eq!(request_sha256.unwrap(), hex_sha256(text.as_bytes()));
        }
        assert_eq!(
            store.call_io_request(call_id).unwrap(),
            Some(b"plain".to_vec())
        );

        // T211: invalid UTF-8 must still hash and round-trip as the exact wire bytes, not
        // the `from_utf8_lossy` text stored for display.
        let bad = [b'b', b'a', b'd', 0xff, 0xfe, b'o', b'k'];
        let call_id2 = store
            .insert_call("s", "mcp", "mcp_call", Some(1), None, None, None, None)
            .unwrap();
        store
            .insert_call_io(call_id2, Some(&bad), None, 1 << 20, None)
            .unwrap();
        {
            let mut conn = store.lock().unwrap();
            let (request_json2, request_sha256_2): (Option<String>, Option<String>) =
                call_io::table
                    .filter(call_io::call_id.eq(call_id2))
                    .select((call_io::request_json, call_io::request_sha256))
                    .first(&mut *conn)
                    .unwrap();
            let text2 = request_json2.unwrap();
            assert_eq!(text2, String::from_utf8_lossy(&bad));
            assert_eq!(request_sha256_2.unwrap(), hex_sha256(&bad));
        }
        assert_eq!(store.call_io_request(call_id2).unwrap(), Some(bad.to_vec()));
    }

    #[rstest]
    fn insert_measurement_rejects_out_of_range_estimates() {
        let store = Store::open_in_memory().unwrap();
        let m = Measurement {
            plugin: "cmd",
            kind: "rule",
            before_bytes: 1,
            after_bytes: 1,
            est_before: i32::MAX as u32 + 1,
            est_after: 1,
            ref_id: None,
            call_id: None,
        };
        let err = store.insert_measurement("s", &m).unwrap_err();
        assert!(
            err.to_string().contains("est_before"),
            "expected est_before error, got {err}"
        );
    }

    // T103: `memory_recall_totals` and `call_io_archives` had no direct test.

    fn recall(
        store: &Store,
        session: &str,
        plugin: &'static str,
        kind: &'static str,
        b: u64,
        a: u64,
    ) {
        let m = Measurement {
            plugin,
            kind,
            before_bytes: b,
            after_bytes: a,
            est_before: 0,
            est_after: 0,
            ref_id: None,
            call_id: None,
        };
        store.insert_measurement(session, &m).unwrap();
    }

    #[test]
    fn memory_recall_totals_sums_recalls_in_the_window_only() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.memory_recall_totals(0).unwrap(),
            (0, 0, 0),
            "empty store"
        );

        recall(&store, "s1", "memory", "recall", 1000, 100);
        assert_eq!(
            store.memory_recall_totals(0).unwrap(),
            (1, 1000, 100),
            "one row"
        );

        recall(&store, "s2", "memory", "recall", 500, 50);
        recall(&store, "s3", "memory", "recall", 250, 25);
        // Same plugin, other kind; other plugin, same kind — neither is a recall.
        recall(&store, "s3", "memory", "save", 9999, 9999);
        recall(&store, "s3", "read", "recall", 9999, 9999);
        assert_eq!(
            store.memory_recall_totals(0).unwrap(),
            (3, 1750, 175),
            "many sessions, recalls only"
        );

        // Age s1's row out of the window.
        let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
        {
            let mut conn = store.lock().unwrap();
            diesel::update(measurements::table.filter(measurements::session.eq("s1")))
                .set(measurements::ts.eq(now - 86_400))
                .execute(&mut *conn)
                .unwrap();
        }
        assert_eq!(
            store.memory_recall_totals(now - 3600).unwrap(),
            (2, 750, 75)
        );
        assert_eq!(store.memory_recall_totals(now + 3600).unwrap(), (0, 0, 0));
    }

    #[test]
    fn call_io_archives_names_only_spilled_bodies() {
        let dir = std::env::temp_dir().join(format!("rtok-call-io-arch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("sess", Some(1), None, None, None)
            .unwrap();
        let call = || {
            store
                .insert_call(
                    "sess",
                    "proxy",
                    "api_request",
                    Some(1),
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
        };
        let cap = 16;
        let big = vec![b'x'; 64];

        assert_eq!(
            store.call_io_archives(9999).unwrap(),
            (None, None),
            "no call_io row"
        );

        let inline = call();
        store
            .insert_call_io(inline, Some(b"small"), Some(b"tiny"), cap, Some(&dir))
            .unwrap();
        assert_eq!(
            store.call_io_archives(inline).unwrap(),
            (None, None),
            "inline"
        );

        let req_only = call();
        store
            .insert_call_io(req_only, Some(&big), Some(b"ok"), cap, Some(&dir))
            .unwrap();
        let (req, res) = store.call_io_archives(req_only).unwrap();
        assert_eq!(req.as_deref(), Some(hex_sha256(&big).as_str()));
        assert!(
            dir.join(req.unwrap()).is_file(),
            "the id names the archived body"
        );
        assert_eq!(res, None);

        let both = call();
        let big_res = vec![b'y'; 64];
        store
            .insert_call_io(both, Some(&big), Some(&big_res), cap, Some(&dir))
            .unwrap();
        let (req, res) = store.call_io_archives(both).unwrap();
        assert!(req.is_some());
        assert_eq!(res.as_deref(), Some(hex_sha256(&big_res).as_str()));

        // Over cap without an archive dir (the hook path): metadata only, nothing to expand.
        let no_dir = call();
        store
            .insert_call_io(no_dir, Some(&big), Some(&big), cap, None)
            .unwrap();
        assert_eq!(store.call_io_archives(no_dir).unwrap(), (None, None));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // T104: `MIGRATIONS` and the `table!` macros are both kept by hand.

    /// Every `migrations/*.sql` is in `MIGRATIONS`, in filename order, and nothing else is.
    #[test]
    fn migrations_list_matches_the_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        // Each migration is a `<version>_<slug>/up.sql` directory (Diesel's own layout);
        // `MIGRATIONS` still keys by the pre-T163.4 `NNNN.sql` name, so compare prefixes.
        let mut dirs: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.path().join("up.sql").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        let dir_versions: Vec<&str> = dirs.iter().map(|d| d.split('_').next().unwrap()).collect();
        let listed: Vec<&str> = MIGRATIONS
            .iter()
            .map(|(n, _)| n.strip_suffix(".sql").unwrap())
            .collect();
        assert_eq!(listed, dir_versions, "MIGRATIONS drifted from migrations/");
    }

    // T220: schema-drift guard — per-column type/NOT NULL/PK, the full table set, and (since
    // `table!` models neither) defaults/indexes/triggers via a golden `sqlite_master` dump.
    /// A migrated table with no `table!` macro, and why.
    const RAW_SQL_TABLES: &[&str] = &[
        "note_embeddings",   // 0012: brute-force cosine KNN beside FTS5, sql_query only
        "notes_fts",         // 0001: FTS5 virtual table, sql_query only (T13.1)
        "notes_fts_data",    // FTS5 shadow table for notes_fts
        "notes_fts_idx",     // FTS5 shadow table for notes_fts
        "notes_fts_docsize", // FTS5 shadow table for notes_fts
        "notes_fts_config",  // FTS5 shadow table for notes_fts
        "schema_migrations", // written by Store::migrate itself, not a migrations/*.sql file
    ];

    /// One `diesel::table!`: its name, declared PK columns, and (SQL name, Diesel type) pairs.
    struct SchemaTable {
        name: String,
        pk: Vec<String>,
        cols: Vec<(String, String)>,
    }

    /// Every `diesel::table!` in `schema.rs`, parsed from source.
    fn schema_tables() -> Vec<SchemaTable> {
        let src = include_str!("schema.rs");
        let mut out = Vec::new();
        let mut lines = src.lines().map(str::trim);
        while let Some(line) = lines.next() {
            if line != "diesel::table! {" {
                continue;
            }
            let head = lines.next().unwrap();
            let name = head.split_whitespace().next().unwrap().to_string();
            // No PK column carries `#[sql_name]` today, so the head's names are SQL names too.
            let pk = head[head.find('(').unwrap() + 1..head.find(')').unwrap()]
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let mut cols = Vec::new();
            let mut rename = None;
            for l in lines.by_ref() {
                if l == "}" {
                    break;
                }
                if let Some(n) = l
                    .strip_prefix("#[sql_name = \"")
                    .and_then(|r| r.strip_suffix("\"]"))
                {
                    rename = Some(n.to_string());
                } else if let Some((col, ty)) = l.split_once(" -> ") {
                    let name = rename.take().unwrap_or_else(|| col.to_string());
                    cols.push((name, ty.trim_end_matches(',').to_string()));
                }
            }
            out.push(SchemaTable { name, pk, cols });
        }
        out
    }

    /// SQLite's column-type-affinity rule, collapsed to the 3 affinities `schema.rs` uses:
    /// `table_xinfo` returns the declared type verbatim (`BIGINT`, not `INTEGER`).
    fn sqlite_affinity(declared: &str) -> &'static str {
        let d = declared.to_uppercase();
        if d.contains("INT") {
            "INTEGER"
        } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
            "TEXT"
        } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
            "REAL"
        } else {
            "OTHER"
        }
    }

    /// The affinity a `table!` Diesel type expects — only the types `schema.rs` uses today.
    fn diesel_affinity(ty: &str) -> Option<&'static str> {
        match ty {
            "Integer" | "BigInt" => Some("INTEGER"),
            "Text" => Some("TEXT"),
            "Double" => Some("REAL"),
            _ => None,
        }
    }

    /// `sqlite_master` normalized for a golden diff: tables/indexes/triggers, sorted, `sql`
    /// collapsed to single-spaced so reindenting a migration is not itself drift.
    fn live_schema_snapshot(conn: &mut SqliteConnection) -> String {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            kind: String,
            #[diesel(sql_type = Text)]
            name: String,
            #[diesel(sql_type = Text)]
            tbl_name: String,
            #[diesel(sql_type = Nullable<Text>)]
            sql: Option<String>,
        }
        let mut rows: Vec<Row> = sql_query(
            "SELECT type AS kind, name, tbl_name, sql FROM sqlite_master \
             WHERE type IN ('table', 'index', 'trigger')",
        )
        .load(conn)
        .unwrap();
        rows.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
        rows.into_iter()
            .map(|r| {
                let sql = r.sql.unwrap_or_default();
                let sql = sql.split_whitespace().collect::<Vec<_>>().join(" ");
                format!("{}|{}|{}|{}\n", r.kind, r.name, r.tbl_name, sql)
            })
            .collect()
    }

    /// Every mismatch between `schema.rs`/`schema_snapshot.txt` and a live, migrated
    /// connection, one string each. Takes the connection so a test can run it on a broken DB.
    fn schema_drift(conn: &mut SqliteConnection) -> Vec<String> {
        let mut out = Vec::new();
        let live_snapshot = live_schema_snapshot(conn);
        let live_tables: std::collections::BTreeSet<&str> = live_snapshot
            .lines()
            .filter_map(|l| l.strip_prefix("table|"))
            .map(|l| l.split('|').next().unwrap())
            .collect();
        let tables = schema_tables();
        assert!(tables.len() >= 16, "parsed {} table! macros", tables.len());
        let mut expected: std::collections::BTreeSet<&str> =
            tables.iter().map(|t| t.name.as_str()).collect();
        expected.extend(RAW_SQL_TABLES);
        if expected != live_tables {
            out.push(format!(
                "migrated tables {live_tables:?} vs table! \u{222a} RAW_SQL_TABLES {expected:?}"
            ));
        }

        #[derive(QueryableByName)]
        struct XCol {
            #[diesel(sql_type = Text)]
            name: String,
            #[diesel(sql_type = Text)]
            ty: String,
            #[diesel(sql_type = Integer)]
            notnull: i32,
            #[diesel(sql_type = Integer)]
            pk: i32,
        }
        for t in &tables {
            // `notnull` is a SQLite keyword; the pragma's own column of that name needs quoting.
            let live: Vec<XCol> = sql_query(format!(
                "SELECT name, type AS ty, \"notnull\", pk FROM pragma_table_xinfo('{}')",
                t.name
            ))
            .load(conn)
            .unwrap();
            let live_names: std::collections::BTreeSet<&str> =
                live.iter().map(|c| c.name.as_str()).collect();
            let want_names: std::collections::BTreeSet<&str> =
                t.cols.iter().map(|c| c.0.as_str()).collect();
            if live_names != want_names {
                out.push(format!(
                    "{}: schema.rs columns {want_names:?} vs live {live_names:?}",
                    t.name
                ));
                continue;
            }
            for (name, ty) in &t.cols {
                let live = live.iter().find(|c| &c.name == name).unwrap();
                let (base, nullable) = ty
                    .strip_prefix("Nullable<")
                    .map_or((ty.as_str(), false), |i| (i.trim_end_matches('>'), true));
                let want_pk = t.pk.iter().any(|p| p == name);
                let ty_ok = diesel_affinity(base).is_none_or(|w| sqlite_affinity(&live.ty) == w);
                let pk_ok = want_pk == (live.pk > 0);
                // A bare SQLite `PRIMARY KEY` does not itself imply `NOT NULL` (unlike standard
                // SQL, and several migrations rely on it), so a PK column's live `notnull` is
                // never compared against `table!`'s always-non-`Nullable` Rust type.
                let notnull_ok = want_pk || nullable != (live.notnull != 0);
                if !(ty_ok && pk_ok && notnull_ok) {
                    out.push(format!(
                        "{}.{name}: schema.rs `{ty}` pk={want_pk} vs live `{}` notnull={} pk={}",
                        t.name, live.ty, live.notnull, live.pk
                    ));
                }
            }
        }

        let want_snapshot = include_str!("schema_snapshot.txt");
        if live_snapshot != want_snapshot {
            out.push(format!("sqlite_master drifted from schema_snapshot.txt (defaults, indexes or triggers) — regenerate with `RTOK_BLESS=1 mise exec -- cargo test --lib schema_matches_the_migrated_tables_and_snapshot`\n--- want\n{want_snapshot}--- live\n{live_snapshot}"));
        }
        out
    }

    /// After every migration, `schema.rs` and `schema_snapshot.txt` match a fresh DB exactly.
    #[test]
    fn schema_matches_the_migrated_tables_and_snapshot() {
        let store = Store::open_in_memory().unwrap();
        let mut conn = store.lock().unwrap();
        if std::env::var_os("RTOK_BLESS").is_some() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/store/schema_snapshot.txt");
            std::fs::write(path, live_schema_snapshot(&mut conn)).unwrap();
        }
        let mismatches = schema_drift(&mut conn);
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }

    /// Runs `sql` on a fresh migrated in-memory DB, then the guard — for the mutation tests.
    fn drift_after(sql: &str) -> Vec<String> {
        let store = Store::open_in_memory().unwrap();
        let mut conn = store.lock().unwrap();
        conn.batch_execute(sql).unwrap();
        schema_drift(&mut conn)
    }

    // A changed default and a dropped index are invisible to `table!`; only the snapshot
    // catches them. A column dropped from a live table still fails, as it always has.
    #[test]
    fn schema_drift_catches_a_changed_default() {
        let m = drift_after(
            "DROP TABLE otel_export; CREATE TABLE otel_export (stream TEXT PRIMARY KEY, mark BIGINT NOT NULL DEFAULT 1)",
        );
        assert!(m.iter().any(|s| s.contains("schema_snapshot.txt")), "{m:?}");
    }

    #[test]
    fn schema_drift_catches_a_dropped_index() {
        let m = drift_after("DROP INDEX usage_call"); // 0013
        assert!(m.iter().any(|s| s.contains("schema_snapshot.txt")), "{m:?}");
    }

    #[test]
    fn schema_drift_catches_a_column_removed_from_the_live_table() {
        let m = drift_after(
            "DROP TABLE otel_export; CREATE TABLE otel_export (stream TEXT PRIMARY KEY)",
        );
        assert!(m.iter().any(|s| s.starts_with("otel_export:")), "{m:?}");
    }
}
