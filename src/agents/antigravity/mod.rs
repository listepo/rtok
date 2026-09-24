//! Antigravity CLI + Antigravity 2.0 / IDE (`rtok agents install antigravity`, plan T91.1, D21).
//!
//! The plugin is the only unit (T90): `plugins/antigravity` carries `mcp_config.json` and no
//! hooks — Antigravity's `PreToolUse` cannot rewrite tool input and `PostToolUse` cannot add
//! context. Desktop links the tree into `<plugins_path>/rtok`; the CLI's store is `agy`'s own,
//! so rtok prints the `agy plugin install` line instead (the Kimi rule, T86).

use std::path::PathBuf;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant};
use crate::config::Config;
use anyhow::Result;

/// Antigravity: one plugin tree, linked (desktop) or named for `agy` (CLI).
pub struct Antigravity;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Antigravity CLI",
        bins: &["agy"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Antigravity",
        bins: &[],
        apps: &["/Applications/Antigravity.app"],
    },
];

impl Agent for Antigravity {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, kind: Kind, module: &str) -> Support {
        match (module, kind) {
            ("plugin", Kind::Cli) => Support::Offer("--yes"),
            ("plugin", Kind::Desktop) => Support::Flag("--yes"),
            ("hooks", _) => Support::No(
                "Antigravity's PreToolUse cannot rewrite tool input and PostToolUse cannot add context",
            ),
            ("mcp", _) => Support::No("rtok's MCP ships inside the plugin, the only install path"),
            _ => Support::No("Antigravity documents no model base-URL override"),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[rtok_plugin_sdk::Surface::Mcp]
    }

    fn files(&self, _cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        Vec::new()
    }

    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let s = &cfg.setup.antigravity;
        match kind {
            Kind::Cli => vec![cli_plugin_dest(cfg), s.cli_plugins_path.clone()],
            Kind::Desktop => vec![plugin_dest(cfg), s.plugins_path.clone()],
        }
    }

    fn installed(&self, cfg: &Config, kind: Kind) -> Vec<&'static str> {
        // T75: only what the host or rtok would take back counts — a foreign tree at either
        // dest must not hold the green mark.
        let found = match kind {
            Kind::Cli => cli_plugin_detected(cfg),
            Kind::Desktop => PLUGIN.ours(cfg),
        };
        if found { vec!["plugin"] } else { Vec::new() }
    }

    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        Ok(vec![match kind {
            Kind::Cli => offer_cli(cfg, remove),
            Kind::Desktop => PLUGIN.offer(cfg, remove)?,
        }])
    }
}

const PLUGIN_DIR_NAME: &str = "rtok";

/// Link / unlink `plugins/antigravity` for Antigravity 2.0 and IDE (D21). `--yes` only
/// (`default_install: false`): whether the desktop loader follows a symlinked plugin
/// directory is undocumented.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/antigravity",
    host: "Antigravity",
    label: None,
    dest: plugin_dest,
    default_install: false,
};

/// Desktop dest: `<plugins_path>/rtok` (default `~/.gemini/config/plugins/rtok`).
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.antigravity.plugins_path.join(PLUGIN_DIR_NAME)
}

/// CLI staged copy: `<cli_plugins_path>/rtok` (default `~/.gemini/antigravity-cli/plugins/rtok`).
pub fn cli_plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup.antigravity.cli_plugins_path.join(PLUGIN_DIR_NAME)
}

/// `agy plugin install` staged our manifest (its `name` is `rtok`).
pub fn cli_plugin_detected(cfg: &Config) -> bool {
    super::manifest_names(&cli_plugin_dest(cfg), "plugin.json", "rtok")
}

