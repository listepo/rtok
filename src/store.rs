//! One SQLite file (plan T0.3, decision D8): WAL mode, FTS5, migrations keyed by filename.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::plugin::Measurement;

/// Embedded migrations, applied in order, each exactly once.
const MIGRATIONS: &[(&str, &str)] = &[("0001.sql", include_str!("../migrations/0001.sql"))];

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating directories and the file as needed) and migrate.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path).with_context(|| path.display().to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    /// Fresh in-memory store for tests and examples.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Apply pending migrations; returns how many ran. Idempotent.
    pub fn migrate(&self) -> Result<usize> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch()))",
        )?;
        let mut applied = 0;
        for (name, sql) in MIGRATIONS {
            let done: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = ?1)",
                [name],
                |r| r.get(0),
            )?;
            if done {
                continue;
            }
            self.conn
                .execute_batch(sql)
                .with_context(|| format!("migration {name}"))?;
            self.conn
                .execute("INSERT INTO schema_migrations (name) VALUES (?1)", [name])?;
            applied += 1;
        }
        Ok(applied)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// One `measurements` row. Prefer `Ctx::record`, which supplies the session.
    pub fn insert_measurement(&self, session: &str, m: &Measurement) -> Result<()> {
        self.conn.execute(
            "INSERT INTO measurements (session, plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session,
                m.plugin,
                m.kind,
                i64::try_from(m.before_bytes).unwrap_or(i64::MAX),
                i64::try_from(m.after_bytes).unwrap_or(i64::MAX),
                m.est_before,
                m.est_after,
                m.ref_id
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.migrate().unwrap(),
            0,
            "init already applied everything"
        );
        let tables: i64 = store
            .conn()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN
                 ('events','measurements','archive','read_cache','notes','usage')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 6);
    }

    #[test]
    fn fts5_match_finds_inserted_note() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO notes (project, kind, title, body) VALUES ('rtok', 'decision', 'WAL mode', 'use sqlite wal journal')",
                [],
            )
            .unwrap();
        let title: String = store
            .conn()
            .query_row(
                "SELECT n.title FROM notes_fts f JOIN notes n ON n.id = f.rowid WHERE notes_fts MATCH 'journal'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "WAL mode");
    }

    #[test]
    fn open_on_disk_uses_wal() {
        let dir = std::env::temp_dir().join(format!("rtok-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir.join("rtok.db")).unwrap();
        let mode: String = store
            .conn()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        drop(store);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
