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
        conn.query("CREATE REL TABLE IF NOT EXISTS CALLS(FROM Symbol TO Symbol)")
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

    /// Caller def → callee def for every reference in `root` (T8.13). `MERGE` so two
    /// sites in one function do not abort the index (`let _ = delete_symbols_missing`).
    fn materialize_calls(&self, root: &str) -> Result<()> {
        self.tx(|conn| {
            run(
                conn,
                "MATCH (a:Symbol)-[e:CALLS]->(:Symbol) WHERE a.root = $root DELETE e",
                vec![("root", s(root))],
            )?;
            run(
                conn,
                "MATCH (r:Symbol), (callee:Symbol), (caller:Symbol)
                 WHERE r.root = $root AND r.is_def = 0 AND r.scope <> ''
                   AND callee.root = $root AND callee.is_def = 1 AND callee.name = r.name
                   AND caller.root = $root AND caller.is_def = 1
                   AND caller.name = r.scope AND caller.path = r.path
                 MERGE (caller)-[:CALLS]->(callee)",
                vec![("root", s(root))],
            )?;
            Ok(())
        })
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

/// Line numbers are `INT32` in both stores; an aggregate may still come back wider.
fn small(v: Option<&Value>) -> i32 {
    i32::try_from(int(v)).unwrap_or(0)
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
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DETACH DELETE s",
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
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DETACH DELETE s",
                vec![("root", s(root)), ("path", s(&p))],
            )?;
            self.graph.rows(
                "MATCH (f:File) WHERE f.key = $key DELETE f",
                vec![("key", s(&key(root, &p)))],
            )?;
        }
        self.graph.materialize_calls(root)?;
        Ok(n)
    }

    /// Drop rows for one canonical absolute file path. No indexing on the hook path.
    /// Matched as `root || '/' || path` so a same-named file in another repo survives.
    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        self.graph.rows(
            "MATCH (s:Symbol) WHERE concat(s.root, '/', s.path) = $abs DETACH DELETE s",
            vec![("abs", s(abs_path))],
        )?;
        self.graph.rows(
            "MATCH (f:File) WHERE concat(f.root, '/', f.path) = $abs DELETE f",
            vec![("abs", s(abs_path))],
        )?;
        Ok(())
    }

    pub fn extractor_fingerprint(&self, root: &str) -> Result<Option<String>> {
        use diesel::prelude::*;
        use diesel::sql_query;
        use diesel::sql_types::Text;
        let mut conn = self.lock()?;
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            fingerprint: String,
        }
        let rows: Vec<Row> = sql_query("SELECT fingerprint FROM extractor WHERE root = ?")
            .bind::<Text, _>(root)
            .load(&mut *conn)?;
        Ok(rows.first().map(|r| r.fingerprint.clone()))
    }

    pub fn set_extractor_fingerprint(&self, root: &str, fp: &str) -> Result<()> {
        use diesel::sql_query;
        use diesel::sql_types::Text;
        let mut conn = self.lock()?;
        sql_query(
            "INSERT INTO extractor (root, fingerprint) VALUES (?, ?)
             ON CONFLICT(root) DO UPDATE SET fingerprint = excluded.fingerprint",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(fp)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Definitions of `name` as `(path, kind, line)`, ordered by path then line (T8.2 `symbol`).
    pub fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 1
             RETURN s.path, s.kind, s.line, s.end_line ORDER BY s.path, s.line",
            vec![("root", s(root)), ("name", s(name))],
        )?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    text(r.first()),
                    text(r.get(1)),
                    small(r.get(2)),
                    small(r.get(3)),
                )
            })
            .collect())
    }

    /// Reference sites of `name` as `(path, line)`, ordered by path then line (T8.2 `callers`).
    pub fn symbol_refs(&self, root: &str, name: &str) -> Result<Vec<(String, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 0
             RETURN s.path, s.line ORDER BY s.path, s.line",
            vec![("root", s(root)), ("name", s(name))],
        )?;
        Ok(rows
            .iter()
            .map(|r| (text(r.first()), small(r.get(1))))
            .collect())
    }

    /// Reference sites of `name` collapsed to one row per calling definition (T8.5):
    /// `(path, scope, count, first line)`, `scope` empty at file level. The grouping is the
    /// call edge — the same rows ungrouped are `symbol_refs`.
    pub fn symbol_ref_groups(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, String, i64, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 0
             RETURN s.path, s.scope, count(s), min(s.line) ORDER BY s.path, s.scope",
            vec![("root", s(root)), ("name", s(name))],
        )?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    text(r.first()),
                    text(r.get(1)),
                    int(r.get(2)),
                    small(r.get(3)),
                )
            })
            .collect())
    }

    pub fn has_symbol_def(&self, root: &str, name: &str) -> Result<bool> {
        Ok(!self.symbol_defs(root, name)?.is_empty())
    }

    pub fn symbol_ref_count(&self, root: &str, name: &str) -> Result<i64> {
        Ok(self.symbol_refs(root, name)?.len() as i64)
    }

    /// Callers of `name` out to `depth`, each `(path, scope)` at its first depth (T8.13).
    /// Depth 1 is the reference sites (a definition of `name` is not required). Further
    /// hops walk `CALLS` backward from those calling definitions.
    pub fn symbol_impact(
        &self,
        root: &str,
        name: &str,
        depth: u32,
    ) -> Result<Vec<(u32, String, String)>> {
        let depth = depth.clamp(1, 4);
        self.graph.materialize_calls(root)?;
        let hop1 = self.graph.rows(
            "MATCH (r:Symbol)
             WHERE r.root = $root AND r.name = $n AND r.is_def = 0
             RETURN DISTINCT r.path, r.scope
             ORDER BY r.path, r.scope",
            vec![("root", s(root)), ("n", s(name))],
        )?;
        let mut out: Vec<(u32, String, String)> = hop1
            .iter()
            .map(|r| (1, text(r.first()), text(r.get(1))))
            .collect();
        if depth > 1 {
            let d1 = depth - 1;
            let q = format!(
                "MATCH (r:Symbol), (start:Symbol)
                 WHERE r.root = $root AND r.name = $n AND r.is_def = 0 AND r.scope <> ''
                   AND start.root = $root AND start.is_def = 1
                   AND start.name = r.scope AND start.path = r.path
                 MATCH p = (anc:Symbol)-[:CALLS* ACYCLIC 1..{d1}]->(start)
                 WHERE anc.root = $root AND anc.is_def = 1
                 RETURN min(1 + length(p)), anc.path, anc.name
                 ORDER BY min(1 + length(p)), anc.path, anc.name"
            );
            let rows = self
                .graph
                .rows(&q, vec![("root", s(root)), ("n", s(name))])?;
            out.extend(
                rows.iter()
                    .map(|r| (int(r.first()) as u32, text(r.get(1)), text(r.get(2)))),
            );
        }
        out.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
        let mut seen = HashSet::new();
        out.retain(|(_, p, s)| seen.insert((p.clone(), s.clone())));
        Ok(out)
    }
}
