//! Cursor installer (`rtok setup cursor`) and field mapping (plan T10.1).
//!
//! Cursor `beforeShellExecution` stdin uses top-level `command` and
//! `conversation_id`. rtok's PreToolUse path expects `tool_name=Bash` and
//! `tool_input.command`. [`crate::hooks::types::HookInput::adapt_cursor`]
//! performs that map when `[hook] host` is `cursor` (also `--host cursor`).
//! Cursor `hooks.json` is `{version, hooks.beforeShellExecution[].command}`.

use std::fs;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::claude::{backup, read_settings, register_stdio_mcp};
use crate::config::Config;

const HOOK_CMD: &str = "rtok hook PreToolUse --host cursor";

/// Apply, dry-run, or remove the Cursor `beforeShellExecution` entry.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.cursor.hooks_path;
    let mut root = read_settings(path)?;
    let report = if remove {
        strip_ours(&mut root)
    } else {
        insert_ours(&mut root)
    };
    if !cfg.setup.dry_run && report != "no changes" {
        if cfg.setup.backup {
            backup(path)?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).ok();
        }
        fs::write(path, serde_json::to_string_pretty(&root)? + "\n")
            .with_context(|| path.display().to_string())?;
    }
    Ok(report)
}

/// Register `rtok mcp` in `~/.cursor/mcp.json` (sibling of `hooks.json`).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let path = cfg.setup.cursor.hooks_path.with_file_name("mcp.json");
    register_stdio_mcp(&path, cfg)
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
    let arr = hooks
        .as_object_mut()
        .unwrap()
        .entry("beforeShellExecution")
        .or_insert_with(|| json!([]));
    if !arr.is_array() {
        *arr = json!([]);
    }
    let arr = arr.as_array_mut().unwrap();
    if arr.iter().any(is_ours) {
        return "no changes".into();
    }
    arr.push(json!({"command": HOOK_CMD}));
    format!("+ beforeShellExecution {HOOK_CMD}")
}

fn strip_ours(root: &mut Value) -> String {
    let Some(arr) = root
        .pointer_mut("/hooks/beforeShellExecution")
        .and_then(Value::as_array_mut)
    else {
        return "no changes".into();
    };
    let before = arr.len();
    arr.retain(|e| !is_ours(e));
    if arr.len() == before {
        "no changes".into()
    } else {
        "- beforeShellExecution".into()
    }
}

fn is_ours(entry: &Value) -> bool {
    entry.get("command").and_then(Value::as_str) == Some(HOOK_CMD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{dispatch, types::HookInput};
    use crate::plugin::Ctx;
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
    fn dry_run_then_apply_is_idempotent() {
        let dir = tmp("setup");
        let path = dir.join("hooks.json");
        let dry = run(&cfg(path.clone(), true), false).unwrap();
        assert!(dry.contains("beforeShellExecution"), "{dry}");
        assert!(!path.exists());
        let c = cfg(path.clone(), false);
        assert!(run(&c, false).unwrap().contains(HOOK_CMD));
        assert_eq!(run(&c, false).unwrap(), "no changes");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\""));
        assert!(raw.contains(HOOK_CMD));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_payload_wraps_command() {
        let dir = tmp("wrap");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let cx = Ctx::open(c, "cur").unwrap();
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
}
