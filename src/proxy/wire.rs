//! API-wire abstraction for proxy request rewriting and usage extraction (plan T11.1).

use anyhow::{Context, Result};
use reqwest::Url;
use serde_json::Value;

// The plugin-visible half of the wire is the published contract (D25); the provider
// dialects that implement it stay here.
pub use rtok_plugin_sdk::{ToolResultRef, ToolResults, WireRequest};

use super::anthropic::ANTHROPIC;
use super::openai_chat::OPENAI_CHAT;
use super::openai_responses::OPENAI_RESPONSES;

/// Provider slug for the Anthropic Messages wire.
pub const ANTHROPIC_PROVIDER: &str = "anthropic";
/// Provider slug shared by both OpenAI wires.
pub const OPENAI_PROVIDER: &str = "openai";

/// `usage.api` value for Anthropic Messages.
pub const API_ANTHROPIC: &str = "anthropic";
/// `usage.api` value for Chat Completions.
pub const API_OPENAI_CHAT: &str = "openai_chat";
/// `usage.api` value for the Responses API.
pub const API_OPENAI_RESPONSES: &str = "openai_responses";

fn wire_ids(wire: &dyn Wire) -> (&'static str, &'static str) {
    if std::ptr::eq(wire, &ANTHROPIC as &dyn Wire) {
        (ANTHROPIC_PROVIDER, API_ANTHROPIC)
    } else if std::ptr::eq(wire, &OPENAI_CHAT as &dyn Wire) {
        (OPENAI_PROVIDER, API_OPENAI_CHAT)
    } else if std::ptr::eq(wire, &OPENAI_RESPONSES as &dyn Wire) {
        (OPENAI_PROVIDER, API_OPENAI_RESPONSES)
    } else {
        unreachable!("unknown wire")
    }
}

/// A provider request/response shape supported by the proxy.
pub trait Wire: ToolResults {
    /// Whether this wire owns the request path.
    fn matches(&self, path: &str) -> bool;

    /// Provider slug used for the request's dimension row. Defaults to `"openai"`,
    /// which both OpenAI wires use; Anthropic overrides.
    fn provider(&self) -> &'static str {
        "openai"
    }

    /// Provider session identity, when the body carries one. Defaults to the `user`
    /// field both OpenAI wires use; Anthropic overrides for `metadata.user_id`.
    fn session_id<'a>(&self, body: &'a Value) -> Option<&'a str> {
        str_field(body, "user")
    }

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

    /// The billable total the `tokens` ledger records. Anthropic's four counters are
    /// disjoint, so they add up; OpenAI's `prompt_tokens` **already contains** the cached
    /// slice (`openai_chat::parse_usage`), so adding `cache_read` again would charge the
    /// same tokens twice — a 30 000-prompt/27 000-cached turn would record 57 200.
    fn provider_total(&self, usage: Usage) -> i64 {
        let sum = || {
            usage
                .input
                .saturating_add(usage.cache_create)
                .saturating_add(usage.cache_read)
                .saturating_add(usage.output)
        };
        if self.provider() == "openai" {
            usage.input.saturating_add(usage.output)
        } else {
            sum()
        }
    }
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

/// Join `proxy.upstream` / `proxy.openai_upstream` with the request path and query.
pub fn join_upstream(base: &str, path: &str, query: Option<&str>) -> Result<String> {
    if base.is_empty() {
        anyhow::bail!("proxy.upstream is empty");
    }
    let mut url = Url::parse(base).context("proxy upstream URL")?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("proxy upstream URL cannot be a base"))?;
        segments.pop_if_empty();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            segments.push(segment);
        }
    }
    if let Some(q) = query {
        url.set_query(Some(q));
    }
    Ok(url.into())
}

/// The wire matching `path`, if this build understands it.
pub fn for_path(path: &str) -> Option<&'static dyn Wire> {
    const WIRES: [&(dyn Wire + 'static); 3] = [&ANTHROPIC, &OPENAI_CHAT, &OPENAI_RESPONSES];
    WIRES.into_iter().find(|wire| wire.matches(path))
}

/// `usage.api` discriminator for `wire` (T11.6).
pub fn api_of(wire: &dyn Wire) -> &'static str {
    wire_ids(wire).1
}

/// Read a non-empty string field, which is how both OpenAI wires carry the session id.
pub(super) fn str_field<'a>(body: &'a Value, name: &str) -> Option<&'a str> {
    body.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// The array at `req[field]` and how many of its entries are a user turn, or `None`
