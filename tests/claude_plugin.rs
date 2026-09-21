//! T115 + D21: `rtok agents install claude --yes` installs `plugins/claude` through the official
//! `claude plugin` commands (a fake `claude` first on PATH records them), and while the plugin is
//! installed it is the only call path — the settings-file hooks and `mcpServers.rtok` go.
#![cfg(unix)]

mod common;

use common::agents::{claude_log, json, rtok, tmp, write_cfg};
use std::fs;

#[test]
fn dry_run_offers_the_claude_commands_and_runs_nothing() {
    let home = tmp("claude-plugin-dry");
    let cfg = write_cfg(&home);
    let out = rtok(
        &["agents", "install", "claude", "--yes", "--dry-run"],
        &cfg,
        &home,
    );
    assert!(out.contains("offer plugins/claude"), "{out}");
    assert!(
        out.contains("claude plugin marketplace add ") && out.contains("plugins/claude"),
        "{out}"
    );
    assert!(out.contains("claude plugin install rtok@rtok"), "{out}");
    assert!(out.contains("ketch install listepo/rtok"), "{out}");
    assert_eq!(claude_log(&home), "", "a dry run calls no claude");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn yes_installs_the_plugin_as_the_only_call_path_and_remove_uninstalls() {
    let home = tmp("claude-plugin-yes");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let claude_json = home.join(".claude.json");

    // A plain install first: settings-file hooks and MCP, no plugin, no claude call.
    let plain = rtok(&["agents", "install", "claude", "--mcp"], &cfg, &home);
    assert!(plain.contains("(accept with --yes)"), "{plain}");
    assert_eq!(claude_log(&home), "");
    assert!(
        fs::read_to_string(&settings)
            .unwrap()
            .contains(" hook PreToolUse")
    );
    assert!(json(&claude_json)["mcpServers"]["rtok"].is_object());

    let yes = rtok(&["agents", "install", "claude", "--yes"], &cfg, &home);
    assert!(yes.contains("+ plugin plugins/claude → rtok@rtok"), "{yes}");
    let log = claude_log(&home);
    let calls: Vec<&str> = log.lines().collect();
    assert_eq!(calls.len(), 2, "{log}");
    assert!(
        calls[0].starts_with("plugin marketplace add ") && calls[0].ends_with("plugins/claude"),
        "{log}"
    );
    assert_eq!(calls[1], "plugin install rtok@rtok");
    // D21 singleton: the plugin serves hooks and MCP now.
    assert!(
        !fs::read_to_string(&settings).unwrap().contains(" hook "),
        "settings hooks stripped"
    );
    assert!(json(&claude_json)["mcpServers"]["rtok"].is_null());
    assert!(yes.contains("plugin"), "{yes}");

    let again = rtok(&["agents", "install", "claude", "--yes"], &cfg, &home);
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
