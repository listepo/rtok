mod.rs 2807L cognitive
// /Users/listepo/GitHub/listepo/apps/rtok/src/store/mod.rs
§ block block (L1-L28)
//! One SQLite file (plan T0.3, T13.1, decision D8): WAL mode, FTS5, migrations keyed by filename.

pub mod embed;
pub mod models;
pub mod otel;
pub mod schema;
// Symbol index (graph plugin) — SQLite only (D18 loser deleted; P39: Ladybug/Grafeo removed).
mod symbols;

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

§ block block (L29-L48)
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
    ("0012.sql", include_str!("../../migrations/0012.sql")),
    ("0013.sql", include_str!("../../migrations/0013.sql")),
    ("0014.sql", include_str!("../../migrations/0014.sql")),
    ("0015.sql", include_str!("../../migrations/0015.sql")),
    ("0016.sql", include_str!("../../migrations/0016.sql")),
    ("0017.sql", include_str!("../../migrations/0017.sql")),
    ("0018.sql", include_str!("../../migrations/0018.sql")),
];
// ... 1 lines omitted
§ type Store (L50-L52)
pub struct Store {
    conn: Mutex<SqliteConnection>,
}
// ... 1 lines omitted
§ function fts_phrase_query(query (L54-L63)
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
// ... 1 lines omitted
§ type Store (L65-L65)
impl Store {
§ function open(path (L66-L82)
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
        Self::init(conn)
    }
// ... 1 lines omitted
§ function open_in_memory (L84-L87)
    /// Fresh in-memory store for tests and examples.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(SqliteConnection::establish(":memory:")?)
    }
// ... 1 lines omitted
§ function init(mut (L89-L96)
    fn init(mut conn: SqliteConnection) -> Result<Self> {
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }
// ... 1 lines omitted
§ function lock(&self (L98-L100)
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, SqliteConnection>> {
        Ok(self.conn.lock().unwrap_or_else(|e| e.into_inner()))
    }
9/157 chunks shown (1067 tokens)
[lean-ctx] full source: read "/Users/listepo/GitHub/listepo/apps/rtok/src/store/mod.rs" directly (no MCP)  ·  or ctx_read("/Users/listepo/GitHub/listepo/apps/rtok/src/store/mod.rs", mode="full")
