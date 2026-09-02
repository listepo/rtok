//! OpenCode installer (`rtok setup opencode --proxy`, plan T11.5).

use std::fs;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::claude::{backup, read_settings};
use crate::config::Config;

/// Set, dry-run, or remove `env.OPENAI_BASE_URL` in OpenCode's JSON config.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.opencode.config_path;
    let mut root = read_settings(path)?;
    if !root.is_object() {
        root = json!({});
    }
    let url = super::openai_proxy_url(cfg);
    let report = if remove {
        strip(&mut root)
    } else {
        insert(&mut root, &url)
    };
    if !cfg.setup.dry_run && report != "no changes" {
        if cfg.setup.backup {
            backup(path)?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).ok();
        }
        fs::write(path, serde_json::to_string_pretty(&root)? + "\n")
            .with_context(|| path.display().to_string())?;
    }
    Ok(report)
}

fn insert(root: &mut Value, url: &str) -> String {
    let want = json!(url);
    let env = root
        .as_object_mut()
        .unwrap()
        .entry("env")
        .or_insert_with(|| json!({}));
    if !env.is_object() {
        *env = json!({});
    }
    let prev = env.get("OPENAI_BASE_URL").cloned();
    if prev.as_ref() == Some(&want) {
        return "no changes".into();
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
        return "no changes".into();
    };
    if env.remove("OPENAI_BASE_URL").is_none() {
        return "no changes".into();
    }
    if env.is_empty() {
        root.as_object_mut().unwrap().remove("env");
    }
    "- env.OPENAI_BASE_URL".into()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(run(&c, false).unwrap(), "no changes");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("OPENAI_BASE_URL"), "{raw}");
        assert!(raw.ends_with('\n'), "{raw}");
        assert_eq!(run(&c, true).unwrap(), "- env.OPENAI_BASE_URL");
        assert_eq!(run(&c, true).unwrap(), "no changes");
        let gone = fs::read_to_string(&path).unwrap();
        assert!(!gone.contains("OPENAI_BASE_URL"), "{gone}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
