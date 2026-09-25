//! Local operator dashboard: one process serves the WebSocket API and the Slint WASM UI.
//!
//! `rtok web --host --port`. Does not enter the hook path. Slint is a separate
//! crate (`crates/rtok-webui`) so the hook binary does not link the UI toolkit.
//! Every value served comes from [`model`], the operator model `rtok tui` renders too (D23).

pub mod model;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower_http::services::ServeDir;

use crate::config::{Config, validate};
use crate::plugins::Registry;

const INDEX: &str = include_str!("index.html");

/// Builds one snapshot frame from a config. Production always uses [`frame`]; tests can
/// substitute a slower or instrumented builder to exercise T206's build coalescing (a real
/// busy store, or just a barrier) without waiting on a real busy store.
pub type BuildFn = Arc<dyn Fn(&Config) -> String + Send + Sync>;

/// T206: at most one snapshot build runs at a time; a tick that lands while one is already
/// running awaits its result instead of starting a redundant build.
enum BuildSlot {
    Idle,
    InFlight(watch::Receiver<Option<String>>),
}

pub struct DashState {
    cfg: Mutex<Config>,
    build: Mutex<BuildSlot>,
    build_fn: BuildFn,
}

impl DashState {
    pub fn new(cfg: Config) -> Self {
        Self::with_builder(cfg, Arc::new(frame))
    }

    /// For tests: swap the snapshot builder, e.g. to block it on a barrier so `/health` can be
    /// asserted to answer while a build is stuck (T206).
    pub fn with_builder(cfg: Config, build_fn: BuildFn) -> Self {
        Self {
            cfg: Mutex::new(cfg),
            build: Mutex::new(BuildSlot::Idle),
            build_fn,
        }
    }

    /// One inbound `/ws` text frame (T60.5 / T60.4). `None` means ignore, or an
    /// accepted `set` whose next snapshot carries the write. `Some` is a message
    /// frame (refused `set`) or an `expand` payload frame.
    pub fn inbound(&self, text: &str) -> Option<String> {
        inbound(self, text)
    }

