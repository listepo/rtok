//! T115 + T139 + D21: `rtok agents install claude` installs `plugins/claude` through the official
//! `claude plugin` commands, by default once `claude` is on PATH — no `--yes` needed — from the
//! GitHub marketplace `listepo/rtok` (a fake `claude` first on PATH records the calls), and while
//! the plugin is installed it is the only call path — the settings-file hooks and `mcpServers.rtok`
//! go.
#![cfg(unix)]

mod common;

use common::agents::{claude_log, rtok, tmp, write_cfg};
use std::fs;

#[test]
fn dry_run_offers_the_claude_commands_and_runs_nothing() {
    let home = tmp("claude-plugin-dry");
    let cfg = write_cfg(&home);
    // No `--yes`: a dry run previews the default install (T139).
    let out = rtok(&["agents", "install", "claude", "--dry-run"], &cfg, &home);
    assert!(out.contains("offer plugins/claude"), "{out}");
    assert!(
        out.contains("claude plugin marketplace add listepo/rtok"),
        "{out}"
    );
    assert!(out.contains("claude plugin install rtok@rtok"), "{out}");
    assert!(out.contains("ketch install listepo/rtok"), "{out}");
    assert_eq!(claude_log(&home), "", "a dry run calls no claude");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn installs_the_plugin_by_default_as_the_only_call_path_and_remove_uninstalls() {
    let home = tmp("claude-plugin-default");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let claude_json = home.join(".claude.json");

    // A plain install (no `--yes`) installs the plugin outright once `claude` is on PATH (T139).
    let plain = rtok(&["agents", "install", "claude"], &cfg, &home);
    assert!(
        plain.contains("+ plugin plugins/claude → rtok@rtok"),
        "{plain}"
    );
    let log = claude_log(&home);
    let calls: Vec<&str> = log.lines().collect();
    assert_eq!(calls.len(), 2, "{log}");
    assert_eq!(calls[0], "plugin marketplace add listepo/rtok");
    assert_eq!(calls[1], "plugin install rtok@rtok");
    // D21 singleton: the plugin serves hooks and MCP instead — the default install goes
    // straight to the plugin, so neither file is ever written (or, if written, carries none).
    assert!(
        !fs::read_to_string(&settings)
            .unwrap_or_default()
            .contains(" hook "),
        "settings hooks stripped"
    );
    assert!(
        !fs::read_to_string(&claude_json)
            .unwrap_or_default()
            .contains("\"rtok\""),
        "mcpServers.rtok not registered"
    );

    let again = rtok(&["agents", "install", "claude"], &cfg, &home);
    assert!(again.contains("already installed"), "{again}");
    assert_eq!(claude_log(&home).lines().count(), 2, "no second install");

    let removed = rtok(&["agents", "remove", "claude"], &cfg, &home);
    assert!(removed.contains("- plugin rtok@rtok"), "{removed}");
    let log = claude_log(&home);
    let tail: Vec<&str> = log.lines().skip(2).collect();
    assert_eq!(
        tail,
        [
            "plugin uninstall rtok@rtok",
            "plugin marketplace remove rtok"
        ]
    );
    let _ = fs::remove_dir_all(&home);
}

/// T178: every plugin hook command execs `rtok` from PATH in Claude Code's own shell, stdin
/// included, and falls back to `scripts/hook.sh` (the fail-open launcher) only when PATH has none.
#[test]
fn hook_commands_exec_rtok_from_path_and_fall_back_to_hook_sh() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let home = tmp("claude-plugin-launch");
    let script = |path: std::path::PathBuf, body: &str| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    script(
        home.join("bin/rtok"),
        r#"printf 'rtok %s %s ' "$1" "$2"; cat"#,
    );
    script(
        home.join("root/scripts/hook.sh"),
        r#"printf 'fallback %s' "$1""#,
    );
    let run = |cmd: &str, path: String| {
        let mut child = Command::new("/bin/sh")
            .args(["-c", cmd])
            .env("PATH", path)
            .env("CLAUDE_PLUGIN_ROOT", home.join("root"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"{}").unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{cmd}");
        String::from_utf8(out.stdout).unwrap()
    };
    let hooks: serde_json::Value =
        serde_json::from_str(include_str!("../plugins/claude/hooks/hooks.json")).unwrap();
    for (event, entries) in hooks["hooks"].as_object().unwrap() {
        for cmd in entries
            .as_array()
            .unwrap()
            .iter()
            .map(|e| &e["hooks"][0]["command"])
        {
            let cmd = cmd.as_str().unwrap();
            let with = format!("{}:/usr/bin:/bin", home.join("bin").display());
            assert_eq!(run(cmd, with), format!("rtok hook {event} {{}}"));
            assert_eq!(
                run(cmd, "/usr/bin:/bin".into()),
                format!("fallback {event}")
            );
        }
    }
    let _ = fs::remove_dir_all(&home);
}
