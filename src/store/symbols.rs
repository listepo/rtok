//! T8.10: the `graph` plugin's symbol index over SQLite (the only backend after P39).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use diesel::alias;
use diesel::dsl::{count_star, exists, min, not};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use super::Store;
use super::schema::{extractor, symbol_stale, symbols};

const INSERT_CHUNK: usize = 999 / 11;

/// SQLite's default `SQLITE_MAX_VARIABLE_NUMBER` is 999; a BFS level's frontier is chunked
/// below that so a wide fan-out never blows the bind limit in one `eq_any`.
const NAME_CHUNK: usize = 500;

/// One level of `symbol_impact`'s walk: references of any name in `frontier`, as
/// `(matched name, path, enclosing scope)` — the direct-caller edge (T163.1 BFS, replaces
/// the old `WITH RECURSIVE` base/step: `s.name = w.scope`). The matched name comes back
/// with each row so a caller batching several frontier tips in one query can route a result
/// to the specific chain(s) it extends.
fn impact_refs(
    conn: &mut SqliteConnection,
    root: &str,
    frontier: &[String],
) -> QueryResult<Vec<(String, String, String)>> {
    let mut out = Vec::new();
    for chunk in frontier.chunks(NAME_CHUNK) {
        out.extend(
            symbols::table
                .filter(symbols::root.eq(root))
                .filter(symbols::name.eq_any(chunk))
                .filter(symbols::is_def.eq(0))
                .filter(symbols::name.ne(""))
                .filter(symbols::kind.ne("import"))
                .select((symbols::name, symbols::path, symbols::scope))
                .load::<(String, String, String)>(conn)?,
        );
    }
    Ok(out)
}

/// One level of `symbol_impact`'s walk: definitions in files that import any name in
/// `frontier`, as `(matched name, path, def name)` — the import-follow edge (T163.1 BFS,
/// replaces the old `WITH RECURSIVE` import branch). See [`impact_refs`] for why the
/// matched name comes back with each row.
fn impact_import_follow(
    conn: &mut SqliteConnection,
    root: &str,
    frontier: &[String],
) -> QueryResult<Vec<(String, String, String)>> {
    let (i, d) = alias!(symbols as i, symbols as d);
    let mut out = Vec::new();
    for chunk in frontier.chunks(NAME_CHUNK) {
        out.extend(
            i.inner_join(
                d.on(d
                    .field(symbols::root)
                    .eq(i.field(symbols::root))
                    .and(d.field(symbols::path).eq(i.field(symbols::path)))
                    .and(d.field(symbols::is_def).eq(1))
                    .and(d.field(symbols::name).ne(""))),
            )
            .filter(i.field(symbols::root).eq(root))
            .filter(i.field(symbols::name).eq_any(chunk))
            .filter(i.field(symbols::kind).eq("import"))
            .filter(i.field(symbols::is_def).eq(0))
            .select((
                i.field(symbols::name),
                d.field(symbols::path),
                d.field(symbols::name),
            ))
            .load::<(String, String, String)>(conn)?,
        );
    }
    Ok(out)
}

/// Groups one BFS level's flat query rows `(matched tip, value)` by that tip, deduping a
/// repeated `value` under the same tip — shared by `symbol_impact`'s two edges and
/// `symbol_paths`'s one so each builds a `tip -> Vec<value>` map of candidates to hand back
/// to the partial chain(s) that produced the tip.
fn group_by_tip<T: PartialEq>(rows: Vec<(String, T)>, by_tip: &mut HashMap<String, Vec<T>>) {
    for (tip, value) in rows {
        let values = by_tip.entry(tip).or_default();
        if !values.contains(&value) {
            values.push(value);
        }
    }
}

/// One step of `symbol_paths`'s walk: for every name in `tips`, the reference rows that
/// could extend a chain through it — `(referenced name, enclosing scope)` (T163.1 BFS,
/// replaces the old `WITH RECURSIVE` step: `s.name = w.tip`).
fn path_next_hops(
    conn: &mut SqliteConnection,
    root: &str,
    tips: &[String],
) -> QueryResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for chunk in tips.chunks(NAME_CHUNK) {
        out.extend(
            symbols::table
                .filter(symbols::root.eq(root))
                .filter(symbols::name.eq_any(chunk))
                .filter(symbols::is_def.eq(0))
                .filter(symbols::scope.ne(""))
                .filter(symbols::kind.ne("import"))
                .select((symbols::name, symbols::scope))
                .load::<(String, String)>(conn)?,
        );
    }
    Ok(out)
}

/// Every row of one file under one root, through the `(root, path)` index.
fn delete_file(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<usize> {
    diesel::delete(symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(path))))
        .execute(conn)
}

