//! One SQLite file (plan T0.3, T13.1, decision D8): WAL mode, FTS5, migrations keyed by filename.

pub mod models;
pub mod schema;

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Text};
use diesel::sqlite::SqliteConnection;
use sha2::{Digest, Sha256};

use crate::plugin::Measurement;

use schema::{archive, call_io, calls, logs, measurements, tokens};

/// Embedded migrations, applied in order, each exactly once.
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001.sql", include_str!("../../migrations/0001.sql")),
    ("0002.sql", include_str!("../../migrations/0002.sql")),
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

    pub fn upsert_model(&self, provider_slug: &str, model_slug: &str) -> Result<i32> {
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
        Ok(i32::try_from(mid.first().context("model")?.n)?)
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
                .execute(&mut *conn)?;
            return Ok((None, Some(sha), n, Some(hex_sha256(body))));
        }
        Ok((None, None, n, Some(sha)))
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

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
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
}
