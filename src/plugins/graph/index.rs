//! Tree-sitter-tags symbol index (plan T8.1).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use ignore::WalkBuilder;

use crate::plugins::read::outline;
use crate::store;
use rtok_plugin_sdk::Ctx;

#[derive(Debug, Default)]
pub struct Report {
    pub indexed: u32,
    pub inserted: usize,
    pub skipped: u32,
    /// Files whose bytes were read (T8.4). A warm run over an untouched tree reads none.
    pub read: u32,
}

/// One symbol row: name, kind, line, is-definition, end line, enclosing definition.
type Row = (String, String, i32, bool, i32, String);

/// `(mtime_nanos, size)` — the freshness key. Nanos keep two edits in the same second apart;
/// an unreadable timestamp reads as 0, which never matches a stored stat, so the file is read.

fn changed_abs(root: &Path, event_path: &Path) -> PathBuf {
    let raw = if event_path.is_absolute() {
        event_path.to_path_buf()
    } else {
        root.join(event_path)
    };
    raw.canonicalize().unwrap_or_else(|_| {
        let name = event_path.file_name();
        event_path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| name.map(|n| p.join(n)))
            .unwrap_or(raw)
    })
}

fn stat_key(md: &std::fs::Metadata) -> (i64, i64) {
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    (mtime, md.len() as i64)
}

