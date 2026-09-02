//! Lifecycle helpers for `rtok proxy` and `rtok setup claude --proxy` (plan T5.2).

use std::fs;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use super::ProxyState;
use crate::config::Config;
use crate::setup::claude::{backup, read_settings};

/// `GET /health` → `{"ok":true,"mode":"passthrough"}`.
pub async fn health(State(state): State<Arc<ProxyState>>) -> Json<Value> {
    Json(json!({"ok": true, "mode": state.mode}))
}

/// Set `env.ANTHROPIC_BASE_URL` in Claude settings.json to this proxy (backup).
pub fn register_proxy(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.claude.settings_path;
    let mut root = read_settings(path)?;
    if !root.is_object() {
        root = json!({});
    }
    let url = format!("http://{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let want = json!(url);
    let env = root
        .as_object_mut()
        .unwrap()
        .entry("env")
        .or_insert_with(|| json!({}));
    if !env.is_object() {
        *env = json!({});
    }
    let prev = env.get("ANTHROPIC_BASE_URL").cloned();
    if prev.as_ref() == Some(&want) {
        return Ok("no changes".into());
    }
    env["ANTHROPIC_BASE_URL"] = want;
    if !cfg.setup.dry_run {
        if cfg.setup.backup {
            backup(path)?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).ok();
        }
        fs::write(path, serde_json::to_string_pretty(&root)? + "\n")
            .with_context(|| path.display().to_string())?;
    }
    let revert = match prev.and_then(|v| v.as_str().map(str::to_string)) {
        Some(old) => format!("revert: set env.ANTHROPIC_BASE_URL to {old}"),
        None => "revert: remove env.ANTHROPIC_BASE_URL".into(),
    };
    Ok(format!("env.ANTHROPIC_BASE_URL: {url}\n{revert}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn proxy_env_dry_run_then_apply_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("rtok-proxy-setup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut c = Config::default();
        c.setup.claude.settings_path = path.clone();
        c.setup.backup = false;
        c.setup.dry_run = true;
        let dry = register_proxy(&c).unwrap();
        assert!(dry.contains("ANTHROPIC_BASE_URL"), "{dry}");
        assert!(dry.contains("revert:"), "{dry}");
        assert!(!path.exists());
        c.setup.dry_run = false;
        let first = register_proxy(&c).unwrap();
        assert!(first.contains("8790"), "{first}");
        assert_eq!(register_proxy(&c).unwrap(), "no changes");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("127.0.0.1:8790"), "{raw}");
        let _ = fs::remove_dir_all(dir);
    }
}
