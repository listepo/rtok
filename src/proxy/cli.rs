//! Lifecycle helpers for `rtok proxy` and `rtok agent setup claude --proxy` (plan T5.2).

use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use rtok_agent_sdk::{NO_CHANGES, read_json, write_json};
use serde_json::{Value, json};

use super::ProxyState;
use crate::config::Config;
use crate::setup::apply;

/// `GET /health` → `{"ok":true,"mode":"passthrough"}`.
pub async fn health(State(state): State<Arc<ProxyState>>) -> Json<Value> {
    Json(json!({"ok": true, "mode": state.mode}))
}

/// Set `env.ANTHROPIC_BASE_URL` in Claude settings.json to this proxy (backup).
pub fn register_proxy(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.claude.settings_path;
    let mut root = read_json(path)?;
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
        return Ok(NO_CHANGES.into());
    }
    env["ANTHROPIC_BASE_URL"] = want;
    let revert = match prev.and_then(|v| v.as_str().map(str::to_string)) {
        Some(old) => format!("revert: set env.ANTHROPIC_BASE_URL to {old}"),
        None => "revert: remove env.ANTHROPIC_BASE_URL".into(),
    };
    let report = format!("env.ANTHROPIC_BASE_URL: {url}\n{revert}");
    write_json(&apply(cfg), path, &root, &report)?;
    Ok(report)
}

/// Clear `env.ANTHROPIC_BASE_URL` (`rtok agent remove claude`), but only while it still
/// points at this proxy — a URL the user set themselves is not ours to delete.
pub fn unregister_proxy(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.claude.settings_path;
    if !path.exists() {
        return Ok(NO_CHANGES.into());
    }
    let mut root = read_json(path)?;
    let url = format!("http://{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) else {
        return Ok(NO_CHANGES.into());
    };
    if env.get("ANTHROPIC_BASE_URL").and_then(Value::as_str) != Some(url.as_str()) {
        return Ok(NO_CHANGES.into());
    }
    env.remove("ANTHROPIC_BASE_URL");
    if env.is_empty() {
        root.as_object_mut().unwrap().remove("env");
    }
    let report = "- env.ANTHROPIC_BASE_URL";
    write_json(&apply(cfg), path, &root, report)?;
    Ok(report.into())
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