/// Canonical absolute path as one string: the index key of a root, and the match key of a
/// file for `mark_symbols_stale` (T8.3). Rows are scoped to it so one store holds many repos.
pub fn canon(p: &Path) -> String {
    p.canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

/// Incremental index of `root`. Returns how many rows were newly written.
/// `dry_run` walks and parses exactly as a real run does but writes no rows, so the report
/// says what the index would gain without touching the store.
pub fn run(cx: &Ctx, root: &Path, dry_run: bool) -> Result<Report> {
    run_with(cx, root, dry_run, &indicatif::ProgressBar::hidden())
}

/// Index only `changed` event paths (T35.4). Vanished or unsupported paths drop their rows;
/// never calls `delete_symbols_missing`.
pub fn run_changed(cx: &Ctx, root: &Path, changed: &HashSet<PathBuf>) -> Result<Report> {
    run_changed_with(cx, root, changed, false, &indicatif::ProgressBar::hidden())
}

/// [`run`] reporting each source file it reaches to `pb`. Only `rtok graph index` passes a real
/// bar; the MCP tool and the background watcher pass `ProgressBar::hidden()`, because neither
/// owns the terminal it would be drawing on.
///
/// The walk and the stat gate run here. The files that pass the gate are read, hashed and parsed
/// on one worker per core (T35.2); every write stays on this thread (D18), in walk order.
// PERF(T35.3) where: `cx.symbol_stat` below. What: load the root's `(path, sha, mtime, size)`
// once into a map. Why: one SELECT per file on every run, warm runs included.
// PERF(T35.5) where: the stat gate below and the sha gate in `parse`. What: an extractor
// fingerprint per root; a mismatch drops the root's rows and indexes cold. Why: the gates see
// only file changes, so after a tags-query, grammar or `scoped` change an untouched file keeps
// the old extractor's rows.
pub fn run_with(
    cx: &Ctx,
    root: &Path,
    dry_run: bool,
    pb: &indicatif::ProgressBar,
) -> Result<Report> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let rk = canon(&root);
    let current_fp = extractor_fingerprint();
    let stored_fp = cx.extractor_fingerprint(&rk)?;
    let fp_stale = stored_fp.as_deref() != Some(current_fp.as_str());
    if fp_stale && !dry_run {
        let _ = cx.delete_symbols_missing(&rk, &HashSet::new());
    }
    let mut report = Report::default();
    let mut keep = HashSet::new();
    let mut jobs = Vec::new();
    for entry in WalkBuilder::new(&root).hidden(false).build() {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !outline::supported(path) {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        keep.insert(rel.clone());
        let stat = entry.metadata().as_ref().map(stat_key).unwrap_or((0, 0));
        let known = cx.symbol_stat(&rk, &rel)?;
        // Same mtime and size: git's rule for "unchanged". Nothing is opened.
        if known.as_ref().is_some_and(|(_, m, s)| (*m, *s) == stat) && stat != (0, 0) {
            report.skipped += 1;
            pb.inc(1);
            continue;
        }
        jobs.push(Job {
            path: path.to_path_buf(),
            rel,
            stat,
            known: known.map(|(sha, _, _)| sha),
        });
    }
    each_parsed(&jobs, |job, parsed| {
        pb.inc(1);
        match parsed {
            Parsed::Unreadable => {}
            Parsed::Unparsed => report.read += 1,
            Parsed::Same => {
                // Touched but not changed: move the freshness key so the next run skips on stat.
                report.read += 1;
                report.skipped += 1;
                if !dry_run {
                    cx.touch_symbols(&rk, &job.rel, job.stat.0, job.stat.1)?;
                }
            }
            Parsed::Rows(sha, rows) => {
                report.read += 1;
                report.indexed += 1;
                report.inserted += if dry_run {
                    rows.len()
                } else {
                    cx.replace_symbols(&rk, &job.rel, &sha, job.stat, &rows)?
                };
            }
        }
        Ok(())
    })?;
    if !dry_run {
        let _ = cx.delete_symbols_missing(&rk, &keep);
        if fp_stale {
            cx.set_extractor_fingerprint(&rk, &current_fp)?;
        }
    }
    pb.finish_and_clear();
    Ok(report)
}

fn run_changed_with(
    cx: &Ctx,
    root: &Path,
    changed: &HashSet<PathBuf>,
    dry_run: bool,
    pb: &indicatif::ProgressBar,
) -> Result<Report> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let rk = canon(&root);
    let mut report = Report::default();
    let mut jobs = Vec::new();
    for event_path in changed {
        let abs = changed_abs(&root, event_path);
        let rel = abs
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"));
        let Ok(rel) = rel else {
            continue;
        };
        if !abs.exists() || !outline::supported(&abs) {
            if !dry_run {
                let _ = cx.mark_symbols_stale(&canon(&abs));
            }
            continue;
        }
        let stat = std::fs::metadata(&abs)
            .as_ref()
            .map(stat_key)
            .unwrap_or((0, 0));
        let known = cx.symbol_stat(&rk, &rel)?;
        if known.as_ref().is_some_and(|(_, m, s)| (*m, *s) == stat) && stat != (0, 0) {
            report.skipped += 1;
            pb.inc(1);
            continue;
        }
        jobs.push(Job {
            path: abs,
            rel,
            stat,
            known: known.map(|(sha, _, _)| sha),
        });
    }
    each_parsed(&jobs, |job, parsed| {
        pb.inc(1);
        match parsed {
            Parsed::Unreadable => {}
            Parsed::Unparsed => report.read += 1,
            Parsed::Same => {
                report.read += 1;
                report.skipped += 1;
                if !dry_run {
                    cx.touch_symbols(&rk, &job.rel, job.stat.0, job.stat.1)?;
                }
            }
            Parsed::Rows(sha, rows) => {
                report.read += 1;
                report.indexed += 1;
                report.inserted += if dry_run {
                    rows.len()
                } else {
                    cx.replace_symbols(&rk, &job.rel, &sha, job.stat, &rows)?
                };
            }
        }
        Ok(())
    })?;
    pb.finish_and_clear();
    Ok(report)
}

/// A file the stat gate could not skip, with the sha the store holds for it.
struct Job {
    path: PathBuf,
    rel: String,
    stat: (i64, i64),
    known: Option<String>,
}

/// What a worker made of a [`Job`]; the calling thread turns it into store writes.
enum Parsed {
    Unreadable,
    /// Read, but the grammar returned an error.
    Unparsed,
    /// Same bytes as the stored sha.
    Same,
    Rows(String, Vec<Row>),
}

