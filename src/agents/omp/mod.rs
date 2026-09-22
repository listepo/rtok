//! oh my pi installer (`rtok agents install omp`, plan T92.2, D21).
//!
//! omp is a pi fork: its loader discovers a symlinked directory whose `package.json`
//! declares legacy `pi.extensions`, so `plugins/pi` is linked as is — no `plugins/omp/`.
//! The extension owns the bash call path, context and compaction; unlike pi, omp has
//! native MCP, so the tools come from `rtok mcp` in `mcp.json` and the extension's
//! `registerTool` stays off under omp (one call path per capability). Missing `rtok`
//! fails open and names the ketch install.

use std::path::PathBuf;

use serde_json::json;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;
use anyhow::Result;

/// oh my pi: the linked pi extension plus `mcpServers.rtok`.
pub struct Omp;

/// No desktop app: Zed ACP runs the same binary and config.
static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "oh my pi",
    bins: &["omp"],
    apps: &[],
}];

const NAME: &str = "rtok";

impl Agent for Omp {
    fn id(&self) -> &'static str {
        "omp"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "plugin" => Support::Flag("--yes"),
            "mcp" => Support::Yes,
            "hooks" => Support::No(
                "omp hooks are in-process TS modules; the linked pi extension owns that path",
            ),
            _ => {
                Support::No("omp provider base URLs live in models.yml, which setup does not edit")
            }
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[rtok_plugin_sdk::Surface::Cli]
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.omp.mcp_path.clone()]
    }

    fn markers(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![plugin_dest(cfg), cfg.setup.omp.extensions_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        // T75: a foreign directory at the plugin dest must not hold the green mark.
        if PLUGIN.ours(cfg) {
            out.push("plugin");
        }
        if super::read(&cfg.setup.omp.mcp_path).contains("\"rtok\"") {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = vec![PLUGIN.offer(cfg, remove)?];
        if remove {
            lines.push(unregister_mcp(cfg)?);
        } else if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        Ok(lines)
    }
}

/// Offer / link / unlink `plugins/pi` into omp's extensions directory. Dry-run and the
/// unaccepted offer MUST contain `plugins/pi` and `ketch install listepo/rtok`.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/pi",
    host: "oh my pi",
    label: None,
    dest: plugin_dest,
    // Out of scope for T164: left asking with `--yes`, unlike the five local-link-only
    // hosts (omp is a pi fork with its own README and its own creator decision pending).
    default_install: false,
};

/// Extension dest: `<extensions_path>/rtok` (default `~/.omp/agent/extensions/rtok`).
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.omp.extensions_path.join(NAME)
}

/// `mcpServers.rtok = {command, args}` — omp's documented shape carries no `type`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &cfg.setup.omp.mcp_path,
        "mcpServers",
        NAME,
        json!({"command": cmd, "args": ["mcp"]}),
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcpServers.rtok`, keeping every foreign server.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_server(&apply(cfg), &cfg.setup.omp.mcp_path, "mcpServers", NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-omp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(dir: &std::path::Path, dry: bool, yes: bool) -> Config {
        let mut c = Config::default();
        c.setup.omp.extensions_path = dir.join("extensions");
        c.setup.omp.mcp_path = dir.join("mcp.json");
        c.setup.dry_run = dry;
        c.setup.yes = yes;
        c.setup.backup = false;
        c
    }

    #[test]
    fn dry_run_offers_the_pi_extension_and_writes_nothing() {
        let dir = tmp("dry");
        let c = cfg(&dir, true, false);
        let lines = Omp.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(lines[0].contains("plugins/pi"), "{lines:?}");
        assert!(lines[0].contains("ketch install listepo/rtok"), "{lines:?}");
        assert!(!plugin_dest(&c).exists());
        assert!(!c.setup.omp.mcp_path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_and_registers_then_remove_takes_back_only_ours() {
        let dir = tmp("yes");
        let c = cfg(&dir, false, true);
        fs::write(
            &c.setup.omp.mcp_path,
            r#"{"mcpServers":{"other":{"command":"other","args":[]}}}"#,
        )
        .unwrap();
        let first = Omp.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(first[0].starts_with("+ plugin"), "{first:?}");
        assert!(PLUGIN.linked(&c));
        let doc: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&c.setup.omp.mcp_path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["rtok"]["args"], json!(["mcp"]));
        assert!(doc["mcpServers"]["rtok"].get("type").is_none());
        assert_eq!(Omp.installed(&c, Kind::Cli), vec!["plugin", "mcp"]);

        let again = Omp.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(again, vec![NO_CHANGES, NO_CHANGES]);

        Omp.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert!(!PLUGIN.linked(&c));
        let doc: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&c.setup.omp.mcp_path).unwrap()).unwrap();
        assert!(doc["mcpServers"].get("rtok").is_none(), "{doc}");
        assert!(doc["mcpServers"].get("other").is_some(), "{doc}");
        assert!(Omp.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_foreign_directory_at_the_dest_is_not_installed() {
        let dir = tmp("foreign");
        let c = cfg(&dir, false, true);
        fs::create_dir_all(plugin_dest(&c)).unwrap();
        assert!(!Omp.installed(&c, Kind::Cli).contains(&"plugin"));
        let _ = fs::remove_dir_all(dir);
    }
}
