//! Lifecycle helpers for `rtok proxy` and `rtok agents install claude --proxy` (plan T5.2).

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::State;
use rtok_agent_sdk::{NO_CHANGES, edit_json, object_at};
use serde_json::{Value, json};

use super::ProxyState;
use crate::agents::apply;
use crate::config::Config;

/// `GET /health` → `{"ok":true,"mode":…}`. When config disables proxy/core business
/// logic: `mode=passthrough`, `enabled=false`, `recording=false` (listener still up).
pub async fn health(State(state): State<Arc<ProxyState>>) -> Json<Value> {
    if state.plain() {
        Json(json!({
            "ok": true,
            "mode": "passthrough",
            "enabled": false,
            "recording": false,
            "live": super::live::snapshot().len(),
        }))
    } else {
        Json(json!({"ok": true, "mode": state.mode, "enabled": true, "recording": true}))
    }
}

/// `GET /live` → newest-first plain-proxy request summaries (in-memory only).
pub async fn live_calls() -> Json<Value> {
    Json(json!(super::live::snapshot()))
}

/// Set `env.ANTHROPIC_BASE_URL` in Claude settings.json to this proxy (backup).
pub fn register_proxy(cfg: &Config) -> Result<String> {
    let url = crate::agents::anthropic_proxy_url(cfg);
    edit_json(&apply(cfg), &cfg.setup.claude.settings_path, |root| {
        let want = json!(url);
        let env = object_at(root, "env");
        let prev = env.get("ANTHROPIC_BASE_URL").cloned();
        if prev.as_ref() == Some(&want) {
            return NO_CHANGES.into();
        }
        env["ANTHROPIC_BASE_URL"] = want;
        let revert = match prev.and_then(|v| v.as_str().map(str::to_string)) {
            Some(old) => format!("revert: set env.ANTHROPIC_BASE_URL to {old}"),
            None => "revert: remove env.ANTHROPIC_BASE_URL".into(),
        };
        format!("env.ANTHROPIC_BASE_URL: {url}\n{revert}")
    })
}

/// Clear `env.ANTHROPIC_BASE_URL` (`rtok agents remove claude`), but only while it still
/// points at this proxy — a URL the user set themselves is not ours to delete.
pub fn unregister_proxy(cfg: &Config) -> Result<String> {
    let url = crate::agents::anthropic_proxy_url(cfg);
    edit_json(&apply(cfg), &cfg.setup.claude.settings_path, |root| {
        let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) else {
            return NO_CHANGES.into();
        };
        if env.get("ANTHROPIC_BASE_URL").and_then(Value::as_str) != Some(url.as_str()) {
            return NO_CHANGES.into();
        }
        env.remove("ANTHROPIC_BASE_URL");
        if env.is_empty() {
            root.as_object_mut().unwrap().remove("env");
        }
        "- env.ANTHROPIC_BASE_URL".into()
    })
}

/// `rtok wrap -- <cmd>` (plan T51.4): run one agent process with both base URLs pointed
/// at this proxy, starting the proxy in-process when nothing answers `/health`.
///
/// The proxy lives only as long as `wrap` does (persistent serving stays `rtok demon
/// start proxy`); `ensure_proxy` below is the whole "via demon or in-process" choice.
/// Silent on success: the child inherits stdio and only its own bytes reach the terminal.
/// Returns the child's exit code — a signal death becomes 128+signo, the shell convention —
/// so the caller exits with it. Terminal signals (Ctrl-C) already reach the child through
/// the shared foreground process group, which is why no signal-handling dependency is
/// needed here.
pub fn wrap(cfg: &Config, command: &[String]) -> Result<i32> {
    let (program, args) = command
        .split_first()
        .context("usage: rtok wrap -- <cmd> [args...]")?;
    if !ensure_proxy(cfg) {
        // Fail open (D1): a proxy that cannot start must not break the agent run — the
        // command goes through with the operator's own environment instead.
        eprintln!("rtok wrap: proxy unavailable, running without base-URL overrides");
        let status = std::process::Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("spawn {program}"))?;
        return Ok(exit_code(status));
    }
    let (anthropic, openai) = (
        crate::agents::anthropic_proxy_url(cfg),
        crate::agents::openai_proxy_url(cfg),
    );
    let status = std::process::Command::new(program)
        .args(args)
        .env("ANTHROPIC_BASE_URL", anthropic)
        .env("OPENAI_BASE_URL", openai)
        .status()
        .with_context(|| format!("spawn {program}"))?;
    Ok(exit_code(status))
}

/// The code `wrap` exits with: the child's code, or 128+signo when a signal killed it.
fn exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        128 + status.signal().unwrap_or(0)
    }
    #[cfg(not(unix))]
    {
        1
    }
}

/// True once `GET /health` on this proxy answers 200 with an rtok body. A bare TCP
/// connect is not enough — another service could own the port.
fn health_ok(cfg: &Config) -> bool {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    let addr = format!("{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let Ok(mut addrs) = addr.to_socket_addrs() else {
        return false;
    };
    let Some(sock) = addrs.next() else {
        return false;
    };
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&sock, std::time::Duration::from_millis(300))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: rtok\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut body = Vec::new();
    if stream.read_to_end(&mut body).is_err() {
        return false;
    }
    let text = String::from_utf8_lossy(&body);
    text.starts_with("HTTP/1.1 200") && text.contains("\"recording\"")
}

/// Make sure the proxy answers, starting it in-process on a background thread when it
/// does not. True when the proxy is reachable afterwards; false means fail open.
pub fn ensure_proxy(cfg: &Config) -> bool {
    if health_ok(cfg) {
        return true;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let serving = cfg.clone();
    std::thread::spawn(move || {
        let bound = (|| -> Result<()> {
            let state = std::sync::Arc::new(super::ProxyState::new(&serving)?);
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("tokio runtime")?;
            rt.block_on(async {
                let addr = format!("{}:{}", serving.proxy.bind, serving.proxy.port);
                let listener = tokio::net::TcpListener::bind(&addr)
                    .await
                    .with_context(|| format!("bind {addr}"))?;
                let _ = tx.send(());
                axum::serve(listener, super::app(state))
                    .await
                    .context("proxy server")?;
                Ok::<(), anyhow::Error>(())
            })
        })();
        if let Err(e) = bound {
            eprintln!("rtok wrap: proxy failed to start: {e:#}");
        }
    });
    // Bound is not yet serving: poll health briefly either way.
    if rx.recv_timeout(std::time::Duration::from_secs(5)).is_err() {
        return false;
    }
    for _ in 0..100 {
        if health_ok(cfg) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
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
