//! Background re-index of the graph store. One writer (D18): a thread in `rtok mcp`.
#![allow(unexpected_cfgs)]
use crate::plugin::Ctx;
use notify::{RecursiveMode, Watcher, event::Event};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const QUIET: Duration = Duration::from_millis(250);

pub fn run(cx: &Ctx, root: &Path, stop: &AtomicBool) {
    run_with(
        cx,
        root,
        stop,
        &AtomicUsize::new(0),
        |e: notify::Result<Event>| e.ok().map(|ev| ev.paths).unwrap_or_default(),
    );
}

pub fn run_with<F>(cx: &Ctx, root: &Path, stop: &AtomicBool, runs: &AtomicUsize, events: F)
where
    F: FnMut(notify::Result<Event>) -> Vec<PathBuf>,
{
    if cx.config.plugins.graph.watch == "watchman" {
        if let Err(err) = try_watchman(cx, root, stop, runs) {
            eprintln!("watchman: {err} falling back to notify");
            notify_loop(cx, root, stop, runs, events);
        }
        return;
    }
    notify_loop(cx, root, stop, runs, events);
}

fn try_watchman(
    cx: &Ctx,
    root: &Path,
    stop: &AtomicBool,
    runs: &AtomicUsize,
) -> Result<(), String> {
    #[cfg(feature = "graph-watchman")]
    {
        watchman_connect(cx, root, stop, runs)
    }
    #[cfg(not(feature = "graph-watchman"))]
    {
        let _ = (cx, root, stop, runs);
        Err("no socket".into())
    }
}

#[cfg(feature = "graph-watchman")]
fn watchman_connect(
    cx: &Ctx,
    root: &Path,
    stop: &AtomicBool,
    runs: &AtomicUsize,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(watchman_loop(cx, root, stop, runs))
}

#[cfg(feature = "graph-watchman")]
async fn watchman_loop(
    cx: &Ctx,
    root: &Path,
    stop: &AtomicBool,
    runs: &AtomicUsize,
) -> Result<(), String> {
    use watchman_client::{SubscriptionData, prelude::*};
    let client = Connector::new()
        .connect()
        .await
        .map_err(|e| e.to_string())?;
    let canon = CanonicalPath::canonicalize(root).map_err(|e| e.to_string())?;
    let resolved = client
        .resolve_root(canon)
        .await
        .map_err(|e| e.to_string())?;
    let (mut sub, _) = client
        .subscribe::<NameOnly>(
            &resolved,
            SubscribeRequest {
                expression: Some(Expr::Suffix(suffixes())),
                fields: vec!["name"],
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut last = Instant::now();
    let mut dirty = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            item = sub.next() => {
                match item {
                    Err(e) => return Err(e.to_string()),
                    Ok(SubscriptionData::Canceled) => break,
                    Ok(SubscriptionData::FilesChanged(payload)) => {
                        for f in payload.files.unwrap_or_default() {
                            if relevant(&root.join(f.name.as_path())) {
                                dirty = true;
                                last = Instant::now();
                            }
                        }
                    }
                    Ok(_) => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
        settle(cx, root, runs, &mut last, &mut dirty);
    }
    Ok(())
}

#[cfg(feature = "graph-watchman")]
fn suffixes() -> Vec<PathBuf> {
    let mut s = Vec::new();
    #[cfg(feature = "lang-rust")]
    s.push(PathBuf::from("rs"));
    #[cfg(feature = "lang-ts")]
    {
        s.push(PathBuf::from("ts"));
        s.push(PathBuf::from("tsx"));
    }
    #[cfg(feature = "lang-js")]
    {
        s.push(PathBuf::from("js"));
        s.push(PathBuf::from("mjs"));
        s.push(PathBuf::from("cjs"));
    }
    #[cfg(feature = "lang-python")]
    s.push(PathBuf::from("py"));
    #[cfg(feature = "lang-dart")]
    s.push(PathBuf::from("dart"));
    #[cfg(feature = "lang-c")]
    {
        s.push(PathBuf::from("c"));
        s.push(PathBuf::from("h"));
    }
    #[cfg(feature = "lang-go")]
    s.push(PathBuf::from("go"));
    s
}

fn notify_loop<F>(cx: &Ctx, root: &Path, stop: &AtomicBool, runs: &AtomicUsize, mut events: F)
where
    F: FnMut(notify::Result<Event>) -> Vec<PathBuf>,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("watch: {e}");
            return;
        }
    };
    if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
        eprintln!("watch: {e}");
        return;
    }
    let mut last = Instant::now();
    let mut dirty = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ev) => {
                if events(ev).into_iter().any(|p| relevant(&p)) {
                    dirty = true;
                    last = Instant::now();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        settle(cx, root, runs, &mut last, &mut dirty);
    }
}

fn settle(cx: &Ctx, root: &Path, runs: &AtomicUsize, last: &mut Instant, dirty: &mut bool) {
    if *dirty && last.elapsed() >= QUIET {
        let _ = super::index::run(cx, root, false);
        runs.fetch_add(1, Ordering::Relaxed);
        *last = Instant::now();
        *dirty = false;
    }
}

