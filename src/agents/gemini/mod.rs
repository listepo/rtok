//! Gemini CLI installer (`rtok agents install gemini`, plan T118.2).
//!
//! Gemini CLI reads one file, `[setup.gemini] dir`/`settings.json` (default
//! `~/.gemini/settings.json`, https://geminicli.com/docs/cli/settings/, fetched 2026-09-24):
//! hooks live under `hooks.<Event>[]` — Gemini's own event names, the reverse of
//! `hooks::types::gemini_event` (T118.1) — as `{hooks: [{type: "command", command, timeout}]}`,
//! and `mcpServers.<name>` sits beside it (https://geminicli.com/docs/tools/mcp-server/). No
//! `matcher` is written: Gemini's matcher filters on its *own* tool-name spelling
//! (`run_shell_command`, `read_file`, …), so a Claude-shaped matcher list (`Bash`, `Read`)
//! would silently never fire — every event runs on every call instead, the way `PostToolUse`
//! already does on every other host. The extension tree (`gemini extensions install/link`,
//! `plugins/gemini/`) is T118.3; until then this host writes the files directly.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Gemini's own event names paired with the Claude event `rtok hook` runs for them (the
/// reverse of `hooks::types::gemini_event`). `AfterAgent`/`BeforeModel`/`BeforeToolSelection`/
/// `AfterModel`/`Notification` have no rtok plugin hook (T118.1) and are left uninstalled.
pub const EVENTS: &[(&str, &str)] = &[
    ("BeforeTool", "PreToolUse"),
    ("AfterTool", "PostToolUse"),
    ("BeforeAgent", "UserPromptSubmit"),
    ("SessionStart", "SessionStart"),
    ("SessionEnd", "SessionEnd"),
    ("PreCompress", "PreCompact"),
];

/// Gemini CLI: one `settings.json`, no separate desktop app.
pub struct Gemini;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "Gemini CLI",
    bins: &["gemini"],
    apps: &[],
}];

/// `<dir>/settings.json`.
pub fn settings_path(cfg: &Config) -> PathBuf {
    cfg.setup.gemini.dir.join("settings.json")
}

fn command(bin: &str, claude_event: &str) -> String {
    format!("{bin} hook {claude_event} --host gemini")
}

/// Exactly `<rtok-bin> hook <event> --host gemini`, matching only the tail so an absolute
/// path (Windows) still counts.
fn is_ours(cmd: &str, claude_event: &str) -> bool {
    let suffix = format!(" hook {claude_event} --host gemini");
    cmd.strip_suffix(&suffix)
        .is_some_and(|bin| super::is_rtok_bin(super::unquote_bin(bin)))
}

fn has_ours(entry: &Value, claude_event: &str) -> bool {
    let Some(cmds) = entry.get("hooks").and_then(Value::as_array) else {
        return false;
    };
    cmds.iter()
        .filter_map(|h| h.get("command").and_then(Value::as_str))
        .any(|c| is_ours(c, claude_event))
}

/// Apply, dry-run, or remove rtok's `hooks.<Event>[]` entries; foreign entries survive.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &settings_path(cfg), |root| {
        let hooks = object_at(root, "hooks");
        if remove {
            strip_ours(hooks)
        } else {
            insert_ours(hooks, &super::rtok_hook_bin(), cfg.setup.hook_timeout_s)
        }
    })
}

fn insert_ours(hooks: &mut Value, bin: &str, timeout_s: u64) -> String {
    let mut added = Vec::new();
    let mut updated = 0usize;
    for &(gevent, cevent) in EVENTS {
        let cmd = command(bin, cevent);
        let ms = timeout_s * 1000;
        // T242.5: an rtok hook on another binary path or timeout is rewritten in its slot.
        let mut found = false;
        for entry in array_at(hooks, gevent).iter_mut() {
            for h in entry["hooks"].as_array_mut().into_iter().flatten() {
                if !h["command"].as_str().is_some_and(|c| is_ours(c, cevent)) {
                    continue;
                }
                found = true;
                if h["command"] != json!(cmd) || h["timeout"] != json!(ms) {
                    h["command"] = json!(cmd);
                    h["timeout"] = json!(ms);
                    added.push(format!("~ {gevent} {cmd}"));
                    updated += 1;
                }
            }
        }
        if found {
            continue;
        }
        let entry = json!({"hooks": [{"type": "command", "command": cmd, "timeout": ms}]});
        array_at(hooks, gevent).push(entry);
        added.push(format!("+ {gevent} {cmd}"));
    }
    if added.is_empty() {
        NO_CHANGES.into()
    } else {
        let counts = [(added.len() - updated, "additions"), (updated, "updates")]
            .iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, what)| format!("{n} {what}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}\n{counts}", added.join("\n"))
    }
}

fn strip_ours(hooks: &mut Value) -> String {
    let Some(obj) = hooks.as_object_mut() else {
        return NO_CHANGES.into();
    };
    let mut removed = 0usize;
    for &(gevent, cevent) in EVENTS {
        let Some(arr) = obj.get_mut(gevent).and_then(Value::as_array_mut) else {
            continue;
        };
        let n = arr.len();
        arr.retain(|e| !has_ours(e, cevent));
        removed += n - arr.len();
    }
    obj.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if removed == 0 {
        NO_CHANGES.into()
    } else {
        format!("{removed} removed")
    }
}

/// The `mcpServers.rtok` entry [`register_mcp`] writes.
fn mcp_entry(cmd: &str) -> Value {
    json!({"command": cmd, "args": ["mcp"]})
}

/// `mcpServers.rtok = {command, args}` — the minimal stdio shape the Gemini MCP docs show.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let bin = super::rtok_command();
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &settings_path(cfg),
        "mcpServers",
        NAME,
        mcp_entry(&bin),
        &format!("{bin} mcp"),
    )
}

