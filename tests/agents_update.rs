//! T242: re-running the installer brings what rtok already wrote up to date — the files
//! change where they are stale and stay byte-identical where they are current.
//!
//! Check: seed a host file the way an older rtok left it, run the command, assert the file
//! now holds what the current binary writes (foreign keys and hooks kept) with one backup of
//! the seed; run it again and assert the bytes and the backups do not move.

mod common;

use common::agents::{backups, json, rtok_without_claude, tmp, write_cfg};
use std::fs;

/// What an older install left in `~/.claude/settings.json`: a hook on a versioned store path
/// with another timeout, a `PostToolUse` matcher rtok no longer uses, and a `Stop` hook rtok
/// no longer installs sitting beside a foreign one.
const STALE_SETTINGS: &str = r#"{"theme":"dark","hooks":{
  "PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/old/store/rtok/v0.1.0/rtok hook PreToolUse","timeout":1}]}],
  "PostToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtok hook PostToolUse"}]}],
  "Stop":[{"hooks":[{"type":"command","command":"rtok hook Stop"},{"type":"command","command":"notify-send done"}]}]
}}"#;

#[test]
fn install_rewrites_stale_claude_hooks_and_a_rerun_changes_nothing() {
    let home = tmp("update-stale-hooks");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, STALE_SETTINGS).unwrap();

    let out = rtok_without_claude(&["agents", "install", "claude", "--cli"], &cfg, &home);
    let after = fs::read_to_string(&settings).unwrap();
    assert_ne!(after, STALE_SETTINGS, "{out}");
    assert!(!after.contains("/old/store"), "{after}");

    let root = json(&settings);
    let hooks = &root["hooks"];
    // The binary a fresh hook got — bare `rtok`, or the absolute `rtok.exe` on Windows.
    let fresh = hooks["SessionEnd"][0]["hooks"][0]["command"].as_str().unwrap();
    let bin = fresh.strip_suffix(" hook SessionEnd").unwrap();
    let pre = &hooks["PreToolUse"][0]["hooks"][0];
    assert_eq!(pre["command"], format!("{bin} hook PreToolUse"), "{after}");
    // The timeout every freshly added hook got is the one the stale hook now carries.
    assert_eq!(
        pre["timeout"],
        hooks["SessionEnd"][0]["hooks"][0]["timeout"]
    );
    assert_ne!(pre["timeout"], 1);
    let post = hooks["PostToolUse"].as_array().unwrap();
    assert_eq!(post.len(), 1, "one rtok hook per event: {after}");
    assert_eq!(post[0]["matcher"], "*");
    assert_eq!(
        hooks["Stop"][0]["hooks"],
        serde_json::json!([{"type":"command","command":"notify-send done"}])
    );
    assert_eq!(root["theme"], "dark");

    let baks = backups(&settings);
    assert_eq!(baks.len(), 1, "{baks:?}");
    assert_eq!(fs::read_to_string(&baks[0]).unwrap(), STALE_SETTINGS);

    // Without `claude` on PATH the plugin offer line stays open on every run, so the block
    // never says `already installed`; the hooks themselves must report nothing and the file
    // must keep its bytes.
    let again = rtok_without_claude(&["agents", "install", "claude", "--cli"], &cfg, &home);
    assert!(!again.contains(" hook "), "{again}");
    assert_eq!(fs::read_to_string(&settings).unwrap(), after);
}
