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
use rtok_agent_sdk::{KETCH_INSTALL, PluginLink};

use super::{apply, plugin_src};
use crate::config::Config;

/// One host's plugin: the values that differ between hosts, and nothing else.
pub struct HostPlugin {
    /// Repo-relative source, named in every report (`plugins/cursor`).
    pub src_rel: &'static str,
    /// The host's name as its users spell it (`Cursor`, `OpenCode`, `pi`).
    pub host: &'static str,
    /// Home-relative label when the real destination reads worse than it (`~/.cursor/plugins/local`).
    pub label: Option<&'static str>,
    /// Where the plugin lands under a given config — the one part that needs the config.
    pub dest: fn(&Config) -> PathBuf,
    /// True for a host whose official docs have no GitHub/subdir install, so the local
    /// link is the only path there is (T164: pi, opencode, kilo, zcode, cursor) — installed
    /// without asking, once the host itself is detected. False keeps the old `--yes`
    /// question (omp, a pi fork, is unchanged for now).
    pub default_install: bool,
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

    /// Offer, link, or unlink the plugin. Dry-run, the "already current" no-op, the stale-
    /// version relink and the "leave a foreign tree alone" rule all stay in the SDK
    /// ([`PluginLink::run`]); this only decides whether the install question needs asking
    /// at all (T164), and never lets a write error abort the rest of `agents install` —
    /// it fails open, like a missing host CLI already does (T139).
    pub fn offer(&self, cfg: &Config, remove: bool) -> Result<String> {
        let mut apply = apply(cfg);
        if self.default_install && !remove {
            apply.yes = true;
        }
        Ok(self.link(cfg).run(&apply, remove).unwrap_or_else(|e| {
            if remove {
                format!("{} plugin failed: {e}", self.host)
            } else {
                let desc = self
                    .label
                    .map(str::to_string)
                    .unwrap_or_else(|| self.path(cfg).display().to_string());
                format!(
                    "offer {} → {desc} ({} failed: {e}) {KETCH_INSTALL}",
                    self.src_rel, self.host
                )
            }
        }))
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

    /// Every declared `HostPlugin`, for tests that must cover the whole set rather than
    /// pick a few by hand (T164).
    fn all_host_plugins() -> [&'static HostPlugin; 7] {
        [
            &super::super::cursor::PLUGIN,
            &super::super::opencode::PLUGIN,
            &super::super::pi::PLUGIN,
            &super::super::kilo::PLUGIN,
            &super::super::zcode::PLUGIN,
            &super::super::omp::PLUGIN,
            &super::super::antigravity::PLUGIN,
        ]
    }

    /// Every host plugin names a distinct host, so a table keyed by host name (below) and a
    /// report line can never point at two declarations. Two hosts (kilo, omp) do share
    /// another host's `src_rel` on purpose — kilo links opencode's JS plugin, omp links
    /// pi's extension, neither duplicates the tree — so this does not check sources.
    #[test]
    fn declared_host_plugins_have_distinct_hosts() {
        let mut seen = Vec::new();
        for p in all_host_plugins() {
            assert!(
                p.src_rel.starts_with("plugins/"),
                "{} source is not in plugins/: {}",
                p.host,
                p.src_rel
            );
            assert!(!seen.contains(&p.host), "duplicate host {}", p.host);
            seen.push(p.host);
        }
    }

    /// T164: exactly pi, opencode, kilo, zcode and cursor install without asking — every
    /// host whose official docs have no GitHub/subdir install, so the local link is the
    /// only path there is. omp (a pi fork) is unchanged for now. A table, not one
    /// assertion per host, so a new host plugin is forced to state its answer here too.
    #[test]
    fn default_install_is_exactly_the_local_link_only_hosts() {
        let expected: &[(&str, bool)] = &[
            ("Cursor", true),
            ("OpenCode", true),
            ("pi", true),
            ("Kilo Code", true),
            ("ZCode", true),
            ("oh my pi", false),
            ("Antigravity", false),
        ];
        for p in all_host_plugins() {
            let want = expected
                .iter()
                .find(|(host, _)| *host == p.host)
                .unwrap_or_else(|| panic!("{} missing from the expected table", p.host))
                .1;
            assert_eq!(
                p.default_install, want,
                "{} default_install should be {want}",
                p.host
            );
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
