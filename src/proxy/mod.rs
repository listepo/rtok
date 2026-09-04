//! `src/proxy` — the local API proxy (plan P5). `rtok proxy` serves here.
//!
//! T5.1 scope: passthrough. Every request is forwarded byte-identical to
//! `proxy.upstream` (default `https://api.anthropic.com`; point it at
//! `http://127.0.0.1:8788` to chain behind another proxy during A/B). SSE responses
//! stream through unchanged — chunks are tee'd into a buffer *while* the client
//! receives them, never before (a spawned task does the bookkeeping afterwards).
//! `/v1/chat/completions` goes to `proxy.openai_upstream` (T11.2), adding
//! `stream_options.include_usage` to streaming requests that omit it so the final
//! chunk reports usage. `/v1/responses` goes there too (T11.3) and needs no shaping —
//! it reports usage on its final `response.completed` event unasked.
//!
//! Bookkeeping per request (all fail-open, logged, never alter the response):
//! one `calls` row (`kind = api_request`, `surface = proxy`) with provider+model
//! upserted from the request body; `call_io` with request/response bytes (inline
//! under `core.call_io_inline_bytes`, else archived); a `tokens` row
//! (`source = provider`) with the four counters; and one `usage` row whose
//! `call_id` points at the `calls` row. Session id: `metadata.user_id`, else the
//! `x-rtok-session` header, else the sha256 of the request body.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderName, Request, Response, StatusCode};
use axum::response::Response as AxumResponse;
use axum::routing::get;
use futures_util::StreamExt;
use futures_util::stream::unfold;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::plugin::Ctx;
use crate::plugins::Registry;
use crate::store::Store;
use wire::{Wire, WireRequest};

pub mod anthropic;
pub mod cli;
pub mod openai_chat;
pub mod openai_responses;
pub mod wire;

/// Request bodies are JSON and bounded by the Anthropic/OpenAI API limits; cap the
/// in-memory read well above them.
const MAX_REQUEST_BYTES: usize = 256 * 1024 * 1024;

/// Shared server state: the DB, the upstream client and the effective `[proxy]` settings.
pub struct ProxyState {
    pub store: Store,
    client: Client,
    upstream: String,
    /// Where the OpenAI wires go; Anthropic paths keep using `upstream` (D11).
    openai_upstream: String,
    host_id: Option<i32>,
    inline_cap: usize,
    archive_dir: Option<PathBuf>,
    pub mode: String,
    /// `compress` mode (T5.3): every enabled plugin's `proxy_filter` runs on `/v1/messages`.
    registry: Registry,
    cfg: Config,
}

impl ProxyState {
    pub fn new(cfg: &Config) -> Result<Self> {
        let store = Store::open(&cfg.core.db_path)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(cfg.proxy.timeout_s.max(1)))
            .build()
            .context("reqwest client")?;
        // Host agent: the `[hook] host` setting (T5.1 says `core.host`, which T12 removed —
        // see plan.md §6 amendment in the T5.1 commit). Unknown slugs fall back to `other` (6).
        let host_id = store.host_id(&cfg.hook.host)?.or(Some(6));
        Ok(Self {
            store,
            client,
            upstream: cfg.proxy.upstream.trim_end_matches('/').to_string(),
            openai_upstream: cfg.proxy.openai_upstream.trim_end_matches('/').to_string(),
            host_id,
            inline_cap: cfg.core.call_io_inline_bytes as usize,
            archive_dir: Some(cfg.core.archive_dir.clone()),
            mode: cfg.proxy.mode.clone(),
            registry: Registry::new(cfg),
            cfg: cfg.clone(),
        })
    }

    /// The upstream owning `wire`. Paths this build has no wire for keep the Anthropic
    /// default, which is what they did before P11 (`/v1/responses` until T11.3).
    fn upstream_for(&self, wire: Option<&'static dyn Wire>) -> &str {
        match wire.map(Wire::provider) {
            Some("openai") => &self.openai_upstream,
            _ => &self.upstream,
        }
    }
}

