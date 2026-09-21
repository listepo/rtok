//! Kilo Code installer (`rtok agents install kilo`, plan T97).
//!
//! Kilo Code 7 runs on the OpenCode server: the `kilo` CLI and the VS Code extension read one
//! global config dir (`~/.config/kilo/`), take `mcp.<name>` in OpenCode's local shape and load
//! every `*.ts` in `<config dir>/{plugin,plugins}/`. So this host drives the OpenCode helpers
//! and links `plugins/opencode/rtok.ts` against `[setup.kilo] config_path` — no copy of either.
//! rtok writes `kilo.json`, which Kilo merges with a user's `kilo.jsonc`; the JSONC file, whose
//! comments `serde_json` would drop, is never rewritten (T79).

use std::path::PathBuf;

use anyhow::Result;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, opencode};
use crate::config::Config;

/// Kilo Code: `mcp.rtok` and the linked OpenCode plugin. The CLI and the VS Code extension
/// share `~/.config/kilo/`, so both variants install the same files.
pub struct Kilo;

/// The extension's directory name carries its version, so the desktop variant is found by
/// the editor it runs in.
static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Kilo Code CLI",
        bins: &["kilo"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Kilo Code for VS Code",
        bins: &[],
        apps: &[
            "/Applications/Visual Studio Code.app",
            "$LOCALAPPDATA/Programs/Microsoft VS Code/Code.exe",
        ],
    },
];

/// The config with OpenCode's `config_path` pointed at Kilo's file, so the OpenCode helpers
/// write Kilo's config.
fn as_opencode(cfg: &Config) -> Config {
    let mut c = cfg.clone();
    c.setup.opencode.config_path = cfg.setup.kilo.config_path.clone();
    c
}

impl Agent for Kilo {
    fn id(&self) -> &'static str {
        "kilo"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn shared(&self) -> bool {
        true
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "mcp" => Support::Yes,
            "plugin" => Support::Flag("--yes"),
            "hooks" => Support::No(
                "Kilo Code has no shell hook events; the linked plugin filters bash output instead",
            ),
            _ => Support::No(
                "Kilo Code keeps provider base URLs in its provider settings, which setup does not edit",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[rtok_plugin_sdk::Surface::Cli]
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.kilo.config_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if super::read(&cfg.setup.kilo.config_path).contains("\"rtok\"") {
            out.push("mcp");
        }
        if PLUGIN.ours(cfg) {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let c = as_opencode(cfg);
        let mut lines = Vec::new();
        if remove {
            lines.push(opencode::unregister_mcp(&c)?);
        } else if c.setup.mcp {
            lines.push(opencode::register_mcp(&c)?);
        }
        lines.push(PLUGIN.offer(cfg, remove)?);
        Ok(lines)
    }
}

/// Offer / link / unlink the OpenCode plugin into Kilo's config dir (D21). Dry-run and the
/// unaccepted offer name `plugins/opencode` and `ketch install listepo/rtok`.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/opencode/rtok.ts",
    host: "Kilo Code",
    label: None,
    dest: plugin_dest,
};

/// Plugin dest: `<kilo config dir>/plugins/rtok.ts` — Kilo loads `{plugin,plugins}/*.{ts,js}`.
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    opencode::plugin_dest(&as_opencode(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use serde_json::Value;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-kilo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.setup.kilo.config_path = dir.join("kilo.json");
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, dir)
    }

    fn json(c: &Config) -> Value {
        serde_json::from_str(&fs::read_to_string(&c.setup.kilo.config_path).unwrap()).unwrap()
    }

