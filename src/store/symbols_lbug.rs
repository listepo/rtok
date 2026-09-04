//! T8.11 (P8c): the `graph` plugin's symbol index on LadybugDB, the `cfg` sibling of
//! `symbols.rs`. Same signatures, same tuple shapes — nothing above `src/store/` may tell the
//! two apart. `src/plugins/graph/` still sees only `Store`.
//!
//! The schema splits what SQLite denormalised: `File` carries a file's freshness key once,
//! `Symbol` carries the rows. A file that parsed to no tags still gets one empty `Symbol`, as
//! in SQLite, so `symbol_count` answers the same number under both backends.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use lbug::{Connection, Database, SystemConfig, Value};

use super::Store;

/// `lbug::Error` is not `anyhow`-compatible on its own; carry the message.
fn oops(e: lbug::Error) -> anyhow::Error {
    anyhow!("lbug: {e}")
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

/// `graph.lbdb` beside `rtok.db`. A `Connection` borrows its `Database`, so one is made per
/// call instead of stored: a self-referential `Store` costs more than opening a connection.
pub struct Graph {
    db: Database,
}

impl Graph {
    /// In-memory when the SQLite store is (`dir` is `None`), matching `Store::open_in_memory`.
    pub fn open(dir: Option<&Path>) -> Result<Self> {
        let db = match dir {
            Some(d) => Database::new(d.join("graph.lbdb"), SystemConfig::default()),
            None => Database::in_memory(SystemConfig::default()),
        }
        .map_err(oops)?;
        let graph = Self { db };
        graph.migrate()?;
        Ok(graph)
    }

    fn migrate(&self) -> Result<()> {
        let conn = Connection::new(&self.db).map_err(oops)?;
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS File(key STRING, root STRING, path STRING,
                 sha STRING, mtime INT64, size INT64, PRIMARY KEY(key))",
        )
        .map_err(oops)?;
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Symbol(id SERIAL, root STRING, path STRING,
                 name STRING, kind STRING, line INT32, end_line INT32, is_def INT32,
                 scope STRING, PRIMARY KEY(id))",
        )
        .map_err(oops)?;
        Ok(())
    }

    fn rows(&self, cypher: &str, params: Vec<(&str, Value)>) -> Result<Vec<Vec<Value>>> {
        let conn = Connection::new(&self.db).map_err(oops)?;
        Ok(run(&conn, cypher, params)?.collect())
    }

    /// One transaction, as `replace_symbols` has been since T8.3: thousands of autocommit
    /// inserts dominated index time.
    fn tx<T>(&self, f: impl FnOnce(&Connection<'_>) -> Result<T>) -> Result<T> {
        let conn = Connection::new(&self.db).map_err(oops)?;
        conn.query("BEGIN TRANSACTION").map_err(oops)?;
        match f(&conn) {
            Ok(v) => {
                conn.query("COMMIT").map_err(oops)?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.query("ROLLBACK");
                Err(e)
            }
        }
    }
}

fn run<'a>(
    conn: &Connection<'a>,
    cypher: &str,
    params: Vec<(&str, Value)>,
) -> Result<lbug::QueryResult<'a>> {
    let mut prepared = conn.prepare(cypher).map_err(oops)?;
    conn.execute(&mut prepared, params).map_err(oops)
}

fn int(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Int64(n)) => *n,
        Some(Value::Int32(n)) => i64::from(*n),
        _ => 0,
    }
}

fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(t)) => t.clone(),
        _ => String::new(),
    }
}

/// A file's identity in one column, so `File` has a primary key and `Symbol` rows stay flat.
fn key(root: &str, path: &str) -> String {
    format!("{root}\t{path}")
}

