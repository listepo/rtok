//! T38.1/T38.5: e2e for the commands without direct coverage — `hook`, `run`,
//! `expand`, `plugins`, `config`, `bench --dry-run`, `doctor`, `stats` — driven
//! through `assert_cmd` (plan Working agreement).
//!
//! Check: one case per command through the binary with an isolated HOME.

use assert_cmd::Command as AssertCmd;
use std::fs;
use std::path::{Path, PathBuf};

fn cmd(args: &[&str], home: &Path) -> AssertCmd {
    let mut c = AssertCmd::cargo_bin("rtok").unwrap();
    c.args(args).env("RTOK_HOME", home).env("HOME", home);
    c
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

/// Run to success, return stdout.
fn ok(args: &[&str], home: &Path) -> String {
    let out = cmd(args, home).assert().success().get_output().clone();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn hook(event: &str, fixture: &[u8], home: &Path) -> String {
    let out = cmd(&["hook", event], home)
        .write_stdin(fixture)
        .assert()
        .success()
        .get_output()
        .clone();
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

/// T50.4: native Grep/Glob pass through by default; with
/// `RTOK_PLUGINS_GUARD_DENY_GREP_GLOB=true` the hook exits 0 with a deny
/// naming the MCP replacement (`search` for Grep, `tree` for Glob).
#[test]
fn hook_grep_glob_deny_is_opt_in() {
    for (fixture, tool, pointer) in [
        (
            include_bytes!("fixtures/hooks/pre_tool_grep.json").as_slice(),
            "Grep",
            "search",
        ),
        (
            include_bytes!("fixtures/hooks/pre_tool_glob.json").as_slice(),
            "Glob",
            "tree",
        ),
    ] {
        // Default: no deny.
        let home = tmp("native-off");
        let stdout = hook("PreToolUse", fixture, &home);
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_ne!(
            v.pointer("/hookSpecificOutput/permissionDecision")
                .and_then(|x| x.as_str()),
            Some("deny"),
            "{tool} must pass by default: {stdout}"
        );
        let _ = fs::remove_dir_all(&home);
        // Opt-in: deny with a pointer, still exit 0 (fail open at the process).
        let home = tmp("native-on");
        let out = cmd(&["hook", "PreToolUse"], &home)
            .env("RTOK_PLUGINS_GUARD_DENY_GREP_GLOB", "true")
            .write_stdin(fixture)
            .assert()
            .success()
            .get_output()
            .clone();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            v.pointer("/hookSpecificOutput/permissionDecision")
                .and_then(|x| x.as_str()),
            Some("deny"),
            "{tool} must deny when opted in: {stdout}"
        );
        let reason = v
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        assert!(reason.contains(pointer), "{tool}: {reason}");
        let _ = fs::remove_dir_all(&home);
    }
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
    let out = cmd(&["expand", "no-such-id"], &home)
        .assert()
        .failure()
        .get_output()
        .clone();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
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

#[test]
fn info_prints_paths_sizes_and_proxy() {
    let home = tmp("info");
    let out = ok(&["info"], &home);
    for want in [
        "rtok ",
        "binary ",
        &format!("home {}", home.display()),
        "config.toml",
        "rtok.db (-)",
        "archive",
        "errors",
        "proxy ",
        "8790",
        "otel off",
        "store calls",
        "disk ",
    ] {
        assert!(out.contains(want), "{want} missing:\n{out}");
    }
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn info_counts_error_lines_and_json_parses() {
    let home = tmp("info-err");
    ok(&["info"], &home);
    let log = home.join("logs").join("rtok.log");
    fs::create_dir_all(log.parent().unwrap()).unwrap();
    fs::write(
        &log,
        "2026-09-09 15:04:05 error t/n: boom\n2026-09-09 15:04:06 info t/n: ok\n",
    )
    .unwrap();
    let out = ok(&["info"], &home);
    assert!(out.contains("2 lines, 1 errors"), "{out}");
    let json = ok(&["info", "--json"], &home);
    let v: serde_json::Value = serde_json::from_str(&json).expect("info --json is JSON");
    assert_eq!(v["log"]["errors"], 1);
    assert_eq!(v["proxy"]["port"], 8790);
    assert_eq!(v["otel"]["enabled"], false);
    assert!(v["db"]["bytes"].is_number(), "{v}");
    let _ = fs::remove_dir_all(&home);
}
