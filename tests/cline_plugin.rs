//! T95 + D21: the Cline plugin tree is one POSIX script plus its README.
//!
//! Check: `plugins/cline/hooks/rtok-hook` is executable, fails open without `rtok`
//! on `PATH` (`{}`, exit 0, ketch hint on stderr), takes its event from its own
//! file name, and every event name the installer links (T96: one link per event
//! into `~/Documents/Cline/Hooks/`) is one `adapt_cline` knows.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use rtok::hooks::types::HookInput;

/// The one script T96 links once per event (`PreToolUse`, `PostToolUse`, …).
fn hook() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/cline/hooks/rtok-hook")
}

/// Event file names the installer links the hook as. Each must survive
/// `adapt_cline` as something a plugin can act on — never `Noop`.
/// (T94 `cline_event`: tool events → tool events, lifecycle → session/prompt
/// starts and ends; `agent_error`, `agent_abort` and unknowns are no-ops.)
const LINKED_EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "TaskStart",
    "UserPromptSubmit",
    "SessionEnd",
];

#[test]
fn hook_script_is_executable_posix_sh() {
    let hook = hook();
    let meta = fs::metadata(&hook).unwrap_or_else(|e| panic!("{}: {e}", hook.display()));
    assert!(
        meta.permissions().mode() & 0o111 != 0,
        "{}: hook must be executable",
        hook.display()
    );
    let text = fs::read_to_string(&hook).expect("read hook");
    let first = text.lines().next().unwrap_or("");
    assert_eq!(first.trim(), "#!/bin/sh", "{}", hook.display());
}

/// Without `rtok` on `PATH` the hook fails open: `{}` on stdout, exit 0, and the
/// ketch install hint on stderr (T95; D21). `T96` resolves the binary the same
/// way, so this dir must stay free of any fake `rtok`.
#[test]
fn hook_fails_open_without_rtok_on_path() {
    let dir = std::env::temp_dir().join(format!(
        "rtok-cline-hook-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // `$HOME/.ketch/bin/rtok` exists on dev machines, so point HOME at the empty
    // dir and keep PATH to bare system dirs without `rtok`.
    let out = Command::new(hook())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", &dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(br#"{"hookName":"tool_call","taskId":"t"}"#)?;
            child.wait_with_output()
        })
        .expect("run hook without rtok");
    assert_eq!(out.status.code(), Some(0), "fail open exits 0");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "{}",
        "fail open prints {{}}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ketch install listepo/rtok"),
        "stderr must say how to install rtok: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn every_linked_event_is_one_adapt_cline_knows() {
    for event in LINKED_EVENTS {
        let mut input = HookInput::default();
        input.adapt_cline(event);
        assert_ne!(
            input.hook_event_name, "Noop",
            "{event}: installer-linked names must not be no-ops"
        );
    }
}
