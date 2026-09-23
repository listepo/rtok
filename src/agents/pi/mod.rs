//! pi installer (`rtok agents install pi`, plan T10.6, D21).
//!
//! pi philosophy is no MCP: the plugin owns the bash call path
//! (`rtok run` / `rtok filter`) and, when `[setup.pi] tools` is true,
//! `pi.registerTool` wrappers around `rtok mcp --call` (T70.3). Desktop
//! and CLI see the same `~/.pi/agent/extensions` tree. Missing `rtok`
//! fails open and names the ketch install.

use std::path::PathBuf;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant};
use crate::config::Config;
use anyhow::Result;

/// pi: one linked extension under `~/.pi/agent/extensions`, nothing else.
pub struct Pi;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "pi",
    bins: &["pi"],
    apps: &[],
}];

impl Agent for Pi {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "plugin" => Support::Yes,
            "hooks" => Support::No("pi has no hook events; the extension owns the bash call path"),
            "mcp" => Support::No(
                "pi philosophy is no MCP; registerTool is the plugin path when setup.pi.tools is true",
            ),
            _ => Support::No(
                "pi provider base URLs live in its models config, which setup does not edit",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[rtok_plugin_sdk::Surface::Cli, rtok_plugin_sdk::Surface::Mcp]
    }

    fn files(&self, _cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        Vec::new()
    }

    fn markers(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![plugin_dest(cfg), cfg.setup.pi.extensions_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        // T75: only what remove will take back counts as installed — a foreign
        // directory at the plugin dest must not hold the green mark.
        if PLUGIN.ours(cfg) {
            vec!["plugin"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        Ok(vec![
            PLUGIN.offer(cfg, remove)?,
            super::skill::sync("pi", cfg, remove)?,
        ])
    }
}

const PLUGIN_DIR_NAME: &str = "rtok";

/// Offer / link / unlink `plugins/pi` (D21, T10.6).
/// Dry-run and the unaccepted offer MUST contain the substrings `plugins/pi`
/// and `ketch install listepo/rtok`.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/pi",
    host: "pi",
    label: None,
    dest: plugin_dest,
    // pi's own docs have no GitHub/subdir plugin install; the local link is the only path
    // there is, so it installs by default once pi itself is detected (T164).
    default_install: true,
};

/// Extension dest: `<extensions_path>/rtok` (default `~/.pi/agent/extensions/rtok`).
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.pi.extensions_path.join(PLUGIN_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-pi-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(ext: PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.pi.extensions_path = ext;
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    #[test]
    fn dry_run_offer_names_plugin_and_ketch() {
        let dir = tmp("offer-dry");
        let c = cfg(dir.join("extensions"), true);
        let s = PLUGIN.offer(&c, false).unwrap();
        assert!(s.contains("plugins/pi"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_dest(&c).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_plugin_second_apply_no_changes() {
        let dir = tmp("offer-yes");
        let mut c = cfg(dir.join("extensions"), false);
        c.setup.yes = true;
        c.setup.backup = false;
        let first = PLUGIN.offer(&c, false).unwrap();
        assert!(first.starts_with("+ plugin"), "{first}");
        assert!(PLUGIN.linked(&c));
        assert_eq!(PLUGIN.offer(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(
            PLUGIN.offer(&c, true).unwrap(),
            format!("- plugin {}", plugin_dest(&c).display())
        );
        assert!(!PLUGIN.linked(&c));
        let _ = fs::remove_dir_all(dir);
    }
}