fn parse(job: &Job) -> Parsed {
    let Ok(src) = std::fs::read_to_string(&job.path) else {
        return Parsed::Unreadable;
    };
    let sha = store::hex_sha256(src.as_bytes());
    if job.known.as_deref() == Some(sha.as_str()) {
        return Parsed::Same;
    }
    match outline::tags(&job.path, &src) {
        Ok(hits) => Parsed::Rows(sha, scoped(&hits)),
        Err(_) => Parsed::Unparsed,
    }
}

/// Parses `jobs` on up to one worker per core and hands each result to `write` on this thread
/// in `jobs` order, so the store sees exactly the writes of a sequential run. The channel is
/// bounded, so a slow writer stalls the workers instead of piling rows up in memory. An `Err`
/// from `write` drops the receiver, and each worker stops at its next send.
fn each_parsed(jobs: &[Job], mut write: impl FnMut(&Job, Parsed) -> Result<()>) -> Result<()> {
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(jobs.len());
    let next = AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::sync_channel(workers.max(1) * 2);
    std::thread::scope(|s| {
        for _ in 0..workers {
            let (tx, next) = (tx.clone(), &next);
            s.spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    if tx.send((i, parse(job))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        // Results arrive in finishing order; an early one waits here for its turn.
        let mut early = BTreeMap::new();
        let mut due = 0;
        for (i, parsed) in rx {
            early.insert(i, parsed);
            while let Some(parsed) = early.remove(&due) {
                write(&jobs[due], parsed)?;
                due += 1;
            }
        }
        Ok(())
    })
}

/// Bump when [`scoped`] changes (T35.5).
const INDEX_VERSION: u32 = 1;

/// Hex sha256 of `INDEX_VERSION` and every query string [`outline::config`] compiles —
/// tags **and** locals, because a language whose locals query changed produces different
/// rows. The TypeScript pair was hashed twice, which hid a `LOCALS_QUERY` bump for
/// ts/tsx/js/dart behind an unchanged fingerprint.
fn extractor_fingerprint() -> String {
    let mut bytes = INDEX_VERSION.to_le_bytes().to_vec();
    #[cfg(feature = "lang-c")]
    bytes.extend_from_slice(tree_sitter_c::TAGS_QUERY.as_bytes());
    #[cfg(feature = "lang-dart")]
    {
        bytes.extend_from_slice(tree_sitter_dart::TAGS_QUERY.as_bytes());
        bytes.extend_from_slice(tree_sitter_dart::LOCALS_QUERY.as_bytes());
    }
    #[cfg(feature = "lang-go")]
    bytes.extend_from_slice(tree_sitter_go::TAGS_QUERY.as_bytes());
    #[cfg(feature = "lang-js")]
    {
        bytes.extend_from_slice(tree_sitter_javascript::TAGS_QUERY.as_bytes());
        bytes.extend_from_slice(tree_sitter_javascript::LOCALS_QUERY.as_bytes());
    }
    #[cfg(feature = "lang-python")]
    bytes.extend_from_slice(tree_sitter_python::TAGS_QUERY.as_bytes());
    #[cfg(feature = "lang-rust")]
    {
        bytes.extend_from_slice(tree_sitter_rust::TAGS_QUERY.as_bytes());
        bytes.extend_from_slice(outline::RUST_SCOPED_CALL.as_bytes());
    }
    #[cfg(feature = "lang-ts")]
    {
        bytes.extend_from_slice(tree_sitter_typescript::TAGS_QUERY.as_bytes());
        bytes.extend_from_slice(tree_sitter_typescript::LOCALS_QUERY.as_bytes());
    }
    store::hex_sha256(&bytes)
}

/// Rows for one file, each reference tagged with the innermost definition enclosing it
/// (T8.5). Ties break to the smaller span, so a nested `fn` wins over the `impl` around it;
/// a reference outside every definition gets `""`, which reads as file level.
fn scoped(hits: &[outline::TagHit]) -> Vec<Row> {
    let defs: Vec<(usize, usize, &str)> = hits
        .iter()
        .filter(|h| h.is_def)
        .map(|h| (h.line, h.end_line.max(h.line), h.name.as_str()))
        .collect();
    hits.iter()
        .map(|h| {
            let scope = if h.is_def {
                String::new()
            } else {
                defs.iter()
                    .filter(|(s, e, _)| *s <= h.line && h.line <= *e)
                    .min_by_key(|(s, e, _)| e - s)
                    .map(|(_, _, n)| n.to_string())
                    .unwrap_or_default()
            };
            (
                h.name.clone(),
                h.kind.clone(),
                h.line as i32,
                h.is_def,
                h.end_line as i32,
                scope,
            )
        })
        .collect()
}

/// Index `root` only when it has no rows yet (first tool call in that repo).
pub fn ensure(cx: &Ctx, root: &Path) -> Result<Report> {
    if cx.symbol_count(&canon(root))? > 0 {
        return Ok(Report::default());
    }
    run(cx, root, false)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Fresh DB + archive dir under the temp dir; shared with the `mod.rs` tool tests.
    pub(crate) fn cx(name: &str) -> (crate::plugin::Runtime, PathBuf) {
        crate::testutil::runtime(name)
    }

    #[test]
    fn index_crate_has_main_def_and_registry_ref() {
        let (cx, dir) = cx("crate");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        run(&Ctx::new(&cx), &root, false).unwrap();
        let k = canon(&root);
        assert!(cx.store.has_symbol_def(&k, "main").unwrap(), "main def");
        assert!(
            cx.store.symbol_ref_count(&k, "Registry").unwrap() >= 1,
            "Registry ref"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.3: two repos in one store. Indexing B must not evict A, answer for A, or lose
    /// B's `src/main.rs` when A's same-named file is marked stale.
    #[test]
    fn two_roots_do_not_evict_each_other() {
        let (cx, dir) = cx("roots");
        let (a, b) = (dir.join("a"), dir.join("b"));
        for (root, extra) in [(&a, "alpha"), (&b, "beta")] {
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(
                root.join("src/main.rs"),
                format!("fn main() {{}}\nfn {extra}() {{}}\n"),
            )
            .unwrap();
        }
        run(&Ctx::new(&cx), &a, false).unwrap();
        let (ka, kb) = (canon(&a), canon(&b));
        let a_rows = cx.store.symbol_count(&ka).unwrap();
        run(&Ctx::new(&cx), &b, false).unwrap();
        assert_eq!(
            cx.store.symbol_count(&ka).unwrap(),
            a_rows,
            "indexing b evicted a"
        );
        assert_eq!(
            cx.store.symbol_defs(&ka, "main").unwrap().len(),
            1,
            "symbol(main) under a must not list b's file"
        );
        assert!(cx.store.has_symbol_def(&ka, "alpha").unwrap());
        assert!(
            !cx.store.has_symbol_def(&ka, "beta").unwrap(),
            "b answered for a"
        );
        cx.store
            .mark_symbols_stale(&canon(&a.join("src/main.rs")))
            .unwrap();
        assert!(
            !cx.store.has_symbol_def(&ka, "alpha").unwrap(),
            "a not stale"
        );
        assert!(
            cx.store.has_symbol_def(&kb, "beta").unwrap(),
            "a's stale mark dropped b's src/main.rs"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T35.5: a stale extractor fingerprint drops the root and indexes cold.
    #[test]
    fn reindexes_when_extractor_fingerprint_mismatches() {
        let (cx, dir) = cx("fp");
        for i in 0..3 {
            fs::write(dir.join(format!("f{i}.rs")), format!("fn f{i}() {{}}\n")).unwrap();
        }
        let ctx = Ctx::new(&cx);
        let first = run(&ctx, &dir, false).unwrap();
        assert_eq!(first.read, 3, "cold run reads every file");
        let warm = run(&ctx, &dir, false).unwrap();
        assert!(
            warm.skipped > 0 || warm.read == 0,
            "second run must be warm"
        );
        let k = canon(&dir);
        cx.store.set_extractor_fingerprint(&k, "deadbeef").unwrap();
        let reindex = run(&ctx, &dir, false).unwrap();
        assert_eq!(reindex.read, 3, "mismatch must re-read every file");
        assert_eq!(reindex.indexed, 3, "mismatch must re-index every file");
        assert_eq!(
            run(&ctx, &dir, false).unwrap().read,
            0,
            "third run is warm again"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.4: a warm run over an untouched tree opens no file; rewriting identical bytes
    /// costs one read and no rows. The 3 000-file wall time is a release measurement
    /// (`done.md`); at this size the cold run is one transaction per file and would put
    /// ~90 s of fixture setup into every `just check`.
    #[test]
    fn warm_run_reads_nothing_and_touch_inserts_zero() {
        const FILES: usize = 20;
        let (cx, dir) = cx("stat");
        for i in 0..FILES {
            fs::write(dir.join(format!("f{i}.rs")), format!("fn f{i}() {{}}\n")).unwrap();
        }
        let cold = run(&Ctx::new(&cx), &dir, false).unwrap();
        assert_eq!(cold.read as usize, FILES, "cold run reads every file");
        let warm = run(&Ctx::new(&cx), &dir, false).unwrap();
        assert_eq!(warm.read, 0, "warm run must not open a file");
        assert_eq!(warm.skipped as usize, FILES);
        assert_eq!(warm.inserted, 0);
        fs::write(dir.join("f0.rs"), "fn f0() {}\n").unwrap();
        let touched = run(&Ctx::new(&cx), &dir, false).unwrap();
        assert_eq!(touched.read, 1, "only the touched file is read");
        assert_eq!(touched.inserted, 0, "identical bytes must not re-insert");
        assert_eq!(
            run(&Ctx::new(&cx), &dir, false).unwrap().read,
            0,
            "new stat was recorded"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T21.2: the bar counts the source files the walk reached — one tick per indexed or
    /// skipped file, and none for the noise the walker filtered out.
    #[test]
    fn the_progress_bar_counts_the_files_the_walk_reached() {
        let (cx, dir) = cx("progress");
        for i in 0..7 {
            fs::write(dir.join(format!("f{i}.rs")), format!("fn f{i}() {{}}\n")).unwrap();
        }
        fs::write(dir.join("notes.md"), "not a source file\n").unwrap();
        let pb = indicatif::ProgressBar::hidden();
        let r = run_with(&Ctx::new(&cx), &dir, false, &pb).unwrap();
        assert_eq!(r.indexed, 7);
        assert_eq!(pb.position(), 7, "the bar and the report must agree");
        let _ = fs::remove_dir_all(dir);
    }

    /// T35.2: jobs whose parse time falls with their index, so later workers finish first.
    fn uneven_jobs(dir: &Path) -> Vec<Job> {
        (0..32)
            .map(|i| {
                let rel = format!("f{i:02}.rs");
                let body: String = (0..(32 - i) * 40)
                    .map(|j| format!("fn f{i}_{j}() {{ g(); }}\n"))
                    .collect();
                fs::write(dir.join(&rel), body).unwrap();
                Job {
                    path: dir.join(&rel),
                    rel,
                    stat: (0, 0),
                    known: None,
                }
            })
            .collect()
    }

    /// T35.2: workers finish in any order, but each result reaches the writer in walk
    /// order, so the store gets a sequential run's writes in a sequential run's order.
    #[test]
    fn parsed_files_are_written_in_walk_order() {
        let (_cx, dir) = cx("order");
        let jobs = uneven_jobs(&dir);
        let mut seen = Vec::new();
        each_parsed(&jobs, |job, parsed| {
            assert!(
                matches!(parsed, Parsed::Rows(..)),
                "{} did not parse",
                job.rel
            );
            seen.push(job.rel.clone());
            Ok(())
        })
        .unwrap();
        let want: Vec<&str> = jobs.iter().map(|j| j.rel.as_str()).collect();
        assert_eq!(seen, want);
        let _ = fs::remove_dir_all(dir);
    }

    /// T35.2: a failed write ends the run with its error; the workers see the closed
    /// channel and stop rather than parse the rest or block on a full channel.
    #[test]
    fn a_failed_write_stops_the_workers() {
        let (_cx, dir) = cx("stop");
        let jobs = uneven_jobs(&dir);
        let mut writes = 0;
        let err = each_parsed(&jobs, |_, _| {
            writes += 1;
            anyhow::bail!("disk full")
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "disk full");
        assert_eq!(writes, 1, "no write after the first error");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn second_run_inserts_zero() {
        let (cx, dir) = cx("twice");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        run(&Ctx::new(&cx), &root, false).unwrap();
        let r = run(&Ctx::new(&cx), &root, false).unwrap();
        assert_eq!(r.inserted, 0, "second run must insert 0 rows");
        assert_eq!(r.indexed, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn run_changed_indexes_only_touched_file() {
        const FILES: usize = 20;
        let (cx, dir) = cx("changed");
        for i in 0..FILES {
            fs::write(
                dir.join(format!("f{i}.rs")),
                format!(
                    "fn f{i}() {{}}
"
                ),
            )
            .unwrap();
        }
        run(&Ctx::new(&cx), &dir, false).unwrap();
        let k = canon(&dir);
        let before = cx.store.symbol_count(&k).unwrap();
        fs::write(
            dir.join("f0.rs"),
            "fn f0() {}
fn touched() {}
",
        )
        .unwrap();
        let changed = HashSet::from([dir.join("f0.rs")]);
        let r = run_changed(&Ctx::new(&cx), &dir, &changed).unwrap();
        assert_eq!(r.read, 1, "only the edited path is read");
        assert_eq!(cx.store.symbol_count(&k).unwrap(), before + 1);
        for i in 1..FILES {
            assert!(
                cx.store.has_symbol_def(&k, &format!("f{i}")).unwrap(),
                "f{i} untouched"
            );
        }
        assert!(cx.store.has_symbol_def(&k, "touched").unwrap());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn edit_fixture_reindexes_only_that_file() {
        let (cx, dir) = cx("edit");
        let a = dir.join("a.rs");
        let b = dir.join("b.rs");
        fs::write(&a, "fn alpha() {}\n").unwrap();
        fs::write(&b, "fn beta() {}\n").unwrap();
        run(&Ctx::new(&cx), &dir, false).unwrap();
        fs::write(&a, "fn alpha() {}\nfn gamma() {}\n").unwrap();
        let r = run(&Ctx::new(&cx), &dir, false).unwrap();
        assert_eq!(r.indexed, 1, "only a.rs changed");
        let k = canon(&dir);
        assert!(cx.store.has_symbol_def(&k, "gamma").unwrap());
        assert!(cx.store.has_symbol_def(&k, "beta").unwrap());
        let _ = fs::remove_dir_all(dir);
    }
    /// T8.5: a reference's scope is the definition that encloses it, so `callers` names the
    /// caller rather than a line number. A call outside every definition is file level.
    #[test]
    fn references_carry_their_enclosing_definition() {
        let (cx, dir) = cx("scope");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nstatic S: u8 = top();\n",
        )
        .unwrap();
        run(&Ctx::new(&cx), &dir, false).unwrap();
        let k = canon(&dir);
        let scope = |n: &str| {
            cx.store
                .symbol_ref_groups(&k, n)
                .unwrap()
                .into_iter()
                .map(|(p, s, c, _)| format!("{p}/{s}x{c}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(scope("c"), ["chain.rs/bx1"], "c is called by b");
        assert_eq!(scope("b"), ["chain.rs/ax1"], "b is called by a");
        assert_eq!(
            scope("top"),
            ["chain.rs/x1"],
            "file-level call has no scope"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