    #[test]
    fn dry_run_names_the_opencode_plugin_and_ketch_and_writes_nothing() {
        let (c, dir) = cfg("dry", true);
        let lines = Kilo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        let s = lines.join("\n");
        assert!(s.contains("plugins/opencode"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!c.setup.kilo.config_path.exists());
        assert!(!plugin_dest(&c).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_and_registers_second_apply_no_changes_remove_keeps_foreign() {
        let (mut c, dir) = cfg("yes", false);
        c.setup.yes = true;
        fs::write(
            &c.setup.kilo.config_path,
            r#"{"mcp":{"other":{"type":"local","command":["x"]}}}"#,
        )
        .unwrap();
        Kilo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(plugin_dest(&c), dir.join("plugins/rtok.ts"));
        assert!(PLUGIN.linked(&c));
        let root = json(&c);
        assert_eq!(root["mcp"]["rtok"]["type"], "local");
        assert_eq!(root["mcp"]["rtok"]["command"][1], "mcp");
        assert!(root["env"].is_null(), "no proxy key: {root}");
        assert_eq!(Kilo.installed(&c, Kind::Desktop), ["mcp", "plugin"]);

        let again = Kilo.apply(&c, Kind::Desktop, Mode::Install).unwrap();
        assert!(again.iter().all(|l| l == NO_CHANGES), "{again:?}");

        Kilo.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        let root = json(&c);
        assert!(root["mcp"]["rtok"].is_null(), "{root}");
        assert_eq!(root["mcp"]["other"]["command"][0], "x");
        assert!(!PLUGIN.linked(&c));
        assert!(Kilo.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    /// Every file under `dir` with its bytes (symlinks as their target), sorted.
    fn snapshot(dir: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                let meta = fs::symlink_metadata(&p).unwrap();
                if meta.file_type().is_symlink() {
                    let target = fs::read_link(&p).unwrap();
                    out.push((p, target.to_string_lossy().into_owned().into_bytes()));
                } else if meta.is_dir() {
                    stack.push(p);
                } else {
                    out.push((p.clone(), fs::read(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    /// A user's config and a foreign plugin beside the files rtok owns.
    fn seeded(name: &str, dry: bool) -> (Config, PathBuf) {
        let (mut c, dir) = cfg(name, dry);
        c.setup.yes = true;
        fs::write(dir.join("kilo.jsonc"), "{\n  // mine\n  \"mcp\": {},\n}\n").unwrap();
        fs::create_dir_all(dir.join("plugin")).unwrap();
        fs::write(dir.join("plugin/rtok.ts"), "export default {} // mine\n").unwrap();
        (c, dir)
    }

    #[test]
    fn dry_run_leaves_the_tree_byte_for_byte() {
        let (c, dir) = seeded("dry-bytes", true);
        let before = snapshot(&dir);
        Kilo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        Kilo.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert_eq!(snapshot(&dir), before);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn user_jsonc_and_foreign_plugin_survive_install_and_remove() {
        let (c, dir) = seeded("foreign", false);
        let jsonc = fs::read(dir.join("kilo.jsonc")).unwrap();
        let theirs = fs::read(dir.join("plugin/rtok.ts")).unwrap();
        Kilo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(fs::read(dir.join("kilo.jsonc")).unwrap(), jsonc);
        assert_eq!(fs::read(dir.join("plugin/rtok.ts")).unwrap(), theirs);
        Kilo.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert_eq!(fs::read(dir.join("kilo.jsonc")).unwrap(), jsonc);
        assert_eq!(fs::read(dir.join("plugin/rtok.ts")).unwrap(), theirs);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_plugin_link_is_repaired() {
        let (mut c, dir) = cfg("dangling", false);
        c.setup.yes = true;
        fs::create_dir_all(dir.join("plugins")).unwrap();
        std::os::unix::fs::symlink(dir.join("gone.ts"), plugin_dest(&c)).unwrap();
        Kilo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        let body = fs::read_to_string(plugin_dest(&c)).unwrap();
        assert!(
            body.contains("rtok"),
            "link does not reach plugins/opencode/rtok.ts"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_on_a_clean_home_changes_nothing() {
        let (c, dir) = cfg("clean", false);
        let lines = Kilo.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert!(lines.iter().all(|l| l == NO_CHANGES), "{lines:?}");
        assert!(!c.setup.kilo.config_path.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
