//! T5.0: httpmock upstream harness for Anthropic and OpenAI wires.
//!
//! Cargo compiles this as `tests/proxy.rs` (not `tests/proxy/mod.rs`) so
//! `cargo test proxy_mock` is a real test filter. Point rtok at
//! [`MockUpstream::base_url`] via `proxy.upstream` / `proxy.openai_upstream`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use httpmock::Mock;
use httpmock::prelude::*;

const ANTHROPIC_MESSAGES_BODY: &[u8] =
    include_bytes!("fixtures/proxy/anthropic_messages_body.json");
const ANTHROPIC_MESSAGES_STREAM: &[u8] =
    include_bytes!("fixtures/proxy/anthropic_messages_stream.json");
const OPENAI_CHAT_BODY: &[u8] = include_bytes!("fixtures/proxy/openai_chat_body.json");
const OPENAI_CHAT_STREAM: &[u8] = include_bytes!("fixtures/proxy/openai_chat_stream.json");
const OPENAI_RESPONSES_BODY: &[u8] = include_bytes!("fixtures/proxy/openai_responses_body.json");
const OPENAI_RESPONSES_STREAM: &[u8] =
    include_bytes!("fixtures/proxy/openai_responses_stream.json");

/// One mock route serving a fixture body. T5.1+ reuse this for passthrough checks.
pub struct MockUpstream {
    pub server: &'static MockServer,
    mock: Mock<'static>,
    fixture: &'static [u8],
}

impl MockUpstream {
    pub fn anthropic_messages_body() -> Self {
        Self::mount("/v1/messages", ANTHROPIC_MESSAGES_BODY, "application/json")
    }
    pub fn anthropic_messages_stream() -> Self {
        Self::mount(
            "/v1/messages",
            ANTHROPIC_MESSAGES_STREAM,
            "text/event-stream",
        )
    }
    pub fn openai_chat_body() -> Self {
        Self::mount("/v1/chat/completions", OPENAI_CHAT_BODY, "application/json")
    }
    pub fn openai_chat_stream() -> Self {
        Self::mount(
            "/v1/chat/completions",
            OPENAI_CHAT_STREAM,
            "text/event-stream",
        )
    }
    pub fn openai_responses_body() -> Self {
        Self::mount("/v1/responses", OPENAI_RESPONSES_BODY, "application/json")
    }
    pub fn openai_responses_stream() -> Self {
        Self::mount(
            "/v1/responses",
            OPENAI_RESPONSES_STREAM,
            "text/event-stream",
        )
    }

    fn mount(path: &str, fixture: &'static [u8], content_type: &str) -> Self {
        let server = Box::leak(Box::new(MockServer::start()));
        let mock = server.mock(|when, then| {
            when.method(POST).path(path);
            then.status(200)
                .header("content-type", content_type)
                .body(fixture);
        });
        Self {
            server,
            mock,
            fixture,
        }
    }

    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    pub fn assert_passthrough_bytes(&self, got: &[u8]) {
        assert_eq!(got, self.fixture, "response bytes must match the fixture");
    }

    pub fn assert_upstream_called_once(&self) {
        self.mock.assert();
    }
}

fn post(up: &MockUpstream, path: &str) -> Vec<u8> {
    let host = up.server.host();
    let port = up.server.port();
    let mut stream = TcpStream::connect((host.as_str(), port)).expect("connect mock");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let body = br#"{"model":"test"}"#;
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(req.as_bytes()).expect("headers");
    stream.write_all(body).expect("body");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read");
    let sep = b"\r\n\r\n";
    let at = buf
        .windows(4)
        .position(|w| w == sep)
        .expect("HTTP header end");
    buf[at + 4..].to_vec()
}

fn roundtrip(up: MockUpstream, path: &str) {
    let got = post(&up, path);
    up.assert_passthrough_bytes(&got);
    up.assert_upstream_called_once();
}

#[test]
fn proxy_mock_anthropic_messages_body() {
    roundtrip(MockUpstream::anthropic_messages_body(), "/v1/messages");
}
#[test]
fn proxy_mock_anthropic_messages_stream() {
    roundtrip(MockUpstream::anthropic_messages_stream(), "/v1/messages");
}
#[test]
fn proxy_mock_openai_chat_body() {
    roundtrip(MockUpstream::openai_chat_body(), "/v1/chat/completions");
}
#[test]
fn proxy_mock_openai_chat_stream() {
    roundtrip(MockUpstream::openai_chat_stream(), "/v1/chat/completions");
}
#[test]
fn proxy_mock_openai_responses_body() {
    roundtrip(MockUpstream::openai_responses_body(), "/v1/responses");
}
#[test]
fn proxy_mock_openai_responses_stream() {
    roundtrip(MockUpstream::openai_responses_stream(), "/v1/responses");
}