    /// The snapshot frame `/ws` sends next, after any accepted `set`.
    pub fn snapshot_json(&self) -> String {
        frame(&self.cfg.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// The snapshot frame for one `/ws` tick (T206). The `Config` mutex is held only long
    /// enough to clone it — never across the build — and the build itself runs in
    /// `spawn_blocking` so it cannot stall the executor or `health()`/`inbound()`'s lock.
    /// Ticks that land while a build is already in flight share its result.
    async fn snapshot_shared(&self) -> String {
        // The check-and-set that either joins an in-flight build or claims the builder role
        // happens under one lock acquisition, so two ticks landing at once cannot both start a
        // build. A `MutexGuard` never lives across an `.await` (each is scoped to its own
        // block), so this future stays `Send` (required by `on_upgrade`).
        enum Claim {
            Join(watch::Receiver<Option<String>>),
            Build(watch::Sender<Option<String>>),
        }
        let claim = {
            let mut guard = self.build.lock().unwrap_or_else(PoisonError::into_inner);
            match &*guard {
                BuildSlot::InFlight(rx) => Claim::Join(rx.clone()),
                BuildSlot::Idle => {
                    let (tx, rx) = watch::channel(None);
                    *guard = BuildSlot::InFlight(rx);
                    Claim::Build(tx)
                }
            }
        };
        let tx = match claim {
            Claim::Build(tx) => tx,
            Claim::Join(mut rx) => {
                if rx.changed().await.is_ok()
                    && let Some(snap) = rx.borrow().clone()
                {
                    return snap;
                }
                // The build ahead of us never sent (it panicked): claim it ourselves instead
                // of hanging this socket forever.
                let mut guard = self.build.lock().unwrap_or_else(PoisonError::into_inner);
                let (tx, rx) = watch::channel(None);
                *guard = BuildSlot::InFlight(rx);
                tx
            }
        };

        let cfg = self
            .cfg
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let build_fn = self.build_fn.clone();
        let snap = tokio::task::spawn_blocking(move || build_fn(&cfg))
            .await
            .unwrap_or_else(|_| {
                json!({"type": "snapshot", "error": "snapshot build panicked"}).to_string()
            });

        *self.build.lock().unwrap_or_else(PoisonError::into_inner) = BuildSlot::Idle;
        let _ = tx.send(Some(snap.clone()));
        snap
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
    // Loopback dashboard address for the operator; no socket is opened from this string.
    let listen = format!("http://{addr} ws://{addr}/ws"); // nosemgrep: javascript.lang.security.detect-insecure-websocket.detect-insecure-websocket
    eprintln!("rtok web {listen}");
    crate::log::append(&cfg, "info", "web", "serve", &listen);
    let pkg = resolve_pkg();
    if matches!(pkg, Pkg::Missing) {
        let msg = pkg_missing_text();
        eprintln!("{msg}");
        crate::log::append(&cfg, "warn", "web", "pkg", &msg);
    }
    axum::serve(listener, app_with_pkg(Arc::new(DashState::new(cfg)), pkg))
        .await
        .context("dashboard server")
}

pub fn app(state: Arc<DashState>) -> Router {
    app_with_pkg(state, resolve_pkg())
}

/// Where `/pkg` comes from.
#[derive(Debug, Clone)]
pub enum Pkg {
    /// A built bundle on disk (`RTOK_WEB_PKG`, beside the binary, the source tree).
    Dir(PathBuf),
    /// The bundle compiled into this binary (T111) — what a ketch install has.
    Embedded,
    /// Neither: `/pkg` answers 503 with how to get one (T80).
    Missing,
}

/// A bundle on disk wins, so `RTOK_WEB_PKG` and a fresh `just web` override the
/// copy baked in at build time; the embedded one serves every plain install.
pub fn resolve_pkg() -> Pkg {
    match pkg_dir() {
        Some(dir) => Pkg::Dir(dir),
        None if embedded::AVAILABLE => Pkg::Embedded,
        None => Pkg::Missing,
    }
}

/// `app` with the asset source already resolved, so tests cover every surface
/// without touching the process environment.
pub fn app_with_pkg(state: Arc<DashState>, pkg: Pkg) -> Router {
    let r = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(state);
    match pkg {
        Pkg::Dir(dir) => {
            let service = ServeDir::new(dir).precompressed_br().precompressed_gzip();
            r.nest_service("/pkg", service)
        }
        Pkg::Embedded => r.route("/pkg/{*path}", get(embedded::serve)),
        // T80: a 404 here reads as a broken build. Say what is missing instead.
        Pkg::Missing => r.route("/pkg/{*path}", get(pkg_missing)),
    }
}

/// The Slint bundle `build.rs` found at compile time (T111). Without it the
/// binary still builds; `AVAILABLE` is false and `/pkg` falls back to disk.
pub mod embedded {
    use axum::extract::Path;
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;

    #[cfg(rtok_web_embed)]
    const FILES: &[(&str, &str, &[u8])] = &[
        (
            "rtok_webui.js",
            "text/javascript",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/crates/rtok-webui/pkg/rtok_webui.js"
            )),
        ),
        (
            "rtok_webui_bg.wasm",
            "application/wasm",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/crates/rtok-webui/pkg/rtok_webui_bg.wasm"
            )),
        ),
    ];
    #[cfg(not(rtok_web_embed))]
    const FILES: &[(&str, &str, &[u8])] = &[];

    /// True when this binary carries the bundle.
    pub const AVAILABLE: bool = !FILES.is_empty();

    /// Bytes and media type of one embedded file, by its name under `/pkg/`.
    pub fn get(name: &str) -> Option<(&'static str, &'static [u8])> {
        FILES
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, mime, bytes)| (*mime, *bytes))
    }

    pub(super) async fn serve(Path(name): Path<String>) -> impl IntoResponse {
        match get(&name) {
            Some((mime, bytes)) => {
                (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

/// Where the Slint WASM bundle lives at run time. `env!("CARGO_MANIFEST_DIR")` is
/// baked at compile time, so a released binary would look inside the CI runner's
/// checkout (T80); the source tree is the last candidate, not the only one.
pub fn pkg_dir() -> Option<PathBuf> {
    pkg_candidates().into_iter().find(|p| p.is_dir())
}

fn pkg_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = std::env::var_os(PKG_ENV) {
        out.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(bin) = exe.parent()
    {
        // A release archive unpacks `pkg/` beside the binary; a prefix install
        // puts it under `share/rtok/` beside or one level above `bin/`.
        out.push(bin.join("pkg"));
        out.push(bin.join("share").join("rtok").join("pkg"));
        if let Some(prefix) = bin.parent() {
            out.push(prefix.join("share").join("rtok").join("pkg"));
        }
    }
    // Dev fallback only: an embedded build already carries this tree's bundle, and
    // `build.rs` re-embeds it whenever it changes.
    if !embedded::AVAILABLE {
        out.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/rtok-webui/pkg"));
    }
    out
}

const PKG_ENV: &str = "RTOK_WEB_PKG";

/// One line for the 503 body and for the startup warning — same text, one source.
fn pkg_missing_text() -> String {
    let tried = pkg_candidates()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    format!(
        "rtok web: the Slint WASM bundle is not on this machine, so the UI cannot load \
         (the API and /ws are up). Build it with `just web`, or point {PKG_ENV} at a pkg/ \
         directory. Looked in:\n  {tried}"
    )
}

async fn pkg_missing() -> impl IntoResponse {
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        pkg_missing_text(),
    )
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

async fn health(State(state): State<Arc<DashState>>) -> Json<Value> {
    let cfg = state.cfg.lock().unwrap_or_else(|e| e.into_inner());
    Json(json!({
        "ok": true,
        "host": cfg.web.host,
        "port": cfg.web.port,
    }))
}

async fn ws_upgrade(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashState>>,
) -> impl IntoResponse {
    if !origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| socket_loop(socket, state))
        .into_response()
}

/// Browsers send `Origin` on cross-site WebSocket upgrades; non-browser clients
/// send none. Reject a present `Origin` whose host differs from `Host`, and one
/// whose `Host` is a DNS name other than `localhost`: a rebinding page
/// (`evil.example` resolved to 127.0.0.1) sends a matching `Origin` and `Host`,
/// and an IP literal or `localhost` is the only name it cannot point here.
fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let (Some(o), Some(h)) = (origin_authority(origin), host_authority(host)) else {
        return false;
    };
    // The port is part of the origin: another local app's page must not get in.
    let default = default_port(origin);
    let same_port = port_of(o).unwrap_or(default) == port_of(h).unwrap_or(default);
    let (o, h) = (strip_port(o), strip_port(h));
    o.eq_ignore_ascii_case(h) && same_port && rebind_safe(h)
}

const HTTP_DEFAULT_PORT: &str = "80";
const HTTPS_DEFAULT_PORT: &str = "443";

/// The port a browser implies for `origin`'s scheme when the authority names none.
fn default_port(origin: &str) -> &'static str {
    let scheme = origin.split("://").next().unwrap_or_default();
    if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("wss") {
        HTTPS_DEFAULT_PORT
    } else {
        HTTP_DEFAULT_PORT
    }
}

fn rebind_safe(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
        || host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
}

fn origin_authority(origin: &str) -> Option<&str> {
    let rest = origin.split("://").nth(1)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    authority.split('@').next_back()
}

fn host_authority(host: &str) -> Option<&str> {
    let host = host.split(',').next_back()?.trim();
    if host.is_empty() {
        return None;
    }
    host.split('@').next_back()
}

/// The explicit port of `authority` (`host:port` or `[v6]:port`), if any.
fn port_of(authority: &str) -> Option<&str> {
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split_once(']')?.1.strip_prefix(':');
    }
    if authority.matches(':').count() == 1 {
        return authority.split_once(':').map(|(_, port)| port);
    }
    None
}

