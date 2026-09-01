//! `~/.rtok/config.toml` (plan T0.2). `RTOK_HOME` overrides the directory.
//!
//! Missing file → written with defaults. Missing keys → defaults. Unknown keys → kept
//! (per-plugin settings live under `[plugins.<id>]` next to `enabled`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Plugin catalogue: `(id, default_on)`. The registry's manifests must match this list
/// (asserted by a test in `plugins`); the default config file lists every id.
pub const CATALOGUE: [(&str, bool); 10] = [
    ("measure", true),
    ("cmd", true),
    ("read", true),
    ("archive", true),
    ("proxy", true),
    ("inject", true),
    ("guard", true),
    ("memory", true),
    ("graph", true),
    ("toon", false),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub core: Core,
    pub estimator: Estimator,
    pub plugins: BTreeMap<String, PluginCfg>,
    /// Directory the config was loaded from; not serialised.
    #[serde(skip)]
    pub home: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Core {
    /// Empty → `<home>/rtok.db`.
    pub db_path: PathBuf,
    /// Empty → `<home>/archive`.
    pub archive_dir: PathBuf,
    /// Per-turn cap for SessionStart/UserPromptSubmit injections (decision D5).
    pub inject_budget_tokens: u32,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            db_path: PathBuf::new(),
            archive_dir: PathBuf::new(),
            inject_budget_tokens: 800,
        }
    }
}

/// Chars per token per text class (plan T0.5). Recalibrated by `rtok stats --calibrate`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Estimator {
    pub code: f32,
    pub prose: f32,
    pub json: f32,
    pub cjk: f32,
}

impl Default for Estimator {
    fn default() -> Self {
        Self {
            code: 3.5,
            prose: 4.2,
            json: 3.0,
            cjk: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCfg {
    /// `None` → the plugin's `Manifest::default_on`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Plugin-specific keys (`rewrite`, `native_max_bytes`, …), owned by that plugin.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Config {
    /// `$RTOK_HOME` or `$HOME/.rtok`.
    pub fn home_dir() -> PathBuf {
        if let Some(h) = std::env::var_os("RTOK_HOME") {
            return PathBuf::from(h);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        home.join(".rtok")
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&Self::home_dir())
    }

    /// Read `<home>/config.toml`, creating it with defaults when absent.
    pub fn load_from(home: &Path) -> Result<Self> {
        let path = home.join("config.toml");
        let mut cfg = if path.exists() {
            let text =
                std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
            toml::from_str::<Config>(&text).with_context(|| path.display().to_string())?
        } else {
            let cfg = Self::defaults(home);
            std::fs::create_dir_all(home)?;
            std::fs::write(&path, toml::to_string_pretty(&cfg)?)?;
            cfg
        };
        cfg.home = home.to_path_buf();
        if cfg.core.db_path.as_os_str().is_empty() {
            cfg.core.db_path = home.join("rtok.db");
        }
        if cfg.core.archive_dir.as_os_str().is_empty() {
            cfg.core.archive_dir = home.join("archive");
        }
        Ok(cfg)
    }

    fn defaults(home: &Path) -> Self {
        let plugins = CATALOGUE
            .iter()
            .map(|(id, on)| {
                let cfg = PluginCfg {
                    enabled: Some(*on),
                    extra: toml::Table::new(),
                };
                ((*id).to_string(), cfg)
            })
            .collect();
        Self {
            core: Core {
                db_path: home.join("rtok.db"),
                archive_dir: home.join("archive"),
                ..Core::default()
            },
            estimator: Estimator::default(),
            plugins,
            home: home.to_path_buf(),
        }
    }

    /// `[plugins.<id>] enabled`, or `default_on` when unset.
    pub fn plugin_enabled(&self, id: &str, default_on: bool) -> bool {
        self.plugins
            .get(id)
            .and_then(|p| p.enabled)
            .unwrap_or(default_on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn creates_defaults_and_budget_is_800() {
        let home = tmp();
        let cfg = Config::load_from(&home).unwrap();
        assert!(home.join("config.toml").exists());
        assert_eq!(cfg.core.inject_budget_tokens, 800);
        assert_eq!(cfg.core.db_path, home.join("rtok.db"));
        assert_eq!(cfg.plugins.len(), CATALOGUE.len());
        assert!(!cfg.plugin_enabled("toon", false));
        // Second load reads the file back identically.
        let again = Config::load_from(&home).unwrap();
        assert_eq!(
            again.core.inject_budget_tokens,
            cfg.core.inject_budget_tokens
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn partial_file_keeps_defaults_and_extra_keys() {
        let cfg: Config =
            toml::from_str("[plugins.cmd]\nrewrite = false\n[estimator]\ncode = 4.0\n").unwrap();
        assert_eq!(cfg.core.inject_budget_tokens, 800);
        assert_eq!(cfg.estimator.code, 4.0);
        assert_eq!(cfg.estimator.prose, 4.2);
        assert!(cfg.plugin_enabled("cmd", true));
        assert_eq!(cfg.plugins["cmd"].extra["rewrite"].as_bool(), Some(false));
    }
}