/// if the field is absent/not an array. Every wire's `tool_results` starts here,
/// then walks the array itself since what counts as a result differs per wire — be
/// it Anthropic's `messages`, Chat Completions' `messages`, or Responses' `input`.
pub(super) fn turn_setup<'a>(
    req: &'a mut Value,
    field: &str,
) -> Option<(&'a mut Vec<Value>, usize)> {
    let entries = req.get_mut(field).and_then(Value::as_array_mut)?;
    let total = entries
        .iter()
        .filter(|entry| entry["role"] == "user")
        .count();
    Some((entries, total))
}

/// Read an integer usage counter, treating an absent or non-numeric field as zero.
/// Whole-number JSON floats (`10.0`) count too — some providers emit them.
pub(super) fn int_field(usage: &Value, name: &str) -> i64 {
    let Some(v) = usage.get(name) else {
        return 0;
    };
    if let Some(n) = v.as_i64() {
        return n;
    }
    v.as_f64().map(|n| n as i64).unwrap_or(0)
}

/// Field names one wire's usage block needs to build a [`Usage`] — the three wires'
/// `usage_block` functions differed only in where the object sits and which keys it
/// reads, so this is the one place that walk lives. `alt_parent` covers Anthropic's
/// `message.usage` and Responses' `response.usage` aliases; `cache_read_details` covers
/// the OpenAI wires' nested `*_tokens_details` object. `cache_create` is `None` on both
/// OpenAI wires, which report no cache-write signal — that difference stays explicit
/// here rather than being papered over with a fake field name.
pub(super) struct UsageFields {
    pub alt_parent: Option<&'static str>,
    pub input: &'static str,
    pub output: &'static str,
    pub cache_create: Option<&'static str>,
    pub cache_read: &'static str,
    pub cache_read_details: Option<&'static str>,
}

/// Find `value`'s usage object (optionally nested under `fields.alt_parent`) and read it
/// through `fields`. Shared by all three wires' `usage_block`.
pub(super) fn find_usage(value: &Value, fields: &UsageFields) -> Option<Usage> {
    let usage = value
        .get("usage")
        .or_else(|| {
            fields
                .alt_parent
                .and_then(|parent| value.get(parent))
                .and_then(|parent| parent.get("usage"))
        })
        .filter(|usage| usage.is_object())?;
    let cache_read = match fields.cache_read_details {
        Some(details) => usage
            .get(details)
            .map(|d| int_field(d, fields.cache_read))
            .unwrap_or_default(),
        None => int_field(usage, fields.cache_read),
    };
    Some(Usage {
        input: int_field(usage, fields.input),
        cache_create: fields
            .cache_create
            .map(|f| int_field(usage, f))
            .unwrap_or(0),
        cache_read,
        output: int_field(usage, fields.output),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn int_field_accepts_whole_number_floats() {
        let usage = json!({"input_tokens": 10.0, "output_tokens": 2});
        assert_eq!(int_field(&usage, "input_tokens"), 10);
        assert_eq!(int_field(&usage, "output_tokens"), 2);
        assert_eq!(int_field(&usage, "missing"), 0);
    }

    #[test]
    fn wire_reports_own_provider_and_api() {
        let cases = [
            ("/v1/messages", ANTHROPIC_PROVIDER, API_ANTHROPIC),
            ("/v1/chat/completions", OPENAI_PROVIDER, API_OPENAI_CHAT),
            ("/v1/responses", OPENAI_PROVIDER, API_OPENAI_RESPONSES),
        ];
        for (path, provider, api) in cases {
            let wire = for_path(path).expect("wire");
            assert_eq!(wire.provider(), provider, "{path}");
            assert_eq!(api_of(wire), api, "{path}");
        }
    }

    #[test]
    fn join_upstream_avoids_doubled_slashes_and_queries() {
        assert_eq!(
            join_upstream("http://127.0.0.1", "/v1/messages", None).expect("join"),
            "http://127.0.0.1/v1/messages"
        );
        assert_eq!(
            join_upstream("http://127.0.0.1/prefix", "/v1/messages", None).expect("join"),
            "http://127.0.0.1/prefix/v1/messages"
        );
        assert_eq!(
            join_upstream(
                "http://127.0.0.1/prefix?keep=1",
                "/v1/messages",
                Some("q=1")
            )
            .expect("join"),
            "http://127.0.0.1/prefix/v1/messages?q=1"
        );
    }
}
