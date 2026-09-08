//! pi installer (`rtok setup pi`, plan T10.6, D21).
//!
//! pi philosophy is no MCP: the plugin owns the bash call path only —
//! `tool_call` bash rewrites to `rtok run -- …`, `tool_result` bash
//! compresses through `rtok filter`. No `read`/`search` tools, no second
//! registration, desktop and CLI see the same `~/.pi/agent/extensions`
//! tree. Missing `rtok` fails open and names the ketch install.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::Config;

const PLUGIN_SRC_REL: &str = "plugins/pi";
const PLUGIN_DIR_NAME: &str = "rtok";
const KETCH_INSTALL: &str = "ketch install listepo/rtok";

/// Source tree shipped in this repo.
pub fn plugin_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PLUGIN_SRC_REL)
}

/// Extension dest: `<extensions_path>/rtok` (default `~/.pi/agent/extensions/rtok`).
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.pi.extensions_path.join(PLUGIN_DIR_NAME)
}

fn plugin_present(dest: &std::path::Path) -> bool {
    dest.symlink_metadata().is_ok()
}

/// Offer / link / unlink `plugins/pi` (D21, T10.6).
/// Dry-run and the unaccepted offer MUST contain the substrings `plugins/pi`
/// and `ketch install listepo/rtok`.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    let dest = plugin_dest(cfg);
    let src = plugin_src();
    if cfg.setup.dry_run {
        return Ok(format!(
            "offer {PLUGIN_SRC_REL} → {} ({}) {KETCH_INSTALL}",
            dest.display(),
            src.display(),
        ));
    }
    if remove {
        if !plugin_present(&dest) {
            return Ok("no changes".into());
        }
        if dest.is_dir() && !dest.is_symlink() {
            fs::remove_dir_all(&dest)?;
        } else {
            fs::remove_file(&dest)?;
        }
        return Ok(format!("- plugin {}", dest.display()));
    }
    if plugin_present(&dest) {
        return Ok("no changes".into());
    }
    if !cfg.setup.yes {
        return Ok(format!(
            "offer {PLUGIN_SRC_REL} → {} (accept with --yes) {KETCH_INSTALL}",
            dest.display(),
        ));
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).ok();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&src, &dest)
        .with_context(|| format!("symlink {} → {}", src.display(), dest.display()))?;
    #[cfg(not(unix))]
    {
        let _ = (&src, &dest);
        anyhow::bail!("plugin link requires unix");
    }
    Ok(format!("+ plugin {PLUGIN_SRC_REL} → {}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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
        assert!(plugin_present(&plugin_dest(&c)));
        assert_eq!(offer_plugin(&c, false).unwrap(), "no changes");
        assert_eq!(
            offer_plugin(&c, true).unwrap(),
            format!("- plugin {}", plugin_dest(&c).display())
        );
        assert!(!plugin_present(&plugin_dest(&c)));
        let _ = fs::remove_dir_all(dir);
    }
}
