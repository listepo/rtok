//! T99 + D21: the Grok Build plugin tree is one unit — a manifest, rtok's hooks and one MCP server.

use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

fn read(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugins/grok")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// T250.4: the shell one-liner every hook runs — PATH first, then ketch's install dir, else a
/// silent no-op (fail open; Grok's PowerShell runner has no per-OS field, so Windows users get
/// `rtok agents install claude` instead — plugin stays macOS/Linux only).
fn hook_command(event: &str) -> String {
    format!(
        "command -v rtok >/dev/null 2>&1 && exec rtok hook {event} --host grok; \
[ -x \"$HOME/.ketch/bin/rtok\" ] && exec \"$HOME/.ketch/bin/rtok\" hook {event} --host grok; exit 0"
    )
}

#[test]
fn manifest_is_rtok() {
    assert_eq!(read(".grok-plugin/plugin.json")["name"], "rtok");
}

#[test]
fn mcp_is_exactly_rtok() {
    assert_eq!(
        read(".mcp.json"),
        json!({"mcpServers": {"rtok": {"command": "rtok", "args": ["mcp"]}}})
    );
}

/// Claude's installer entries (`agents::claude::ENTRIES`) minus `Read` and `Skill` (plan T98):
/// Grok's matcher is a regex, so the catch-all PostToolUse omits it rather than use Claude's `*`.
#[test]
fn hooks_are_the_claude_set_without_read_and_skill() {
    let want = [
        ("PreToolUse", Some("Bash")),
        ("PostToolUse", None),
        ("UserPromptSubmit", None),
        ("SessionStart", None),
        ("PreCompact", None),
        ("PostCompact", None),
        ("SessionEnd", None),
    ];
    let hooks = read("hooks/hooks.json")["hooks"].clone();
    assert_eq!(hooks.as_object().unwrap().len(), want.len(), "{hooks}");
    for (event, matcher) in want {
        let mut group = json!({"hooks": [{
            "type": "command",
            "command": hook_command(event),
            "timeout": 5
        }]});
        if let Some(m) = matcher {
            group["matcher"] = json!(m);
        }
        assert_eq!(hooks[event], json!([group]), "{event}");
    }
}

/// T250.4 check: each event's hook command resolves rtok from `PATH`, falls back to
/// `~/.ketch/bin/rtok`, and exits 0 silently — no note, not even on `SessionStart` — when
/// neither exists (fail open); with only the ketch copy present, it runs that one.
#[cfg(unix)]
#[test]
fn hooks_resolve_rtok_from_path_then_ketch_else_exit_0_silently() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let hooks = read("hooks/hooks.json")["hooks"].clone();
    let events: Vec<String> = hooks.as_object().unwrap().keys().cloned().collect();
    assert!(!events.is_empty());

    let unique = format!(
        "rtok-t250.4-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let home = std::env::temp_dir().join(format!("{unique}-home"));
    let empty_path = std::env::temp_dir().join(format!("{unique}-path"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&empty_path).unwrap();

    let run = |command: &str| -> (bool, String) {
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .env("HOME", &home)
            .env("PATH", &empty_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // A fail-open hook with no rtok exits without reading stdin: the write may hit BrokenPipe.
        let _ = child.stdin.take().unwrap().write_all(b"{}");
        let out = child.wait_with_output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    };

    for event in &events {
        let command = hooks[event][0]["hooks"][0]["command"].as_str().unwrap();
        let (ok, stdout) = run(command);
        assert!(ok, "{event}: expected exit 0 with no rtok anywhere");
        assert_eq!(stdout, "", "{event}: expected silence, got {stdout:?}");
    }

    let ketch_dir = home.join(".ketch").join("bin");
    fs::create_dir_all(&ketch_dir).unwrap();
    let ketch_rtok = ketch_dir.join("rtok");
    fs::write(&ketch_rtok, "#!/bin/sh\nprintf 'ketch %s' \"$2\"\n").unwrap();
    fs::set_permissions(&ketch_rtok, fs::Permissions::from_mode(0o755)).unwrap();

    for event in &events {
        let command = hooks[event][0]["hooks"][0]["command"].as_str().unwrap();
        let (ok, stdout) = run(command);
        assert!(ok, "{event}: expected exit 0 with ketch rtok");
        assert_eq!(stdout, format!("ketch {event}"), "{event}");
    }

    let _ = fs::remove_dir_all(&home);
    let _ = fs::remove_dir_all(&empty_path);
}
