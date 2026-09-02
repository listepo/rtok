//! `rtok setup claude --replace` (plan T9.3): drop legacy token hooks, retarget the proxy.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::config::Config;

const LEGACY: &[&str] = &[
    "rtk hook",
    "lean-ctx hook",
    "caveman-proxy",
    "caveman shrink-hook",
    "python-launcher.sh",
];
const DROP_MCP: &[&str] = &["lean-ctx", "code-review-graph"];

/// Apply `--replace` to Claude settings (and `~/.claude.json` MCP map when present).
pub fn run(cfg: &Config) -> Result<String> {
    if !cfg.setup.dry_run && !cfg.setup.yes {
        bail!("--replace requires --yes (or --dry-run)");
    }
    let path = &cfg.setup.claude.settings_path;
    let mut root = read_json(path)?;
    let mut diff = apply(&mut root);
    let claude_json = &cfg.doctor.claude_json;
    if claude_json.exists() && claude_json != path {
        let mut mcp_root = read_json(claude_json)?;
        let mcp_diff = strip_mcp(&mut mcp_root);
        if !mcp_diff.is_empty() {
            diff.push_str(&format!("\n{mcp_diff}"));
            if !cfg.setup.dry_run {
                write_json(claude_json, &mcp_root, cfg.setup.backup)?;
            }
        }
    }
    if !cfg.setup.dry_run {
        write_json(path, &root, cfg.setup.backup)?;
    }
    if diff.is_empty() {
        Ok("no changes".into())
    } else {
        Ok(diff)
    }
}

fn apply(root: &mut Value) -> String {
    let mut lines = Vec::new();
    lines.extend(strip_hooks(root));
    if let Some(line) = retarget_proxy(root) {
        lines.push(line);
    }
    let mcp = strip_mcp(root);
    if !mcp.is_empty() {
        lines.push(mcp);
    }
    let rtok = count_rtok(root);
    lines.push(format!("remaining rtok hooks: {rtok}"));
    lines.join("\n")
}

fn strip_hooks(root: &mut Value) -> Vec<String> {
    let mut removed = Vec::new();
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return removed;
    };
    for (_, arr) in hooks.iter_mut() {
        let Some(entries) = arr.as_array_mut() else {
            continue;
        };
        entries.retain_mut(|entry| {
            let Some(list) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                return !is_legacy_cmd(entry["command"].as_str().unwrap_or(""));
            };
            list.retain(|h| {
                let cmd = h["command"].as_str().unwrap_or("");
                if is_legacy_cmd(cmd) {
                    removed.push(format!("- {cmd}"));
                    false
                } else {
                    true
                }
            });
            !list.is_empty()
        });
    }
    removed
}

fn is_legacy_cmd(cmd: &str) -> bool {
    LEGACY.iter().any(|p| cmd.contains(p))
}

fn retarget_proxy(root: &mut Value) -> Option<String> {
    let url = root
        .pointer("/env/ANTHROPIC_BASE_URL")?
        .as_str()?
        .to_string();
    if !url.contains("8788") && !url.contains("8787") {
        return None;
    }
    let next = url.replace("8788", "8790").replace("8787", "8790");
    root["env"]["ANTHROPIC_BASE_URL"] = json!(next.clone());
    Some(format!("ANTHROPIC_BASE_URL {url} -> {next}"))
}

fn strip_mcp(root: &mut Value) -> String {
    let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return String::new();
    };
    let mut dropped = Vec::new();
    for name in DROP_MCP {
        if servers.remove(*name).is_some() {
            dropped.push((*name).to_string());
        }
    }
    if dropped.is_empty() {
        String::new()
    } else {
        format!("disabled MCP: {}", dropped.join(", "))
    }
}

fn count_rtok(root: &Value) -> usize {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return 0;
    };
    hooks
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .flat_map(|e| {
            e.get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|h| h["command"].as_str().unwrap_or("").contains("rtok hook"))
        .count()
}

fn read_json(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).with_context(|| path.display().to_string())
}

fn write_json(path: &Path, root: &Value, do_backup: bool) -> Result<()> {
    if do_backup && path.exists() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let bak = path.with_file_name(format!("{name}.bak-{ts}"));
        fs::copy(path, &bak).with_context(|| bak.display().to_string())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    fs::write(path, serde_json::to_string_pretty(root)? + "\n")
        .with_context(|| path.display().to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> Value {
        json!({
            "env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8788"},
            "mcpServers": {
                "lean-ctx": {"command": "lean-ctx"},
                "code-review-graph": {"command": "crg"},
                "serena": {"command": "serena"}
            },
            "hooks": {
                "PreToolUse": [
                    {"matcher":"Bash","hooks":[{"type":"command","command":"rtok hook PreToolUse"}]},
                    {"matcher":"Read","hooks":[{"type":"command","command":"rtok hook PreToolUse"}]},
                    {"matcher":"Bash","hooks":[{"type":"command","command":"rtk hook PreToolUse"}]},
                    {"matcher":"Bash","hooks":[{"type":"command","command":"lean-ctx hook PreToolUse"}]}
                ],
                "PostToolUse": [
                    {"matcher":"*","hooks":[{"type":"command","command":"rtok hook PostToolUse"}]},
                    {"hooks":[{"type":"command","command":"caveman-proxy"}]}
                ],
                "UserPromptSubmit": [
                    {"hooks":[{"type":"command","command":"rtok hook UserPromptSubmit"}]},
                    {"hooks":[{"type":"command","command":"caveman shrink-hook"}]}
                ],
                "SessionStart": [
                    {"hooks":[{"type":"command","command":"rtok hook SessionStart"}]},
                    {"hooks":[{"type":"command","command":"orca hook"}]}
                ],
                "PreCompact": [{"hooks":[{"type":"command","command":"rtok hook PreCompact"}]}],
                "PostCompact": [{"hooks":[{"type":"command","command":"rtok hook PostCompact"}]}],
                "Stop": [{"hooks":[{"type":"command","command":"rtok hook Stop"}]}],
                "Notification": [
                    {"hooks":[{"type":"command","command":"/opt/token-optimizer/python-launcher.sh"}]},
                    {"hooks":[{"type":"command","command":"holdmylid"}]},
                    {"hooks":[{"type":"command","command":"tokenbar"}]},
                    {"hooks":[{"type":"command","command":"cbm"}]}
                ]
            }
        })
    }

    #[test]
    fn dry_run_keeps_eight_rtok_and_non_token_hooks() {
        let mut root = today();
        let report = apply(&mut root);
        assert!(report.contains("remaining rtok hooks: 8"), "{report}");
        assert_eq!(count_rtok(&root), 8);
        let raw = serde_json::to_string(&root).unwrap();
        assert!(serde_json::from_str::<Value>(&raw).is_ok());
        assert!(raw.contains("rtok hook"));
        for keep in ["orca", "holdmylid", "tokenbar", "cbm", "serena"] {
            assert!(raw.contains(keep), "kept {keep}");
        }
        for gone in [
            "rtk hook",
            "lean-ctx hook",
            "caveman-proxy",
            "caveman shrink-hook",
            "python-launcher.sh",
        ] {
            assert!(!raw.contains(gone), "removed {gone}");
        }
        assert!(!raw.contains("\"lean-ctx\""));
        assert!(!raw.contains("code-review-graph"));
        assert_eq!(root["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:8790");
    }

    #[test]
    fn replace_without_yes_is_err() {
        let mut c = Config::default();
        c.setup.dry_run = false;
        c.setup.yes = false;
        assert!(run(&c).unwrap_err().to_string().contains("--yes"));
    }
}
