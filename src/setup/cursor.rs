//! Cursor installer (`rtok agent setup cursor`) and field mapping (plan T10.1).
//!
//! Cursor shell stdin uses top-level `command` and `conversation_id`.
//! `beforeShellExecution` → PreToolUse; `afterShellExecution` → PostToolUse
//! (`output`/`stdout` → `tool_response`) so guard/read caches populate.
//! [`crate::hooks::types::HookInput::adapt_cursor`] performs that map when
//! `[hook] host` is `cursor` (also `--host cursor`).
//! Cursor `hooks.json` is `{version, hooks.before|afterShellExecution[].command}`.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, PluginLink, read_json, write_json};
use serde_json::{Value, json};

use super::{apply, plugin_src};
use crate::config::Config;

const PRE_CMD: &str = "rtok hook PreToolUse --host cursor";
const POST_CMD: &str = "rtok hook PostToolUse --host cursor";

/// Apply, dry-run, or remove Cursor before/after shell hook entries.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.cursor.hooks_path;
    let mut root = read_json(path)?;
    let report = if remove {
        strip_ours(&mut root)
    } else {
        insert_ours(&mut root)
    };
    write_json(&apply(cfg), path, &root, &report)?;
    Ok(report)
}

/// `~/.cursor/mcp.json` — the sibling of `hooks.json`.
fn mcp_path(cfg: &Config) -> PathBuf {
    cfg.setup.cursor.hooks_path.with_file_name("mcp.json")
}

/// Register `rtok mcp` in `~/.cursor/mcp.json` (sibling of `hooks.json`).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::register_mcp(&apply(cfg), &mcp_path(cfg), "rtok", "rtok", &["mcp"])
}

/// Drop `mcpServers.rtok` from `~/.cursor/mcp.json` (`rtok agent remove cursor`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_mcp(&apply(cfg), &mcp_path(cfg), "rtok")
}

const PLUGIN_SRC_REL: &str = "plugins/cursor";
const PLUGIN_LOCAL: &str = "~/.cursor/plugins/local";

/// Local Cursor plugin dest: sibling of hooks.json → `<cursor-dir>/plugins/local/rtok`.
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup
        .cursor
        .hooks_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("plugins/local/rtok")
}

fn link(cfg: &Config) -> PluginLink<'static> {
    PluginLink {
        src_rel: PLUGIN_SRC_REL,
        src: plugin_src(PLUGIN_SRC_REL),
        dest: plugin_dest(cfg),
        label: Some(PLUGIN_LOCAL),
        host: "Cursor",
    }
}

/// Offer / link / unlink `plugins/cursor` (D21, T10.5).
/// Dry-run and the unaccepted offer MUST contain the substrings `plugins/cursor`
/// and `~/.cursor/plugins/local` and `ketch install listepo/rtok`.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    let report = link(cfg).run(&apply(cfg), remove)?;
    // Singleton (D21): the plugin is the MCP, so a previous `mcpServers.rtok`
    // entry from a plain install must go, else two writers serve one store.
    if report.starts_with("+ plugin") {
        let _ = unregister_mcp(cfg);
    }
    Ok(report)
}

/// True when `--yes` accepted the plugin, so `mcp.json` must not also register rtok.
/// Also true when the plugin link already exists: the plugin *is* the MCP (D21
/// singleton), so a later plain `rtok agent setup cursor` must not add a second entry.
pub fn plugin_is_mcp(cfg: &Config, remove: bool) -> bool {
    !remove && (cfg.setup.yes || link(cfg).linked())
}

fn insert_ours(root: &mut Value) -> String {
    if !root.is_object() {
        *root = json!({});
    }
    root.as_object_mut()
        .unwrap()
        .entry("version")
        .or_insert_with(|| json!(1));
    let hooks = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let mut added = Vec::new();
    for (event, cmd) in [
        ("beforeShellExecution", PRE_CMD),
        ("afterShellExecution", POST_CMD),
    ] {
        if insert_hook(hooks, event, cmd) {
            added.push(format!("+ {event} {cmd}"));
        }
    }
    if added.is_empty() {
        NO_CHANGES.into()
    } else {
        added.join("\n")
    }
}

