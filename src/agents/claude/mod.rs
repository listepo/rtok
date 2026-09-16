//! Install rtok hooks into Claude Code `settings.json` (plan T2.3).
//!
//! What is Claude-specific is the `hooks` shape below; backup, the write gate and the
//! `mcpServers` entry come from `rtok-agent-sdk` (D28).

pub mod migrate;

use std::path::PathBuf;

use crate::config::Config;
use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};

/// `(event, matcher)` — empty matcher omits the field. `SessionEnd` was missing, so no session
/// row got `ended_at` and no OTel session root ever shipped (`hooks::dispatch` handles it).
const ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "Bash"),
    ("PreToolUse", "Read"),
    ("PostToolUse", "*"),
    ("UserPromptSubmit", ""),
    ("SessionStart", ""),
    ("PreCompact", ""),
    ("PostCompact", ""),
    ("SessionEnd", ""),
];

fn command(event: &str) -> String {
    format!("{} hook {event}", super::rtok_hook_bin())
}

/// Exactly `<rtok-bin> hook <event>`. Matching tokens anywhere claimed a user's
/// chain (`notify-send hi && rtok hook Stop`). Suffix match keeps absolute paths
/// with spaces (quoted) as ours on Windows.
fn is_ours(cmd: &str, event: &str) -> bool {
    let suffix = format!(" hook {event}");
    let Some(bin) = cmd.strip_suffix(&suffix) else {
        return false;
    };
    super::is_rtok_bin(super::unquote_bin(bin))
}

/// Apply, dry-run, or remove rtok hook entries.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &cfg.setup.claude.settings_path, |root| {
        if remove {
            strip_ours(root)
        } else {
            insert_ours(root, cfg.setup.hook_timeout_s)
        }
    })
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
    let hooks = object_at(root, "hooks");
    let mut added = Vec::new();
    for &(event, matcher) in ENTRIES {
        if array_at(hooks, event)
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
        array_at(hooks, event).push(Value::Object(obj));
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
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_mcp(&apply(cfg), &cfg.doctor.claude_json, "rtok", &cmd, &["mcp"])
}

/// Drop `mcpServers.rtok` from `~/.claude.json` (`rtok agents remove claude`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_mcp(&apply(cfg), &cfg.doctor.claude_json, "rtok")
}

/// Claude Code: hooks in `settings.json`, MCP in `~/.claude.json`, the proxy as
/// `env.ANTHROPIC_BASE_URL`.
pub struct Claude;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "Claude Code",
    bins: &["claude"],
    apps: &[],
}];

impl Agent for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "hooks" | "mcp" => Support::Yes,
            "proxy" => Support::Flag("--proxy"),
            _ => Support::No(
                "Claude Code loads hooks and MCP from its own settings; there is no plugin directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![
            cfg.setup.claude.settings_path.clone(),
            cfg.doctor.claude_json.clone(),
        ]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let s = super::read(&cfg.setup.claude.settings_path);
        let m = super::read(&cfg.doctor.claude_json);
        let mut out = Vec::new();
        if s.contains("rtok hook") {
            out.push("hooks");
        }
        if m.contains("\"rtok\"") {
            out.push("mcp");
        }
        // The URL `register_proxy` writes. Matching the default port `8790` anywhere in the
        // file missed a proxy on another `[proxy] port`.
        let base = serde_json::from_str::<Value>(&s)
            .ok()
            .and_then(|v| v["env"]["ANTHROPIC_BASE_URL"].as_str().map(str::to_string));
        if base.as_deref() == Some(super::anthropic_proxy_url(cfg).as_str()) {
            out.push("proxy");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        match mode {
            Mode::Replace => Ok(vec![migrate::run(cfg)?]),
            Mode::Remove => Ok(vec![
                run(cfg, true)?,
                unregister_mcp(cfg)?,
                crate::proxy::cli::unregister_proxy(cfg)?,
            ]),
            Mode::Install => {
                let mut lines = vec![run(cfg, false)?];
                if cfg.setup.mcp {
                    lines.push(register_mcp(cfg)?);
                }
                if cfg.setup.proxy {
                    lines.push(crate::proxy::cli::register_proxy(cfg)?);
                }
                Ok(lines)
            }
        }
    }
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
    fn dry_run_empty_is_eight_additions() {
        let path = tmp("setup-dry");
        let report = run(&cfg(path.clone(), true), false).unwrap();
        assert!(report.contains("8 additions"), "{report}");
        assert!(
            report.contains("+ SessionEnd rtok hook SessionEnd"),
            "{report}"
        );
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
        assert!(first.contains("8 additions"), "{first}");
        assert_eq!(run(&cfg(path.clone(), false), false).unwrap(), NO_CHANGES);
        let rm = run(&cfg(path.clone(), false), true).unwrap();
        assert!(rm.contains("removed"), "{rm}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok hook"));
    }

    /// A user command that merely contains `rtok hook <event>` is theirs: remove keeps it.
    #[test]
    fn remove_keeps_a_user_command_that_chains_rtok() {
        let path = tmp("setup-chain");
        let chain = "notify-send hi && rtok hook PreToolUse";
        fs::write(
            &path,
            json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":chain}]}]}})
                .to_string(),
        )
        .unwrap();
        run(&cfg(path.clone(), false), false).unwrap();
        run(&cfg(path.clone(), false), true).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(chain), "{raw}");
        assert!(!raw.contains("\"rtok hook PreToolUse\""), "{raw}");
    }

    /// A settings file whose `hooks` is not an object is user data, not a reason to panic
    /// (`hooks: []` used to hit `as_object_mut().unwrap()`).
    #[test]
    fn wrong_shaped_hooks_key_is_replaced_not_panicked_on() {
        for body in [
            r#"{"hooks":[]}"#,
            r#"{"hooks":"nope"}"#,
            r#"{"hooks":{"PreToolUse":"nope"}}"#,
        ] {
            let path = tmp(&format!("setup-shape-{}", body.len()));
            fs::write(&path, body).unwrap();
            let report = run(&cfg(path.clone(), false), false).unwrap();
            assert!(report.contains("8 additions"), "{body} → {report}");
            let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert!(root["hooks"]["PreToolUse"].is_array(), "{body}");
        }
    }
}