fn relevant(p: &Path) -> bool {
    crate::plugins::read::outline::supported(p) && !p.components().any(|c| c.as_os_str() == ".git")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::graph::index::tests::cx as mk;
    use std::fs;
    use std::time::Duration;

    fn arm(cx: &mut Ctx, watch: &str) {
        cx.config.plugins.graph.auto_index = false;
        cx.config.plugins.graph.watch = watch.into();
    }

    fn wait_contains(cx: &Ctx, dir: &Path, name: &str, needle: &str) -> bool {
        for _ in 0..25 {
            std::thread::sleep(Duration::from_millis(50));
            if super::super::symbol(cx, dir, name)
                .unwrap()
                .contains(needle)
            {
                return true;
            }
        }
        false
    }

    /// FSEvents delivers the first event on a fresh stream late (~1 s on this
    /// machine); later events arrive in microseconds. Prime the stream with a
    /// probe file so the timed write below measures the watcher, not setup.
    fn warm_watcher(cx: &Ctx, dir: &Path) {
        fs::write(dir.join("warm.rs"), "pub fn warm_probe() {}\n").unwrap();
        let _ = wait_contains(cx, dir, "warm_probe", "warm.rs:1");
        let _ = fs::remove_file(dir.join("warm.rs"));
        let _ = wait_contains(cx, dir, "warm_probe", "no definition of warm_probe");
    }

    #[test]
    fn watcher_reindexes_new_file_while_calls_read_nothing() {
        let (mut cx, dir) = mk("watch-new");
        arm(&mut cx, "notify");
        fs::write(dir.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&cx, &dir, false).unwrap();
        let stop = AtomicBool::new(false);
        let (found, within_1s, read, gone) = std::thread::scope(|s| {
            s.spawn(|| run(&cx, &dir, &stop));
            std::thread::sleep(Duration::from_millis(80));
            warm_watcher(&cx, &dir);
            let t0 = Instant::now();
            fs::write(dir.join("watched.rs"), "pub fn watched() {}\n").unwrap();
            let found = wait_contains(&cx, &dir, "watched", "watched.rs:1");
            let within_1s = t0.elapsed() <= Duration::from_secs(1);
            let read = super::super::index_for(&cx, &dir).unwrap().read;
            let _ = fs::remove_file(dir.join("watched.rs"));
            let gone = wait_contains(&cx, &dir, "watched", "no definition of watched");
            stop.store(true, Ordering::Relaxed);
            (found, within_1s, read, gone)
        });
        assert!(found && within_1s, "watcher did not re-index within 1 s");
        assert_eq!(read, 0, "the call itself must open no file");
        assert!(gone, "deleted file kept its rows");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bursts_coalesce_to_few_runs() {
        let (mut cx, dir) = mk("watch-burst");
        arm(&mut cx, "notify");
        fs::write(dir.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&cx, &dir, false).unwrap();
        let stop = AtomicBool::new(false);
        let runs = AtomicUsize::new(0);
        let n = std::thread::scope(|s| {
            s.spawn(|| {
                run_with(&cx, &dir, &stop, &runs, |_| vec![dir.join("burst.rs")]);
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
        assert!(!relevant(Path::new(".git/HEAD")));
        assert!(!relevant(Path::new("foo/bar.md")));
        assert!(relevant(Path::new("src/lib.rs")));
    }

    /// T8.17 Gate P8d (1) under `watchman`: the daemon's edit is visible in
    /// `symbol` within 1 s while the call itself reads nothing. Skipped
    /// without the feature or without the daemon on PATH.
    #[test]
    fn watchman_sees_daemon_edit_within_1s_reading_nothing() {
        if option_env!("CARGO_FEATURE_GRAPH_WATCHMAN").is_none() {
            return;
        }
        if std::process::Command::new("/opt/homebrew/bin/watchman")
            .arg("version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            return;
        }
        let (mut cx, dir) = mk("watch-wman");
        arm(&mut cx, "watchman");
        fs::write(dir.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&cx, &dir, false).unwrap();
        let stop = AtomicBool::new(false);
        let (found, within_1s, read) = std::thread::scope(|s| {
            s.spawn(|| run(&cx, &dir, &stop));
            // The daemon connect + subscribe happens off any timer: wait for
            // the root to register before the timed write measures delivery.
            let root = dir.canonicalize().unwrap();
            let mut registered = false;
            for _ in 0..50 {
                let list = std::process::Command::new("/opt/homebrew/bin/watchman")
                    .arg("watch-list")
                    .output()
                    .expect("watch-list");
                if String::from_utf8_lossy(&list.stdout).contains(&root.display().to_string()) {
                    registered = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert!(registered, "watchman never listed {}", root.display());
            let t0 = Instant::now();
            fs::write(dir.join("watched.rs"), "pub fn watched() {}\n").unwrap();
            let found = wait_contains(&cx, &dir, "watched", "watched.rs:1");
            let within_1s = t0.elapsed() <= Duration::from_secs(1);
            let read = super::super::index_for(&cx, &dir).unwrap().read;
            stop.store(true, Ordering::Relaxed);
            (found, within_1s, read)
        });
        let _ = std::process::Command::new("/opt/homebrew/bin/watchman")
            .args([
                "watch-del",
                &dir.canonicalize().unwrap().display().to_string(),
            ])
            .output();
        assert!(found && within_1s, "watchman edit not visible within 1 s");
        assert_eq!(read, 0, "the call itself must open no file");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn watchman_without_socket_falls_back_to_notify() {
        let (mut cx, dir) = mk("watch-fb");
        arm(&mut cx, "watchman");
        fs::write(dir.join("lib.rs"), "pub fn seed() {}\n").unwrap();
        super::super::index::run(&cx, &dir, false).unwrap();
        let stop = AtomicBool::new(false);
        let found = std::thread::scope(|s| {
            s.spawn(|| run(&cx, &dir, &stop));
            std::thread::sleep(Duration::from_millis(80));
            warm_watcher(&cx, &dir);
            fs::write(dir.join("watched.rs"), "pub fn watched() {}\n").unwrap();
            let found = wait_contains(&cx, &dir, "watched", "watched.rs:1");
            stop.store(true, Ordering::Relaxed);
            found
        });
        assert!(found, "fallback notify did not re-index");
        let _ = fs::remove_dir_all(dir);
    }
}
