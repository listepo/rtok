//! T8.20 (P8e): `graph` symbol index on Grafeo, the `cfg` sibling of `symbols.rs`.
//! Same signatures as `symbols_lbug.rs`. GQL lives here only (D13). Ledgers stay in `rtok.db`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Result, anyhow};
use grafeo::{Config, GrafeoDB, NodeId, Value};

use super::Store;

fn oops(e: grafeo::Error) -> anyhow::Error {
    anyhow!("grafeo: {e}")
}

fn s(v: &str) -> Value {
    Value::from(v)
}

fn n(v: i64) -> Value {
    Value::from(v)
}

fn p<'a>(pairs: impl IntoIterator<Item = (&'a str, Value)>) -> HashMap<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// `graph.grafeo` beside `rtok.db`. A `Session` is made per call (same reason as lbug).
pub struct Graph {
    db: GrafeoDB,
}

impl Graph {
    pub fn open(dir: Option<&Path>) -> Result<Self> {
        let db = match dir {
            Some(d) => {
                GrafeoDB::with_config(Config::persistent(d.join("graph.grafeo"))).map_err(oops)?
            }
            None => GrafeoDB::new_in_memory(),
        };
        let graph = Self { db };
        // Best-effort; a second open of a persistent file may already have them.
        for q in [
            "CREATE INDEX FOR (f:File) ON (f.key)",
            "CREATE INDEX FOR (s:Symbol) ON (s.root)",
            "CREATE INDEX FOR (s:Symbol) ON (s.name)",
            "CREATE INDEX FOR (s:Symbol) ON (s.path)",
        ] {
            let _ = graph.rows(q, HashMap::new());
        }
        Ok(graph)
    }

    fn rows(&self, gql: &str, params: HashMap<String, Value>) -> Result<Vec<Vec<Value>>> {
        let session = self.db.session();
        Ok(session
            .execute_with_params(gql, params)
            .map_err(oops)?
            .into_rows())
    }

    /// Caller def → callee def via native edges. A 3-way GQL INSERT was ~70s on 80 nodes.
    fn materialize_calls(&self, root: &str) -> Result<()> {
        self.rows(
            "MATCH (a:Symbol)-[e:CALLS]->(:Symbol) WHERE a.root = $root DELETE e",
            p([("root", s(root))]),
        )?;
        let defs = self.rows(
            "MATCH (d:Symbol) WHERE d.root = $root AND d.is_def = 1
             RETURN id(d), d.name, d.path",
            p([("root", s(root))]),
        )?;
        let refs = self.rows(
            "MATCH (r:Symbol) WHERE r.root = $root AND r.is_def = 0 AND r.scope <> ''
             RETURN r.name, r.scope, r.path",
            p([("root", s(root))]),
        )?;
        let mut by_name: HashMap<String, Vec<(String, NodeId)>> = HashMap::new();
        for row in &defs {
            let id = NodeId::from(int(row.first()) as u64);
            by_name
                .entry(text(row.get(1)))
                .or_default()
                .push((text(row.get(2)), id));
        }
        let session = self.db.session();
        let mut seen = HashSet::new();
        for row in &refs {
            let callee = text(row.first());
            let caller = text(row.get(1));
            let path = text(row.get(2));
            let Some(callees) = by_name.get(&callee) else {
                continue;
            };
            let Some((_, src)) = by_name
                .get(&caller)
                .and_then(|ps| ps.iter().find(|(p, _)| p == &path))
            else {
                continue;
            };
            for (_, dst) in callees {
                if seen.insert((*src, *dst)) {
                    session.create_edge(*src, *dst, "CALLS");
                }
            }
        }
        Ok(())
    }
}

fn int(v: Option<&Value>) -> i64 {
    v.and_then(Value::as_int64).unwrap_or(0)
}

fn small(v: Option<&Value>) -> i32 {
    i32::try_from(int(v)).unwrap_or(0)
}

fn text(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_string()
}

fn key(root: &str, path: &str) -> String {
    format!("{root}\t{path}")
}

fn abs(root: &str, path: &str) -> String {
    format!("{root}/{path}")
}

