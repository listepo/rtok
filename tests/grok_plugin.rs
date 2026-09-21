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
            "command": format!("rtok hook {event} --host grok"),
            "timeout": 5
        }]});
        if let Some(m) = matcher {
            group["matcher"] = json!(m);
        }
        assert_eq!(hooks[event], json!([group]), "{event}");
    }
}
