//! OpenCode installer (`rtok agents install opencode`, plan T11.5, T44.5).
//!
//! Three modules, one call path each (D21): `env.OPENAI_BASE_URL` points the host at the
//! proxy, `mcp.rtok` serves `read`/`search`/`memory`/`graph` (OpenCode's own shape,
//! `{type: "local", command: [..], enabled}`), and the linked `plugins/opencode/rtok.ts`
//! filters bash output through `rtok filter`. OpenCode has no shell hook protocol.

use std::path::{Path, PathBuf};

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, edit_json, object_at};
use serde_json::{Value, json};

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply, register_local_mcp, unregister_local_mcp};
use crate::config::Config;

/// OpenCode: `env.OPENAI_BASE_URL`, `mcp.rtok` and a linked plugin. The CLI and the desktop
/// app keep separate config directories, so each variant installs alone.
pub struct OpenCode;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "OpenCode",
        bins: &["opencode"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "OpenCode Desktop",
        bins: &["opencode-desktop"],
        apps: &[
            "/Applications/OpenCode.app",
            "$LOCALAPPDATA/Programs/OpenCode/OpenCode.exe",
        ],
    },
];

/// Where the OpenCode desktop app keeps its global config. The CLI lives at
/// `[setup.opencode] config_path` (`~/.config/opencode/opencode.json`); the
/// desktop build resolves a sibling app dir instead.
pub fn desktop_path() -> PathBuf {
    let home = super::home_dir();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/ai.opencode.desktop/opencode.json")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("ai.opencode.desktop/opencode.json")
    } else {
        home.join(".config/ai.opencode.desktop/opencode.json")
    }
}

fn path_for(cfg: &Config, kind: Kind) -> PathBuf {
    match kind {
        Kind::Desktop => desktop_path(),
        Kind::Cli => cfg.setup.opencode.config_path.clone(),
    }
}

/// The config with `config_path` pointed at this variant's file, so every step below reads
/// one key.
fn for_kind(cfg: &Config, kind: Kind) -> Config {
    let mut c = cfg.clone();
    c.setup.opencode.config_path = path_for(cfg, kind);
    c
}

impl Agent for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "proxy" | "mcp" | "plugin" => Support::Yes,
            _ => Support::No(
                "OpenCode has no shell hook events; the linked plugin filters bash output instead",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[rtok_plugin_sdk::Surface::Cli]
    }

    fn files(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        vec![path_for(cfg, kind)]
    }

    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let mut paths = self.files(cfg, kind);
        paths.push(plugin_dest(&for_kind(cfg, kind)));
        paths
    }

    fn installed(&self, cfg: &Config, kind: Kind) -> Vec<&'static str> {
        let c = for_kind(cfg, kind);
        let s = super::read(&c.setup.opencode.config_path);
        let mut out = Vec::new();
        if s.contains("OPENAI_BASE_URL") {
            out.push("proxy");
        }
        if s.contains("\"rtok\"") {
            out.push("mcp");
        }
        // T75: only what remove will take back counts as installed — a foreign
        // directory at the plugin dest must not hold the green mark.
        if PLUGIN.ours(&c) {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let c = for_kind(cfg, kind);
        let mut lines = vec![run(&c, remove)?];
        if remove {
            lines.push(unregister_mcp(&c)?);
        } else if c.setup.mcp {
            lines.push(register_mcp(&c)?);
        }
        lines.push(PLUGIN.offer(&c, remove)?);
        lines.push(super::skill::sync("opencode", &c, remove)?);
        Ok(lines)
    }
}

/// Register `rtok mcp` as `mcp.rtok` — OpenCode's local server shape, `command` as argv.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    register_local_mcp(cfg, &cfg.setup.opencode.config_path, "mcp")
}

/// Drop `mcp.rtok` (`rtok agents remove opencode`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    unregister_local_mcp(cfg, &cfg.setup.opencode.config_path, "mcp")
}

/// Offer / link / unlink `plugins/opencode/rtok.ts` (D21, T44.5). Dry-run and the unaccepted
/// offer name `plugins/opencode` and `ketch install listepo/rtok`.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/opencode/rtok.ts",
    host: "OpenCode",
    label: None,
    dest: plugin_dest,
    // OpenCode's own docs have no GitHub/subdir plugin install; the local link is the
    // only path there is, so it installs by default once OpenCode itself is detected
    // (T164).
    default_install: true,
};

