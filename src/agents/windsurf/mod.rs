//! Windsurf installer (`rtok agents install windsurf`, plan T48.5).
//!
//! Windsurf's Cascade agent reads MCP servers from `~/.codeium/windsurf/mcp_config.json`
//! (`[setup.windsurf] config_path`): `mcpServers.<name> = {command, args}`, no `type` field
//! on stdio entries. Cascade hooks (`~/.codeium/windsurf/hooks.json`) speak their own
//! protocol (`agent_action_name` / `tool_info` stdin over twelve Cascade events), so `rtok
//! hook` cannot serve them without a `--host windsurf` mapping (cf. T46.3); setup does not
//! write them.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Windsurf: MCP in one `mcp_config.json`; a desktop app only.
pub struct Windsurf;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Desktop,
    name: "Windsurf",
    bins: &[],
    apps: &[
        "/Applications/Windsurf.app",
        "$LOCALAPPDATA/Programs/Windsurf/Windsurf.exe",
    ],
}];

impl Agent for Windsurf {
    fn id(&self) -> &'static str {
        "windsurf"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "mcp" => Support::Yes,
            "hooks" => Support::No(
                "Cascade hooks speak agent_action_name/tool_info, not hook_event_name/tool_name; rtok hook has no --host windsurf mapping",
            ),
            "proxy" => Support::No(
                "Windsurf serves its own models; there is no documented base-URL setting to point at the proxy",
            ),
            _ => Support::No(
                "Cascade has no local plugin directory to link; rules and memories live in .windsurf/",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.windsurf.config_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        if super::read(&cfg.setup.windsurf.config_path).contains("\"rtok\"") {
            vec!["mcp"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        if mode == Mode::Remove {
            Ok(vec![unregister_mcp(cfg)?])
        } else if cfg.setup.mcp {
            Ok(vec![register_mcp(cfg)?])
        } else {
            Ok(vec![rtok_agent_sdk::NO_CHANGES.into()])
        }
    }
}

/// The `mcpServers.rtok` entry [`register_mcp`] writes.
fn mcp_entry(cmd: &str) -> Value {
    json!({"command": cmd, "args": ["mcp"]})
}

/// `mcpServers.rtok = {command, args}` in `mcp_config.json` — the stdio shape the Windsurf
/// MCP docs show carries no `type`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &cfg.setup.windsurf.config_path,
        "mcpServers",
        NAME,
        mcp_entry(&cmd),
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcpServers.rtok` from `mcp_config.json`, unless the user edited it (T246.2).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    super::unregister_ours(
        cfg,
        &cfg.setup.windsurf.config_path,
        "mcpServers",
        NAME,
        &mcp_entry("rtok"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-windsurf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mcp_config.json");
        let mut c = Config::default();
        c.setup.windsurf.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_names_the_file_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let out = register_mcp(&c).unwrap();
        assert_eq!(
            out,
            format!("mcpServers.rtok: {} mcp", super::super::rtok_command())
        );
        assert!(!path.exists());
        assert!(Windsurf.installed(&c, Kind::Desktop).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_and_remove_keeps_foreign() {
        let (c, path) = cfg("apply", false);
        fs::write(&path, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();
        let first = register_mcp(&c).unwrap();
        assert!(first.starts_with("mcpServers.rtok: "), "{first}");
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("\"type\""), "{raw}");
        assert!(raw.contains("\"mcp\""), "{raw}");
        assert_eq!(Windsurf.installed(&c, Kind::Desktop), ["mcp"]);

        assert_eq!(unregister_mcp(&c).unwrap(), "- mcpServers.rtok");
        assert_eq!(unregister_mcp(&c).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("foreign"), "foreign server stays: {raw}");
        assert!(!raw.contains("rtok"), "{raw}");
        assert!(Windsurf.installed(&c, Kind::Desktop).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
