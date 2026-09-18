//! T8.10: the `graph` plugin's symbol index over SQLite (the only backend after P39).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;

use super::Store;
use super::schema::symbols;

const INSERT_CHUNK: usize = 999 / 11;

/// Every row of one file under one root, through the `(root, path)` index.
fn delete_file(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<usize> {
    diesel::delete(symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(path))))
        .execute(conn)
}

fn note_stale(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<()> {
    sql_query("INSERT OR IGNORE INTO symbol_stale (root, path) VALUES (?, ?)")
        .bind::<Text, _>(root)
        .bind::<Text, _>(path)
        .execute(conn)?;
    Ok(())
}

fn clear_stale(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<()> {
    sql_query("DELETE FROM symbol_stale WHERE root = ? AND path = ?")
        .bind::<Text, _>(root)
        .bind::<Text, _>(path)
        .execute(conn)?;
    Ok(())
}

impl Store {
    /// Rows indexed under one repo root (T8.3). Every symbol call is scoped to a root, so
    /// two repos in the one store (D8) never evict or answer for each other.
    pub fn symbol_count(&self, root: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(symbols::root.eq(root))
            .count()
            .get_result(&mut *conn)?)
    }

    /// Distinct indexed file paths under `root` (T68.3 `graph status`).
    pub fn symbol_file_count(&self, root: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(symbols::root.eq(root))
            .select(symbols::path)
            .distinct()
            .count()
            .get_result(&mut *conn)?)
    }

    /// Paths still carrying the T8.3 stale mark (hook delete, not yet re-indexed).
    pub fn symbol_stale_paths(&self, root: &str) -> Result<Vec<String>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            path: String,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query("SELECT path FROM symbol_stale WHERE root = ? ORDER BY path")
            .bind::<Text, _>(root)
            .load(&mut *conn)?;
        Ok(rows.into_iter().map(|r| r.path).collect())
    }

    /// Pending files: hook-staled rows plus indexed paths whose stat no longer matches disk.
    pub fn symbol_pending(&self, root: &str, root_path: &std::path::Path) -> Result<Vec<String>> {
        let mut pending: HashSet<String> = self.symbol_stale_paths(root)?.into_iter().collect();
        for (path, (_, mtime, size)) in self.symbol_stats(root)? {
            let abs = root_path.join(&path);
            let stat = abs
                .metadata()
                .map(|md| {
                    let mtime = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos() as i64)
                        .unwrap_or(0);
                    (mtime, md.len() as i64)
                })
                .unwrap_or((0, 0));
            if stat != (mtime, size) && stat != (0, 0) {
                pending.insert(path);
            }
        }
        let mut out: Vec<String> = pending.into_iter().collect();
        out.sort();
        Ok(out)
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

    pub fn symbol_stats(&self, root: &str) -> Result<HashMap<String, (String, i64, i64)>> {
        let mut conn = self.lock()?;
        let rows: Vec<(String, String, i64, i64)> = symbols::table
            .filter(symbols::root.eq(root))
            .select((
                symbols::path,
                symbols::file_sha,
                symbols::mtime,
                symbols::size,
            ))
            .distinct()
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(p, s, m, z)| (p, (s, m, z)))
            .collect())
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

    pub fn replace_symbol_files(
        &self,
        root: &str,
        files: &rtok_plugin_sdk::SymbolFileBatch,
    ) -> Result<usize> {
        if files.is_empty() {
            return Ok(0);
        }
        let mut conn = self.lock()?;
        Ok(conn.transaction::<usize, diesel::result::Error, _>(|conn| {
            let mut inserted = 0usize;
            for (path, file_sha, stat, rows) in files {
                diesel::delete(
                    symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(&path))),
                )
                .execute(conn)?;
                if rows.is_empty() {
                    diesel::insert_into(symbols::table)
                        .values((
                            symbols::root.eq(root),
                            symbols::path.eq(&path),
                            symbols::name.eq(""),
                            symbols::kind.eq(""),
                            symbols::line.eq(0),
                            symbols::is_def.eq(0),
                            symbols::file_sha.eq(&file_sha),
                            symbols::mtime.eq(stat.0),
                            symbols::size.eq(stat.1),
                        ))
                        .execute(conn)?;
                    continue;
                }
                for chunk in rows.chunks(INSERT_CHUNK) {
                    let values: Vec<_> = chunk
                        .iter()
                        .map(|(name, kind, line, is_def, end_line, scope)| {
                            (
                                symbols::root.eq(root),
                                symbols::path.eq(&path),
                                symbols::name.eq(name),
                                symbols::kind.eq(kind),
                                symbols::line.eq(line),
                                symbols::is_def.eq(i32::from(*is_def)),
                                symbols::file_sha.eq(&file_sha),
                                symbols::mtime.eq(stat.0),
                                symbols::size.eq(stat.1),
                                symbols::end_line.eq(end_line),
                                symbols::scope.eq(scope),
                            )
                        })
                        .collect();
                    diesel::insert_into(symbols::table)
                        .values(&values)
                        .execute(conn)?;
                }
                clear_stale(conn, root, &path)?;
                inserted += rows.len();
            }
            Ok(inserted)
        })?)
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
        // PERF(T35.3) where: the row loop below, per file of a cold index (this repo: 18 093
        // rows over 127 files). What: multi-row INSERTs chunked under SQLite's variable limit,
        // one transaction per batch of files. Why: ~140 single-row INSERTs per file. Not yet
        // measured apart from the parse — measure before changing.
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
            for chunk in rows.chunks(INSERT_CHUNK) {
                let values: Vec<_> = chunk
                    .iter()
                    .map(|(name, kind, line, is_def, end_line, scope)| {
                        (
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
                        )
                    })
                    .collect();
                diesel::insert_into(symbols::table)
                    .values(&values)
                    .execute(conn)?;
            }
            clear_stale(conn, root, path)?;
            Ok(rows.len())
        })?)
    }

    pub fn delete_symbols_missing(&self, root: &str, keep: &HashSet<String>) -> Result<usize> {
        let mut conn = self.lock()?;
        if keep.is_empty() {
            return Ok(
                diesel::delete(symbols::table.filter(symbols::root.eq(root)))
                    .execute(&mut *conn)?,
            );
        }
        let have: Vec<String> = symbols::table
            .filter(symbols::root.eq(root))
            .select(symbols::path)
            .distinct()
            .load(&mut *conn)?;
        let missing: Vec<&str> = have
            .iter()
            .filter(|p| !keep.contains(p.as_str()))
            .map(String::as_str)
            .collect();
        if missing.is_empty() {
            return Ok(0);
        }
        Ok(diesel::delete(
            symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq_any(&missing))),
        )
        .execute(&mut *conn)?)
    }

    /// Drop rows for one canonical absolute file path. No indexing on the hook path.
    /// A same-named file in another repo survives: the row must match on `(root, path)`.
    /// The caller does not know the root, so every `/` split of the path is tried through
    /// [`Store::mark_symbols_stale_in`] — a dozen indexed point deletes, where
    /// `WHERE ? = root || '/' || path` scanned the whole table on each `Edit`/`Write`.
    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for (i, _) in abs_path.match_indices('/') {
                let root = &abs_path[..i];
                let rel = &abs_path[i + 1..];
                if delete_file(conn, root, rel)? > 0 {
                    note_stale(conn, root, rel)?;
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Drop the rows of `rel_path` under `root`: one indexed delete for a caller that knows
    /// the root.
    pub fn mark_symbols_stale_in(&self, root: &str, rel_path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            if delete_file(conn, root, rel_path)? > 0 {
                note_stale(conn, root, rel_path)?;
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn extractor_fingerprint(&self, root: &str) -> Result<Option<String>> {
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

    pub fn symbol_indexed_at(&self, root: &str) -> Result<Option<i64>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Nullable<BigInt>)]
            indexed_at: Option<i64>,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query("SELECT indexed_at FROM extractor WHERE root = ?")
            .bind::<Text, _>(root)
            .load(&mut *conn)?;
        Ok(rows.first().and_then(|r| r.indexed_at))
    }

    pub fn touch_symbol_indexed_at(&self, root: &str, ts: i64) -> Result<()> {
        let mut conn = self.lock()?;
        sql_query(
            "INSERT INTO extractor (root, fingerprint, indexed_at)
             VALUES (?, COALESCE((SELECT fingerprint FROM extractor WHERE root = ?), ''), ?)
             ON CONFLICT(root) DO UPDATE SET indexed_at = excluded.indexed_at",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(root)
        .bind::<BigInt, _>(ts)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Definitions of `name` as `(path, kind, line)`, ordered by path then line (T8.2 `symbol`).
    pub fn symbol_defs(&self, root: &str, name: &str) -> Result<Vec<(String, String, i32, i32)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(name))
                    .and(symbols::is_def.eq(1)),
            )
            .order((symbols::path.asc(), symbols::line.asc()))
            .select((
                symbols::path,
                symbols::kind,
                symbols::line,
                symbols::end_line,
            ))
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
                    .and(symbols::is_def.eq(0))
                    .and(symbols::kind.ne("import")),
            )
            .order((symbols::path.asc(), symbols::line.asc()))
            .select((symbols::path, symbols::line))
            .load(&mut *conn)?)
    }

    /// Callees of each definition of `name`: `(path, line, callee, first_ref_line)` (T68.2).
    /// A reference row counts when it shares the definition's path and its `scope` is the
    /// definition's name; results are ordered by definition site then first reference line.
    pub fn symbol_callees(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, i32, String, i32)>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            path: String,
            #[diesel(sql_type = Integer)]
            line: i32,
            #[diesel(sql_type = Text)]
            callee: String,
            #[diesel(sql_type = Integer)]
            first_line: i32,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT d.path AS path, d.line AS line, r.name AS callee, MIN(r.line) AS first_line
             FROM symbols d
             JOIN symbols r
               ON r.root = d.root AND r.path = d.path AND r.is_def = 0 AND r.scope = d.name
             WHERE d.root = ? AND d.is_def = 1 AND d.name = ? AND r.name != ''
             GROUP BY d.path, d.line, r.name
             ORDER BY d.path, d.line, first_line, r.name",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(name)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.path, r.line, r.callee, r.first_line))
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
        let mut conn = self.lock()?;
        let rows: Vec<(String, String, i64, Option<i32>)> = symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(name))
                    .and(symbols::is_def.eq(0))
                    .and(symbols::kind.ne("import")),
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

    /// T52.3: names under `root` ranked by reference count, with one def site
    /// `(path, line)`. `ORDER BY refs DESC, name ASC` (byte-stable). The def
    /// site is first by `path ASC, line ASC`. Import rows are not refs.
    pub fn symbol_top_refs(
        &self,
        root: &str,
        limit: i64,
    ) -> Result<Vec<(String, i64, String, i32)>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            name: String,
            #[diesel(sql_type = BigInt)]
            refs: i64,
            #[diesel(sql_type = Text)]
            path: String,
            #[diesel(sql_type = Integer)]
            line: i32,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT d.name AS name,
                    (SELECT COUNT(*) FROM symbols r
                      WHERE r.root = d.root AND r.name = d.name AND r.is_def = 0
                        AND r.kind != 'import') AS refs,
                    d.path AS path,
                    d.line AS line
             FROM symbols d
             WHERE d.root = ? AND d.is_def = 1 AND d.name != ''
               AND NOT EXISTS (
                 SELECT 1 FROM symbols e
                  WHERE e.root = d.root AND e.name = d.name AND e.is_def = 1
                    AND (e.path < d.path OR (e.path = d.path AND e.line < d.line)))
             ORDER BY refs DESC, name ASC
             LIMIT ?",
        )
        .bind::<Text, _>(root)
        .bind::<Integer, _>(limit as i32)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.name, r.refs, r.path, r.line))
            .collect())
    }

    /// T52.4: definitions with no same-name reference row under `root`,
    /// as `(path, name, kind, line)`. Name-based, like `callers`: a shared
    /// name keeps every same-named definition live. Callers filter pub,
    /// trait impls, tests and macros from this candidate set.
    pub fn symbol_dead_candidates(&self, root: &str) -> Result<Vec<(String, String, String, i32)>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            path: String,
            #[diesel(sql_type = Text)]
            name: String,
            #[diesel(sql_type = Text)]
            kind: String,
            #[diesel(sql_type = Integer)]
            line: i32,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT d.path AS path, d.name AS name, d.kind AS kind, d.line AS line
             FROM symbols d
             WHERE d.root = ? AND d.is_def = 1 AND d.name != ''
               AND NOT EXISTS (SELECT 1 FROM symbols r
                 WHERE r.root = d.root AND r.name = d.name AND r.is_def = 0
                   AND r.kind != 'import')
             ORDER BY d.path, d.line",
        )
        .bind::<Text, _>(root)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.path, r.name, r.kind, r.line))
            .collect())
    }

    /// Callers of `name` out to `depth`, each `(path, scope)` at its first depth (T8.13).
    pub fn symbol_impact(
        &self,
        root: &str,
        name: &str,
        depth: u32,
    ) -> Result<Vec<(u32, String, String)>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Integer)]
            depth: i32,
            #[diesel(sql_type = Text)]
            path: String,
            #[diesel(sql_type = Text)]
            scope: String,
        }
        let depth = i32::try_from(depth.clamp(1, 4)).unwrap_or(4);
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "WITH RECURSIVE walk(depth, path, scope, seen) AS (
                SELECT 1, path, scope, ',' || scope || ','
                FROM symbols
                WHERE root = ? AND name = ? AND is_def = 0 AND name != '' AND kind != 'import'
                UNION ALL
                SELECT 1, d.path, d.name, ',' || d.name || ','
                FROM symbols i
                JOIN symbols d
                  ON d.root = i.root AND d.path = i.path AND d.is_def = 1 AND d.name != ''
                WHERE i.root = ? AND i.name = ? AND i.kind = 'import' AND i.is_def = 0
                UNION ALL
                SELECT w.depth + 1, s.path, s.scope, w.seen || s.scope || ','
                FROM walk w
                JOIN symbols s
                  ON s.root = ? AND s.name = w.scope AND s.is_def = 0 AND s.name != ''
                 AND s.kind != 'import'
                WHERE w.depth < ? AND w.scope != ''
                  AND instr(w.seen, ',' || s.scope || ',') = 0
                UNION ALL
                SELECT w.depth + 1, d.path, d.name, w.seen || d.name || ','
                FROM walk w
                JOIN symbols i
                  ON i.root = ? AND i.name = w.scope AND i.kind = 'import' AND i.is_def = 0
                JOIN symbols d
                  ON d.root = i.root AND d.path = i.path AND d.is_def = 1 AND d.name != ''
                WHERE w.depth < ? AND w.scope != ''
                  AND instr(w.seen, ',' || d.name || ',') = 0
            )
            SELECT MIN(depth) AS depth, path, scope
            FROM walk
            GROUP BY path, scope
            ORDER BY depth, path, scope",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(name)
        .bind::<Text, _>(root)
        .bind::<Text, _>(name)
        .bind::<Text, _>(root)
        .bind::<Integer, _>(depth)
        .bind::<Text, _>(root)
        .bind::<Integer, _>(depth)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.depth as u32, r.path, r.scope))
            .collect())
    }

    /// T68.1: distinct definition names starting with `prefix`, best `limit` by
    /// reference count (ties by name, byte-stable) — `explore`'s fallback when a
    /// query token is not an exact definition name.
    pub fn symbol_name_prefix(&self, root: &str, prefix: &str, limit: i64) -> Result<Vec<String>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            name: String,
        }
        let mut conn = self.lock()?;
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like = format!("{escaped}%");
        let rows: Vec<Row> = sql_query(
            "SELECT d.name AS name,
                    (SELECT COUNT(*) FROM symbols r
                      WHERE r.root = ? AND r.name = d.name AND r.is_def = 0
                        AND r.kind != 'import') AS refs
             FROM symbols d
             WHERE d.root = ? AND d.is_def = 1 AND d.name != ''
               AND d.name LIKE ? ESCAPE '\\'
             GROUP BY d.name
             ORDER BY refs DESC, name ASC
             LIMIT ?",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(root)
        .bind::<Text, _>(like)
        .bind::<Integer, _>(limit as i32)
        .load(&mut *conn)?;
        Ok(rows.into_iter().map(|r| r.name).collect())
    }

    /// T68.1: call chains `from → … → to` walked in the caller direction (the
    /// `impact` edges: each step is a definition that references the previous
    /// one). Simple paths only — a name already in the chain is never revisited
    /// and a branch stops growing once it reaches `to` — so cycles terminate and
    /// the shortest forms come first. `explore` prints these between the symbols
    /// a question resolved to; T68.4 reuses the query for `impact --to`.
    pub fn symbol_paths(
        &self,
        root: &str,
        from: &str,
        to: &str,
        depth: u32,
    ) -> Result<Vec<String>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            chain: String,
        }
        let depth = i32::try_from(depth.clamp(1, 4)).unwrap_or(4);
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "WITH RECURSIVE walk(depth, chain, tip, seen) AS (
                SELECT 1, ?, ?, ',' || ? || ','
                UNION ALL
                SELECT w.depth + 1,
                       w.chain || ' → ' || s.scope,
                       s.scope,
                       w.seen || s.scope || ','
                FROM walk w
                JOIN symbols s ON s.root = ? AND s.name = w.tip
                              AND s.is_def = 0 AND s.scope != ''
                              AND s.kind != 'import'
                WHERE w.depth < ? AND w.tip != ?
                  AND instr(w.seen, ',' || s.scope || ',') = 0
            )
            SELECT chain, MIN(depth) AS depth FROM walk
            WHERE tip = ? GROUP BY chain ORDER BY depth, chain",
        )
        .bind::<Text, _>(from)
        .bind::<Text, _>(from)
        .bind::<Text, _>(from)
        .bind::<Text, _>(root)
        .bind::<Integer, _>(depth)
        .bind::<Text, _>(to)
        .bind::<Text, _>(to)
        .load(&mut *conn)?;
        Ok(rows.into_iter().map(|r| r.chain).collect())
    }

    /// T68.6: import rows of `path` as `(name, line)`, first-seen order.
    pub fn symbol_imports(&self, root: &str, path: &str) -> Result<Vec<(String, i32)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::path.eq(path))
                    .and(symbols::kind.eq("import"))
                    .and(symbols::is_def.eq(0)),
            )
            .order(symbols::line.asc())
            .select((symbols::name, symbols::line))
            .load(&mut *conn)?)
    }

    /// T68.6: files that import `module` as `(path, line)`.
    pub fn symbol_importers(&self, root: &str, module: &str) -> Result<Vec<(String, i32)>> {
        let mut conn = self.lock()?;
        Ok(symbols::table
            .filter(
                symbols::root
                    .eq(root)
                    .and(symbols::name.eq(module))
                    .and(symbols::kind.eq("import"))
                    .and(symbols::is_def.eq(0)),
            )
            .order((symbols::path.asc(), symbols::line.asc()))
            .select((symbols::path, symbols::line))
            .load(&mut *conn)?)
    }

    /// T68.6: definitions in files that import `name` — the extra impact hop.
    pub fn symbol_import_follow(&self, root: &str, name: &str) -> Result<Vec<(String, String)>> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Text)]
            path: String,
            #[diesel(sql_type = Text)]
            name: String,
        }
        let mut conn = self.lock()?;
        let rows: Vec<Row> = sql_query(
            "SELECT d.path AS path, d.name AS name
             FROM symbols i
             JOIN symbols d
               ON d.root = i.root AND d.path = i.path AND d.is_def = 1 AND d.name != ''
             WHERE i.root = ? AND i.name = ? AND i.kind = 'import' AND i.is_def = 0
             ORDER BY d.path, d.line",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(name)
        .load(&mut *conn)?;
        Ok(rows.into_iter().map(|r| (r.path, r.name)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, line: i32, is_def: bool) -> (String, String, i32, bool, i32, String) {
        (
            name.into(),
            "function".into(),
            line,
            is_def,
            line,
            String::new(),
        )
    }

    fn import(name: &str, line: i32) -> (String, String, i32, bool, i32, String) {
        (
            name.into(),
            "import".into(),
            line,
            false,
            line,
            String::new(),
        )
    }

    #[test]
    fn top_refs_rank_by_count_then_name() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_symbols(
                "/r",
                "a.rs",
                "s",
                (0, 0),
                &[
                    row("foo", 1, true),
                    row("bar", 2, true),
                    row("aaa", 3, true),
                    row("zed", 4, true),
                    row("foo", 10, false),
                    row("foo", 11, false),
                    row("bar", 12, false),
                    row("bar", 13, false),
                    row("aaa", 14, false),
                    import("foo", 20),
                ],
            )
            .unwrap();
        let got = store.symbol_top_refs("/r", 10).unwrap();
        assert_eq!(
            got.iter().map(|r| (r.0.as_str(), r.1)).collect::<Vec<_>>(),
            [("bar", 2), ("foo", 2), ("aaa", 1), ("zed", 0)]
        );
        assert_eq!(store.symbol_top_refs("/r", 10).unwrap(), got);
    }

    #[test]
    fn top_refs_picks_first_def_site() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_symbols("/r", "b.rs", "s", (0, 0), &[row("dup", 5, true)])
            .unwrap();
        store
            .replace_symbols(
                "/r",
                "a.rs",
                "s",
                (0, 0),
                &[
                    row("dup", 9, true),
                    row("dup", 3, true),
                    row("dup", 1, false),
                ],
            )
            .unwrap();
        let got = store.symbol_top_refs("/r", 4).unwrap();
        assert_eq!(got, vec![("dup".into(), 1, "a.rs".into(), 3)]);
    }
}
