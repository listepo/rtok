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
                "command": format!("rtok hook {event}"),
                "timeout": 5
            }]}]),
            "{event}"
        );
    }
}
