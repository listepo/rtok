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