fn insert_hook(hooks: &mut Value, event: &str, cmd: &str) -> bool {
    let arr = hooks
        .as_object_mut()
        .unwrap()
        .entry(event)
        .or_insert_with(|| json!([]));
    if !arr.is_array() {
        *arr = json!([]);
    }
    let arr = arr.as_array_mut().unwrap();
    if arr.iter().any(|e| is_cmd(e, cmd)) {
        return false;
    }
    arr.push(json!({"command": cmd}));
    true
}

fn strip_ours(root: &mut Value) -> String {
    let mut removed = Vec::new();
    for event in ["beforeShellExecution", "afterShellExecution"] {
        let Some(arr) = root
            .pointer_mut(&format!("/hooks/{event}"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = arr.len();
        arr.retain(|e| !is_ours(e));
        if arr.len() != before {
            removed.push(format!("- {event}"));
        }
    }
    if removed.is_empty() {
        NO_CHANGES.into()
    } else {
        removed.join("\n")
    }
}

fn is_cmd(entry: &Value, cmd: &str) -> bool {
    entry.get("command").and_then(Value::as_str) == Some(cmd)
}

fn is_ours(entry: &Value) -> bool {
    matches!(
        entry.get("command").and_then(Value::as_str),
        Some(PRE_CMD) | Some(POST_CMD)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{dispatch, types::HookInput};
    use crate::plugin::Runtime;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-cursor-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(hooks: PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.cursor.hooks_path = hooks;
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    #[test]
    fn dry_run_offer_names_plugin_and_local() {
        let dir = tmp("offer-dry");
        let path = dir.join("hooks.json");
        let c = cfg(path, true);
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("plugins/cursor"), "{s}");
        assert!(s.contains("~/.cursor/plugins/local"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_dest(&c).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_plugin_second_apply_no_changes() {
        let dir = tmp("offer-yes");
        let mut c = cfg(dir.join("hooks.json"), false);
        c.setup.yes = true;
        c.setup.backup = false;
        let first = offer_plugin(&c, false).unwrap();
        assert!(first.starts_with("+ plugin"), "{first}");
        assert!(link(&c).linked());
        assert_eq!(offer_plugin(&c, false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dry_run_then_apply_is_idempotent() {
        let dir = tmp("setup");
        let path = dir.join("hooks.json");
        let dry = run(&cfg(path.clone(), true), false).unwrap();
        assert!(dry.contains("beforeShellExecution"), "{dry}");
        assert!(dry.contains("afterShellExecution"), "{dry}");
        assert!(!path.exists());
        let c = cfg(path.clone(), false);
        assert!(run(&c, false).unwrap().contains(PRE_CMD));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\""));
        assert!(raw.contains(PRE_CMD));
        assert!(raw.contains(POST_CMD));
        assert!(raw.contains("afterShellExecution"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_payload_wraps_command() {
        let dir = tmp("wrap");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let cx = Runtime::open(c, "cur").unwrap();
        let raw = json!({
            "hook_event_name": "beforeShellExecution",
            "command": "ls -la",
            "cwd": "/tmp",
            "conversation_id": "sess-1"
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let mut input: HookInput = serde_json::from_slice(&bytes).unwrap();
        input.adapt_cursor("PreToolUse");
        let out = dispatch(&bytes, &input, &cx);
        let v: Value = serde_json::from_slice(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap_or("");
        assert!(cmd.contains("rtok run --") && cmd.contains("ls -la"), "{v}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_after_shell_reaches_post_tool_use() {
        let dir = tmp("after");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let cx = Runtime::open(c, "cur").unwrap();
        let raw = json!({
            "hook_event_name": "afterShellExecution",
            "command": "echo hi",
            "output": "hi\n",
            "cwd": "/tmp",
            "conversation_id": "sess-2"
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let mut input: HookInput = serde_json::from_slice(&bytes).unwrap();
        input.adapt_cursor("PostToolUse");
        assert_eq!(input.hook_event_name, "PostToolUse");
        assert!(input.post_tool().is_some());
        let out = dispatch(&bytes, &input, &cx);
        // Fail open: PostToolUse may be {} when plugins add no context.
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert!(v.is_object(), "{v}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn setup_writes_after_shell_hook() {
        let dir = tmp("after-setup");
        let path = dir.join("hooks.json");
        let c = cfg(path.clone(), false);
        let report = run(&c, false).unwrap();
        assert!(report.contains("afterShellExecution"), "{report}");
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let after = root["hooks"]["afterShellExecution"].as_array().unwrap();
        assert!(
            after.iter().any(|e| e["command"] == POST_CMD),
            "{root}"
        );
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }
}