// ── T5.1: passthrough proxy — identical bytes plus usage/calls/call_io/tokens rows ──

use std::sync::Arc;

use rtok::config::Config;
use rtok::proxy::{ProxyState, app};
use rtok::store::Store;
use rtok::store::UsageRow;

const T51_MODEL: &str = "claude-sonnet-4-20250514";
const T51_SESSION: &str = "sess-t51";

fn t51_request() -> Vec<u8> {
    format!(
        r#"{{"model":"{T51_MODEL}","max_tokens":8,"messages":[{{"role":"user","content":"hi"}}],"metadata":{{"user_id":"{T51_SESSION}"}}}}"#
    )
    .into_bytes()
}

async fn t51_server(
    label: &str,
    up: &MockUpstream,
) -> (
    String,
    Arc<ProxyState>,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let dir = std::env::temp_dir().join(format!("rtok-proxy-t51-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = Config::load_from(&dir).expect("config");
    cfg.proxy.upstream = up.base_url();
    let state = Arc::new(ProxyState::new(&cfg).expect("proxy state"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr").to_string();
    let task = tokio::spawn(axum::serve(listener, app(state.clone())).into_future());
    (addr, state, task)
}

async fn t51_post(addr: &str, body: Vec<u8>) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{addr}/v1/messages"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("request through the proxy")
}

/// The recorder task writes rows after the body was forwarded; poll briefly for them.
async fn t51_usage(state: &Store, session: &str) -> Vec<UsageRow> {
    for _ in 0..400 {
        let rows = state.usage_rows(session).expect("usage read");
        if !rows.is_empty() {
            return rows;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("usage row for {session} never appeared");
}

#[tokio::test]
async fn proxy_passthrough_body_records_usage_rows() {
    let up = MockUpstream::anthropic_messages_body();
    let (addr, state, task) = t51_server("body", &up).await;
    let resp = t51_post(&addr, t51_request()).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let bytes = resp.bytes().await.expect("response body");
    up.assert_passthrough_bytes(&bytes);
    up.assert_upstream_called_once();

    let rows = t51_usage(&state.store, T51_SESSION).await;
    assert_eq!(rows.len(), 1, "exactly one usage row");
    let u = &rows[0];
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (10, 0, 0, 2),
        "usage row carries the four provider counters"
    );
    assert_eq!(u.model.as_deref(), Some(T51_MODEL));
    let call_id = u.call_id.expect("usage.call_id points at the calls row") as i32;
    assert_eq!(
        state
            .store
            .model_slug_of_call(call_id)
            .expect("slug")
            .as_deref(),
        Some(T51_MODEL),
        "models.slug must equal the request model"
    );
    assert_eq!(state.store.count_kind("api_request").expect("calls"), 1);
    assert_eq!(state.store.count_call_io().expect("call_io"), 1);
    assert_eq!(state.store.count_tokens().expect("tokens"), 1);
    task.abort();
}

#[tokio::test]
async fn proxy_passthrough_stream_is_byte_identical_and_records_usage() {
    let up = MockUpstream::anthropic_messages_stream();
    let (addr, state, task) = t51_server("stream", &up).await;
    let resp = t51_post(&addr, t51_request()).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|c| c.contains("text/event-stream"))
            .unwrap_or(false),
        "SSE content-type must pass through"
    );
    let bytes = resp.bytes().await.expect("response body");
    up.assert_passthrough_bytes(&bytes);
    up.assert_upstream_called_once();

    let rows = t51_usage(&state.store, T51_SESSION).await;
    assert_eq!(rows.len(), 1, "exactly one usage row");
    let u = &rows[0];
    // The fixture's message_start carries no usage; the final message_delta has output only.
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (0, 0, 0, 2),
        "usage merged from SSE message_delta"
    );
    assert_eq!(state.store.count_kind("api_request").expect("calls"), 1);
    assert_eq!(state.store.count_call_io().expect("call_io"), 1);
    task.abort();
}

#[tokio::test]
async fn proxy_health_reports_ok_and_mode() {
    let up = MockUpstream::anthropic_messages_body();
    let (addr, _state, task) = t51_server("health", &up).await;
    let resp = reqwest::Client::new()
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("health");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body = resp.text().await.expect("health body");
    let v: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(v, serde_json::json!({"ok": true, "mode": "passthrough"}));
    task.abort();
}