/// Drop `mcpServers.rtok` from `settings.json`, unless the user edited it (T246.2).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    super::unregister_ours(
        cfg,
        &settings_path(cfg),
        "mcpServers",
        NAME,
        &mcp_entry("rtok"),
    )
}

impl Agent for Gemini {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "proxy" => Support::No(
                "Gemini CLI has no documented base-URL setting; its own HTTP_PROXY/HTTPS_PROXY covers MCP server transport only, not the model API",
            ),
            "hooks" | "mcp" => Support::Yes,
            _ => Support::No(
                "the extension tree ships in T118.3; no local plugin directory to link yet",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[
            rtok_plugin_sdk::Surface::Mcp,
            rtok_plugin_sdk::Surface::Hook,
        ]
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![settings_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let text = super::read(&settings_path(cfg));
        let needles = [
            ("hooks", " hook PreToolUse --host gemini"),
            ("mcp", "\"rtok\""),
        ];
        needles
            .into_iter()
            .filter_map(|(module, needle)| text.contains(needle).then_some(module))
            .collect()
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = vec![run(cfg, remove)?];
        let mcp_line = if remove {
            Some(unregister_mcp(cfg)?)
        } else if cfg.setup.mcp {
            Some(register_mcp(cfg)?)
        } else {
            None
        };
        lines.extend(mcp_line);
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-gemini-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.setup.gemini.dir = dir.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, dir)
    }

    #[test]
    fn dry_run_names_six_events_and_creates_nothing() {
        let (c, dir) = cfg("dry", true);
        let out = run(&c, false).unwrap();
        assert!(
            out.starts_with("+ BeforeTool ") && out.contains("6 additions"),
            "{out}"
        );
        assert!(!settings_path(&c).exists());
        assert!(Gemini.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_is_idempotent_and_remove_keeps_foreign() {
        let (c, dir) = cfg("apply", false);
        fs::write(
            settings_path(&c),
            json!({
                "hooks": {"Notification": [{"hooks": [{"type": "command", "command": "echo hi"}]}]},
                "mcpServers": {"other": {"command": "npx", "args": ["x"]}}
            })
            .to_string(),
        )
        .unwrap();
        assert!(register_mcp(&c).unwrap().starts_with("mcpServers.rtok: "));
        assert!(run(&c, false).unwrap().contains("6 additions"));
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);

        let raw_after_install = fs::read_to_string(settings_path(&c)).unwrap();
        let root = serde_json::from_str::<Value>(&raw_after_install).unwrap();
        let before = &root["hooks"]["BeforeTool"][0];
        assert!(before.get("matcher").is_none(), "{before}");
        let cmd = before["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.ends_with("rtok hook PreToolUse --host gemini"), "{cmd}");
        assert_eq!(before["hooks"][0]["timeout"], 5000);
        assert!(
            root["hooks"]["Notification"].is_array(),
            "foreign event stays"
        );
        assert_eq!(root["mcpServers"]["rtok"]["args"][0], "mcp");
        assert_eq!(root["mcpServers"]["other"]["command"], "npx");
        assert_eq!(Gemini.installed(&c, Kind::Cli), ["hooks", "mcp"]);

        assert!(run(&c, true).unwrap().contains("removed"));
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcpServers.rtok");
        let raw = fs::read_to_string(settings_path(&c)).unwrap();
        assert!(!raw.contains("rtok hook"), "{raw}");
        let root: Value = serde_json::from_str(&raw).unwrap();
        assert!(
            root["hooks"]["Notification"].is_array(),
            "foreign event survives remove"
        );
        assert_eq!(root["mcpServers"]["other"]["command"], "npx");
        assert!(Gemini.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn support_matches_hooks_mcp_yes_proxy_and_plugin_no() {
        assert!(matches!(Gemini.support(Kind::Cli, "hooks"), Support::Yes));
        assert!(matches!(Gemini.support(Kind::Cli, "mcp"), Support::Yes));
        assert!(matches!(Gemini.support(Kind::Cli, "proxy"), Support::No(_)));
        assert!(matches!(
            Gemini.support(Kind::Cli, "plugin"),
            Support::No(_)
        ));
    }

    /// T242.5: an rtok hook on another binary path and timeout is rewritten in its slot, a
    /// foreign hook in the same entry stays, and a second pass changes nothing.
    #[test]
    fn stale_rtok_hook_is_rewritten_in_place() {
        let (gevent, cevent) = EVENTS[0];
        let stale = command("/old/store/rtok/v0.1.0/rtok", cevent);
        let mut hooks = json!({gevent: [{"matcher": "run_shell_command", "hooks": [
            {"type": "command", "command": "audit.sh"},
            {"type": "command", "command": stale, "timeout": 1000}
        ]}]});
        let out = insert_ours(&mut hooks, "rtok", 5);
        let want = command("rtok", cevent);
        assert!(out.contains(&format!("~ {gevent} {want}")), "{out}");
        assert!(out.ends_with(" updates"), "{out}");
        let entries = hooks[gevent].as_array().unwrap();
        assert_eq!(entries.len(), 1, "no second entry: {hooks}");
        assert_eq!(entries[0]["matcher"], "run_shell_command");
        assert_eq!(entries[0]["hooks"][0]["command"], "audit.sh");
        assert_eq!(entries[0]["hooks"][1]["command"], want.as_str());
        assert_eq!(entries[0]["hooks"][1]["timeout"], 5000);
        let current = hooks.clone();
        assert_eq!(insert_ours(&mut hooks, "rtok", 5), NO_CHANGES);
        assert_eq!(hooks, current);
    }
}
