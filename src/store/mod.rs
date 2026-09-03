//! One SQLite file (plan T0.3, T13.1, decision D8): WAL mode, FTS5, migrations keyed by filename.

pub mod models;
pub mod schema;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use sha2::{Digest, Sha256};

use crate::plugin::Measurement;

use schema::{
    archive, call_io, calls, hosts, logs, measurements, notes, read_cache, symbols, tokens,
};

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
];

pub struct Store {
    conn: Mutex<SqliteConnection>,
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
        conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        Self::init(conn)
    }

    /// Fresh in-memory store for tests and examples.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(SqliteConnection::establish(":memory:")?)
    }

    fn init(mut conn: SqliteConnection) -> Result<Self> {
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, SqliteConnection>> {
        Ok(self.conn.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Apply pending migrations; returns how many ran. Idempotent.
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
            conn.batch_execute(sql)
                .with_context(|| format!("migration {name}"))?;
            sql_query("INSERT INTO schema_migrations (name) VALUES (?)")
                .bind::<Text, _>(*name)
                .execute(&mut *conn)?;
            applied += 1;
        }
        Ok(applied)
    }

    /// One `measurements` row. Prefer `Ctx::record`, which supplies the session.
    pub fn insert_measurement(&self, session: &str, m: &Measurement) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(measurements::table)
            .values((
                measurements::session.eq(session),
                measurements::plugin.eq(m.plugin),
                measurements::kind.eq(m.kind),
                measurements::before_bytes.eq(i64::try_from(m.before_bytes).unwrap_or(i64::MAX)),
                measurements::after_bytes.eq(i64::try_from(m.after_bytes).unwrap_or(i64::MAX)),
                measurements::est_before.eq(i32::try_from(m.est_before).unwrap_or(i32::MAX)),
                measurements::est_after.eq(i32::try_from(m.est_after).unwrap_or(i32::MAX)),
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
        sql_query(
            "INSERT INTO sessions (id, host_id, project, cwd, source) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               host_id = excluded.host_id,
               project = excluded.project,
               cwd = excluded.cwd,
               source = excluded.source",
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

    pub fn count_call_io(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(call_io::table.count().get_result(&mut *conn)?)
    }

    pub fn count_tokens(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(tokens::table.count().get_result(&mut *conn)?)
    }

    pub fn insert_call_io(
        &self,
        call_id: i32,
        request: Option<&[u8]>,
        response: Option<&[u8]>,
        inline_cap: usize,
        archive_dir: Option<&Path>,
    ) -> Result<()> {
        let (req_json, req_arch, req_bytes, req_sha) =
            self.spill(request, inline_cap, archive_dir)?;
        let (res_json, res_arch, res_bytes, res_sha) =
            self.spill(response, inline_cap, archive_dir)?;
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

    fn spill(&self, body: Option<&[u8]>, cap: usize, archive_dir: Option<&Path>) -> Result<Spill> {
        let Some(body) = body else {
            return Ok((None, None, 0, None));
        };
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        let sha = hex_sha256(body);
        if body.len() <= cap {
            return Ok((
                Some(String::from_utf8_lossy(body).into_owned()),
                None,
                n,
                Some(sha),
            ));
        }
        // Over cap: metadata always. Archive only when a directory is supplied (never on hook).
        if let Some(dir) = archive_dir {
            std::fs::create_dir_all(dir)?;
            let path = dir.join(&sha);
            std::fs::write(&path, body)?;
            let mut conn = self.lock()?;
            diesel::insert_into(archive::table)
                .values((
                    archive::id.eq(&sha),
                    archive::session.eq(""),
                    archive::bytes.eq(n),
                    archive::path.eq(path.to_string_lossy().as_ref()),
                    archive::sha256.eq(&sha),
                ))
                .on_conflict(archive::id)
                .do_nothing() // the same body twice (T5.3 repeat requests) is one archive row
                .execute(&mut *conn)?;
            return Ok((None, Some(sha), n, Some(hex_sha256(body))));
        }
        Ok((None, None, n, Some(sha)))
    }

    /// Write `body` to `dir/<sha256>` and upsert the `archive` row. Returns the id.
    pub fn put_archive(&self, session: &str, body: &[u8], dir: &Path) -> Result<String> {
        let sha = hex_sha256(body);
        std::fs::create_dir_all(dir)?;
        let path = dir.join(&sha);
        std::fs::write(&path, body)?;
        let n = i64::try_from(body.len()).unwrap_or(i64::MAX);
        let mut conn = self.lock()?;
        diesel::insert_into(archive::table)
            .values((
                archive::id.eq(&sha),
                archive::session.eq(session),
                archive::tool.eq(Some("cmd")),
                archive::bytes.eq(n),
                archive::path.eq(path.to_string_lossy().as_ref()),
                archive::sha256.eq(&sha),
            ))
            .on_conflict(archive::id)
            .do_nothing()
            .execute(&mut *conn)?;
        Ok(sha)
    }

    /// T5.3: the persisted decision for a `tool_use_id`, if the archive plugin made one.
    pub fn archive_decision(&self, tool_use_id: &str) -> Result<Option<ArchiveDecision>> {
        let mut conn = self.lock()?;
        let rows: Vec<ArchiveDecision> = sql_query(
            "SELECT archive_id, pointer, expanded_ts IS NOT NULL AS expanded
             FROM archive_decisions WHERE tool_use_id = ?",
        )
        .bind::<Text, _>(tool_use_id)
        .load(&mut *conn)?;
        Ok(rows.into_iter().next())
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
            Some((None, Some(id))) => self.get_archive(&id),
            _ => Ok(None),
        }
    }

    /// Path and bytes for `rtok expand <id>`. `None` if the id is unknown.
    pub fn get_archive(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.lock()?;
        let path: Option<String> = archive::table
            .find(id)
            .select(archive::path)
            .first(&mut *conn)
            .optional()?;
        let Some(path) = path else {
            return Ok(None);
        };
        Ok(Some(std::fs::read(&path).with_context(|| path)?))
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
    pub fn search_notes(&self, query: &str, limit: u32) -> Result<Vec<NoteHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.lock()?;
        let q = query.replace('"', " ");
        sql_query(
            "SELECT n.id AS id, n.title AS title, substr(n.body, 1, 120) AS snippet
             FROM notes_fts f JOIN notes n ON n.id = f.rowid
             WHERE notes_fts MATCH ?
             ORDER BY bm25(notes_fts)
             LIMIT ?",
        )
        .bind::<Text, _>(q)
        .bind::<Integer, _>(i32::try_from(limit).unwrap_or(5))
        .load(&mut *conn)
        .map_err(Into::into)
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

    /// Drop cache rows for `path` and `path\t…` mode/range keys (T4.4).
    pub fn clear_read_cache(&self, session: &str, path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        sql_query("DELETE FROM read_cache WHERE session = ? AND (path = ? OR path LIKE ?)")
            .bind::<Text, _>(session)
            .bind::<Text, _>(path)
            .bind::<Text, _>(format!("{path}\t%"))
            .execute(&mut *conn)?;
        Ok(())
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

    /// Provider counters for an api_request (plan T5.1): one `tokens` row,
    /// `phase = 'after'`, `source = 'provider'`, carrying the four Anthropic counters
    /// (the `tokens` total is their sum).
    pub fn insert_provider_tokens(
        &self,
        call_id: i32,
        input: i64,
        cache_create: i64,
        cache_read: i64,
        output: i64,
    ) -> Result<()> {
        let total = input
            .saturating_add(cache_create)
            .saturating_add(cache_read)
            .saturating_add(output);
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

    /// Rows indexed under one repo root (T8.3). Every symbol call is scoped to a root, so
    /// two repos in the one store (D8) never evict or answer for each other.
    pub fn symbol_count(&self, root: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(symbols::root.eq(root))
            .count()
            .get_result(&mut *conn)?)
    }

    /// What the index knows about one file: `(sha256, mtime_nanos, size)` (T8.4). A caller
    /// whose stat matches is skipped without the file being opened.
    pub fn symbol_stat(&self, root: &str, path: &str) -> Result<Option<(String, i64, i64)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(symbols::root.eq(root).and(symbols::path.eq(path)))
            .select((symbols::file_sha, symbols::mtime, symbols::size))
            .first::<(String, i64, i64)>(&mut *conn)
            .optional()?)
    }

    /// Record a new stat for a file whose content hashed the same (T8.4): the rows stand,
    /// only the freshness key moves, so the next run skips it on the stat alone.
    pub fn touch_symbols(&self, root: &str, path: &str, mtime: i64, size: i64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(path))))
            .set((symbols::mtime.eq(mtime), symbols::size.eq(size)))
            .execute(&mut *conn)?;
        Ok(())
    }

    pub fn replace_symbols(
        &self,
        root: &str,
        path: &str,
        file_sha: &str,
        stat: (i64, i64),
        rows: &[(String, String, i32, bool, i32, String)],
    ) -> Result<usize> {
        let mut conn = self.lock()?;
        // One transaction per file: thousands of autocommit inserts dominated index time.
        Ok(conn.transaction::<usize, diesel::result::Error, _>(|conn| {
            diesel::delete(
                symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(path))),
            )
            .execute(conn)?;
            if rows.is_empty() {
                // Keep file_sha so an unchanged tagless file is skipped next run.
                diesel::insert_into(symbols::table)
                    .values((
                        symbols::root.eq(root),
                        symbols::path.eq(path),
                        symbols::name.eq(""),
                        symbols::kind.eq(""),
                        symbols::line.eq(0),
                        symbols::is_def.eq(0),
                        symbols::file_sha.eq(file_sha),
                        symbols::mtime.eq(stat.0),
                        symbols::size.eq(stat.1),
                    ))
                    .execute(conn)?;
                return Ok(0);
            }
            for (name, kind, line, is_def, end_line, scope) in rows {
                diesel::insert_into(symbols::table)
                    .values((
                        symbols::root.eq(root),
                        symbols::path.eq(path),
                        symbols::name.eq(name),
                        symbols::kind.eq(kind),
                        symbols::line.eq(line),
                        symbols::is_def.eq(i32::from(*is_def)),
                        symbols::file_sha.eq(file_sha),
                        symbols::mtime.eq(stat.0),
                        symbols::size.eq(stat.1),
                        symbols::end_line.eq(end_line),
                        symbols::scope.eq(scope),
                    ))
                    .execute(conn)?;
            }
            Ok(rows.len())
        })?)
    }

    pub fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize> {
        let mut conn = self.lock()?;
        let have: Vec<String> = symbols::table
            .filter(symbols::root.eq(root))
            .select(symbols::path)
            .distinct()
            .load(&mut *conn)?;
        let mut n = 0usize;
        for p in have {
            if !keep.contains(&p) {
                n += diesel::delete(
                    symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(&p))),
                )
                .execute(&mut *conn)?;
            }
        }
        Ok(n)
    }

    /// Drop rows for one canonical absolute file path. No indexing on the hook path.
    /// Matched as `root || '/' || path` so a same-named file in another repo survives.
    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        sql_query("DELETE FROM symbols WHERE ? = root || '/' || path")
            .bind::<Text, _>(abs_path)
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Definitions of `name` as `(path, kind, line)`, ordered by path then line (T8.2 `symbol`).
    pub fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(name))
                    .and(symbols::is_def.eq(1)),
            )
            .order((symbols::path.asc(), symbols::line.asc()))
            .select((symbols::path, symbols::kind, symbols::line))
            .load(&mut *conn)?)
    }

    /// Reference sites of `name` as `(path, line)`, ordered by path then line (T8.2 `callers`).
    pub fn symbol_refs(&self, root: &str, name: &str) -> Result<Vec<(String, i32)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(name))
                    .and(symbols::is_def.eq(0)),
            )
            .order((symbols::path.asc(), symbols::line.asc()))
            .select((symbols::path, symbols::line))
            .load(&mut *conn)?)
    }

    /// Reference sites of `name` collapsed to one row per calling definition (T8.5):
    /// `(path, scope, count, first line)`, `scope` empty at file level. The grouping is the
    /// call edge — the same rows ungrouped are `symbol_refs`.
    pub fn symbol_ref_groups(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, String, i64, i32)>> {
        let mut conn = self.lock()?;
        let rows: Vec<(String, String, i64, Option<i32>)> = symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(name))
                    .and(symbols::is_def.eq(0)),
            )
            .group_by((symbols::path, symbols::scope))
            .order((symbols::path.asc(), symbols::scope.asc()))
            .select((
                symbols::path,
                symbols::scope,
                diesel::dsl::count_star(),
                diesel::dsl::min(symbols::line),
            ))
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(p, s, n, l)| (p, s, n, l.unwrap_or(0)))
            .collect())
    }

    pub fn has_symbol_def(&self, root: &str, name: &str) -> Result<bool> {
        Ok(!self.symbol_defs(root, name)?.is_empty())
    }

    pub fn symbol_ref_count(&self, root: &str, name: &str) -> Result<i64> {
        Ok(self.symbol_refs(root, name)?.len() as i64)
    }

    pub fn purge_calls_older_than(&self, days: i64) -> Result<usize> {
        if days <= 0 {
            return Ok(0);
        }
        let mut conn = self.lock()?;
        sql_query("DELETE FROM logs WHERE ts < unixepoch() - ? * 86400")
            .bind::<BigInt, _>(days)
            .execute(&mut *conn)?;
        sql_query("DELETE FROM tokens WHERE ts < unixepoch() - ? * 86400")
            .bind::<BigInt, _>(days)
            .execute(&mut *conn)?;
        sql_query(
            "DELETE FROM call_io WHERE call_id IN (SELECT id FROM calls WHERE ts < unixepoch() - ? * 86400)",
        )
        .bind::<BigInt, _>(days)
        .execute(&mut *conn)?;
        let n = sql_query("DELETE FROM calls WHERE ts < unixepoch() - ? * 86400")
            .bind::<BigInt, _>(days)
            .execute(&mut *conn)?;
        Ok(n)
    }

    #[cfg(test)]
    pub fn set_query_only(&self) -> Result<()> {
        self.lock()?.batch_execute("PRAGMA query_only = ON;")?;
        Ok(())
    }
}

type Spill = (Option<String>, Option<String>, i64, Option<String>);

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

/// FTS5 search hit (T6.1).
#[derive(Debug, QueryableByName)]
pub struct NoteHit {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Text)]
    pub snippet: String,
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

/// T5.3 archive decision: the frozen pointer text for one `tool_use_id`.
#[derive(Debug, QueryableByName)]
pub struct ArchiveDecision {
    #[diesel(sql_type = Text)]
    pub archive_id: String,
    #[diesel(sql_type = Text)]
    pub pointer: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub expanded: bool,
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

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(hosts[0].n, 6);
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
}