fn strip_port(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        return &rest[..end];
    }
    if authority.matches(':').count() == 1
        && let Some((h, _)) = authority.split_once(':')
    {
        return h;
    }
    authority
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
        let snap = state.snapshot_shared().await;
        if socket.send(Message::text(snap)).await.is_err() {
            break;
        }
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Text(text))) => {
                        if let Some(reply) = inbound(&state, text.as_str())
                            && socket.send(Message::text(reply)).await.is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

fn inbound(state: &DashState, text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    if let Some(id) = v.get("expand").and_then(Value::as_str) {
        let cfg = state
            .cfg
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        return Some(match model::expand_payload(&cfg, id, None) {
            Some(body) => json!({ "type": "expand", "id": id, "text": body }).to_string(),
            None => message_frame(&format!("unknown archive id: {id}")),
        });
    }
    let set = v.get("set")?;
    let key = set.get("key").and_then(Value::as_str).unwrap_or("");
    let Some(value) = set.get("value").and_then(Value::as_bool) else {
        return Some(message_frame("set.value must be a bool"));
    };
    let mut cfg = state.cfg.lock().unwrap_or_else(PoisonError::into_inner);
    if !allowlisted_plugin_enabled(&cfg, key) {
        return Some(message_frame(&format!("refused key {key}")));
    }
    match validate::set(&cfg.home, key, &value.to_string(), false) {
        Ok(_) => {
            if let Ok(reloaded) = Config::load_from(&cfg.home) {
                *cfg = reloaded;
            }
            None
        }
        Err(e) => Some(message_frame(&format!("config set {key}: {e:#}"))),
    }
}

/// `plugins.<id>.enabled` for a catalogue id (D23: Registry, not a second list).
fn allowlisted_plugin_enabled(cfg: &Config, key: &str) -> bool {
    let Some(rest) = key.strip_prefix("plugins.") else {
        return false;
    };
    let Some(id) = rest.strip_suffix(".enabled") else {
        return false;
    };
    if id.is_empty() || id.contains('.') {
        return false;
    }
    Registry::new(cfg)
        .manifests()
        .iter()
        .any(|(m, _)| m.id == id)
}

fn message_frame(text: &str) -> String {
    json!({ "type": "message", "text": text }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(origin: Option<&str>, host: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("host", host.parse().unwrap());
        if let Some(o) = origin {
            h.insert("origin", o.parse().unwrap());
        }
        h
    }

    /// T193: same-origin loopback and header-less clients pass; a foreign page, an
    /// opaque `null` origin and a DNS-rebinding name (matching Origin and Host) do not.
    #[test]
    fn origin_gate_blocks_cross_site_and_rebinding() {
        for (origin, host) in [
            (None, "evil.example:4444"),
            (Some("http://127.0.0.1:4444"), "127.0.0.1:4444"),
            (Some("http://localhost:4444"), "localhost:4444"),
            (Some("http://[::1]:4444"), "[::1]:4444"),
            (Some("http://192.168.1.5:4444"), "192.168.1.5:4444"),
        ] {
            assert!(origin_allowed(&headers(origin, host)), "{origin:?} {host}");
        }
        for (origin, host) in [
            ("http://evil.example", "127.0.0.1:4444"),
            ("null", "127.0.0.1:4444"),
            ("http://evil.example:4444", "evil.example:4444"),
        ] {
            assert!(
                !origin_allowed(&headers(Some(origin), host)),
                "{origin} {host}"
            );
        }
    }

    /// The port is part of the origin: a page served by another local app
    /// (`http://localhost:3000`) must not open the dashboard socket on `:4444`, where it
    /// could read archived payloads through `expand` and toggle plugins through `set`.
    #[test]
    fn origin_gate_blocks_another_port_on_the_same_host() {
        for (origin, host) in [
            ("http://localhost:3000", "localhost:4444"),
            ("http://127.0.0.1:3000", "127.0.0.1:4444"),
            ("http://[::1]:3000", "[::1]:4444"),
            ("http://127.0.0.1", "127.0.0.1:4444"),
        ] {
            assert!(
                !origin_allowed(&headers(Some(origin), host)),
                "{origin} {host}"
            );
        }
        for (origin, host) in [
            ("http://127.0.0.1", "127.0.0.1"),
            ("http://127.0.0.1:80", "127.0.0.1"),
            ("https://localhost", "localhost:443"),
        ] {
            assert!(
                origin_allowed(&headers(Some(origin), host)),
                "{origin} {host}"
            );
        }
    }
}
