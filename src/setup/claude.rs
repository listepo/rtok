//! Install rtok hooks into Claude Code `settings.json` (plan T2.3).

use crate::config::Config;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let mut root = read_settings(path)?;
    let report = if remove {
        strip_ours(&mut root)
    } else {
        insert_ours(&mut root, cfg.setup.hook_timeout_s)
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

fn read_settings(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).with_context(|| path.display().to_string())
}

fn backup(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let bak = path.with_file_name(format!("{name}.bak-{ts}"));
    fs::copy(path, &bak).with_context(|| bak.display().to_string())?;
    Ok(())
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
        "no changes".into()
    } else {
        format!("{}\n{} additions", added.join("\n"), added.len())
    }
}

fn strip_ours(root: &mut Value) -> String {
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return "no changes".into();
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
        "no changes".into()
    } else {
        format!("{removed} removed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn apply_twice_then_remove_keeps_foreign() {
        let path = tmp("setup-apply");
        fs::write(&path, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo other"}]}]}}"#).unwrap();
        let first = run(&cfg(path.clone(), false), false).unwrap();
        assert!(first.contains("7 additions"), "{first}");
        assert_eq!(run(&cfg(path.clone(), false), false).unwrap(), "no changes");
        let rm = run(&cfg(path.clone(), false), true).unwrap();
        assert!(rm.contains("removed"), "{rm}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok hook"));
    }
}
