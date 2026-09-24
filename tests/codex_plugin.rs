//! T121 + D21: the Codex plugin tree is one unit — a manifest, rtok's hooks and one MCP server.

use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

fn read(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugins/codex")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn manifest_is_rtok_and_points_at_its_files() {
    let m = read(".codex-plugin/plugin.json");
    assert_eq!(m["name"], "rtok");
    assert_eq!(m["hooks"], "./hooks/hooks.json");
    assert_eq!(m["mcpServers"], "./.mcp.json");
}

#[test]
fn marketplace_lists_this_folder() {
    let m = read(".agents/plugins/marketplace.json");
    assert_eq!(m["plugins"][0]["name"], "rtok");
    assert_eq!(
        m["plugins"][0]["source"],
        json!({"source": "local", "path": "./"})
    );
}

/// The repo-root marketplace `codex plugin marketplace add listepo/rtok` resolves (T140):
/// same shape as the nested dev marketplace above, but its one plugin points at the
/// `plugins/codex` subdirectory instead of `./`, since the marketplace file itself lives at
/// the repo root, not inside the plugin's own folder.
#[test]
fn root_marketplace_points_at_the_plugins_codex_subdirectory() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".agents/plugins/marketplace.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let m: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(m["name"], "rtok");
    assert_eq!(m["plugins"][0]["name"], "rtok");
    assert_eq!(
        m["plugins"][0]["source"],
        json!({"source": "local", "path": "./plugins/codex"})
    );
}

#[test]
fn mcp_is_exactly_rtok() {
    assert_eq!(
        read(".mcp.json"),
        json!({"mcpServers": {"rtok": {"command": "rtok", "args": ["mcp"]}}})
    );
}

/// The installer's event set (`agents::codex::COMPACT`): the plugin fires what the installer does.
#[test]
fn hooks_are_the_installer_compaction_pair() {
    let hooks = read("hooks/hooks.json")["hooks"].clone();
    let want = ["PreCompact", "PostCompact"];
    assert_eq!(hooks.as_object().unwrap().len(), want.len(), "{hooks}");
    for event in want {
        assert_eq!(
            hooks[event],
            json!([{"hooks": [{
                "type": "command",
                "command": format!(
                    "command -v rtok >/dev/null 2>&1 && exec rtok hook {event}; \
                     [ -x \"$HOME/.ketch/bin/rtok\" ] && exec \"$HOME/.ketch/bin/rtok\" hook {event}; \
                     exit 0"
                ),
                "commandWindows": format!("rtok hook {event}"),
                "timeout": 5
            }]}]),
            "{event}"
        );
    }
}

/// T250.1 check: on Unix, each event's `command` resolves `rtok` from PATH first, then falls
/// back to `~/.ketch/bin/rtok`, and exits 0 silently when neither exists — matching the fail-open
/// launcher pattern used by `plugins/claude/scripts/hook.sh`.
#[test]
#[cfg(unix)]
fn hook_commands_resolve_path_then_ketch_then_exit_open() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let home = std::env::temp_dir().join(format!("rtok-t250.1-codex-hooks-{unique}"));
    let empty_path = std::env::temp_dir().join(format!("rtok-t250.1-codex-path-{unique}"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&empty_path).unwrap();

    let hooks = read("hooks/hooks.json")["hooks"].clone();
    for event in ["PreCompact", "PostCompact"] {
        let command = hooks[event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .to_string();

        let run = || {
            let mut child = Command::new("/bin/sh")
                .args(["-c", &command])
                .env("HOME", &home)
                .env("PATH", &empty_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(b"{}").unwrap();
            drop(stdin);
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{event}: {command}");
            String::from_utf8(out.stdout).unwrap()
        };

        // Neither PATH nor ~/.ketch/bin has rtok: fail open, no output.
        assert_eq!(run(), "", "{event}");

        // Only ~/.ketch/bin/rtok exists: it is the one that gets exec'd.
        let ketch_bin = home.join(".ketch/bin");
        fs::create_dir_all(&ketch_bin).unwrap();
        fs::write(
            ketch_bin.join("rtok"),
            "#!/bin/sh\nprintf 'ketch %s' \"$2\"\n",
        )
        .unwrap();
        fs::set_permissions(ketch_bin.join("rtok"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(run(), format!("ketch {event}"), "{event}");
        fs::remove_dir_all(home.join(".ketch")).unwrap();
    }

    let _ = fs::remove_dir_all(&home);
    let _ = fs::remove_dir_all(&empty_path);
}
