// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Background re-index of the graph store. One writer (D18): a thread in `rtok mcp`.
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{RecursiveMode, Watcher, event::Event};
use rtok_plugin_sdk::Ctx;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const QUIET: Duration = Duration::from_millis(250);
const PENDING_CAP: usize = 1024;

pub fn run(cx: &Ctx, root: &Path, stop: &AtomicBool) {
    run_with(cx, root, stop, &AtomicUsize::new(0), paths);
}

/// The paths an event names; a watcher error names none.
fn paths(e: notify::Result<Event>) -> Vec<PathBuf> {
    e.ok().map(|ev| ev.paths).unwrap_or_default()
}

pub fn run_with<F>(cx: &Ctx, root: &Path, stop: &AtomicBool, runs: &AtomicUsize, events: F)
where
    F: FnMut(notify::Result<Event>) -> Vec<PathBuf>,
{
    notify_loop(cx, root, stop, runs, events);
}

fn notify_loop<F>(cx: &Ctx, root: &Path, stop: &AtomicBool, runs: &AtomicUsize, events: F)
where
    F: FnMut(notify::Result<Event>) -> Vec<PathBuf>,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            let msg = format!("notify watcher: {e}");
            eprintln!("watch: {msg}");
            cx.log("error", "graph", "watch", &msg);
            return;
        }
    };
    if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
        let msg = format!("watch {}: {e}", root.display());
        eprintln!("watch: {msg}");
        cx.log("error", "graph", "watch", &msg);
        return;
    }
    pump(cx, root, stop, runs, &rx, events);
}

/// The debounce loop, apart from the OS watcher so tests can feed it events directly. Ends on
/// `stop` or when the sender is gone (the watcher died), never spins.
fn pump<F>(
    cx: &Ctx,
    root: &Path,
    stop: &AtomicBool,
    runs: &AtomicUsize,
    rx: &std::sync::mpsc::Receiver<notify::Result<Event>>,
    mut events: F,
) where
    F: FnMut(notify::Result<Event>) -> Vec<PathBuf>,
{
    let ig = gitignore(root);
    let matcher = crate::plugins::graph::walk::Matcher::new(
        root,
        &cx.plugin_config::<crate::config::Graph>("graph"),
    );
    let mut last = Instant::now();
    let mut pending = HashSet::new();
    let mut rescan = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ev) => {
                if ev.as_ref().ok().is_some_and(|e| e.need_rescan()) {
                    rescan = true;
                    pending.clear();
                }
                let mut touched = false;
                for p in events(ev) {
                    if absorb_event(p, &ig, &matcher, &mut pending, &mut rescan) {
                        touched = true;
                    }
                }
                if touched {
                    last = Instant::now();
                }
                if pending.len() > PENDING_CAP {
                    rescan = true;
                    pending.clear();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        sync_watch_pending(cx, root, &pending);
        settle(cx, root, runs, &mut last, &mut pending, &mut rescan);
    }
}

// PERF(T35.4) where: here, once per quiet burst of edits. What: keep the event paths and index
// only those; walk everything only on a rescan or overflow event. Why: one changed file
// re-walks and stats the whole root — 50 ms for this 127-file repo in debug (2026-09-10),
// growing with the tree, against the P8d promise of an edit visible within 1 s.
fn settle(
    cx: &Ctx,
    root: &Path,
    runs: &AtomicUsize,
    last: &mut Instant,
    pending: &mut HashSet<PathBuf>,
    rescan: &mut bool,
) {
    if last.elapsed() < QUIET || (!*rescan && pending.is_empty()) {
        return;
    }
    if *rescan {
        let _ = super::index::run(cx, root, false);
    } else {
        let _ = super::index::run_changed(cx, root, pending);
    }
    runs.fetch_add(1, Ordering::Relaxed);
    *last = Instant::now();
    sync_watch_pending(cx, root, pending);
    pending.clear();
    cx.publish_graph_watch_pending(&[]);
    *rescan = false;
}

/// The root's `.gitignore`, so watcher-delivered paths obey it like the walk does: without
/// it `target/debug/build/*/out/*.rs` was indexed as soon as a build wrote it.
// ponytail: root `.gitignore` only; add nested ones and `.git/info/exclude` if they show up.
fn gitignore(root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    b.add(root.join(".gitignore"));
    b.build().unwrap_or_else(|_| Gitignore::empty())
}

