//! T20.1: `rtok demon` keeps a service up, and every verb tells the truth about it.
//!
//! Check: the supervised service here is `rtok mcp` with no stdin — it reaches EOF and exits at
//! once, which is the crash loop a supervisor exists for. Liveness is asked of the kernel
//! (`kill -0`), never read out of the state file, so a stale file cannot pass a test.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-t201-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // `mcp` restarts in tens of milliseconds here; the shipped backoff is seconds.
    fs::write(
        dir.join("config.toml"),
        "[demon]\nservices = [\"mcp\"]\nbackoff_ms = 50\nmax_backoff_ms = 100\npoll_ms = 50\n",
    )
    .unwrap();
    dir
}

fn rtok(args: &[&str], home: &Path) -> String {
    let out = Command::new(bin())
        .args(args)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rtok {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn state(home: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(home.join("demon/mcp.json")).ok()?).ok()
}

/// The kernel's answer, not the state file's.
fn alive(pid: i64) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Wait for the supervisor to have restarted the service at least `n` times.
fn wait_restarts(home: &Path, n: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(v) = state(home)
            && v["restarts"].as_u64().unwrap_or(0) >= n
        {
            return v;
        }
        assert!(Instant::now() < deadline, "no {n} restarts within 10s");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_service_that_exits_comes_back_and_stop_takes_the_whole_tree_down() {
    let h = home("cycle");
    rtok(&["demon", "start"], &h); // no name: `[demon] services`
    let st = wait_restarts(&h, 2);
    let sup = st["supervisor"].as_i64().unwrap();
    assert!(alive(sup), "supervisor died");
    assert!(rtok(&["demon", "status"], &h).contains("running"), "status");

    rtok(&["demon", "stop"], &h);
    assert!(state(&h).is_none(), "stop left the state file behind");
    assert!(!alive(sup), "stop left the supervisor running");
    // The marker outlives the supervisor, so nothing can resurrect the service behind our back.
    assert!(h.join("demon/mcp.stop").exists());
    std::thread::sleep(Duration::from_millis(300));
    assert!(state(&h).is_none(), "something restarted after stop");
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn status_asks_the_kernel_rather_than_believing_the_state_file() {
    let h = home("truth");
    rtok(&["demon", "start", "mcp"], &h);
    let st = wait_restarts(&h, 1);
    let sup = st["supervisor"].as_i64().unwrap();
    // Kill it the way a machine would — the state file stays, saying "supervisor <pid>".
    Command::new("kill")
        .args(["-9", &sup.to_string()])
        .output()
        .unwrap();
    for _ in 0..40 {
        if !alive(sup) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        state(&h).is_some(),
        "the stale file is the point of the test"
    );
    let out = rtok(&["demon", "status", "mcp"], &h);
    assert!(out.contains("stopped"), "{out}");
    assert!(!out.contains("running"), "{out}");
    rtok(&["demon", "kill", "mcp"], &h);
    assert!(state(&h).is_none(), "kill left the state file behind");
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn a_second_start_is_refused_and_list_names_every_service() {
    let h = home("once");
    rtok(&["demon", "start", "mcp"], &h);
    let first = wait_restarts(&h, 1)["supervisor"].as_i64().unwrap();
    let out = rtok(&["demon", "start", "mcp"], &h);
    assert!(out.contains("already running"), "{out}");
    assert_eq!(
        state(&h).unwrap()["supervisor"].as_i64().unwrap(),
        first,
        "a second supervisor took the service over"
    );

    let list = rtok(&["demon", "list"], &h);
    for s in ["proxy", "mcp", "web"] {
        assert!(list.contains(s), "list is missing {s}:\n{list}");
    }
    // `proxy` and `dashboard` were never started, so they must read as stopped, not as absent.
    assert_eq!(list.matches("stopped").count(), 2, "{list}");

    let bad = Command::new(bin())
        .args(["demon", "start", "rm -rf /"])
        .env("RTOK_HOME", &h)
        .env("HOME", &h)
        .output()
        .unwrap();
    assert!(!bad.status.success(), "an arbitrary command was accepted");
    // The refusal is clap's, generated from the `Service` ValueEnum, and it names the choices.
    let err = String::from_utf8_lossy(&bad.stderr);
    assert!(err.contains("invalid value"), "{err}");
    assert!(err.contains("proxy") && err.contains("web"), "{err}");

    rtok(&["demon", "stop", "mcp"], &h);
    let _ = fs::remove_dir_all(&h);
}
