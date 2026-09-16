//! OpenCode installer (`rtok agents setup opencode --proxy`, plan T11.5).

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// OpenCode: `env.OPENAI_BASE_URL` in `opencode.json`. The CLI and the desktop app keep
/// separate config files, so each variant installs alone.
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
            "proxy" => Support::Yes,
            "mcp" => Support::No(
                "OpenCode reads MCP from opencode.json, but setup does not write the `mcp` table yet",
            ),
            "hooks" => Support::No(
                "OpenCode has no shell hook events; hosts/opencode/rtok.ts filters tool output instead",
            ),
            _ => Support::No(
                "the OpenCode plugin (hosts/opencode/rtok.ts) is copied by hand; setup does not link it yet",
            ),
        }
    }

    fn files(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        vec![path_for(cfg, kind)]
    }

    fn installed(&self, cfg: &Config, kind: Kind) -> Vec<&'static str> {
        let s = super::read(&path_for(cfg, kind));
        let mut out = Vec::new();
        if s.contains("OPENAI_BASE_URL") {
            out.push("proxy");
        }
        if s.contains("\"rtok\"") {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if kind == Kind::Desktop {
            let mut desktop = cfg.clone();
            desktop.setup.opencode.config_path = desktop_path();
            return Ok(vec![run(&desktop, remove)?]);
        }
        Ok(vec![run(cfg, remove)?])
    }
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
}
