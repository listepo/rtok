//! Local operator dashboard: one process serves the WebSocket API and the Slint WASM UI.
//!
//! `rtok dashboard --host --port`. Does not enter the hook path. Slint is a separate
//! crate (`crates/rtok-webui`) so the hook binary does not link the UI toolkit.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::config::Config;
use crate::plugin::DashboardPage;
use crate::plugins::Registry;
use crate::store::Store;

const INDEX: &str = include_str!("index.html");

pub struct DashState {
    cfg: Config,
}

impl DashState {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }
}

/// `rtok dashboard`: bind `host:port` and serve until killed.
pub fn serve_blocking(cfg: Config) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;
    rt.block_on(serve(cfg))
}

pub async fn serve(cfg: Config) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", cfg.web.host, cfg.web.port)
        .parse()
        .with_context(|| format!("dashboard bind {}:{}", cfg.web.host, cfg.web.port))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    eprintln!("rtok dashboard http://{addr}  (ws://{addr}/ws)");
    axum::serve(listener, app(Arc::new(DashState::new(cfg))))
        .await
        .context("dashboard server")
}

pub fn app(state: Arc<DashState>) -> Router {
    let mut r = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(state);
    let pkg = pkg_dir();
    if pkg.is_dir() {
        r = r.nest_service("/pkg", ServeDir::new(pkg));
    }
    r
}

fn pkg_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/rtok-webui/pkg")
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

async fn health(State(state): State<Arc<DashState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "host": state.cfg.web.host,
        "port": state.cfg.web.port,
    }))
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| socket_loop(socket, state))
}

async fn socket_loop(mut socket: WebSocket, state: Arc<DashState>) {
    loop {
        let body = snapshot(&state.cfg).to_string();
        if socket.send(Message::text(body)).await.is_err() {
            break;
        }
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

pub fn snapshot(cfg: &Config) -> Value {
    let store = Store::open(&cfg.core.db_path).ok();
    snapshot_with(cfg, store.as_ref())
}

fn snapshot_with(cfg: &Config, store: Option<&Store>) -> Value {
    let usage = usage_totals(store);
    let plugins: Vec<Value> = Registry::new(cfg)
        .manifests()
        .into_iter()
        .map(|(m, enabled)| {
            let mut page = DashboardPage::from_id(m.id);
            page.fields.extend(config_fields(m.id, cfg));
            let stats = if page.saves_tokens {
                Some(plugin_stats(store, m.id))
            } else {
                None
            };
            json!({
                "id": m.id,
                "enabled": enabled,
                "surfaces": m.surfaces.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "title": page.title,
                "summary": page.summary,
                "saves_tokens": page.saves_tokens,
                "fields": page.fields,
                "stats": stats,
            })
        })
        .collect();
    json!({
        "type": "snapshot",
        "usage": usage,
        "plugins": plugins,
    })
}

#[derive(Serialize)]
struct Stats {
    input: i64,
    output: i64,
    cache_create: i64,
    cache_read: i64,
    est_before: i64,
    est_after: i64,
    rows: u64,
}

fn usage_totals(store: Option<&Store>) -> Stats {
    let mut s = Stats {
        input: 0,
        output: 0,
        cache_create: 0,
        cache_read: 0,
        est_before: 0,
        est_after: 0,
        rows: 0,
    };
    let Some(store) = store else {
        return s;
    };
    if let Ok(rows) = store.usage_by_api() {
        for r in rows {
            s.input += r.input;
            s.output += r.output;
            s.cache_create += r.cache_create;
            s.cache_read += r.cache_read;
        }
    }
    s
}

fn plugin_stats(store: Option<&Store>, id: &str) -> Stats {
    let mut s = Stats {
        input: 0,
        output: 0,
        cache_create: 0,
        cache_read: 0,
        est_before: 0,
        est_after: 0,
        rows: 0,
    };
    let Some(store) = store else {
        return s;
    };
    if let Ok(rows) = store.list_measurements(id) {
        s.rows = rows.len() as u64;
        for r in rows {
            s.est_before += i64::from(r.est_before);
            s.est_after += i64::from(r.est_after);
        }
    }
    s
}

fn config_fields(id: &str, cfg: &Config) -> Vec<(String, String)> {
    let p = &cfg.plugins;
    match id {
        "cmd" => vec![
            kv("rewrite", p.cmd.rewrite),
            kv("trailer_min_lines", p.cmd.trailer_min_lines),
        ],
        "read" => vec![
            kv("default_mode", &p.read.default_mode),
            kv("max_chars", p.read.max_chars),
        ],
        "archive" => vec![
            kv("keep_turns", p.archive.keep_turns),
            kv("min_tokens", p.archive.min_tokens),
        ],
        "inject" => vec![kv("budget_tokens", p.inject.budget_tokens)],
        "guard" => vec![kv("window_turns", p.guard.window_turns)],
        "memory" => vec![
            kv("recall_titles", p.memory.recall_titles),
            kv("recall_tokens", p.memory.recall_tokens),
        ],
        "graph" => vec![kv("max_tokens", p.graph.max_tokens)],
        "toon" => vec![kv("min_rows", p.toon.min_rows)],
        "proxy" => vec![kv("mode", &cfg.proxy.mode), kv("port", cfg.proxy.port)],
        "measure" => vec![kv("since", &cfg.stats.since)],
        _ => vec![],
    }
}

fn kv(k: &str, v: impl ToString) -> (String, String) {
    (k.into(), v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Ctx, Measurement};

    #[test]
    fn snapshot_lists_catalogue_and_hides_stats_on_measure() {
        let cx = Ctx::in_memory("dash").unwrap();
        cx.record(&Measurement {
            plugin: "cmd",
            kind: "filter",
            before_bytes: 100,
            after_bytes: 40,
            est_before: 25,
            est_after: 10,
            ref_id: None,
            call_id: None,
        })
        .unwrap();
        let v = snapshot_with(&cx.config, Some(&cx.store));
        let plugins = v["plugins"].as_array().unwrap();
        assert!(plugins.iter().any(|p| p["id"] == "cmd"));
        let measure = plugins.iter().find(|p| p["id"] == "measure").unwrap();
        assert_eq!(measure["saves_tokens"], false);
        assert!(measure["stats"].is_null());
        let cmd = plugins.iter().find(|p| p["id"] == "cmd").unwrap();
        assert_eq!(cmd["saves_tokens"], true);
        assert_eq!(cmd["stats"]["est_before"], 25);
        assert_eq!(cmd["stats"]["est_after"], 10);
    }
}
