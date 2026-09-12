//! T8.10: the `graph` plugin's symbol index over SQLite (the only backend after P39).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Integer, Text};

use super::Store;
use super::schema::symbols;

const INSERT_CHUNK: usize = 999 / 11;
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
    /// Matched as `root || '/' || path` so a same-named file in another repo survives.
    pub fn mark_symbols_stale(&self, abs_path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        sql_query("DELETE FROM symbols WHERE ? = root || '/' || path")
            .bind::<Text, _>(abs_path)
            .execute(&mut *conn)?;
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
                WHERE root = ? AND name = ? AND is_def = 0 AND name != ''
                UNION ALL
                SELECT w.depth + 1, s.path, s.scope, w.seen || s.scope || ','
                FROM walk w
                JOIN symbols s
                  ON s.root = ? AND s.name = w.scope AND s.is_def = 0 AND s.name != ''
                WHERE w.depth < ? AND w.scope != ''
                  AND instr(w.seen, ',' || s.scope || ',') = 0
            )
            SELECT MIN(depth) AS depth, path, scope
            FROM walk
            GROUP BY path, scope
            ORDER BY depth, path, scope",
        )
        .bind::<Text, _>(root)
        .bind::<Text, _>(name)
        .bind::<Text, _>(root)
        .bind::<Integer, _>(depth)
        .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|r| (r.depth as u32, r.path, r.scope))
            .collect())
    }
}
