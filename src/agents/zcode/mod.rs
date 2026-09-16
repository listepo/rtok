//! ZCode installer (`rtok agents install zcode`, plan T46.1).
//!
//! Z.ai's ZCode desktop app reads a Claude-compatible hook protocol from
//! `~/.zcode/cli/config.json` — `hooks.enabled`, `hooks.events.<Event>[]` with `timeoutMs` —
//! and MCP from `mcp.servers.<name> = {command, args}`. The hook entries are Claude's helpers;
//! only the shape around them differs. The app starts without a shell PATH, so both carry the
//! absolute `rtok` binary.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, edit_json, object_at};
use serde_json::{Value, json};

use super::claude::{ENTRIES, desktop_command, insert_ours, strip_ours};
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// ZCode: hooks and MCP in one `config.json`; a desktop app only.
pub struct Zcode;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Desktop,
    name: "ZCode",
    bins: &[],
    apps: &[
        "/Applications/ZCode.app",
        "$LOCALAPPDATA/Programs/ZCode/ZCode.exe",
    ],
}];

/// The Claude entries ZCode documents: no PreCompact, PostCompact or SessionEnd events.
fn events() -> &'static [(&'static str, &'static str)] {
    &ENTRIES[..5]
}

const NAME: &str = "rtok";

/// Apply, dry-run, or remove the hook entries under `hooks.events` (and `hooks.enabled`).
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &cfg.setup.zcode.config_path, |root| {
        if remove {
            return strip_ours(root.get_mut("hooks").and_then(|h| h.get_mut("events")));
        }
        let hooks = object_at(root, "hooks");
        let enable = hooks.get("enabled") != Some(&json!(true));
        hooks["enabled"] = json!(true);
        let report = insert_ours(
            object_at(hooks, "events"),
            events(),
            &desktop_command(),
            "timeoutMs",
            cfg.setup.hook_timeout_s * 1000,
        );
        match (enable, report == NO_CHANGES) {
            (false, _) => report,
            (true, true) => "+ hooks.enabled = true".into(),
            (true, false) => format!("+ hooks.enabled = true\n{report}"),
        }
    })
}

/// `mcp.servers.rtok` → `<abs rtok> mcp`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = desktop_command();
    let entry = json!({"command": cmd, "args": ["mcp"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &cfg.setup.zcode.config_path,
        "mcp.servers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcp.servers.rtok` (`rtok agents remove zcode`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_server(
        &apply(cfg),
        &cfg.setup.zcode.config_path,
        "mcp.servers",
        NAME,
    )
}

impl Agent for Zcode {
    fn id(&self) -> &'static str {
        "zcode"
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
            "proxy" => Support::No(
                "ZCode providers are per-id tables with their own keys and base URLs; setup does not edit them",
            ),
            _ => Support::No(
                "ZCode plugins come from the Z.ai marketplace; there is no local plugin directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.zcode.config_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let s = super::read(&cfg.setup.zcode.config_path);
        let mut out = Vec::new();
        // The command is an absolute path, so match the tail rather than a bare `rtok hook`.
        if s.contains(" hook PreToolUse") {
            out.push("hooks");
        }
        let root: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
        if root["mcp"]["servers"][NAME].is_object() {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = vec![run(cfg, remove)?];
        if remove {
            lines.push(unregister_mcp(cfg)?);
        } else if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-zcode-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let mut c = Config::default();
        c.setup.zcode.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_names_five_entries_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let report = run(&c, false).unwrap();
        assert!(report.starts_with("+ hooks.enabled = true\n"), "{report}");
        assert!(report.contains("5 additions"), "{report}");
        assert!(report.contains("+ SessionStart "), "{report}");
        assert!(!report.contains("SessionEnd"), "{report}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_and_remove_keeps_foreign() {
        let (c, path) = cfg("apply", false);
        fs::write(
            &path,
            json!({"hooks":{"enabled":false,"events":{"Stop":[{"hooks":[{"type":"command","command":"echo other"}]}]}},
                   "mcp":{"servers":{"other":{"command":"npx","args":["x"]}}}})
            .to_string(),
        )
        .unwrap();
        assert!(run(&c, false).unwrap().contains("5 additions"));
        assert!(register_mcp(&c).unwrap().starts_with("mcp.servers.rtok: "));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["hooks"]["enabled"], true);
        let pre = &root["hooks"]["events"]["PreToolUse"][0];
        assert_eq!(pre["matcher"], "Bash");
        assert_eq!(pre["hooks"][0]["timeoutMs"], 5000);
        assert!(
            pre["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .ends_with(" hook PreToolUse")
        );
        assert_eq!(root["mcp"]["servers"]["rtok"]["args"][0], "mcp");
        assert_eq!(Zcode.installed(&c, Kind::Desktop), ["hooks", "mcp"]);

        assert!(run(&c, true).unwrap().contains("removed"));
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcp.servers.rtok");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok"), "{raw}");
        let root: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(root["mcp"]["servers"]["other"]["command"], "npx");
        assert!(Zcode.installed(&c, Kind::Desktop).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