fn git_path(p: &Path) -> bool {
    p.components().any(|c| c.as_os_str() == ".git")
}

fn relevant(p: &Path, matcher: &crate::plugins::graph::walk::Matcher) -> bool {
    matcher.indexable(p) && !git_path(p)
}

/// Source files go to `pending`. A directory (still there, or just removed with no extension)
/// forces a rescan so vanished children drop (T36.17). Existing unsupported files — the store,
/// WAL, logs, docs — must not: they used to clear `pending` and reset the quiet window on every
/// SQLite write when the DB lived under the watch root (Linux inotify reports each WAL write).
fn absorb_event(
    p: PathBuf,
    ig: &Gitignore,
    matcher: &crate::plugins::graph::walk::Matcher,
    pending: &mut HashSet<PathBuf>,
    rescan: &mut bool,
) -> bool {
    if git_path(&p) || ig.matched_path_or_any_parents(&p, p.is_dir()).is_ignore() {
        return false;
    }
    if relevant(&p, matcher) {
        pending.insert(p);
        return true;
    }
    let directory = p.is_dir() || (!p.exists() && p.extension().is_none());
    if directory {
        *rescan = true;
        pending.clear();
        true
    } else {
        false
    }
}

fn sync_watch_pending(cx: &Ctx, root: &Path, pending: &HashSet<PathBuf>) {
    let mut out: Vec<String> = pending
        .iter()
        .filter_map(|p| {
            pathdiff::diff_paths(p, root).map(|r| r.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    out.sort();
    cx.publish_graph_watch_pending(&out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Graph;
    use crate::plugins::graph::index::tests::cx as mk;
    use rstest::rstest;
    use std::fs;
    use std::time::Duration;

    fn test_matcher(dir: &Path) -> crate::plugins::graph::walk::Matcher {
        crate::plugins::graph::walk::Matcher::new(dir, &Graph::default())
    }

    fn arm(cx: &mut crate::plugin::Runtime, watch: &str) {
        cx.config.plugins.graph.auto_index = false;
        cx.config.plugins.graph.watch = watch.into();
    }

    /// Generous cap: a loaded machine can make FSEvents deliver in over a
    /// second, so a hard 1 s deadline flakes on real re-indexing, not on a
    /// broken watcher. Reaching this cap means the watcher never re-indexed.
    const REINDEX_CAP: Duration = Duration::from_secs(10);
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    fn wait_contains(cx: &Ctx, dir: &Path, name: &str, needle: &str) -> bool {
        let deadline = Instant::now() + REINDEX_CAP;
        loop {
            if super::super::symbol(cx, dir, name)
                .unwrap()
                .contains(needle)
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// `found` came from a `wait_contains` poll already run to the cap; this
    /// turns a miss into a panic naming what the store actually held, not a
    /// stopwatch verdict.
    fn assert_reindexed(found: bool, cx: &Ctx, dir: &Path, name: &str, needle: &str) {
        if found {
            return;
        }
        let actual = super::super::symbol(cx, dir, name).unwrap();
        panic!(
            "watcher did not re-index within {REINDEX_CAP:?}: expected {name} to contain \
             {needle:?}, store holds {actual:?}"
        );
    }

    /// Prime the stream so the timed write after it measures the watcher, not setup. A write
    /// made before the FSEvents stream is live is never delivered, and nothing signals "live":
    /// waiting on a single probe write sat out the whole `REINDEX_CAP` whenever it was lost.
    /// Rewrite the probe every two quiet windows until the watcher indexes it, so priming costs
    /// the stream's start-up and no more.
    fn warm_watcher(cx: &Ctx, dir: &Path) {
        let probe = dir.join("warm.rs");
        let indexed = || {
            super::super::symbol(cx, dir, "warm_probe")
                .unwrap()
                .contains("warm.rs:1")
        };
        let cap = Instant::now() + REINDEX_CAP;
        for n in 0.. {
            fs::write(&probe, format!("pub fn warm_probe() {{}}\n// {n}\n")).unwrap();
            let retry = Instant::now() + 2 * QUIET;
            while Instant::now() < retry && !indexed() {
                std::thread::sleep(POLL_INTERVAL);
            }
            if indexed() || Instant::now() >= cap {
                break;
            }
        }
        let _ = fs::remove_file(&probe);
        let gone = wait_contains(cx, dir, "warm_probe", "no definition of warm_probe");
        assert_reindexed(gone, cx, dir, "warm_probe", "no definition of warm_probe");
    }

    /// The one end-to-end check through a real FSEvents stream: a new file is indexed, the call
    /// itself reads nothing, a deleted file's rows go. Slow by nature: priming the stream plus
    /// three quiet windows (probe, write, delete), each a real OS round trip. The loop's logic
    /// is covered without the OS by the `pump` tests below — add cases there, not here.
    #[test]
    fn watcher_reindexes_new_file_while_calls_read_nothing() {
        let (mut cx, dir) = mk("watch-new");
        arm(&mut cx, "notify");
        // The store lives in `dir`; watch a sibling tree so WAL writes are not inotify events.
        let root = dir.join("proj");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&Ctx::new(&cx), &root, false).unwrap();
        let stop = AtomicBool::new(false);
        let (found, read, gone) = std::thread::scope(|s| {
            s.spawn(|| run(&Ctx::new(&cx), &root, &stop));
            std::thread::sleep(Duration::from_millis(80));
            warm_watcher(&Ctx::new(&cx), &root);
            fs::write(root.join("watched.rs"), "pub fn watched() {}\n").unwrap();
            let found = wait_contains(&Ctx::new(&cx), &root, "watched", "watched.rs:1");
            let read = super::super::index_for(&Ctx::new(&cx), &root).unwrap().read;
            // Drain create-side CLOSE_WRITE/ATTRIB so they do not share a settle with the delete.
            std::thread::sleep(QUIET + POLL_INTERVAL);
            let _ = fs::remove_file(root.join("watched.rs"));
            let gone = wait_contains(&Ctx::new(&cx), &root, "watched", "no definition of watched");
            stop.store(true, Ordering::Relaxed);
            (found, read, gone)
        });
        assert_reindexed(found, &Ctx::new(&cx), &root, "watched", "watched.rs:1");
        assert_eq!(read, 0, "the call itself must open no file");
        assert!(
            gone,
            "deleted file kept its rows: store still contains a definition of watched"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// 200 real writes coalesce into ≤ 3 runs. ~0.6 s: the writes plus one quiet window, no
    /// priming. Only a bound — FSEvents batches as it likes, and 0 also passes when the stream
    /// goes live after the writes; `irrelevant_events_never_run_and_a_burst_runs_once` pins the
    /// exact count.
    #[test]
    fn bursts_coalesce_to_few_runs() {
        let (mut cx, dir) = mk("watch-burst");
        arm(&mut cx, "notify");
        fs::write(dir.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&Ctx::new(&cx), &dir, false).unwrap();
        let stop = AtomicBool::new(false);
        let runs = AtomicUsize::new(0);
        let n = std::thread::scope(|s| {
            s.spawn(|| {
                run_with(&Ctx::new(&cx), &dir, &stop, &runs, |_| {
                    vec![dir.join("burst.rs")]
                });
            });
            for i in 0..200 {
                fs::write(dir.join("burst.rs"), format!("pub fn f{i}() {{}}\n")).unwrap();
            }
            std::thread::sleep(QUIET + Duration::from_millis(80));
            stop.store(true, Ordering::Relaxed);
            runs.load(Ordering::Relaxed)
        });
        assert!(n <= 3, "200 writes coalesced to {n} runs, want ≤ 3");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn git_and_unsupported_paths_are_irrelevant() {
        let m = test_matcher(Path::new("."));
        assert!(!relevant(Path::new(".git/HEAD"), &m));
        assert!(!relevant(Path::new("foo/bar.md"), &m));
        assert!(relevant(Path::new("src/lib.rs"), &m));
    }

    /// A live store/log/doc file must not swallow a pending source path (the Linux flake:
    /// WAL writes cleared `pending` and reset quiet, so a delete never settled).
    #[test]
    fn unsupported_file_does_not_clear_pending_or_rescan() {
        let (_rt, dir) = mk("watch-db-noise");
        fs::write(dir.join("rtok.db"), "x").unwrap();
        let mut pending = HashSet::from([dir.join("watched.rs")]);
        let mut rescan = false;
        let ig = Gitignore::empty();
        let matcher = test_matcher(&dir);
        assert!(!absorb_event(
            dir.join("rtok.db"),
            &ig,
            &matcher,
            &mut pending,
            &mut rescan
        ));
        assert!(
            !rescan && pending.len() == 1,
            "existing unsupported file triggered a rescan"
        );
        let gone_dir = dir.join("src/module");
        assert!(absorb_event(
            gone_dir,
            &ig,
            &matcher,
            &mut pending,
            &mut rescan
        ));
        assert!(
            rescan && pending.is_empty(),
            "removed directory must rescan"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// A path the root `.gitignore` covers never reaches `pending`, even under a missing dir.
    #[test]
    fn gitignored_event_is_dropped() {
        let (_rt, dir) = mk("watch-gitignore");
        fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        let ig = gitignore(&dir);
        let matcher = test_matcher(&dir);
        let mut pending = HashSet::new();
        let mut rescan = false;
        let built = dir.join("target/debug/build/x/out/y.rs");
        assert!(!absorb_event(
            built,
            &ig,
            &matcher,
            &mut pending,
            &mut rescan
        ));
        assert!(pending.is_empty() && !rescan);
        assert!(absorb_event(
            dir.join("src/lib.rs"),
            &ig,
            &matcher,
            &mut pending,
            &mut rescan
        ));
        assert_eq!(pending.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// What FSEvents would deliver for `p`, without the OS in the loop.
    fn ev(p: PathBuf) -> notify::Result<Event> {
        Ok(Event::new(notify::EventKind::Any).add_path(p))
    }

    /// Feeds `sent` to `pump` and lets it run for one quiet window past the last run it was
    /// waiting for, so a surplus run shows up too. Returns the run count. Costs QUIET plus a
    /// few 50 ms ticks; there is no stream to start, so no ~1 s first-event delay to prime.
    fn pump_events(
        rt: &crate::plugin::Runtime,
        dir: &Path,
        sent: Vec<PathBuf>,
        want: usize,
    ) -> usize {
        let (stop, runs) = (AtomicBool::new(false), AtomicUsize::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        for p in sent {
            tx.send(ev(p)).unwrap();
        }
        let (stop_r, runs_r) = (&stop, &runs);
        std::thread::scope(|s| {
            s.spawn(move || pump(&Ctx::new(rt), dir, stop_r, runs_r, &rx, paths));
            let cap = Instant::now() + REINDEX_CAP;
            while runs.load(Ordering::Relaxed) < want && Instant::now() < cap {
                std::thread::sleep(POLL_INTERVAL);
            }
            // One more quiet window: a surplus run would land in it.
            std::thread::sleep(QUIET + 4 * POLL_INTERVAL);
            stop.store(true, Ordering::Relaxed);
        });
        drop(tx);
        runs.load(Ordering::Relaxed)
    }

    /// `settle` alone: inside QUIET nothing runs, after it exactly one index run clears `pending`,
    /// a clean state never runs. No thread and no sleep — time passes by moving `last` back —
    /// so the cost is one one-file index.
    #[test]
    fn settle_runs_once_after_quiet_and_never_when_clean() {
        let (rt, dir) = mk("watch-settle");
        fs::write(dir.join("a.rs"), "pub fn settled() {}\n").unwrap();
        let cx = Ctx::new(&rt);
        let key = super::super::index::canon(&dir);
        let runs = AtomicUsize::new(0);
        let (mut last, mut pending, mut rescan) =
            (Instant::now(), HashSet::from([dir.join("a.rs")]), false);
        settle(&cx, &dir, &runs, &mut last, &mut pending, &mut rescan);
        assert_eq!(
            (runs.load(Ordering::Relaxed), pending.is_empty(), rescan),
            (0, false, false),
            "ran inside QUIET"
        );
        last = Instant::now() - QUIET;
        settle(&cx, &dir, &runs, &mut last, &mut pending, &mut rescan);
        assert_eq!(
            (runs.load(Ordering::Relaxed), pending.is_empty(), rescan),
            (1, true, false)
        );
        assert_eq!(cx.symbol_defs(&key, "settled").unwrap().len(), 1);
        last = Instant::now() - QUIET;
        settle(&cx, &dir, &runs, &mut last, &mut pending, &mut rescan);
        assert_eq!(runs.load(Ordering::Relaxed), 1, "a clean state ran again");
        let _ = fs::remove_dir_all(dir);
    }

    /// `.git/` churn never wakes the indexer; a burst of relevant events
    /// runs it exactly once. Exact, where `bursts_coalesce_to_few_runs` can only bound the count
    /// (≤ 3), because FSEvents decides how the 200 writes are batched.
    #[test]
    fn irrelevant_events_never_run_and_a_burst_runs_once() {
        let (rt, dir) = mk("watch-noise");
        fs::write(dir.join("a.rs"), "pub fn seed() {}\n").unwrap();
        let noise = [".git/index", ".git/HEAD"].map(|p| dir.join(p));
        assert_eq!(pump_events(&rt, &dir, noise.to_vec(), 0), 0);
        let burst = (0..50).map(|_| dir.join("a.rs")).collect();
        assert_eq!(pump_events(&rt, &dir, burst, 1), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// T36.17: `rm -rf src/<dir>` names the directory, not each `.rs` file; without a rescan
    /// trigger the deleted files' rows linger and `symbol` still names them.
    #[rstest]
    fn removed_directory_drops_index_rows() {
        let (mut rt, dir) = mk("watch-rmdir");
        arm(&mut rt, "notify");
        let module = dir.join("src/module");
        fs::create_dir_all(&module).unwrap();
        fs::write(module.join("gone.rs"), "pub fn vanished() {}\n").unwrap();
        let cx = Ctx::new(&rt);
        super::super::index::run(&cx, &dir, false).unwrap();
        let key = super::super::index::canon(&dir);
        assert_eq!(cx.symbol_defs(&key, "vanished").unwrap().len(), 1);
        fs::remove_dir_all(&module).unwrap();
        assert_eq!(
            pump_events(&rt, &dir, vec![module], 1),
            1,
            "directory removal must re-index once"
        );
        assert!(
            cx.symbol_defs(&key, "vanished").unwrap().is_empty(),
            "deleted directory kept its rows"
        );
        assert_eq!(
            super::super::index_for(&cx, &dir).unwrap().read,
            0,
            "the call that follows must not walk"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// An edit and a rename each re-index once; the rename drops the old path's rows (the
    /// `mark_symbols_stale` on the old path). Two quiet windows, well under a second.
    #[test]
    fn edit_and_rename_events_reindex_once_each() {
        let (rt, dir) = mk("watch-pump");
        fs::write(dir.join("a.rs"), "pub fn old_name() {}\n").unwrap();
        let cx = Ctx::new(&rt);
        super::super::index::run(&cx, &dir, false).unwrap();
        let key = super::super::index::canon(&dir);
        let defs = |n: &str| -> Vec<String> {
            let rows = cx.symbol_defs(&key, n).unwrap();
            rows.into_iter().map(|(p, ..)| p).collect()
        };
        // A different length, so the (mtime, size) gate cannot call the edit unchanged.
        fs::write(dir.join("a.rs"), "pub fn new_name_longer() {}\n").unwrap();
        assert_eq!(pump_events(&rt, &dir, vec![dir.join("a.rs")], 1), 1);
        assert_eq!(defs("new_name_longer"), ["a.rs"]);
        assert!(defs("old_name").is_empty(), "edit kept the old definition");
        fs::rename(dir.join("a.rs"), dir.join("b.rs")).unwrap();
        let moved = vec![dir.join("a.rs"), dir.join("b.rs")];
        assert_eq!(pump_events(&rt, &dir, moved, 1), 1);
        assert_eq!(defs("new_name_longer"), ["b.rs"], "rename kept a.rs rows");
        let _ = fs::remove_dir_all(dir);
    }

    /// The loop ends on `stop` while its channel is still open, and on a dead channel (the
    /// watcher dropped) without `stop`: the MCP thread outlives neither. A regression hangs.
    #[test]
    fn pump_ends_on_stop_or_on_a_dead_channel() {
        let (rt, dir) = mk("watch-end");
        let cx = Ctx::new(&rt);
        let runs = AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        pump(&cx, &dir, &AtomicBool::new(true), &runs, &rx, paths);
        drop(tx);
        pump(&cx, &dir, &AtomicBool::new(false), &runs, &rx, paths);
        assert_eq!(runs.load(Ordering::Relaxed), 0);
        let _ = fs::remove_dir_all(dir);
    }
}
