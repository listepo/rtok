//! Install rtok hooks into Claude Code `settings.json` (plan T2.3).
//!
//! What is Claude-specific is the `hooks` shape below; backup, the write gate and the
//! `mcpServers` entry come from `rtok-agent-sdk` (D28).

use crate::config::Config;
use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, read_json, write_json};
use serde_json::{Value, json};

use super::apply;

/// `(event, matcher)` — empty matcher omits the field.
const ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "Bash"),
    ("PreToolUse", "Read"),
    ("PostToolUse", "*"),
    ("UserPromptSubmit", ""),
    ("SessionStart", ""),
    ("PreCompact", ""),
    ("PostCompact", ""),
];

fn command(event: &str) -> String {
    format!("rtok hook {event}")
}

fn is_ours(cmd: &str, event: &str) -> bool {
    cmd.split_whitespace()
        .collect::<Vec<_>>()
        .windows(3)
        .any(|w| w == ["rtok", "hook", event])
}

/// Apply, dry-run, or remove rtok hook entries.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.claude.settings_path;
    let mut root = read_json(path)?;
    let report = if remove {
        strip_ours(&mut root)
    } else {
        insert_ours(&mut root, cfg.setup.hook_timeout_s)
    };
    write_json(&apply(cfg), path, &root, &report)?;
    Ok(report)
}

fn event_array<'a>(root: &'a mut Value, event: &str) -> &'a mut Vec<Value> {
    let key = event.to_string();
    let hooks = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();
    let arr = hooks.entry(key).or_insert_with(|| json!([]));
    if !arr.is_array() {
        *arr = json!([]);
    }
    arr.as_array_mut().unwrap()
}

fn has_ours(entry: &Value, event: &str, matcher: &str) -> bool {
    let got = entry.get("matcher").and_then(Value::as_str).unwrap_or("");
    got == matcher
        && entry
            .get("hooks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|h| h.get("command").and_then(Value::as_str))
            .any(|c| is_ours(c, event))
}

fn insert_ours(root: &mut Value, timeout: u64) -> String {
    if !root.is_object() {
        *root = json!({});
    }
    let mut added = Vec::new();
    for &(event, matcher) in ENTRIES {
        if event_array(root, event)
            .iter()
            .any(|e| has_ours(e, event, matcher))
        {
            continue;
        }
        let mut obj = serde_json::Map::new();
        if !matcher.is_empty() {
            obj.insert("matcher".into(), json!(matcher));
        }
        obj.insert(
            "hooks".into(),
            json!([{"type":"command","command":command(event),"timeout":timeout}]),
        );
        event_array(root, event).push(Value::Object(obj));
        let m = if matcher.is_empty() {
            String::new()
        } else {
            format!(" {matcher}")
        };
        added.push(format!("+ {event}{m} {}", command(event)));
    }
    if added.is_empty() {
        NO_CHANGES.into()
    } else {
        format!("{}\n{} additions", added.join("\n"), added.len())
    }
}

fn strip_ours(root: &mut Value) -> String {
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return NO_CHANGES.into();
    };
    let mut removed = 0usize;
    for (event, entries) in hooks.iter_mut() {
        let Some(arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in arr.iter_mut() {
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            let n = inner.len();
            inner.retain(|h| {
                !h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| is_ours(c, event))
            });
            removed += n - inner.len();
        }
        arr.retain(|e| {
            e.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|a| !a.is_empty())
        });
    }
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if removed == 0 {
        NO_CHANGES.into()
    } else {
        format!("{removed} removed")
    }
}

/// Add `rtok mcp` to `mcpServers` in `~/.claude.json` (T4.7).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::register_mcp(
        &apply(cfg),
        &cfg.doctor.claude_json,
        "rtok",
        "rtok",
        &["mcp"],
    )
}

/// Drop `mcpServers.rtok` from `~/.claude.json` (`rtok agent remove claude`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_mcp(&apply(cfg), &cfg.doctor.claude_json, "rtok")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(path: std::path::PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.claude.settings_path = path;
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn dry_run_empty_is_seven_additions() {
        let path = tmp("setup-dry");
        let report = run(&cfg(path.clone(), true), false).unwrap();
        assert!(report.contains("7 additions"), "{report}");
        assert!(!path.exists());
    }

    #[test]
    fn mcp_dry_run_then_apply_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("rtok-mcp-setup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claude.json");
        let mut c = Config::default();
        c.doctor.claude_json = path.clone();
        c.setup.backup = false;
        c.setup.dry_run = true;
        let dry = register_mcp(&c).unwrap();
        assert!(dry.contains("mcpServers.rtok"), "{dry}");
        assert!(!path.exists());
        c.setup.dry_run = false;
        assert!(register_mcp(&c).unwrap().contains("rtok mcp"));
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"mcp\""), "{raw}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_twice_then_remove_keeps_foreign() {
        let path = tmp("setup-apply");
        fs::write(&path, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo other"}]}]}}"#).unwrap();
        let first = run(&cfg(path.clone(), false), false).unwrap();
        assert!(first.contains("7 additions"), "{first}");
        assert_eq!(run(&cfg(path.clone(), false), false).unwrap(), NO_CHANGES);
        let rm = run(&cfg(path.clone(), false), true).unwrap();
        assert!(rm.contains("removed"), "{rm}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok hook"));
    }
}