impl Store {
    pub fn symbol_count(&self, root: &str) -> Result<i64> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root RETURN count(s)",
            p([("root", s(root))]),
        )?;
        Ok(int(rows.first().and_then(|r| r.first())))
    }

    pub fn symbol_stat(&self, root: &str, path: &str) -> Result<Option<(String, i64, i64)>> {
        let rows = self.graph.rows(
            "MATCH (f:File) WHERE f.key = $key RETURN f.sha, f.mtime, f.size",
            p([("key", s(&key(root, path)))]),
        )?;
        Ok(rows
            .first()
            .map(|r| (text(r.first()), int(r.get(1)), int(r.get(2)))))
    }

    pub fn symbol_stats(&self, root: &str) -> Result<HashMap<String, (String, i64, i64)>> {
        let rows = self.graph.rows(
            "MATCH (f:File) WHERE f.root = $root RETURN f.path, f.sha, f.mtime, f.size",
            p([("root", s(root))]),
        )?;
        let mut out = HashMap::new();
        for r in rows {
            out.insert(
                text(r.first()),
                (text(r.get(1)), int(r.get(2)), int(r.get(3))),
            );
        }
        Ok(out)
    }

    pub fn touch_symbols(&self, root: &str, path: &str, mtime: i64, size: i64) -> Result<()> {
        self.graph.rows(
            "MATCH (f:File) WHERE f.key = $key SET f.mtime = $mtime, f.size = $size",
            p([
                ("key", s(&key(root, path))),
                ("mtime", n(mtime)),
                ("size", n(size)),
            ]),
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
        self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DETACH DELETE s",
            p([("root", s(root)), ("path", s(path))]),
        )?;
        self.graph.rows(
            "MATCH (f:File) WHERE f.key = $key DELETE f",
            p([("key", s(&key(root, path)))]),
        )?;
        let session = self.graph.db.session();
        let abs = abs(root, path);
        session
            .create_node_with_props(
                &["File"],
                [
                    ("key", s(&key(root, path))),
                    ("root", s(root)),
                    ("path", s(path)),
                    ("sha", s(file_sha)),
                    ("mtime", n(stat.0)),
                    ("size", n(stat.1)),
                    ("abs", s(&abs)),
                ],
            )
            .map_err(oops)?;
        let empty = [(String::new(), String::new(), 0, false, 0, String::new())];
        let write = if rows.is_empty() { &empty[..] } else { rows };
        for (name, kind, line, is_def, end_line, scope) in write {
            session
                .create_node_with_props(
                    &["Symbol"],
                    [
                        ("root", s(root)),
                        ("path", s(path)),
                        ("name", s(name)),
                        ("kind", s(kind)),
                        ("line", n(i64::from(*line))),
                        ("end_line", n(i64::from(*end_line))),
                        ("is_def", n(i64::from(*is_def))),
                        ("scope", s(scope)),
                        ("abs", s(&abs)),
                    ],
                )
                .map_err(oops)?;
        }
        Ok(rows.len())
    }

    pub fn replace_symbol_files(
        &self,
        root: &str,
        files: &rtok_plugin_sdk::SymbolFileBatch,
    ) -> Result<usize> {
        let mut inserted = 0usize;
        for (path, file_sha, stat, rows) in files {
            inserted += self.replace_symbols(root, path, file_sha, *stat, rows)?;
        }
        Ok(inserted)
    }

    pub fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize> {
        let have = self.graph.rows(
            "MATCH (f:File) WHERE f.root = $root RETURN DISTINCT f.path",
            p([("root", s(root))]),
        )?;
        let mut n = 0usize;
        for path in have.iter().map(|r| text(r.first())) {
            if keep.contains(&path) {
                continue;
            }
            let gone = self.graph.rows(
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path RETURN count(s)",
                p([("root", s(root)), ("path", s(&path))]),
            )?;
            n += int(gone.first().and_then(|r| r.first())) as usize;
            self.graph.rows(
                "MATCH (s:Symbol) WHERE s.root = $root AND s.path = $path DETACH DELETE s",
                p([("root", s(root)), ("path", s(&path))]),
            )?;
            self.graph.rows(
                "MATCH (f:File) WHERE f.key = $key DELETE f",
                p([("key", s(&key(root, &path)))]),
            )?;
        }
        Ok(n)
    }

    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        self.graph.rows(
            "MATCH (s:Symbol) WHERE s.abs = $abs DETACH DELETE s",
            p([("abs", s(abs_path))]),
        )?;
        self.graph.rows(
            "MATCH (f:File) WHERE f.abs = $abs DELETE f",
            p([("abs", s(abs_path))]),
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
        use diesel::RunQueryDsl;
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

    pub fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 1
             RETURN s.path, s.kind, s.line, s.end_line",
            p([("root", s(root)), ("name", s(name))]),
        )?;
        let mut out: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    text(r.first()),
                    text(r.get(1)),
                    small(r.get(2)),
                    small(r.get(3)),
                )
            })
            .collect();
        out.sort();
        Ok(out)
    }

    pub fn symbol_refs(&self, root: &str, name: &str) -> Result<Vec<(String, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 0
             RETURN s.path, s.line",
            p([("root", s(root)), ("name", s(name))]),
        )?;
        let mut out: Vec<_> = rows
            .iter()
            .map(|r| (text(r.first()), small(r.get(1))))
            .collect();
        out.sort();
        Ok(out)
    }

    pub fn symbol_ref_groups(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, String, i64, i32)>> {
        let rows = self.graph.rows(
            "MATCH (s:Symbol) WHERE s.root = $root AND s.name = $name AND s.is_def = 0
             RETURN s.path, s.scope, s.line",
            p([("root", s(root)), ("name", s(name))]),
        )?;
        let mut groups: HashMap<(String, String), (i64, i32)> = HashMap::new();
        for r in &rows {
            let path = text(r.first());
            let scope = text(r.get(1));
            let line = small(r.get(2));
            let e = groups.entry((path, scope)).or_insert((0, line));
            e.0 += 1;
            if line < e.1 {
                e.1 = line;
            }
        }
        let mut out: Vec<_> = groups
            .into_iter()
            .map(|((path, scope), (n, line))| (path, scope, n, line))
            .collect();
        out.sort();
        Ok(out)
    }

    pub fn has_symbol_def(&self, root: &str, name: &str) -> Result<bool> {
        Ok(!self.symbol_defs(root, name)?.is_empty())
    }

    pub fn symbol_ref_count(&self, root: &str, name: &str) -> Result<i64> {
        Ok(self.symbol_refs(root, name)?.len() as i64)
    }

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
             RETURN DISTINCT r.path, r.scope",
            p([("root", s(root)), ("n", s(name))]),
        )?;
        let mut out: Vec<(u32, String, String)> = hop1
            .iter()
            .map(|r| (1, text(r.first()), text(r.get(1))))
            .collect();
        if depth > 1 {
            let d1 = depth - 1;
            let q = format!(
                "MATCH p = (anc:Symbol)-[:CALLS*1..{d1}]->(start:Symbol), (r:Symbol)
                 WHERE r.root = $root AND r.name = $n AND r.is_def = 0 AND r.scope <> ''
                   AND start.root = $root AND start.is_def = 1
                   AND start.name = r.scope AND start.path = r.path
                   AND anc.root = $root AND anc.is_def = 1
                 RETURN length(p), anc.path, anc.name"
            );
            let rows = self
                .graph
                .rows(&q, p([("root", s(root)), ("n", s(name))]))?;
            out.extend(
                rows.iter()
                    .map(|r| (1 + int(r.first()) as u32, text(r.get(1)), text(r.get(2)))),
            );
        }
        out.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
        let mut seen = HashSet::new();
        out.retain(|(_, path, scope)| seen.insert((path.clone(), scope.clone())));
        Ok(out)
    }
}

