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
    mode: &str,
) -> (
    String,
    Arc<ProxyState>,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let dir = std::env::temp_dir().join(format!("rtok-proxy-t51-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = Config::load_from(&dir).expect("config");
    cfg.proxy.upstream = up.base_url();
    cfg.proxy.mode = mode.to_string();
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

/// The recorder task writes rows after the body was forwarded; poll briefly for `n` of them.
async fn t51_usage_n(state: &Store, session: &str, n: usize) -> Vec<UsageRow> {
    for _ in 0..400 {
        let rows = state.usage_rows(session).expect("usage read");
        if rows.len() >= n {
            return rows;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("usage row for {session} never appeared");
}

async fn t51_usage(state: &Store, session: &str) -> Vec<UsageRow> {
    t51_usage_n(state, session, 1).await
}

#[tokio::test]
async fn proxy_passthrough_body_records_usage_rows() {
    let up = MockUpstream::anthropic_messages_body();
    let (addr, state, task) = t51_server("body", &up, "passthrough").await;
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
    let (addr, state, task) = t51_server("stream", &up, "passthrough").await;
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
    let (addr, _state, task) = t51_server("health", &up, "passthrough").await;
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

// ── T11.2: OpenAI Chat Completions wire — own upstream, usage, stream_options ──

const T112_SESSION: &str = "sess-t112";
const T112_MODEL: &str = "gpt-4o";

/// Points `proxy.openai_upstream` (not `proxy.upstream`) at the mock, so a request that
/// reaches the fixture proves the OpenAI wire picked the OpenAI upstream.
async fn openai_server(
    label: &str,
    up: &MockUpstream,
    mode: &str,
) -> (
    String,
    Arc<ProxyState>,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let dir = std::env::temp_dir().join(format!("rtok-proxy-t11-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = Config::load_from(&dir).expect("config");
    cfg.proxy.upstream = "http://127.0.0.1:1".to_string(); // Anthropic upstream must go unused
    cfg.proxy.openai_upstream = up.base_url();
    cfg.proxy.mode = mode.to_string();
    let state = Arc::new(ProxyState::new(&cfg).expect("proxy state"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr").to_string();
    let task = tokio::spawn(axum::serve(listener, app(state.clone())).into_future());
    (addr, state, task)
}

async fn openai_post(addr: &str, path: &str, body: Vec<u8>) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{addr}{path}"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("request through the proxy")
}

async fn t112_post(addr: &str, body: serde_json::Value) -> reqwest::Response {
    openai_post(
        addr,
        "/v1/chat/completions",
        serde_json::to_vec(&body).expect("request json"),
    )
    .await
}

#[tokio::test]
async fn proxy_openai_chat_body_records_usage_with_cached_tokens() {
    let up = MockUpstream::openai_chat_body();
    let (addr, state, task) = openai_server("chat-body", &up, "passthrough").await;
    let resp = t112_post(
        &addr,
        serde_json::json!({"model": T112_MODEL, "user": T112_SESSION,
                           "messages":[{"role":"user","content":"hi"}]}),
    )
    .await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    up.assert_passthrough_bytes(&resp.bytes().await.expect("response body"));
    up.assert_upstream_called_once();

    let rows = t51_usage(&state.store, T112_SESSION).await;
    assert_eq!(rows.len(), 1, "exactly one usage row");
    let u = &rows[0];
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (10, 0, 7, 2),
        "cache_read comes from prompt_tokens_details.cached_tokens; no cache_create on this wire"
    );
    assert_eq!(u.model.as_deref(), Some(T112_MODEL));
    task.abort();
}

#[tokio::test]
async fn proxy_openai_chat_stream_is_byte_identical_and_adds_include_usage() {
    let up = MockUpstream::openai_chat_stream();
    let (addr, state, task) = openai_server("chat-stream", &up, "passthrough").await;
    let resp = t112_post(
        &addr,
        serde_json::json!({"model": T112_MODEL, "user": T112_SESSION, "stream": true,
                           "messages":[{"role":"user","content":"hi"}]}),
    )
    .await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    up.assert_passthrough_bytes(&resp.bytes().await.expect("response body"));

    let rows = t51_usage(&state.store, T112_SESSION).await;
    let u = &rows[0];
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (10, 0, 7, 2),
        "usage decoded from the final SSE chunk"
    );
    // The one byte-level change passthrough makes: the request now opts into stream usage.
    let sent = state
        .store
        .call_io_request(u.call_id.expect("call id") as i32)
        .expect("call_io")
        .expect("request");
    let sent: serde_json::Value = serde_json::from_slice(&sent).expect("json");
    assert_eq!(
        sent["stream_options"]["include_usage"],
        serde_json::json!(true)
    );
    assert_eq!(
        sent["messages"][0]["content"], "hi",
        "nothing else rewritten"
    );
    task.abort();
}

// ── T11.3: OpenAI Responses wire — own upstream, body/SSE usage, server-side history ──

const T113_SESSION: &str = "sess-t113";
const T113_MODEL: &str = "gpt-4.1";
const T113_COLLIDING_CALL: &str = "call-shared-across-wires";
const T113_NEW_CALL: &str = "call-responses-only";

fn t113_previous_response_request() -> Vec<u8> {
    let output = (1..=400)
        .map(|i| format!("old line {i}: some tool output with words"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::to_vec(&serde_json::json!({
        "model": T113_MODEL,
        "user": T113_SESSION,
        "previous_response_id": "resp_previous",
        "input": [
            {"type": "function_call_output", "call_id": "call-old", "output": output},
            {"role": "user", "content": "one"},
            {"role": "user", "content": "two"},
            {"role": "user", "content": "three"}
        ]
    }))
    .expect("request json")
}

fn t113_regular_noncanonical_request() -> Vec<u8> {
    let output = (1..=400)
        .map(|i| format!("old line {i}: some tool output with words"))
        .collect::<Vec<_>>()
        .join("\n");
    let output = serde_json::to_string(&output).expect("output json");
    format!(
        r#"{{  "user" : "{T113_SESSION}", "input" : [
          {{ "type" : "function_call_output", "call_id" : "{T113_COLLIDING_CALL}", "output" : {output} }},
          {{ "type" : "function_call_output", "call_id" : "{T113_NEW_CALL}", "output" : {output} }},
          {{ "role" : "user", "content" : "one" }},
          {{ "role" : "user", "content" : "two" }},
          {{ "role" : "user", "content" : "three" }}
        ], "model" : "{T113_MODEL}" }}"#
    )
    .into_bytes()
}

#[tokio::test]
async fn proxy_openai_responses_body_records_usage_without_rewriting_previous_response() {
    let up = MockUpstream::openai_responses_body();
    let (addr, state, task) = openai_server("responses-body", &up, "compress").await;
    let request = t113_previous_response_request();
    let resp = openai_post(&addr, "/v1/responses", request.clone()).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    up.assert_passthrough_bytes(&resp.bytes().await.expect("response body"));
    up.assert_upstream_called_once();

    let rows = t51_usage(&state.store, T113_SESSION).await;
    let u = &rows[0];
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (10, 0, 7, 2),
        "cache_read comes from input_tokens_details.cached_tokens; no cache_create on this wire"
    );
    assert_eq!(u.model.as_deref(), Some(T113_MODEL));
    let sent = state
        .store
        .call_io_request(u.call_id.expect("call id") as i32)
        .expect("call_io")
        .expect("request");
    assert_eq!(
        sent, request,
        "previous_response_id must produce zero rewrites"
    );
    assert_eq!(
        state
            .store
            .measurement_count("archive")
            .expect("measurements"),
        0
    );
    task.abort();
}

#[tokio::test]
async fn proxy_openai_responses_compress_is_byte_exact_and_ignores_archive_decisions() {
    let up = MockUpstream::openai_responses_body();
    let (addr, state, task) = openai_server("responses-compress", &up, "compress").await;
    let archive_dir =
        std::env::temp_dir().join(format!("rtok-proxy-t113-collision-{}", std::process::id()));
    let archive_id = state
        .store
        .put_archive("other-session", b"unrelated output", &archive_dir)
        .expect("seed archive");
    let collision_pointer = "[archived unrelated]";
    state
        .store
        .put_archive_decision(
            T113_COLLIDING_CALL,
            &archive_id,
            "other-session",
            collision_pointer,
        )
        .expect("seed colliding archive decision");

    let request = t113_regular_noncanonical_request();
    let resp = openai_post(&addr, "/v1/responses", request.clone()).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    up.assert_passthrough_bytes(&resp.bytes().await.expect("response body"));

    let rows = t51_usage(&state.store, T113_SESSION).await;
    let sent = state
        .store
        .call_io_request(rows[0].call_id.expect("call id") as i32)
        .expect("call_io")
        .expect("request");
    assert_eq!(
        sent, request,
        "T11.3 must preserve noncanonical Responses JSON byte-for-byte"
    );
    assert_eq!(
        state
            .store
            .archive_decision(T113_COLLIDING_CALL)
            .expect("colliding decision")
            .expect("seeded decision")
            .pointer,
        collision_pointer,
        "a decision from another API/session must not be reused"
    );
    assert!(
        state
            .store
            .archive_decision(T113_NEW_CALL)
            .expect("new decision")
            .is_none(),
        "T11.3 must not create Responses archive decisions"
    );
    assert_eq!(
        state
            .store
            .measurement_count("archive")
            .expect("measurements"),
        0
    );
    task.abort();
}

#[tokio::test]
async fn proxy_openai_responses_stream_is_byte_identical_and_records_usage() {
    let up = MockUpstream::openai_responses_stream();
    let (addr, state, task) = openai_server("responses-stream", &up, "passthrough").await;
    let request = serde_json::to_vec(&serde_json::json!({
        "model": T113_MODEL, "user": T113_SESSION, "stream": true, "input": "hi"
    }))
    .expect("request json");
    let resp = openai_post(&addr, "/v1/responses", request.clone()).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    up.assert_passthrough_bytes(&resp.bytes().await.expect("response body"));
    up.assert_upstream_called_once();

    let rows = t51_usage(&state.store, T113_SESSION).await;
    let u = &rows[0];
    assert_eq!(
        (u.input, u.cache_create, u.cache_read, u.output),
        (10, 0, 7, 2),
        "usage decoded from response.completed, cache_read from cached_tokens"
    );
    let sent = state
        .store
        .call_io_request(u.call_id.expect("call id") as i32)
        .expect("call_io")
        .expect("request");
    assert_eq!(
        sent, request,
        "Responses requests need no passthrough shaping"
    );
    task.abort();
}

const T53_SESSION: &str = "sess-t53";

/// Six user turns, each carrying one 400-line tool_result (well above `archive.min_tokens`).
fn t53_request() -> Vec<u8> {
    let mut messages = Vec::new();
    for t in 1..=6 {
        let text = (1..=400)
            .map(|i| format!("t{t} line {i}: some shell output with words"))
            .collect::<Vec<_>>()
            .join("\n");
        messages.push(serde_json::json!({"role":"user","content":[
            {"type":"tool_result","tool_use_id":format!("tu-{t}"),"content":text}]}));
        messages.push(serde_json::json!({"role":"assistant","content":[
            {"type":"tool_use","id":format!("tu-{}", t + 1),"name":"Bash","input":{}}]}));
    }
    serde_json::to_vec(&serde_json::json!({
        "model": T51_MODEL, "max_tokens": 8, "system": "sys", "tools": [{"name": "Bash"}],
        "messages": messages, "metadata": {"user_id": T53_SESSION}
    }))
    .expect("request json")
}

#[tokio::test]
async fn proxy_compress_rewrites_old_tool_results_identically() {
    let up = MockUpstream::anthropic_messages_body();
    let (addr, state, task) = t51_server("compress", &up, "compress").await;
    for _ in 0..2 {
        let resp = t51_post(&addr, t53_request()).await;
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        up.assert_passthrough_bytes(&resp.bytes().await.expect("body"));
    }
    let rows = t51_usage_n(&state.store, T53_SESSION, 2).await;
    let sent: Vec<String> = rows
        .iter()
        .map(|u| {
            let id = u.call_id.expect("call id") as i32;
            let bytes = state
                .store
                .call_io_request(id)
                .expect("call_io")
                .expect("request");
            String::from_utf8(bytes).expect("utf-8")
        })
        .collect();
    assert_eq!(
        sent[0], sent[1],
        "same request twice → byte-identical upstream bodies"
    );
    let body: serde_json::Value = serde_json::from_str(&sent[0]).expect("json");
    let contents: Vec<&str> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| m["content"][0]["content"].as_str().unwrap())
        .collect();
    assert_eq!(contents.len(), 6);
    assert!(contents[0].starts_with("[archived ") && contents[1].starts_with("[archived "));
    for (i, c) in contents.iter().enumerate().skip(2) {
        assert!(
            c.starts_with(&format!("t{} line 1:", i + 1)),
            "turn {} untouched",
            i + 1
        );
        assert!(!c.contains("[archived"));
    }
    assert_eq!(body["system"], "sys");
    assert_eq!(body["tools"][0]["name"], "Bash");
    assert_eq!(state.store.count_kind("api_request").expect("calls"), 2);
    assert_eq!(
        state.store.count_kind("plugin_run").expect("plugin runs"),
        2
    );
    assert_eq!(
        state
            .store
            .measurement_count("archive")
            .expect("measurements"),
        4
    );
    task.abort();
}

const T114_SESSION: &str = "sess-t114";
const ANTHROPIC_6TURNS: &[u8] = include_bytes!("fixtures/proxy/anthropic_messages_6turns.json");
const OPENAI_CHAT_6TURNS: &[u8] = include_bytes!("fixtures/proxy/openai_chat_6turns.json");
const OPENAI_RESPONSES_6TURNS: &[u8] =
    include_bytes!("fixtures/proxy/openai_responses_6turns.json");

struct T114Case {
    label: &'static str,
    path: &'static str,
    fixture: &'static [u8],
    openai: bool,
}

fn t114_cases() -> [T114Case; 3] {
    [
        T114Case {
            label: "anthropic",
            path: "/v1/messages",
            fixture: ANTHROPIC_6TURNS,
            openai: false,
        },
        T114Case {
            label: "chat",
            path: "/v1/chat/completions",
            fixture: OPENAI_CHAT_6TURNS,
            openai: true,
        },
        T114Case {
            label: "responses",
            path: "/v1/responses",
            fixture: OPENAI_RESPONSES_6TURNS,
            openai: true,
        },
    ]
}

fn t114_upstream(path: &str) -> MockUpstream {
    match path {
        "/v1/messages" => MockUpstream::anthropic_messages_body(),
        "/v1/chat/completions" => MockUpstream::openai_chat_body(),
        "/v1/responses" => MockUpstream::openai_responses_body(),
        _ => unreachable!(),
    }
}

fn t114_texts<'a>(path: &str, body: &'a serde_json::Value) -> Vec<&'a str> {
    match path {
        "/v1/messages" => body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| m["content"][0]["content"].as_str().unwrap())
            .collect(),
        "/v1/chat/completions" => body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "tool")
            .map(|m| m["content"].as_str().unwrap())
            .collect(),
        "/v1/responses" => body["input"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["type"] == "function_call_output")
            .map(|item| item["output"].as_str().unwrap())
            .collect(),
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn proxy_compress_archives_six_turns_on_each_wire() {
    for case in t114_cases() {
        let up = t114_upstream(case.path);
        let (addr, state, task) = if case.openai {
            openai_server(&format!("t114-{}", case.label), &up, "compress").await
        } else {
            t51_server(&format!("t114-{}", case.label), &up, "compress").await
        };
        let request = serde_json::to_vec(
            &serde_json::from_slice::<serde_json::Value>(case.fixture).expect(case.label),
        )
        .expect(case.label);
        for _ in 0..2 {
            let resp = openai_post(&addr, case.path, request.clone()).await;
            assert_eq!(resp.status(), reqwest::StatusCode::OK, "{}", case.label);
            up.assert_passthrough_bytes(&resp.bytes().await.expect("body"));
        }
        let rows = t51_usage_n(&state.store, T114_SESSION, 2).await;
        let sent: Vec<Vec<u8>> = rows
            .iter()
            .map(|u| {
                state
                    .store
                    .call_io_request(u.call_id.expect("call id") as i32)
                    .expect("call_io")
                    .expect("request")
            })
            .collect();
        assert_eq!(sent[0], sent[1], "{} byte-identical", case.label);
        let pos = sent[0]
            .windows(10)
            .position(|window| window == b"[archived ")
            .expect(case.label);
        assert_eq!(&request[..pos], &sent[0][..pos], "{} prefix", case.label);
        let body: serde_json::Value = serde_json::from_slice(&sent[0]).expect("json");
        let contents = t114_texts(case.path, &body);
        assert_eq!(contents.len(), 6, "{}", case.label);
        assert!(
            contents[0].starts_with("[archived ") && contents[1].starts_with("[archived "),
            "{}",
            case.label
        );
        for (i, c) in contents.iter().enumerate().skip(2) {
            assert!(
                c.starts_with(&format!("t{} line 1:", i + 1)),
                "{} turn {}",
                case.label,
                i + 1
            );
        }
        assert_eq!(
            state
                .store
                .measurement_count("archive")
                .expect("measurements"),
            4,
            "{}",
            case.label
        );
        task.abort();
    }
}
