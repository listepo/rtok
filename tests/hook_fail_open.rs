//! T101: the hook fail-open matrix. `tests/extra_cover.rs` checks bad and empty stdin for
//! the default host only; here every value `[hook]` host accepts × every hook event ×
//! every kind of bad stdin must exit 0, print the host's no-op reply, and never rewrite
//! the input (D1). One case per combination in the nextest list.

use assert_cmd::Command as AssertCmd;
use rstest::rstest;
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-fail-open-{name}-{}-{}",
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

fn stdin_case(kind: &str) -> Vec<u8> {
    match kind {
        "empty" => Vec::new(),
        "garbage" => b"not-json{{{".to_vec(),
        "truncated-json" => br#"{"hook_event_name":"PreToolUse","tool_nam"#.to_vec(),
        "non-utf8" => vec![0xff, 0xfe, b'{', 0x00, b'}'],
        "1mib" => vec![b'x'; 1 << 20],
        other => panic!("unknown stdin case: {other}"),
    }
}

/// The hosts with an envelope of their own (the `--host` help's `claude | cursor |
/// copilot | devin`, plus grok — T98) × the events `hooks::dispatch` and the installers
/// know × the bad-stdin kinds. Every cell: exit 0, exactly `{}`, no rewrite.
#[rstest]
fn every_host_and_event_fails_open_on_bad_stdin(
    #[values("claude", "cursor", "copilot", "devin", "grok")] host: &str,
    #[values(
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PreCompact",
        "PostCompact",
        "SessionEnd",
        "Stop"
    )]
    event: &str,
    #[values("empty", "garbage", "truncated-json", "non-utf8", "1mib")] stdin: &str,
) {
    let home = tmp(&format!("{host}-{event}-{stdin}"));
    let out = AssertCmd::cargo_bin("rtok")
        .unwrap()
        .args(["hook", event, "--host", host])
        .env("RTOK_HOME", &home)
        .env("HOME", &home)
        .write_stdin(stdin_case(stdin))
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "{}",
        "{host}/{event}/{stdin}: the host's no-op reply, never a rewrite"
    );
    let _ = fs::remove_dir_all(&home);
}