/// The axum app: `/health` plus a fallback that forwards every other path upstream.
pub fn app(state: Arc<ProxyState>) -> Router {
    Router::new()
        .route("/health", get(cli::health))
        .fallback(proxy)
        .with_state(state)
}

/// `rtok proxy` (cli.rs): run until killed.
pub fn serve_blocking(cfg: Config) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;
    rt.block_on(serve(&cfg))
}

/// Bind `bind:port` and serve. Tests bind their own ephemeral listener and call
/// [`app`] directly instead.
pub async fn serve(cfg: &Config) -> Result<()> {
    let state = Arc::new(ProxyState::new(cfg)?);
    crate::otel::export::spawn_tick(cfg);
    let addr = format!("{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    axum::serve(listener, app(state))
        .await
        .context("proxy server")
}

async fn proxy(State(state): State<Arc<ProxyState>>, req: Request<Body>) -> AxumResponse {
    handle(state, req).await
}

async fn handle(state: Arc<ProxyState>, req: Request<Body>) -> AxumResponse {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let (parts, body) = req.into_parts();
    let headers = parts.headers.clone();

    let request_body = match axum::body::to_bytes(body, MAX_REQUEST_BYTES).await {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    // Request bookkeeping (fail-open: a DB error logs and the request still goes through).
    let parsed = serde_json::from_slice::<Value>(&request_body).ok();
    let wire = wire::for_path(&path);
    let recorded = record(
        &state,
        wire,
        &path,
        parsed.as_ref(),
        &headers,
        &request_body,
    );
    // From here on `request_body` is what upstream sees (and what `call_io` records).
    let request_body = if state.mode == "compress" {
        wire.map_or(request_body.clone(), |wire| {
            compress(&state, wire, parsed, recorded.as_ref(), request_body)
        })
    } else {
        request_body
    };
    // Provider request shaping runs in both modes (T11.2: OpenAI `stream_options`).
    let request_body = match wire {
        Some(wire) => prepare(&state, wire, request_body),
        None => request_body,
    };

    let target = match upstream_url(state.upstream_for(wire), &path, query.as_deref()) {
        Ok(u) => u,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let mut rb = state.client.request(method.clone(), target);
    for (name, value) in headers.iter() {
        if !hop_by_hop(name.as_str()) {
            rb = rb.header(name, value);
        }
    }
    let upstream = match rb.body(request_body.clone()).send().await {
        Ok(r) => r,
        Err(e) => {
            log_err(
                &state,
                &recorded,
                start,
                &format!("upstream {method} {path}: {e}"),
            );
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("rtok proxy: upstream error: {e}"),
            );
        }
    };

    let content_type = upstream
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let status = upstream.status();
    let mut out_headers: Vec<(HeaderName, axum::http::HeaderValue)> = Vec::new();
    for (name, value) in upstream.headers().iter() {
        if !hop_by_hop(name.as_str()) {
            out_headers.push((name.clone(), value.clone()));
        }
    }

    // Tee the upstream body: forward each chunk to the client *and* buffer it for the
    // `call_io`/`usage` rows, which the spawned task writes after the stream ends.
    let (tx, rx) = mpsc::channel::<Result<Bytes, io::Error>>(32);
    let recorder = state.clone();
    let body_stream = upstream.bytes_stream();
    tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::new();
        let mut stream = body_stream;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    buf.extend_from_slice(&bytes);
                    if tx.send(Ok(bytes)).await.is_err() {
                        break; // client went away; record what we have
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(io::Error::other(e))).await;
                    break;
                }
            }
        }
        drop(tx);
        finish(
            &recorder,
            &recorded,
            start,
            wire,
            content_type.as_deref(),
            &request_body,
            &buf,
        )
        .await;
    });

    let client_stream = unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    let response_body = Body::from_stream(client_stream);

    let mut response = Response::builder().status(status);
    for (name, value) in out_headers {
        response = response.header(name, value);
    }
    match response.body(response_body) {
        Ok(r) => r,
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `compress` mode: run every enabled plugin's `proxy_filter` over the parsed body and
/// forward the re-serialised result. Fail open: no parse, no `calls` row, a store error, or
/// no change at all → the original bytes go through untouched.
/// Let the wire shape the outgoing request (T11.2). Fail open: an unparseable or
/// unchanged body is forwarded exactly as it arrived.
fn prepare(state: &ProxyState, wire: &'static dyn Wire, original: Bytes) -> Bytes {
    let Ok(mut body) = serde_json::from_slice::<Value>(&original) else {
        return original;
    };
    if !wire.prepare_request(&mut body, state.cfg.proxy.include_usage) {
        return original;
    }
    serde_json::to_vec(&body).map_or(original, Bytes::from)
}

fn compress(
    state: &ProxyState,
    wire: &'static dyn Wire,
    parsed: Option<Value>,
    recorded: Option<&Recorded>,
    original: Bytes,
) -> Bytes {
    let (Some(mut body), Some(r)) = (parsed, recorded) else {
        return original;
    };
    let mut cx = match Ctx::open(state.cfg.clone(), r.session.clone()) {
        Ok(cx) => cx,
        Err(e) => {
            log(
                &state.store,
                &r.session,
                Some(r.call_id),
                "error",
                &format!("ctx: {e}"),
            );
            return original;
        }
    };
    cx.call_id = Some(r.call_id);
    let changed = {
        let mut changed = false;
        let mut request = WireRequest::new(wire, &mut body);
        for p in state.registry.enabled() {
            for m in p.proxy_filter(&mut request, &cx) {
                changed = true;
                if let Err(e) = cx.record(&m) {
                    log(
                        &state.store,
                        &r.session,
                        Some(r.call_id),
                        "error",
                        &format!("measurement: {e}"),
                    );
                }
            }
        }
        changed
    };
    if !changed {
        return original;
    }
    serde_json::to_vec(&body)
        .map(Bytes::from)
        .unwrap_or(original)
}

/// One request's bookkeeping: session, host, provider+model, and the `calls` row.
fn record(
    state: &ProxyState,
    wire: Option<&'static dyn Wire>,
    path: &str,
    body: Option<&Value>,
    headers: &HeaderMap,
    raw: &[u8],
) -> Option<Recorded> {
    let session = session_for(wire, body, headers, raw);
    let model = body
        .and_then(|v| v.get("model").and_then(Value::as_str))
        .map(str::to_string);
    let provider = wire
        .map(Wire::provider)
        .or_else(|| matches!(path, "/v1/chat/completions" | "/v1/responses").then_some("openai"));
    let result = (|| -> Result<Recorded> {
        state
            .store
            .upsert_session(&session, state.host_id, None, None, Some("proxy"))?;
        let (provider_id, model_id) = match (provider, model.as_deref()) {
            (Some(p), Some(m)) => {
                let (pid, mid) = state.store.upsert_model(p, m)?;
                (Some(pid), Some(mid))
            }
            _ => (None, None),
        };
        let call_id = state.store.insert_call(
            &session,
            "proxy",
            "api_request",
            state.host_id,
            provider_id,
            model_id,
            None,
            Some(path),
        )?;
        Ok(Recorded {
            session: session.clone(),
            model,
            call_id,
        })
    })();
    match result {
        Ok(r) => Some(r),
        Err(e) => {
            log(
                &state.store,
                &session,
                None,
                "error",
                &format!("record {path}: {e}"),
            );
            None
        }
    }
}

/// After the body was fully forwarded: `calls.ms`, `call_io`, then `usage` + provider
/// `tokens` when the response carried a usage block. All best-effort.
async fn finish(
    state: &ProxyState,
    recorded: &Option<Recorded>,
    start: Instant,
    wire: Option<&'static dyn Wire>,
    content_type: Option<&str>,
    request_body: &[u8],
    response_body: &[u8],
) {
    let Some(r) = recorded else { return };
    let session = r.session.clone();
    let log_err = |what: &str, e: anyhow::Error| {
        let _ = state.store.insert_log(
            "error",
            "module",
            "proxy",
            &format!("{what}: {e:#}"),
            Some(&session),
            Some(r.call_id),
            None,
        );
    };
    if let Err(e) = state
        .store
        .set_call_ms(r.call_id, start.elapsed().as_secs_f64() * 1000.0)
    {
        log_err("set_call_ms", e);
    }
    if let Err(e) = state.store.insert_call_io(
        r.call_id,
        Some(request_body),
        Some(response_body),
        state.inline_cap,
        state.archive_dir.as_deref(),
    ) {
        log_err("call_io", e);
    }
    match wire.and_then(|wire| wire::usage_from_response(wire, content_type, response_body)) {
        Some(usage) => {
            if let Err(e) = state.store.insert_usage(
                &r.session,
                r.model.as_deref(),
                wire.map(Wire::api).unwrap_or("anthropic"),
                usage.input,
                usage.cache_create,
                usage.cache_read,
                usage.output,
                r.call_id,
            ) {
                log_err("usage", e);
            }
            if let Err(e) = state.store.insert_provider_tokens(
                r.call_id,
                usage.input,
                usage.cache_create,
                usage.cache_read,
                usage.output,
            ) {
                log_err("tokens", e);
            }
        }
        None => log(
            &state.store,
            &session,
            Some(r.call_id),
            "info",
            "no usage in upstream response",
        ),
    }
}

fn log_err(state: &ProxyState, recorded: &Option<Recorded>, start: Instant, msg: &str) {
    let session = recorded
        .as_ref()
        .map(|r| r.session.clone())
        .unwrap_or_else(|| "?".to_string());
    let call_id = recorded.as_ref().map(|r| r.call_id);
    if let Some(r) = recorded {
        let _ = state
            .store
            .set_call_ms(r.call_id, start.elapsed().as_secs_f64() * 1000.0);
    }
    log(&state.store, &session, call_id, "error", msg);
}

fn log(store: &Store, session: &str, call_id: Option<i32>, level: &str, message: &str) {
    let _ = store.insert_log(
        level,
        "module",
        "proxy",
        message,
        Some(session),
        call_id,
        None,
    );
}

struct Recorded {
    session: String,
    model: Option<String>,
    call_id: i32,
}

fn session_for(
    wire: Option<&dyn Wire>,
    body: Option<&Value>,
    headers: &HeaderMap,
    raw: &[u8],
) -> String {
    if let Some(session) = wire.and_then(|wire| body.and_then(|body| wire.session_id(body))) {
        return session.to_string();
    }
    if let Some(session) = body
        .and_then(|body| body.get("metadata"))
        .and_then(|metadata| metadata.get("user_id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    {
        return session.to_string();
    }
    for name in ["x-rtok-session", "x-session-id"] {
        if let Some(v) = headers
            .get(HeaderName::from_static(name))
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
        {
            return v.to_string();
        }
    }
    let mut h = Sha256::new();
    h.update(raw);
    format!("{:x}", h.finalize())
}

fn upstream_url(base: &str, path: &str, query: Option<&str>) -> Result<String> {
    if base.is_empty() {
        anyhow::bail!("proxy.upstream is empty");
    }
    let mut url = format!("{base}{path}");
    if let Some(q) = query {
        url.push('?');
        url.push_str(q);
    }
    Ok(url)
}

/// Headers that must not be forwarded (HTTP/1.1 hop-by-hop + framing). `content-length`
/// is dropped because the proxy re-chunks; the body bytes themselves are untouched.
fn hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "host"
    )
}

fn error_response(status: StatusCode, message: &str) -> AxumResponse {
    let body = format!(
        r#"{{"type":"error","error":{{"type":"{}","message":{}}}}}"#,
        if status == StatusCode::BAD_GATEWAY {
            "upstream_error"
        } else {
            "invalid_request_error"
        },
        serde_json::to_string(message).unwrap_or_else(|_| "\"proxy error\"".to_string())
    );
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("static error response")
}
