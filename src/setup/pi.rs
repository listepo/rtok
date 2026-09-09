//! pi installer (`rtok agent setup pi`, plan T10.6, D21).
//!
//! pi philosophy is no MCP: the plugin owns the bash call path only —
//! `tool_call` bash rewrites to `rtok run -- …`, `tool_result` bash
//! compresses through `rtok filter`. No `read`/`search` tools, no second
//! registration, desktop and CLI see the same `~/.pi/agent/extensions`
//! tree. Missing `rtok` fails open and names the ketch install.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::PluginLink;

use super::{apply, plugin_src};
use crate::config::Config;

const PLUGIN_SRC_REL: &str = "plugins/pi";
const PLUGIN_DIR_NAME: &str = "rtok";

/// Extension dest: `<extensions_path>/rtok` (default `~/.pi/agent/extensions/rtok`).
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.pi.extensions_path.join(PLUGIN_DIR_NAME)
}

fn link(cfg: &Config) -> PluginLink<'static> {
    PluginLink {
        src_rel: PLUGIN_SRC_REL,
        src: plugin_src(PLUGIN_SRC_REL),
        dest: plugin_dest(cfg),
        label: None,
        host: "pi",
    }
}

/// Offer / link / unlink `plugins/pi` (D21, T10.6).
/// Dry-run and the unaccepted offer MUST contain the substrings `plugins/pi`
/// and `ketch install listepo/rtok`.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    link(cfg).run(&apply(cfg), remove)
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
        let s = offer_plugin(&c, false).unwrap();
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
        let first = offer_plugin(&c, false).unwrap();
        assert!(first.starts_with("+ plugin"), "{first}");
        assert!(link(&c).linked());
        assert_eq!(offer_plugin(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(
            offer_plugin(&c, true).unwrap(),
            format!("- plugin {}", plugin_dest(&c).display())
        );
        assert!(!link(&c).linked());
        let _ = fs::remove_dir_all(dir);
    }
}
