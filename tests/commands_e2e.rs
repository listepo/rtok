//! T38.1: e2e for the commands without direct coverage — `hook`, `run`, `expand`,
//! `plugins`, `config`, `bench --dry-run`, `doctor`, `stats`.
//!
//! Check: one case per command through the binary with an isolated HOME.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-t381-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn rtok(args: &[&str], home: &Path) -> (String, String, i32) {
    let out = Command::new(bin())
        .args(args)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

fn ok(args: &[&str], home: &Path) -> String {
    let (stdout, stderr, code) = rtok(args, home);
    assert_eq!(code, 0, "rtok {args:?} exit {code}: {stderr}");
    stdout
}

fn hook(event: &str, fixture: &[u8], home: &Path) -> String {
    let mut child = Command::new(bin())
        .args(["hook", event])
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(fixture).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "hook {event} must exit 0");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn hook_pre_tool_bash_exits_0_with_json() {
    let home = tmp("pre");
    let stdout = hook(
        "PreToolUse",
        include_bytes!("fixtures/hooks/pre_tool_bash.json"),
        &home,
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_ok_and(|v| v.is_object()),
        "hook output is a JSON object: {stdout}"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn hook_session_start_exits_0_with_json() {
    let home = tmp("start");
    let stdout = hook(
        "SessionStart",
        include_bytes!("fixtures/hooks/session_start.json"),
        &home,
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_ok_and(|v| v.is_object()),
        "hook output is a JSON object: {stdout}"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn run_echo_prints_its_output() {
    let home = tmp("run");
    let out = ok(&["run", "echo", "hello-e2e"], &home);
    assert!(out.contains("hello-e2e"), "{out}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn run_long_output_then_expand_round_trips() {
    let home = tmp("expand");
    let out = ok(
        &["run", "awk", "BEGIN{for(i=1;i<=50;i++)print \"line \"i}"],
        &home,
    );
    let trailer = out
        .lines()
        .find(|l| l.starts_with("[rtok "))
        .expect("{out}");
    let id = trailer.split_whitespace().nth(1).expect("{trailer}");
    let full = ok(&["expand", id], &home);
    assert!(
        full.contains("line 1") && full.contains("line 50"),
        "{full}"
    );
    let head = ok(&["expand", id, "--lines", "1-2"], &home);
    assert_eq!(head.lines().count(), 2, "{head}");
    let (_, err, code) = rtok(&["expand", "no-such-id"], &home);
    assert_ne!(code, 0, "unknown id must fail");
    assert!(err.contains("unknown archive id"), "{err}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn plugins_lists_the_catalogue() {
    let home = tmp("plugins");
    let out = ok(&["plugins"], &home);
    for id in [
        "measure", "cmd", "read", "archive", "proxy", "inject", "guard", "memory", "graph", "toon",
        "compress",
    ] {
        assert!(out.contains(id), "{id} missing:\n{out}");
    }
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn config_init_get_set_validate_round_trips() {
    let home = tmp("config");
    ok(&["config", "init"], &home);
    assert!(home.join("config.toml").exists());
    let path = ok(&["config", "path"], &home);
    assert!(path.contains("config.toml"), "{path}");
    assert!(ok(&["config", "show"], &home).contains("proxy.port"));
    assert!(
        !ok(&["config", "get", "proxy.port"], &home)
            .trim()
            .is_empty()
    );
    ok(&["config", "set", "proxy.port", "8791"], &home);
    assert_eq!(ok(&["config", "get", "proxy.port"], &home).trim(), "8791");
    assert!(ok(&["config", "validate"], &home).starts_with("ok"));
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn bench_dry_run_lists_the_schedule() {
    let home = tmp("bench");
    let tasks = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/tasks.toml");
    let out = ok(&["bench", "--tasks", tasks, "--dry-run"], &home);
    assert_eq!(out.lines().count(), 6 * 2 * 3, "{out}");
    assert!(out.contains("add-fn a 1"), "{out}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn doctor_reports_the_chain() {
    let home = tmp("doctor");
    let out = ok(&["doctor"], &home);
    assert!(out.contains("hooks ") && out.contains("proxy "), "{out}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn stats_json_parses() {
    let home = tmp("stats");
    let out = ok(&["stats", "--json"], &home);
    assert!(
        serde_json::from_str::<serde_json::Value>(&out).is_ok_and(|v| v.get("sessions").is_some()),
        "{out}"
    );
    let _ = fs::remove_dir_all(&home);
}
