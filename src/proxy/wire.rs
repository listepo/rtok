//! API-wire abstraction for proxy request rewriting and usage extraction (plan T11.1).

use serde_json::Value;

use super::anthropic::ANTHROPIC;
use super::openai_chat::OPENAI_CHAT;
use super::openai_responses::OPENAI_RESPONSES;

/// A provider request/response shape supported by the proxy.
pub trait Wire: Send + Sync {
    /// Whether this wire owns the request path.
    fn matches(&self, path: &str) -> bool;

    /// Provider slug used for the request's dimension row.
    fn provider(&self) -> &'static str;

    /// Provider session identity, when the body carries one.
    fn session_id<'a>(&self, body: &'a Value) -> Option<&'a str>;

    /// Every mutable tool-result payload, with its id and number of later user turns.
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>>;

    /// Usage from a complete JSON response body.
    fn usage_from_body(&self, body: &Value) -> Option<Usage>;

    /// Usage from one decoded SSE data event.
    fn usage_from_sse(&self, event: &Value) -> Option<Usage>;

    /// Provider-specific request shaping applied in both proxy modes, before forwarding.
    /// `include_usage` is `[proxy] include_usage`; wires that need no shaping ignore it.
    /// Returns whether `body` changed — only then is the request re-serialised.
    fn prepare_request(&self, _body: &mut Value, _include_usage: bool) -> bool {
        false
    }
}

/// A wire-normalised tool result. `turn` is the number of user turns that follow it.
pub struct ToolResultRef<'a> {
    /// Provider-stable result id, used as the archive-decision key.
    pub id: String,
    /// The provider's mutable result payload.
    pub content: &'a mut Value,
    /// User turns after this result; the archive plugin protects the live tail.
    pub turn: usize,
}

/// Provider usage counters, with absent provider fields represented as zero.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
}

impl Usage {
    /// Keep the latest value of every counter carried by a stream event.
    pub fn merge(&mut self, next: Self) {
        if next.input != 0 {
            self.input = next.input;
        }
        if next.cache_create != 0 {
            self.cache_create = next.cache_create;
        }
        if next.cache_read != 0 {
            self.cache_read = next.cache_read;
        }
        if next.output != 0 {
            self.output = next.output;
        }
    }
}

/// A parsed request paired with its selected provider wire.
pub struct WireRequest<'a> {
    wire: &'static dyn Wire,
    body: &'a mut Value,
}

impl<'a> WireRequest<'a> {
    pub fn new(wire: &'static dyn Wire, body: &'a mut Value) -> Self {
        Self { wire, body }
    }

    /// Expose only provider-normalised tool results to request-rewriting plugins.
    pub fn tool_results(&mut self) -> Vec<ToolResultRef<'_>> {
        self.wire.tool_results(self.body)
    }
}

/// The wire matching `path`, if this build understands it.
pub fn for_path(path: &str) -> Option<&'static dyn Wire> {
    const WIRES: [&(dyn Wire + 'static); 3] = [&ANTHROPIC, &OPENAI_CHAT, &OPENAI_RESPONSES];
    WIRES.into_iter().find(|wire| wire.matches(path))
}

/// Read a non-empty string field, which is how both OpenAI wires carry the session id.
pub(super) fn str_field<'a>(body: &'a Value, name: &str) -> Option<&'a str> {
    body.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// Read an integer usage counter, treating an absent or non-integer field as zero.
pub(super) fn int_field(usage: &Value, name: &str) -> i64 {
    usage.get(name).and_then(Value::as_i64).unwrap_or_default()
}

/// Decode JSON or SSE response usage through the selected provider wire.
pub fn usage_from_response(
    wire: &dyn Wire,
    content_type: Option<&str>,
    body: &[u8],
) -> Option<Usage> {
    let text = std::str::from_utf8(body).ok()?;
    if content_type.is_some_and(|c| c.contains("text/event-stream")) {
        let mut usage = Usage::default();
        let mut found = false;
        for line in text.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(event) = serde_json::from_str(data) else {
                continue;
            };
            if let Some(next) = wire.usage_from_sse(&event) {
                found = true;
                usage.merge(next);
            }
        }
        found.then_some(usage)
    } else {
        wire.usage_from_body(&serde_json::from_slice(body).ok()?)
    }
}
