//! T8.10 (P8c): the `graph` plugin's symbol index over SQLite. A second `impl Store`, so
//! T8.11's `lbug` backend is a sibling file selected by `cfg` and no signature moves.

use std::collections::HashSet;

use anyhow::Result;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;

use super::Store;
use super::schema::symbols;

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
}
