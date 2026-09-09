//! Local operator dashboard: one process serves the WebSocket API and the Slint WASM UI.
//!
//! `rtok web --host --port`. Does not enter the hook path. Slint is a separate
//! crate (`crates/rtok-webui`) so the hook binary does not link the UI toolkit.
//! Every value served comes from [`model`], the operator model `rtok tui` renders too (D23).

pub mod model;

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
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::config::Config;

const INDEX: &str = include_str!("index.html");

pub struct DashState {
    cfg: Config,
}

impl DashState {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }
}

/// `rtok web`: bind `host:port` and serve until killed.
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
    eprintln!("rtok web http://{addr}  (ws://{addr}/ws)");
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

/// One snapshot per tick, rendered from the operator model. Keys come out sorted, which
/// is the wire the P19 UI already reads. Public so the T15.10 parity test reads the
/// frame the surface actually sends on `/ws`.
pub fn frame(cfg: &Config) -> String {
    serde_json::to_value(model::snapshot(cfg))
        .unwrap_or_else(|_| json!({"type": "snapshot"}))
        .to_string()
}

async fn socket_loop(mut socket: WebSocket, state: Arc<DashState>) {
    loop {
        if socket.send(Message::text(frame(&state.cfg))).await.is_err() {
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