/// The `agy plugin install <resolved plugins/antigravity path>` line ([`super::print_offer`]).
fn offer_cli(cfg: &Config, remove: bool) -> String {
    super::print_offer(
        cfg,
        remove,
        cli_plugin_detected(cfg),
        PLUGIN.src_rel,
        &format!(
            "agy plugin install {}",
            super::plugin_src(PLUGIN.src_rel).display()
        ),
        "keep antigravity-cli/plugins/rtok (owned by agy; remove with `agy plugin uninstall rtok`)",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rtok-antigravity-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(dir: &std::path::Path, dry: bool, yes: bool) -> Config {
        let mut c = Config::default();
        c.setup.antigravity.plugins_path = dir.join("plugins");
        c.setup.antigravity.cli_plugins_path = dir.join("cli-plugins");
        c.setup.dry_run = dry;
        c.setup.yes = yes;
        c.setup.backup = false;
        c
    }

    #[test]
    fn cli_offer_names_agy_and_ketch_behind_yes_and_writes_nothing() {
        let dir = tmp("cli-offer");
        for dry in [true, false] {
            assert_eq!(offer_cli(&cfg(&dir, dry, false), false), NO_CHANGES);
            let s = offer_cli(&cfg(&dir, dry, true), false);
            assert!(s.contains("plugins/antigravity"), "{s}");
            assert!(s.contains("agy plugin install "), "{s}");
            assert!(s.contains("ketch install listepo/rtok"), "{s}");
        }
        assert!(fs::read_dir(&dir).unwrap().next().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn staged_cli_copy_is_installed_and_kept_on_remove() {
        let dir = tmp("cli-staged");
        let c = cfg(&dir, false, false);
        assert!(Antigravity.installed(&c, Kind::Cli).is_empty());
        assert_eq!(offer_cli(&c, true), NO_CHANGES);
        fs::create_dir_all(cli_plugin_dest(&c)).unwrap();
        fs::write(
            cli_plugin_dest(&c).join("plugin.json"),
            r#"{"name":"other"}"#,
        )
        .unwrap();
        assert!(Antigravity.installed(&c, Kind::Cli).is_empty());
        fs::write(
            cli_plugin_dest(&c).join("plugin.json"),
            r#"{"name":"rtok"}"#,
        )
        .unwrap();
        assert_eq!(Antigravity.installed(&c, Kind::Cli), ["plugin"]);
        assert!(offer_cli(&c, true).contains("agy plugin uninstall rtok"));
        assert!(cli_plugin_dest(&c).join("plugin.json").is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn desktop_links_on_yes_once_and_removes_exactly_ours() {
        let dir = tmp("desktop");
        let dry = PLUGIN.offer(&cfg(&dir, true, true), false).unwrap();
        assert!(dry.contains("plugins/antigravity"), "{dry}");
        assert!(!plugin_dest(&cfg(&dir, true, true)).exists());
        let c = cfg(&dir, false, true);
        let linked = &Antigravity.apply(&c, Kind::Desktop, Mode::Install).unwrap()[0];
        assert!(
            linked.starts_with("+ plugin plugins/antigravity → "),
            "{linked}"
        );
        assert_eq!(Antigravity.installed(&c, Kind::Desktop), ["plugin"]);
        assert_eq!(PLUGIN.offer(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(
            PLUGIN.offer(&c, true).unwrap(),
            format!("- plugin {}", plugin_dest(&c).display())
        );
        assert!(!PLUGIN.linked(&c));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_foreign_desktop_tree_is_left_alone_and_not_installed() {
        let dir = tmp("foreign");
        let c = cfg(&dir, false, true);
        fs::create_dir_all(plugin_dest(&c)).unwrap();
        fs::write(plugin_dest(&c).join("plugin.json"), "{}").unwrap();
        assert!(Antigravity.installed(&c, Kind::Desktop).is_empty());
        let _ = PLUGIN.offer(&c, false).unwrap();
        let _ = PLUGIN.offer(&c, true).unwrap();
        assert_eq!(
            fs::read_to_string(plugin_dest(&c).join("plugin.json")).unwrap(),
            "{}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