/// Plugin dest: `<config dir>/plugins/rtok.ts` — OpenCode loads every `*.ts` there.
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup
        .opencode
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins")
        .join("rtok.ts")
}

/// Set, dry-run, or remove `env.OPENAI_BASE_URL` in OpenCode's JSON config.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let url = super::openai_proxy_url(cfg);
    edit_json(&apply(cfg), &cfg.setup.opencode.config_path, |root| {
        if remove {
            strip(root)
        } else {
            insert(root, &url)
        }
    })
}

fn insert(root: &mut Value, url: &str) -> String {
    let want = json!(url);
    let env = object_at(root, "env");
    let prev = env.get("OPENAI_BASE_URL").cloned();
    if prev.as_ref() == Some(&want) {
        return NO_CHANGES.into();
    }
    env["OPENAI_BASE_URL"] = want;
    let revert = match prev.and_then(|v| v.as_str().map(str::to_string)) {
        Some(old) => format!("revert: set env.OPENAI_BASE_URL to {old}"),
        None => "revert: remove env.OPENAI_BASE_URL".into(),
    };
    format!("env.OPENAI_BASE_URL: {url}\n{revert}")
}

fn strip(root: &mut Value) -> String {
    let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) else {
        return NO_CHANGES.into();
    };
    if env.remove("OPENAI_BASE_URL").is_none() {
        return NO_CHANGES.into();
    }
    if env.is_empty() {
        root.as_object_mut().unwrap().remove("env");
    }
    "- env.OPENAI_BASE_URL".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn cfg(dir: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-opencode-{dir}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("opencode.json");
        let mut c = Config::default();
        c.setup.opencode.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_shows_one_change_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let out = run(&c, false).unwrap();
        assert!(out.contains("OPENAI_BASE_URL"), "{out}");
        assert!(out.contains("8790/v1"), "{out}");
        assert!(out.contains("revert:"), "{out}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_and_remove_strips() {
        let (c, path) = cfg("apply", false);
        let first = run(&c, false).unwrap();
        assert!(first.contains("8790/v1"), "{first}");
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("OPENAI_BASE_URL"), "{raw}");
        assert!(raw.ends_with('\n'), "{raw}");
        assert_eq!(run(&c, true).unwrap(), "- env.OPENAI_BASE_URL");
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        let gone = fs::read_to_string(&path).unwrap();
        assert!(!gone.contains("OPENAI_BASE_URL"), "{gone}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn mcp_entry_is_local_argv_idempotent_and_remove_keeps_foreign() {
        let (c, path) = cfg("mcp", false);
        crate::agents::assert_local_mcp_roundtrip(
            &path,
            || register_mcp(&c),
            || unregister_mcp(&c),
            || OpenCode.installed(&c, Kind::Cli),
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn plugin_offer_links_one_file_beside_the_config() {
        let (mut c, path) = cfg("plugin", true);
        let dry = PLUGIN.offer(&c, false).unwrap();
        assert!(dry.contains("plugins/opencode"), "{dry}");
        assert!(dry.contains("ketch install listepo/rtok"), "{dry}");
        assert!(!plugin_dest(&c).exists());
        c.setup.dry_run = false;
        c.setup.yes = true;
        assert!(PLUGIN.offer(&c, false).unwrap().starts_with("+ plugin"));
        let dest = plugin_dest(&c);
        assert_eq!(dest, path.parent().unwrap().join("plugins").join("rtok.ts"));
        assert!(
            fs::read_to_string(&dest)
                .unwrap()
                .contains("tool.execute.after")
        );
        assert_eq!(OpenCode.installed(&c, Kind::Cli), ["plugin"]);
        assert_eq!(PLUGIN.offer(&c, false).unwrap(), NO_CHANGES);
        assert!(PLUGIN.offer(&c, true).unwrap().starts_with("- plugin"));
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn desktop_variant_uses_its_own_config_dir() {
        let c = for_kind(&Config::default(), Kind::Desktop);
        assert_eq!(c.setup.opencode.config_path, desktop_path());
        assert_eq!(
            plugin_dest(&c),
            desktop_path()
                .parent()
                .unwrap()
                .join("plugins")
                .join("rtok.ts")
        );
    }
}