#[cfg(test)]
mod smoke {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn memory_insert_and_match() {
        let g = Graph::open(None).expect("open");
        g.rows("INSERT (:Person {name: 'x'})", HashMap::new())
            .expect("insert");
        let rows = g
            .rows("MATCH (p:Person) RETURN p.name", HashMap::new())
            .expect("match");
        assert_eq!(text(rows.first().and_then(|r| r.first())), "x");
    }

    #[test]
    fn persistent_open_insert() {
        let dir = std::env::temp_dir().join(format!("rtok-grafeo-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = Graph::open(Some(&dir)).expect("open persist");
        g.rows("INSERT (:Person {name: 'y'})", HashMap::new())
            .expect("insert");
        drop(g);
        let g = Graph::open(Some(&dir)).expect("reopen");
        let rows = g
            .rows("MATCH (p:Person) RETURN p.name", HashMap::new())
            .expect("match");
        assert_eq!(text(rows.first().and_then(|r| r.first())), "y");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_replace_count_defs() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_symbols(
                "/r",
                "a.rs",
                "sha",
                (1, 1),
                &[("foo".into(), "function".into(), 1, true, 2, String::new())],
            )
            .unwrap();
        assert_eq!(store.symbol_count("/r").unwrap(), 1);
        let defs = store.symbol_defs("/r", "foo").unwrap();
        assert_eq!(defs[0].0, "a.rs");
        assert_eq!(store.symbol_stat("/r", "a.rs").unwrap().unwrap().0, "sha");
        let keep = HashSet::from(["a.rs".into()]);
        assert_eq!(store.delete_symbols_missing("/r", &keep).unwrap(), 0);
        store
            .replace_symbols(
                "/r",
                "a.rs",
                "sha2",
                (1, 1),
                &[
                    ("c".into(), "function".into(), 1, true, 1, String::new()),
                    ("c".into(), "function".into(), 2, false, 2, "b".into()),
                    ("b".into(), "function".into(), 3, true, 3, String::new()),
                ],
            )
            .unwrap();
        assert_eq!(
            store.symbol_count("/r").unwrap(),
            3,
            "defs={:?} refs={:?}",
            store.symbol_defs("/r", "c"),
            store.symbol_refs("/r", "c")
        );
        assert_eq!(store.symbol_refs("/r", "c").unwrap().len(), 1);
        let hop = store.symbol_impact("/r", "c", 1).unwrap();
        assert_eq!(hop, vec![(1, "a.rs".into(), "b".into())], "{hop:?}");
    }
}
