//! The host-plugin offer, declared once (T77, D21).
//!
//! A host plugin is four values — where it comes from in this repo, where it lands under the
//! host's config, how the destination reads to a person, and the host's own name — plus the
//! offer/link/unlink cycle [`rtok_agent_sdk::PluginLink`] already owns (D28). Cursor, OpenCode
//! and pi each spelled those four out beside a private `link()` and an `offer_plugin()` that
//! only forwarded; [`HostPlugin`] is the declaration, so a new host plugin adds a `static` and
//! no cycle of its own.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::PluginLink;

use super::{apply, plugin_src};
use crate::config::Config;

/// One host's plugin: the four values that differ between hosts, and nothing else.
pub struct HostPlugin {
    /// Repo-relative source, named in every report (`plugins/cursor`).
    pub src_rel: &'static str,
    /// The host's name as its users spell it (`Cursor`, `OpenCode`, `pi`).
    pub host: &'static str,
    /// Home-relative label when the real destination reads worse than it (`~/.cursor/plugins/local`).
    pub label: Option<&'static str>,
    /// Where the plugin lands under a given config — the one part that needs the config.
    pub dest: fn(&Config) -> PathBuf,
}

impl HostPlugin {
    /// Where this plugin lands under `cfg`.
    pub fn path(&self, cfg: &Config) -> PathBuf {
        (self.dest)(cfg)
    }

    /// True when anything at all sits at the destination — ours or a host's own.
    pub fn linked(&self, cfg: &Config) -> bool {
        self.link(cfg).linked()
    }

    /// True when the destination is rtok's, i.e. exactly what `remove` will take back (T75).
    /// `installed()` reads this, never bare metadata, so a green mark cannot outlive an uninstall.
    pub fn ours(&self, cfg: &Config) -> bool {
        self.link(cfg).ours()
    }

    /// Offer, link, or unlink the plugin. Dry-run, the backup gate, the `--yes` question and
    /// the "leave a foreign tree alone" rule all stay in the SDK.
    pub fn offer(&self, cfg: &Config, remove: bool) -> Result<String> {
        self.link(cfg).run(&apply(cfg), remove)
    }

    fn link(&self, cfg: &Config) -> PluginLink<'static> {
        PluginLink {
            src_rel: self.src_rel,
            src: plugin_src(self.src_rel),
            dest: (self.dest)(cfg),
            label: self.label,
            host: self.host,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every host plugin declares a distinct source; two hosts sharing one `src_rel` would make
    /// the offer text ambiguous and let one host's remove report another's tree.
    #[test]
    fn declared_host_plugins_have_distinct_sources() {
        let all = [
            &super::super::cursor::PLUGIN,
            &super::super::opencode::PLUGIN,
            &super::super::pi::PLUGIN,
        ];
        let mut seen = Vec::new();
        for p in all {
            assert!(
                p.src_rel.starts_with("plugins/"),
                "{} source is not in plugins/: {}",
                p.host,
                p.src_rel
            );
            assert!(!seen.contains(&p.src_rel), "duplicate source {}", p.src_rel);
            seen.push(p.src_rel);
        }
    }

    /// The descriptor resolves a destination from the config it is handed, not from the
    /// machine's real home — the whole agents test suite depends on that redirection.
    #[test]
    fn path_follows_the_config_it_is_given() {
        let mut cfg = Config::default();
        let home = std::env::temp_dir().join(format!("rtok-hostplugin-{}", std::process::id()));
        cfg.setup.pi.extensions_path = home.join("extensions");
        let dest = super::super::pi::PLUGIN.path(&cfg);
        assert!(dest.starts_with(&home), "{}", dest.display());
    }
}