fn note_stale(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<()> {
    diesel::insert_or_ignore_into(symbol_stale::table)
        .values((symbol_stale::root.eq(root), symbol_stale::path.eq(path)))
        .execute(conn)?;
    Ok(())
}

/// One file's rows, inside the caller's transaction: drop what the file had, insert the
/// new tags in chunks, and return how many landed. A tagless file still gets a row so the
/// `file_sha` stands and the next run skips it on the stat alone.
fn replace_one(
    conn: &mut SqliteConnection,
    root: &str,
    path: &str,
    file_sha: &str,
    stat: (i64, i64),
    rows: &[(String, String, i32, bool, i32, String)],
) -> QueryResult<usize> {
    diesel::delete(symbols::table.filter(symbols::root.eq(root).and(symbols::path.eq(path))))
        .execute(conn)?;
    if rows.is_empty() {
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
}

fn clear_stale(conn: &mut SqliteConnection, root: &str, path: &str) -> QueryResult<()> {
    diesel::delete(
        symbol_stale::table.filter(symbol_stale::root.eq(root).and(symbol_stale::path.eq(path))),
    )
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
        let mut conn = self.lock()?;
        Ok(symbol_stale::table
            .filter(symbol_stale::root.eq(root))
            .select(symbol_stale::path)
            .order(symbol_stale::path.asc())
            .load(&mut *conn)?)
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
                inserted += replace_one(conn, root, path, file_sha, *stat, rows)?;
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
            replace_one(conn, root, path, file_sha, stat, rows)
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
        Ok(extractor::table
            .filter(extractor::root.eq(root))
            .select(extractor::fingerprint)
            .first(&mut *conn)
            .optional()?)
    }

    pub fn set_extractor_fingerprint(&self, root: &str, fp: &str) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(extractor::table)
            .values((extractor::root.eq(root), extractor::fingerprint.eq(fp)))
            .on_conflict(extractor::root)
            .do_update()
            .set(extractor::fingerprint.eq(fp))
            .execute(&mut *conn)?;
        Ok(())
    }

    pub fn symbol_indexed_at(&self, root: &str) -> Result<Option<i64>> {
        let mut conn = self.lock()?;
        let indexed_at: Option<Option<i64>> = extractor::table
            .filter(extractor::root.eq(root))
            .select(extractor::indexed_at)
            .first(&mut *conn)
            .optional()?;
        Ok(indexed_at.flatten())
    }

    pub fn touch_symbol_indexed_at(&self, root: &str, ts: i64) -> Result<()> {
        let mut conn = self.lock()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let existing_fingerprint: Option<String> = extractor::table
                .filter(extractor::root.eq(root))
                .select(extractor::fingerprint)
                .first(conn)
                .optional()?;
            diesel::insert_into(extractor::table)
                .values((
                    extractor::root.eq(root),
                    extractor::fingerprint.eq(existing_fingerprint.unwrap_or_default()),
                    extractor::indexed_at.eq(ts),
                ))
                .on_conflict(extractor::root)
                .do_update()
                .set(extractor::indexed_at.eq(ts))
                .execute(conn)?;
            Ok(())
        })?;
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
    // Self-join `symbols` against itself, grouped by `(d.path, d.line, r.name)`: Diesel's
    // `alias!` self-join fields (`AliasedField`) have no `IsContainedInGroupBy` bridge (only
    // `ValidGrouping<()>`, i.e. no `GROUP BY` at all), so the `GROUP BY` itself can't be
    // expressed through the typed DSL — the join and filter can. Load the ungrouped rows with
    // the typed DSL and do the `GROUP BY MIN(r.line)` / `ORDER BY` in Rust instead (T163.1).
    pub fn symbol_callees(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Vec<(String, i32, String, i32)>> {
        let (d, r) = alias!(symbols as d, symbols as r);
        let mut conn = self.lock()?;
        let rows: Vec<(String, i32, String, i32)> = d
            .inner_join(
                r.on(r
                    .field(symbols::root)
                    .eq(d.field(symbols::root))
                    .and(r.field(symbols::path).eq(d.field(symbols::path)))
                    .and(r.field(symbols::is_def).eq(0))
                    .and(r.field(symbols::scope).eq(d.field(symbols::name)))),
            )
            .filter(
                d.field(symbols::root)
                    .eq(root)
                    .and(d.field(symbols::is_def).eq(1))
                    .and(d.field(symbols::name).eq(name))
                    .and(r.field(symbols::name).ne("")),
            )
            .select((
                d.field(symbols::path),
                d.field(symbols::line),
                r.field(symbols::name),
                r.field(symbols::line),
            ))
            .load(&mut *conn)?;

        // GROUP BY (path, line, callee), keeping MIN(r.line) as first_line.
        let mut groups: HashMap<(String, i32, String), i32> = HashMap::new();
        for (path, line, callee, r_line) in rows {
            groups
                .entry((path, line, callee))
                .and_modify(|first| *first = (*first).min(r_line))
                .or_insert(r_line);
        }
        let mut out: Vec<(String, i32, String, i32)> = groups
            .into_iter()
            .map(|((path, line, callee), first_line)| (path, line, callee, first_line))
            .collect();
        // ORDER BY d.path, d.line, first_line, r.name
        out.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.3.cmp(&b.3))
                .then(a.2.cmp(&b.2))
        });
        Ok(out)
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
                count_star(),
                min(symbols::line),
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
        let (d, r, e) = alias!(symbols as d, symbols as r, symbols as e);
        let mut conn = self.lock()?;
        let refs_count = || {
            r.filter(r.field(symbols::root).eq(d.field(symbols::root)))
                .filter(r.field(symbols::name).eq(d.field(symbols::name)))
                .filter(r.field(symbols::is_def).eq(0))
                .filter(r.field(symbols::kind).ne("import"))
                .count()
                .single_value()
        };
        let earlier_def_exists = exists(
            e.filter(e.field(symbols::root).eq(d.field(symbols::root)))
                .filter(e.field(symbols::name).eq(d.field(symbols::name)))
                .filter(e.field(symbols::is_def).eq(1))
                .filter(
                    e.field(symbols::path).lt(d.field(symbols::path)).or(e
                        .field(symbols::path)
                        .eq(d.field(symbols::path))
                        .and(e.field(symbols::line).lt(d.field(symbols::line)))),
                ),
        );
        let rows: Vec<(String, Option<i64>, String, i32)> = d
            .filter(d.field(symbols::root).eq(root))
            .filter(d.field(symbols::is_def).eq(1))
            .filter(d.field(symbols::name).ne(""))
            .filter(not(earlier_def_exists))
            .select((
                d.field(symbols::name),
                refs_count(),
                d.field(symbols::path),
                d.field(symbols::line),
            ))
            .order((refs_count().desc(), d.field(symbols::name).asc()))
            .limit(limit)
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(name, refs, path, line)| (name, refs.unwrap_or(0), path, line))
            .collect())
    }

    /// T52.4: definitions with no same-name reference row under `root`,
    /// as `(path, name, kind, line)`. Name-based, like `callers`: a shared
    /// name keeps every same-named definition live. Callers filter pub,
    /// trait impls, tests and macros from this candidate set.
    pub fn symbol_dead_candidates(&self, root: &str) -> Result<Vec<(String, String, String, i32)>> {
        let (d, r) = alias!(symbols as d, symbols as r);
        let mut conn = self.lock()?;
        let has_ref = exists(
            r.filter(r.field(symbols::root).eq(d.field(symbols::root)))
                .filter(r.field(symbols::name).eq(d.field(symbols::name)))
                .filter(r.field(symbols::is_def).eq(0))
                .filter(r.field(symbols::kind).ne("import")),
        );
        Ok(d.filter(d.field(symbols::root).eq(root))
            .filter(d.field(symbols::is_def).eq(1))
            .filter(d.field(symbols::name).ne(""))
            .filter(not(has_ref))
            .order((d.field(symbols::path).asc(), d.field(symbols::line).asc()))
            .select((
                d.field(symbols::path),
                d.field(symbols::name),
                d.field(symbols::kind),
                d.field(symbols::line),
            ))
            .load(&mut *conn)?)
    }

    /// Callers of `name` out to `depth`, each `(path, scope)` at its first depth (T8.13).
    ///
    /// Level-by-level BFS (T163.1) over the same two edges the old `WITH RECURSIVE` walk
    /// used: a direct reference (`impact_refs`, edge `s.name = frontier`) and an
    /// import-follow hop (`impact_import_follow`, edge `i.name = frontier`). Like
    /// `symbol_paths`, each partial chain keeps its own `seen` set (the old CTE's per-row
    /// `seen`, seeded with the depth-1 row's own output — not `name` itself, matching the
    /// old seed exactly) rather than a global visited set: a name already on *this* chain's
    /// history is never re-emitted for it, but an unrelated chain that never saw that name
    /// still can, and does, produce its own row. Both edges are tried from every chain's
    /// tip at every level, exactly like the CTE's two recursive branches. `results` is keyed
    /// by `(path, scope)` and filled in strictly increasing depth order, so the first write
    /// per key is its minimum depth — the old `GROUP BY path, scope` + `MIN(depth)`.
    pub fn symbol_impact(
        &self,
        root: &str,
        name: &str,
        depth: u32,
    ) -> Result<Vec<(u32, String, String)>> {
        struct Chain {
            tip: String,
            seen: HashSet<String>,
        }

        let max_depth = depth.clamp(1, 4);
        let mut conn = self.lock()?;
        let mut results: HashMap<(String, String), u32> = HashMap::new();
        let mut current = vec![Chain {
            tip: name.to_string(),
            seen: HashSet::new(),
        }];
        let mut level: u32 = 1;
        while !current.is_empty() && level <= max_depth {
            let tips: Vec<String> = {
                let set: HashSet<&str> = current.iter().map(|c| c.tip.as_str()).collect();
                set.into_iter().map(str::to_string).collect()
            };
            let mut by_tip: HashMap<String, Vec<(String, String)>> = HashMap::new();
            group_by_tip(
                impact_refs(&mut conn, root, &tips)?
                    .into_iter()
                    .map(|(tip, path, scope)| (tip, (path, scope)))
                    .collect(),
                &mut by_tip,
            );
            group_by_tip(
                impact_import_follow(&mut conn, root, &tips)?
                    .into_iter()
                    .map(|(tip, path, def_name)| (tip, (path, def_name)))
                    .collect(),
                &mut by_tip,
            );

            let mut next = Vec::new();
            for c in &current {
                let Some(edges) = by_tip.get(&c.tip) else {
                    continue;
                };
                for (path, out_name) in edges {
                    if c.seen.contains(out_name) {
                        continue;
                    }
                    results
                        .entry((path.clone(), out_name.clone()))
                        .or_insert(level);
                    let mut seen = c.seen.clone();
                    seen.insert(out_name.clone());
                    next.push(Chain {
                        tip: out_name.clone(),
                        seen,
                    });
                }
            }
            current = next;
            level += 1;
        }
        let mut out: Vec<(u32, String, String)> = results
            .into_iter()
            .map(|((path, scope), depth)| (depth, path, scope))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        Ok(out)
    }

    /// T68.1: distinct definition names starting with `prefix`, best `limit` by
    /// reference count (ties by name, byte-stable) — `explore`'s fallback when a
    /// query token is not an exact definition name.
    pub fn symbol_name_prefix(&self, root: &str, prefix: &str, limit: i64) -> Result<Vec<String>> {
        // Only the correlated subquery needs a second `symbols` occurrence (`r`); the outer
        // query groups by the real `symbols::name` column, which keeps `GROUP BY` on a plain
        // (non-aliased) column — see `symbol_callees` for why an aliased self-join can't.
        let r = alias!(symbols as r);
        let mut conn = self.lock()?;
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like = format!("{escaped}%");
        let refs_count = || {
            r.filter(r.field(symbols::root).eq(root))
                .filter(r.field(symbols::name).eq(symbols::name))
                .filter(r.field(symbols::is_def).eq(0))
                .filter(r.field(symbols::kind).ne("import"))
                .count()
                .single_value()
        };
        let rows: Vec<(String, Option<i64>)> = symbols::table
            .filter(symbols::root.eq(root))
            .filter(symbols::is_def.eq(1))
            .filter(symbols::name.ne(""))
            .filter(symbols::name.like(&like).escape('\\'))
            .group_by(symbols::name)
            .select((symbols::name, refs_count()))
            .order((refs_count().desc(), symbols::name.asc()))
            .limit(limit)
            .load(&mut *conn)?;
        Ok(rows.into_iter().map(|(name, _)| name).collect())
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
        // Simple-path enumeration (T163.1 BFS), one level of `path_next_hops` per step: unlike
        // `symbol_impact`, every distinct chain string matters here, not just the shortest
        // reach of a name, so each partial path keeps its own `seen` set of every name already
        // on it (the old CTE's per-row `seen` chain) rather than a global visited set. A path
        // whose tip already equals `to` stops extending (mirrors the old `WHERE w.tip != ?`)
        // but was already recorded as a result at the depth it reached `to`.
        struct Partial {
            chain: String,
            tip: String,
            seen: HashSet<String>,
        }

        let max_depth = depth.clamp(1, 4);
        let mut conn = self.lock()?;
        let mut results: Vec<(u32, String)> = Vec::new();
        if from == to {
            results.push((1, from.to_string()));
        }
        let mut current = vec![Partial {
            chain: from.to_string(),
            tip: from.to_string(),
            seen: HashSet::from([from.to_string()]),
        }];
        let mut level: u32 = 1;
        while level < max_depth {
            let active: Vec<&Partial> = current.iter().filter(|p| p.tip != to).collect();
            if active.is_empty() {
                break;
            }
            let tips: Vec<String> = {
                let set: HashSet<&str> = active.iter().map(|p| p.tip.as_str()).collect();
                set.into_iter().map(str::to_string).collect()
            };
            let mut by_tip: HashMap<String, Vec<String>> = HashMap::new();
            group_by_tip(path_next_hops(&mut conn, root, &tips)?, &mut by_tip);

            let mut next = Vec::new();
            for p in active {
                let Some(scopes) = by_tip.get(&p.tip) else {
                    continue;
                };
                for scope in scopes {
                    if p.seen.contains(scope) {
                        continue;
                    }
                    let chain = format!("{} → {}", p.chain, scope);
                    if scope == to {
                        results.push((level + 1, chain.clone()));
                    }
                    let mut seen = p.seen.clone();
                    seen.insert(scope.clone());
                    next.push(Partial {
                        chain,
                        tip: scope.clone(),
                        seen,
                    });
                }
            }
            current = next;
            level += 1;
        }

        results.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        Ok(results.into_iter().map(|(_, chain)| chain).collect())
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
        let (i, d) = alias!(symbols as i, symbols as d);
        let mut conn = self.lock()?;
        Ok(i.inner_join(
            d.on(d
                .field(symbols::root)
                .eq(i.field(symbols::root))
                .and(d.field(symbols::path).eq(i.field(symbols::path)))
                .and(d.field(symbols::is_def).eq(1))
                .and(d.field(symbols::name).ne(""))),
        )
        .filter(
            i.field(symbols::root)
                .eq(root)
                .and(i.field(symbols::name).eq(name))
                .and(i.field(symbols::kind).eq("import"))
                .and(i.field(symbols::is_def).eq(0)),
        )
        .order((d.field(symbols::path).asc(), d.field(symbols::line).asc()))
        .select((d.field(symbols::path), d.field(symbols::name)))
        .load(&mut *conn)?)
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

    /// A reference row with an explicit enclosing `scope`, unlike `row`/`import` (always
    /// `scope: ""`) — needed to build the `symbol_impact` chains below.
    fn reference(name: &str, line: i32, scope: &str) -> (String, String, i32, bool, i32, String) {
        (
            name.into(),
            "function".into(),
            line,
            false,
            line,
            scope.into(),
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

    // T163.1 regression (PR #206 review): `symbol_impact`'s BFS must exclude a candidate
    // already on *that specific chain's* history, not just prune it from further expansion.
    // Expected rows below were checked against the old `WITH RECURSIVE` query (from
    // `origin/main` before this rework) run on the same fixtures via the sqlite3 CLI.

    #[test]
    fn impact_excludes_ref_edge_name_already_on_the_chain() {
        // a.rs: fn X references N (depth-1 row: a.rs/X, chain seen={X}).
        // b.rs: a different fn X calls X (self-recursive) — the depth-2 candidate is
        // (b.rs, X), but X is already in that chain's seen, so the old CTE never emits it
        // and no other chain reaches it either.
        let store = Store::open_in_memory().unwrap();
        store
            .replace_symbols(
                "/r1",
                "a.rs",
                "s",
                (0, 0),
                &[row("X", 1, true), reference("N", 2, "X")],
            )
            .unwrap();
        store
            .replace_symbols(
                "/r1",
                "b.rs",
                "s",
                (0, 0),
                &[row("X", 1, true), reference("X", 2, "X")],
            )
            .unwrap();
        let got = store.symbol_impact("/r1", "N", 4).unwrap();
        assert_eq!(got, vec![(1, "a.rs".into(), "X".into())]);
    }

    #[test]
    fn impact_excludes_import_follow_name_already_on_the_chain() {
        // c.rs imports N2, which resolves to def M (depth-1 row: c.rs/M, chain seen={M}).
        // e.rs imports M and also defines M — the depth-2 candidate is (e.rs, M), but M is
        // already in that chain's seen, so the old CTE never emits it either.
        let store = Store::open_in_memory().unwrap();
        store
            .replace_symbols(
                "/r2",
                "c.rs",
                "s",
                (0, 0),
                &[import("N2", 1), row("M", 2, true)],
            )
            .unwrap();
        store
            .replace_symbols(
                "/r2",
                "e.rs",
                "s",
                (0, 0),
                &[import("M", 1), row("M", 2, true)],
            )
            .unwrap();
        let got = store.symbol_impact("/r2", "N2", 4).unwrap();
        assert_eq!(got, vec![(1, "c.rs".into(), "M".into())]);
    }
}
