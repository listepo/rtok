//! One SQLite file (plan T0.3, T13.1, decision D8): WAL mode, FTS5, migrations keyed by filename.

pub mod models;
pub mod otel;
pub mod schema;
// T8.11 (P8c): one `impl Store` per backend, selected here and nowhere else.
#[cfg(not(feature = "graph-lbug"))]
mod symbols;
#[cfg(feature = "graph-lbug")]
mod symbols_lbug;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Double, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::plugin::Measurement;
// The two row shapes a plugin sees are the contract's (D25); the diesel rows below feed them.
pub use crate::plugin::{ArchiveDecision, NoteHit};

use schema::{archive, call_io, calls, hosts, logs, measurements, notes, read_cache, tokens};

/// Embedded migrations, applied in order, each exactly once.
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001.sql", include_str!("../../migrations/0001.sql")),
    ("0002.sql", include_str!("../../migrations/0002.sql")),
    ("0003.sql", include_str!("../../migrations/0003.sql")),
    ("0004.sql", include_str!("../../migrations/0004.sql")),
    ("0005.sql", include_str!("../../migrations/0005.sql")),
    ("0006.sql", include_str!("../../migrations/0006.sql")),
    ("0007.sql", include_str!("../../migrations/0007.sql")),
    ("0008.sql", include_str!("../../migrations/0008.sql")),
    ("0009.sql", include_str!("../../migrations/0009.sql")),
    ("0010.sql", include_str!("../../migrations/0010.sql")),
    ("0011.sql", include_str!("../../migrations/0011.sql")),
];

pub struct Store {
    conn: Mutex<SqliteConnection>,
    #[cfg(feature = "graph-lbug")]
    graph: symbols_lbug::Graph,
}

/// Turn arbitrary user text into an FTS5 MATCH phrase query: every blank-separated token is
/// quoted, so `*`, `(`, `-`, `AND` and `"` are searched for as characters instead of being
/// read as FTS5 syntax. `None` when there is no token left to search for.
fn fts_phrase_query(query: &str) -> Option<String> {
    let quoted: Vec<String> = query
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect();
    (!quoted.is_empty()).then(|| quoted.join(" "))
}

