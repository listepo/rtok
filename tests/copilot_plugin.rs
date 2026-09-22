//! T116 + D21: the Copilot CLI plugin tree is one unit — a legacy `plugin.json` manifest,
//! Copilot's camelCase hooks and one MCP server — and `rtok agents install copilot --yes`
//! drives `copilot plugin install <resolved plugins/copilot>` through the `copilot` CLI
//! (the fake shim's log), taking rtok's own hooks/rtok.json and mcp-config.json back.

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

mod common;
use common::agents::{fake_copilot, rtok, tmp, write_cfg};

fn read(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugins/copilot")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn manifest_is_rtok_with_the_two_component_paths() {
    let m = read("plugin.json");
    assert_eq!(m["name"], "rtok");
    assert_eq!(m["hooks"], "hooks/hooks.json");
    assert_eq!(m["mcpServers"], ".mcp.json");
}

/// The tree's hooks file is exactly what `~/.copilot/hooks/rtok.json` writes — one shape,
/// two surfaces (D21), never a drifted copy.
#[test]
fn hooks_are_the_installers_doc_with_the_bare_rtok_command() {
    assert_eq!(
        read("hooks/hooks.json"),
        rtok::agents::copilot::hooks_doc("rtok", 5)
    );
}

#[test]
fn mcp_is_exactly_rtok() {
    assert_eq!(
        read(".mcp.json"),
        json!({"mcpServers": {"rtok": {
            "type": "local",
            "command": "rtok",
            "args": ["mcp"],
            "tools": ["*"]
        }}})
    );
}

#[test]
fn the_plugin_installs_through_the_copilot_cli_and_remove_uninstalls() {
    let home = tmp("copilot-plugin");
    let cfg = write_cfg(&home);
    fake_copilot(&home);
    let log = home.join("copilot.log");
    let marker = home.join(".copilot/installed-plugins/_direct/x/plugin.json");

    // Dry-run names the documented local install and runs nothing.
    let dry = rtok(
        &["agents", "install", "copilot", "--yes", "--dry-run"],
        &cfg,
        &home,
    );
    assert!(dry.contains("copilot plugin install"), "{dry}");
    assert!(dry.contains("plugins/copilot"), "{dry}");
    assert!(!log.exists(), "dry-run runs nothing");

    // `--yes` installs through the CLI; D21 — the plugin is the unit, so rtok's own
    // hooks/rtok.json and mcpServers.rtok never come beside it.
    let first = rtok(&["agents", "install", "copilot", "--yes"], &cfg, &home);
    assert!(first.contains("+ plugin plugins/copilot → rtok"), "{first}");
    assert!(fs::read_to_string(&log).unwrap().contains("plugin install"));
    assert!(marker.is_file(), "the shim installed the plugin");
    assert!(
        !home.join(".copilot/hooks/rtok.json").exists(),
        "D21: no second hooks set: {first}"
    );
    assert!(
        !fs::read_to_string(home.join(".copilot/mcp-config.json"))
            .unwrap_or_default()
            .contains("rtok"),
        "D21: no second rtok mcp"
    );

    // A repeat is a no-op with no second CLI call.
    let again = rtok(&["agents", "install", "copilot", "--yes"], &cfg, &home);
    assert!(again.contains("already installed"), "{again}");
    let calls = fs::read_to_string(&log).unwrap();
    assert_eq!(
        calls
            .lines()
            .filter(|l| l.contains("plugin install"))
            .count(),
        1,
        "{calls}"
    );

    // Remove uninstalls by the manifest's `name`.
    let removed = rtok(&["agents", "remove", "copilot"], &cfg, &home);
    assert!(removed.contains("- plugin rtok"), "{removed}");
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(&home);
}