impl Store {
    /// Rows indexed under one repo root (T8.3). Every symbol call is scoped to a root, so
    /// two repos in the one store (D8) never evict or answer for each other.
    pub fn symbol_count(&self, root: &str) -> Result<i64> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root RETURN count(s)",
            vec![("root", s(root))],
        )?;
        Ok(int(rows.first().and_then(|r| r.first())))
    }

    /// What the index knows about one file: `(sha256, mtime_nanos, size)` (T8.4). A caller
    /// whose stat matches is skipped without the file being opened.
    pub fn symbol_stat(&self, root: &str, path: &str) -> Result<Option<(String, i64, i64)>> {
        let rows = self.graph.rows(
            "MATCH (f:File) WHERE f.key = $key RETURN f.sha, f.mtime, f.size",
            vec![("key", s(&key(root, path)))],
        )?;
        Ok(rows
            .first()
            .map(|r| (text(r.first()), int(r.get(1)), int(r.get(2)))))
    }

    /// Record a new stat for a file whose content hashed the same (T8.4): the rows stand,
    /// only the freshness key moves, so the next run skips it on the stat alone.
    pub fn touch_symbols(&self, root: &str, path: &str, mtime: i64, size: i64) -> Result<()> {
        self.graph.rows(
            "MATCH (f:File) WHERE f.key = $key SET f.mtime = $mtime, f.size = $size",
            vec![
                ("key", s(&key(root, path))),
                ("mtime", Value::Int64(mtime)),
                ("size", Value::Int64(size)),
            ],
        )?;
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
        self.graph.tx(|conn| {
            run(
                conn,
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DELETE s",
                vec![("root", s(root)), ("path", s(path))],
            )?;
            run(
                conn,
                "MERGE (f:File {key: $key})
                 SET f.root = $root, f.path = $path, f.sha = $sha, f.mtime = $mtime, f.size = $size",
                vec![
                    ("key", s(&key(root, path))),
                    ("root", s(root)),
                    ("path", s(path)),
                    ("sha", s(file_sha)),
                    ("mtime", Value::Int64(stat.0)),
                    ("size", Value::Int64(stat.1)),
                ],
            )?;
            // Keep one empty row so an unchanged tagless file counts as it does in SQLite.
            let empty = [(String::new(), String::new(), 0, false, 0, String::new())];
            let write = if rows.is_empty() { &empty[..] } else { rows };
            for (name, kind, line, is_def, end_line, scope) in write {
                run(
                    conn,
                    "CREATE (:Symbol {root: $root, path: $path, name: $name, kind: $kind,
                         line: $line, end_line: $end_line, is_def: $is_def, scope: $scope})",
                    vec![
                        ("root", s(root)),
                        ("path", s(path)),
                        ("name", s(name)),
                        ("kind", s(kind)),
                        ("line", Value::Int32(*line)),
                        ("end_line", Value::Int32(*end_line)),
                        ("is_def", Value::Int32(i32::from(*is_def))),
                        ("scope", s(scope)),
                    ],
                )?;
            }
            Ok(rows.len())
        })
    }

    pub fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize> {
        let have = self.graph.rows(
            "MATCH (f:File) WHERE f.root = $root RETURN DISTINCT f.path",
            vec![("root", s(root))],
        )?;
        let mut n = 0usize;
        for p in have.iter().map(|r| text(r.first())) {
            if keep.contains(&p) {
                continue;
            }
            let gone = self.graph.rows(
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path RETURN count(s)",
                vec![("root", s(root)), ("path", s(&p))],
            )?;
            n += int(gone.first().and_then(|r| r.first())) as usize;
            self.graph.rows(
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DELETE s",
                vec![("root", s(root)), ("path", s(&p))],
            )?;
            self.graph.rows(
                "MATCH (f:File) WHERE f.key = $key DELETE f",
                vec![("key", s(&key(root, &p)))],
            )?;
        }
        Ok(n)
    }

    /// Drop rows for one canonical absolute file path. No indexing on the hook path.
    /// Matched as `root || '/' || path` so a same-named file in another repo survives.
    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        self.graph.rows(
            "MATCH (s:Symbol) WHERE concat(s.root, '/', s.path) = $abs DELETE s",
            vec![("abs", s(abs_path))],
        )?;
        self.graph.rows(
            "MATCH (f:File) WHERE concat(f.root, '/', f.path) = $abs DELETE f",
            vec![("abs", s(abs_path))],
        )?;
        Ok(())
    }

    /// Definitions of `name` as `(path, kind, line)`, ordered by path then line (T8.2 `symbol`).
    pub fn symbol_defs(&self, _root: &str, _name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        unimplemented!("T8.12 writes the reads")
    }

    /// Reference sites of `name` as `(path, line)`, ordered by path then line (T8.2 `callers`).
    pub fn symbol_refs(&self, _root: &str, _name: &str) -> Result<Vec<(String, i32)>> {
        unimplemented!("T8.12 writes the reads")
    }

    /// Reference sites of `name` collapsed to one row per calling definition (T8.5):
    /// `(path, scope, count, first line)`, `scope` empty at file level. The grouping is the
    /// call edge — the same rows ungrouped are `symbol_refs`.
    pub fn symbol_ref_groups(
        &self,
        _root: &str,
        _name: &str,
    ) -> Result<Vec<(String, String, i64, i32)>> {
        unimplemented!("T8.12 writes the reads")
    }

    pub fn has_symbol_def(&self, root: &str, name: &str) -> Result<bool> {
        Ok(!self.symbol_defs(root, name)?.is_empty())
    }

    pub fn symbol_ref_count(&self, root: &str, name: &str) -> Result<i64> {
        Ok(self.symbol_refs(root, name)?.len() as i64)
    }
}