impl Store {
    /// Open (creating directories and the file as needed) and migrate.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let url = path.to_str().context("db path is not UTF-8")?;
        let mut conn =
            SqliteConnection::establish(url).with_context(|| path.display().to_string())?;
        // Hooks, the MCP server, the proxy and the detached `otel flush` child all write this one
        // file. SQLite's default busy timeout is 0, so a second writer failed at once with
        // "database is locked" instead of waiting the few ms the first one holds the lock. First,
        // so switching to WAL waits too; 1 s bounds how long a hook can wait before it fails open.
        conn.batch_execute(
            "PRAGMA busy_timeout = 1000; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
        )?;
        Self::init(conn, path.parent())
    }

    /// Fresh in-memory store for tests and examples.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(SqliteConnection::establish(":memory:")?, None)
    }

    /// `dir` is where a second store may live beside `rtok.db`; `None` means in-memory (T8.11).
    fn init(mut conn: SqliteConnection, dir: Option<&Path>) -> Result<Self> {
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        #[cfg(not(feature = "graph-lbug"))]
        let _ = dir;
        let store = Self {
            conn: Mutex::new(conn),
            #[cfg(feature = "graph-lbug")]
            graph: symbols_lbug::Graph::open(dir)?,
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
    pub fn migrate(&self) -> Result<usize> {
        let mut conn = self.lock()?;
        conn.batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch()))",
        )?;
        let mut applied = 0;
        for (name, sql) in MIGRATIONS {
            let rows: Vec<Count> =
                sql_query("SELECT COUNT(*) AS n FROM schema_migrations WHERE name = ?")
                    .bind::<Text, _>(*name)
                    .load(&mut *conn)?;
            if rows.first().map(|r| r.n).unwrap_or(0) > 0 {
                continue;
            }
            conn.transaction::<_, anyhow::Error, _>(|conn| {
                conn.batch_execute(sql)
                    .with_context(|| format!("migration {name}"))?;
                sql_query("INSERT OR IGNORE INTO schema_migrations (name) VALUES (?)")
                    .bind::<Text, _>(*name)
                    .execute(conn)?;
                Ok(())
            })
            .with_context(|| format!("migration {name}"))?;
            applied += 1;
        }
        Ok(applied)
    }

    /// One `measurements` row. Prefer `Runtime::record`, which supplies the session.
    pub fn insert_measurement(&self, session: &str, m: &Measurement) -> Result<()> {
        let before_bytes = i64::try_from(m.before_bytes).context("measurement before_bytes")?;
        let after_bytes = i64::try_from(m.after_bytes).context("measurement after_bytes")?;
        let est_before = i32::try_from(m.est_before).context("measurement est_before")?;
        let est_after = i32::try_from(m.est_after).context("measurement est_after")?;
        let mut conn = self.lock()?;
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
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Count `measurements` for one plugin. Used by `examples/hello_plugin.rs`.
    pub fn measurement_count(&self, plugin: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        let rows: Vec<Count> = sql_query("SELECT COUNT(*) AS n FROM measurements WHERE plugin = ?")
            .bind::<Text, _>(plugin)
            .load(&mut *conn)?;
        Ok(rows.first().map(|r| r.n).unwrap_or(0))
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
        sql_query(
            "INSERT INTO sessions (id, host_id, project, cwd, source) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               host_id = COALESCE(excluded.host_id, sessions.host_id),
               project = COALESCE(excluded.project, sessions.project),
               cwd = COALESCE(excluded.cwd, sessions.cwd),
               source = COALESCE(excluded.source, sessions.source)",
        )
        .bind::<Text, _>(id)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Integer>, _>(host_id)
        .bind::<diesel::sql_types::Nullable<Text>, _>(project)
        .bind::<diesel::sql_types::Nullable<Text>, _>(cwd)
        .bind::<diesel::sql_types::Nullable<Text>, _>(source)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Upsert provider + model; returns `(provider_id, model_id)`. Proxy ground truth:
    /// the request `model` must resolve to a `models` row (plan T5.1 Check).
    pub fn upsert_model(&self, provider_slug: &str, model_slug: &str) -> Result<(i32, i32)> {
        let mut conn = self.lock()?;
        sql_query("INSERT INTO providers (slug, name) VALUES (?, ?) ON CONFLICT(slug) DO NOTHING")
            .bind::<Text, _>(provider_slug)
            .bind::<Text, _>(provider_slug)
            .execute(&mut *conn)?;
        let pid: Vec<Count> = sql_query("SELECT id AS n FROM providers WHERE slug = ?")
            .bind::<Text, _>(provider_slug)
            .load(&mut *conn)?;
        let provider_id = i32::try_from(pid.first().context("provider")?.n)?;
        sql_query(
            "INSERT INTO models (provider_id, slug) VALUES (?, ?)
             ON CONFLICT(provider_id, slug) DO NOTHING",
        )
        .bind::<diesel::sql_types::Integer, _>(provider_id)
        .bind::<Text, _>(model_slug)
        .execute(&mut *conn)?;
        let mid: Vec<Count> =
            sql_query("SELECT id AS n FROM models WHERE provider_id = ? AND slug = ?")
                .bind::<diesel::sql_types::Integer, _>(provider_id)
                .bind::<Text, _>(model_slug)
                .load(&mut *conn)?;
        Ok((provider_id, i32::try_from(mid.first().context("model")?.n)?))
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
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Nullable<Text>)]
            slug: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            project: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            cwd: Option<String>,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT hosts.slug AS slug, sessions.project AS project, sessions.cwd AS cwd
             FROM sessions LEFT JOIN hosts ON hosts.id = sessions.host_id
             WHERE sessions.id = ?",
        )
        .bind::<Text, _>(id)
        .load(&mut *conn)?;
        Ok(rows.into_iter().next().map(|r| (r.slug, r.project, r.cwd)))
    }

    pub fn count_call_io(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(call_io::table.count().get_result(&mut *conn)?)
    }

    pub fn count_tokens(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(tokens::table.count().get_result(&mut *conn)?)
    }

    #[cfg(test)]
    pub fn count_calls(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(calls::table.count().get_result(&mut *conn)?)
    }

    #[cfg(test)]
    pub fn set_call_ts(&self, call_id: i32, ts: i64) -> Result<()> {
        let mut conn = self.lock()?;
        sql_query("UPDATE calls SET ts = ?1 WHERE id = ?2")
            .bind::<BigInt, _>(ts)
            .bind::<Integer, _>(call_id)
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

    pub fn insert_call_io(
        &self,
        call_id: i32,
        request: Option<&[u8]>,
        response: Option<&[u8]>,
        inline_cap: usize,
        archive_dir: Option<&Path>,
    ) -> Result<()> {
        let session = self.call_session(call_id)?;
        let (req_json, req_arch, req_bytes, req_sha) =
            self.spill(&session, request, inline_cap, archive_dir)?;
        let (res_json, res_arch, res_bytes, res_sha) =
            self.spill(&session, response, inline_cap, archive_dir)?;
        let mut conn = self.lock()?;
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
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    fn spill(
        &self,
        session: &str,
        body: Option<&[u8]>,
        cap: usize,
        archive_dir: Option<&Path>,
    ) -> Result<Spill> {
        let Some(body) = body else {
            return Ok((None, None, 0, None));
        };
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        if body.len() <= cap {
            let (text, sha) = inline_body(body);
            return Ok((Some(text), None, n, Some(sha)));
        }
        // Over cap: metadata always. Archive only when a directory is supplied (never on hook).
        let sha = hex_sha256(body);
        if let Some(dir) = archive_dir {
            self.write_archive(session, body, &sha, dir)?;
            return Ok((None, Some(sha.clone()), n, Some(sha)));
        }
        Ok((None, None, n, Some(sha)))
    }

    /// Write `body` to `dir/<sha256>` and upsert the `archive` row. Returns the id.
    pub fn put_archive(&self, session: &str, body: &[u8], dir: &Path) -> Result<String> {
        let sha = hex_sha256(body);
        self.write_archive(session, body, &sha, dir)?;
        Ok(sha)
    }

    /// The one archive write behind [`Self::put_archive`] and `call_io` spills: the body under
    /// its sha256 in `dir`, then one row per distinct body (the same body twice — T5.3 repeat
    /// requests — is one row). `tool` stays NULL: neither caller knows which plugin archived, and
    /// the column used to say `cmd` for every plugin.
    fn write_archive(&self, session: &str, body: &[u8], sha: &str, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(sha);
        std::fs::write(&path, body)?;
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        let mut conn = self.lock()?;
        diesel::insert_into(archive::table)
            .values((
                archive::id.eq(sha),
                archive::session.eq(session),
                archive::bytes.eq(n),
                archive::path.eq(path.to_string_lossy().as_ref()),
                archive::sha256.eq(sha),
            ))
            .on_conflict(archive::id)
            .do_nothing()
            .execute(&mut *conn)?;
        Ok(())
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
        let rows: Vec<ArchiveDecisionRow> = sql_query(
            "SELECT archive_id, pointer, expanded_ts IS NOT NULL AS expanded
             FROM archive_decisions WHERE session = ? AND tool_use_id = ?",
        )
        .bind::<Text, _>(session)
        .bind::<Text, _>(tool_use_id)
        .load(&mut *conn)?;
        Ok(rows.into_iter().next().map(|r| ArchiveDecision {
            archive_id: r.archive_id,
            pointer: r.pointer,
            expanded: r.expanded,
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
        sql_query(
            "INSERT OR IGNORE INTO archive_decisions (tool_use_id, archive_id, session, pointer)
             VALUES (?, ?, ?, ?)",
        )
        .bind::<Text, _>(tool_use_id)
        .bind::<Text, _>(archive_id)
        .bind::<Text, _>(session)
        .bind::<Text, _>(pointer)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Live-zone pointer text for one archive id (T36.2: attribute expand rows to toon vs archive).
    pub fn live_zone_pointer(&self, archive_id: &str) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        let rows: Vec<PointerRow> =
            sql_query("SELECT pointer FROM archive_decisions WHERE archive_id = ? LIMIT 1")
                .bind::<Text, _>(archive_id)
                .load(&mut *conn)?;
        Ok(rows.into_iter().next().map(|r| r.pointer))
    }

    /// T5.4: an `expand <id>` freezes every decision pointing at that archive id. Returns
    /// how many decisions changed (0 = the id was not a live-zone pointer).
    pub fn mark_expanded(&self, archive_id: &str) -> Result<usize> {
        let mut conn = self.lock()?;
        Ok(sql_query(
            "UPDATE archive_decisions SET expanded_ts = unixepoch()
             WHERE archive_id = ? AND expanded_ts IS NULL",
        )
        .bind::<Text, _>(archive_id)
        .execute(&mut *conn)?)
    }

    /// `(decisions, expanded)` — the expand rate is the archive plugin's honesty metric (T5.4).
    pub fn archive_decision_counts(&self) -> Result<(i64, i64)> {
        let mut conn = self.lock()?;
        let rows: Vec<Count> =
            sql_query("SELECT COUNT(*) AS n FROM archive_decisions").load(&mut *conn)?;
        let total = rows.first().map(|r| r.n).unwrap_or(0);
        let rows: Vec<Count> =
            sql_query("SELECT COUNT(*) AS n FROM archive_decisions WHERE expanded_ts IS NOT NULL")
                .load(&mut *conn)?;
        Ok((total, rows.first().map(|r| r.n).unwrap_or(0)))
    }

    /// The request bytes recorded for a call (inline `call_io.request_json`, else the archive).
    pub fn call_io_request(&self, call_id: i32) -> Result<Option<Vec<u8>>> {
        let row: Option<(Option<String>, Option<String>)> = {
            let mut conn = self.lock()?;
            call_io::table
                .find(call_id)
                .select((call_io::request_json, call_io::request_archive))
                .first(&mut *conn)
                .optional()?
        };
        match row {
            Some((Some(json), _)) => Ok(Some(json.into_bytes())),
            Some((None, Some(id))) => self.get_archive(&id, None),
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
            .values((
                notes::project.eq(project),
                notes::kind.eq(kind),
                notes::title.eq(title),
                notes::body.eq(body),
            ))
            .returning(notes::id)
            .get_result(&mut *conn)
            .map_err(Into::into)
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

    /// Remember a Read/Bash result so `guard` can deny the duplicate (T2.6).
    /// Newest note titles for SessionStart recall (T6.2). Never bodies.
    pub fn list_note_titles(
        &self,
        project: Option<&str>,
        limit: u32,
    ) -> Result<Vec<(i32, String)>> {
        let mut conn = self.lock()?;
        let lim = i64::from(limit.max(1));
        let mut q = notes::table
            .order(notes::id.desc())
            .limit(lim)
            .select((notes::id, notes::title))
            .into_boxed();
        if let Some(p) = project {
            q = q.filter(notes::project.eq(p));
        }
        q.load(&mut *conn).map_err(Into::into)
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
             WHERE notes_fts MATCH ?
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
        sql_query(
            "INSERT INTO read_cache (session, path, sha256, archive_id) VALUES (?, ?, ?, ?)
             ON CONFLICT(session, path) DO UPDATE SET
               sha256 = excluded.sha256,
               ts = unixepoch(),
               archive_id = excluded.archive_id",
        )
        .bind::<Text, _>(session)
        .bind::<Text, _>(path)
        .bind::<Text, _>(sha256)
        .bind::<Nullable<Text>, _>(archive_id)
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
        sql_query(
            "DELETE FROM read_cache WHERE session = ? AND (path = ? OR substr(path, 1, length(?)) = ?)",
        )
        .bind::<Text, _>(session)
        .bind::<Text, _>(path)
        .bind::<Text, _>(&keyed)
        .bind::<Text, _>(&keyed)
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
        #[derive(QueryableByName)]
        struct Body {
            #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
            request_json: Option<String>,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Body> = sql_query(
            "SELECT call_io.request_json AS request_json FROM calls
             JOIN call_io ON call_io.call_id = calls.id
             WHERE calls.session_id = ? AND calls.kind = 'hook'
             ORDER BY calls.id DESC LIMIT ?",
        )
        .bind::<Text, _>(session)
        .bind::<BigInt, _>(limit)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| r.request_json.unwrap_or_default())
            .collect())
    }

    /// Hook/call rows in this session at or after `ts` (window for `guard`).
    pub fn calls_since(&self, session: &str, ts: i64) -> Result<i64> {
        let mut conn = self.lock()?;
        let rows: Vec<Count> =
            sql_query("SELECT COUNT(*) AS n FROM calls WHERE session_id = ? AND ts >= ?")
                .bind::<Text, _>(session)
                .bind::<BigInt, _>(ts)
                .load(&mut *conn)?;
        Ok(rows.first().map(|r| r.n).unwrap_or(0))
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

    /// Proxy ground truth (plan T5.1): one `usage` row per API request. Raw SQL — the
    /// `usage` table predates P13's Diesel schema and has no model.
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
        sql_query(
            "INSERT INTO usage (session, model, api, input, cache_create, cache_read, output, call_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(session)
        .bind::<Nullable<Text>, _>(model)
        .bind::<Text, _>(api)
        .bind::<BigInt, _>(input)
        .bind::<BigInt, _>(cache_create)
        .bind::<BigInt, _>(cache_read)
        .bind::<BigInt, _>(output)
        .bind::<Integer, _>(call_id)
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
        sql_query(
            "INSERT INTO tokens (call_id, phase, source, tokens, input, output, cache_create, cache_read)
             VALUES (?, 'after', 'provider', ?, ?, ?, ?, ?)",
        )
        .bind::<Integer, _>(call_id)
        .bind::<BigInt, _>(total)
        .bind::<BigInt, _>(input)
        .bind::<BigInt, _>(output)
        .bind::<BigInt, _>(cache_create)
        .bind::<BigInt, _>(cache_read)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Sessions that have usage rows, oldest first (`rtok stats --cache`, T5.5).
    pub fn usage_sessions(&self) -> Result<Vec<String>> {
        #[derive(QueryableByName)]
        struct S {
            #[diesel(sql_type = Text)]
            session: String,
        }
        let mut conn = self.lock()?;
        let rows: Vec<S> =
            sql_query("SELECT session FROM usage GROUP BY session ORDER BY MIN(ts), MIN(id)")
                .load(&mut *conn)?;
        Ok(rows.into_iter().map(|r| r.session).collect())
    }

    /// Usage rows for one session, newest first (proxy Check, later `stats`).
    pub fn usage_rows(&self, session: &str) -> Result<Vec<UsageRow>> {
        let mut conn = self.lock()?;
        sql_query(
            "SELECT session, model, api, input, cache_create, cache_read, output, call_id
             FROM usage WHERE session = ? ORDER BY ts DESC, id DESC",
        )
        .bind::<Text, _>(session)
        .load::<UsageRow>(&mut *conn)
        .map_err(Into::into)
    }

    pub fn usage_by_api(&self) -> Result<Vec<ApiUsage>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            api: String,
            #[diesel(sql_type = BigInt)]
            input: i64,
            #[diesel(sql_type = BigInt)]
            cache_create: i64,
            #[diesel(sql_type = BigInt)]
            cache_read: i64,
            #[diesel(sql_type = BigInt)]
            output: i64,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT api,
                    COALESCE(SUM(input),0) AS input,
                    COALESCE(SUM(cache_create),0) AS cache_create,
                    COALESCE(SUM(cache_read),0) AS cache_read,
                    COALESCE(SUM(output),0) AS output
             FROM usage GROUP BY api ORDER BY api",
        )
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| ApiUsage {
                api: r.api,
                input: r.input,
                cache_create: r.cache_create,
                cache_read: r.cache_read,
                output: r.output,
            })
            .collect())
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
             ORDER BY s.started_at DESC, s.id",
        )
        .bind::<BigInt, _>(since)
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
        #[derive(QueryableByName)]
        struct Slug {
            #[diesel(sql_type = Text)]
            slug: String,
        }
        let rows: Vec<Slug> = sql_query(
            "SELECT m.slug AS slug FROM models m JOIN calls c ON c.model_id = m.id WHERE c.id = ?",
        )
        .bind::<Integer, _>(call_id)
        .load(&mut *conn)?;
        Ok(rows.first().map(|r| r.slug.clone()))
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
    pub fn purge_calls_older_than(&self, days: i64) -> Result<usize> {
        if days <= 0 {
            return Ok(0);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        let old = "(SELECT id FROM calls WHERE ts < ?1)";
        let mut conn = self.lock()?;
        let paths = conn
            .transaction::<_, diesel::result::Error, _>(|c| {
                #[derive(QueryableByName)]
                struct ArchPath {
                    #[diesel(sql_type = Text)]
                    id: String,
                    #[diesel(sql_type = Text)]
                    path: String,
                }
                let doomed: Vec<ArchPath> = sql_query(format!(
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
                .load(c)?;
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
            })
            .map_err(anyhow::Error::from)?;
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

type Spill = (Option<String>, Option<String>, i64, Option<String>);

/// `(host slug, project, cwd)` — [`Store::session_row`].
#[cfg(test)]
type SessionRow = (Option<String>, Option<String>, Option<String>);

/// Lossy UTF-8 text stored inline and the sha256 of that exact string.
fn inline_body(body: &[u8]) -> (String, String) {
    let text = String::from_utf8_lossy(body).into_owned();
    let sha = hex_sha256(text.as_bytes());
    (text, sha)
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
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

#[derive(Debug, QueryableByName)]
struct PointerRow {
    #[diesel(sql_type = Text)]
    pointer: String,
}

/// T5.3 archive decision: the frozen pointer text for one `tool_use_id`. Same story as
/// [`NoteHitRow`] — the type plugins see is the contract's.
#[derive(Debug, QueryableByName)]
struct ArchiveDecisionRow {
    #[diesel(sql_type = Text)]
    archive_id: String,
    #[diesel(sql_type = Text)]
    pointer: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    expanded: bool,
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

/// One `usage` row (proxy ground truth, T5.1).
#[derive(Debug, QueryableByName)]
pub struct UsageRow {
    #[diesel(sql_type = Text)]
    pub session: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub model: Option<String>,
    #[diesel(sql_type = Text)]
    pub api: String,
    #[diesel(sql_type = BigInt)]
    pub input: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_create: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_read: i64,
    #[diesel(sql_type = BigInt)]
    pub output: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
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

    #[test]
    fn write_api_round_trip_and_spill() {
        let dir = std::env::temp_dir().join(format!("rtok-io-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        let n: Vec<Count> = sql_query("SELECT count(*) AS n FROM tokens WHERE call_id = ?")
            .bind::<diesel::sql_types::Integer, _>(id)
            .load(&mut *conn)
            .unwrap();
        assert_eq!(n[0].n, 3);
        let logs_n: Vec<Count> =
            sql_query("SELECT count(*) AS n FROM logs WHERE source = 'plugin' AND call_id = ?")
                .bind::<diesel::sql_types::Integer, _>(id)
                .load(&mut *conn)
                .unwrap();
        assert_eq!(logs_n[0].n, 1);
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
        #[derive(QueryableByName)]
        struct Io {
            #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
            request_json: Option<String>,
            #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
            request_archive: Option<String>,
        }
        let io: Vec<Io> =
            sql_query("SELECT request_json, request_archive FROM call_io WHERE call_id = ?")
                .bind::<diesel::sql_types::Integer, _>(id2)
                .load(&mut *conn)
                .unwrap();
        assert!(io[0].request_json.is_none());
        assert!(io[0].request_archive.is_some());
        drop(conn);
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
        #[derive(QueryableByName)]
        struct Src {
            #[diesel(sql_type = Nullable<Text>)]
            source: Option<String>,
        }
        let rows: Vec<Src> = sql_query("SELECT source FROM sessions WHERE id = ?")
            .bind::<Text, _>("s1")
            .load(&mut *conn)
            .unwrap();
        assert_eq!(rows[0].source.as_deref(), Some("proxy"));
        // And a Runtime-shaped upsert (source None) must keep the proxy source.
        drop(conn);
        store
            .upsert_session("s1", Some(claude), None, None, None)
            .unwrap();
        let mut conn = store.lock().unwrap();
        let rows: Vec<Src> = sql_query("SELECT source FROM sessions WHERE id = ?")
            .bind::<Text, _>("s1")
            .load(&mut *conn)
            .unwrap();
        assert_eq!(
            rows[0].source.as_deref(),
            Some("proxy"),
            "source survived a None upsert"
        );
    }

    #[test]
    fn two_apis_are_two_stats_rows() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s1", None, None, None, Some("proxy"))
            .unwrap();
        let id1 = store
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
        let id2 = store
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/chat/completions"),
            )
            .unwrap();
        store
            .insert_usage("s1", Some("m"), "anthropic", 10, 1, 2, 3, id1)
            .unwrap();
        store
            .insert_usage("s1", Some("m"), "openai_chat", 20, 0, 5, 4, id2)
            .unwrap();
        assert_eq!(store.usage_by_api().unwrap().len(), 2);
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
                sql_query("UPDATE sessions SET started_at = ? WHERE id = ?")
                    .bind::<BigInt, _>(started)
                    .bind::<Text, _>(id)
                    .execute(&mut *conn)
                    .unwrap();
            }
            for (rowid, ts) in [(1i64, 1100i64), (2, 1200), (3, 2100)] {
                sql_query("UPDATE usage SET ts = ? WHERE rowid = ?")
                    .bind::<BigInt, _>(ts)
                    .bind::<BigInt, _>(rowid)
                    .execute(&mut *conn)
                    .unwrap();
            }
            for (id, ts) in [(call_a as i64, 1500i64), (call_b as i64, 2100)] {
                sql_query("UPDATE calls SET ts = ? WHERE id = ?")
                    .bind::<BigInt, _>(ts)
                    .bind::<BigInt, _>(id)
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

    #[rstest]
    fn run_retention_purges_old_call_and_archive() {
        let dir = std::env::temp_dir().join(format!("rtok-retain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        assert!(arch_path.is_file());
        assert_eq!(store.count_calls().unwrap(), 1);

        assert_eq!(store.run_retention(cfg.core.retain_calls_days).unwrap(), 1);
        assert_eq!(store.count_calls().unwrap(), 0);
        assert!(!arch_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
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
        #[derive(QueryableByName)]
        struct Arch {
            #[diesel(sql_type = Text)]
            session: String,
        }
        let arch: Arch = sql_query("SELECT session FROM archive LIMIT 1")
            .get_result(&mut *conn)
            .unwrap();
        assert_eq!(arch.session, "sess-a");
        std::fs::remove_dir_all(&dir).unwrap();
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
        #[derive(QueryableByName)]
        struct Io {
            #[diesel(sql_type = Nullable<Text>)]
            request_json: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            request_sha256: Option<String>,
        }
        {
            let mut conn = store.lock().unwrap();
            let row: Io =
                sql_query("SELECT request_json, request_sha256 FROM call_io WHERE call_id = ?")
                    .bind::<Integer, _>(call_id)
                    .get_result(&mut *conn)
                    .unwrap();
            let text = row.request_json.unwrap();
            assert_eq!(text, "plain");
            assert_eq!(row.request_sha256.unwrap(), hex_sha256(text.as_bytes()));
        }

        let bad = [b'b', b'a', b'd', 0xff, 0xfe, b'o', b'k'];
        let call_id2 = store
            .insert_call("s", "mcp", "mcp_call", Some(1), None, None, None, None)
            .unwrap();
        store
            .insert_call_io(call_id2, Some(&bad), None, 1 << 20, None)
            .unwrap();
        let mut conn = store.lock().unwrap();
        let row2: Io =
            sql_query("SELECT request_json, request_sha256 FROM call_io WHERE call_id = ?")
                .bind::<Integer, _>(call_id2)
                .get_result(&mut *conn)
                .unwrap();
        let text2 = row2.request_json.unwrap();
        assert_eq!(text2, String::from_utf8_lossy(&bad));
        assert_eq!(row2.request_sha256.unwrap(), hex_sha256(text2.as_bytes()));
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
}
