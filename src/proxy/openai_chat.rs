//! OpenAI Chat Completions wire (`POST /v1/chat/completions`, plan T11.2).

use serde_json::{Map, Value};

use super::wire::{ToolResultRef, Usage, Wire, int_field, str_field};

pub static OPENAI_CHAT: OpenAiChat = OpenAiChat;

pub struct OpenAiChat;

impl Wire for OpenAiChat {
    fn matches(&self, path: &str) -> bool {
        path == "/v1/chat/completions"
    }

    fn provider(&self) -> &'static str {
        "openai"
    }

    fn session_id<'a>(&self, body: &'a Value) -> Option<&'a str> {
        str_field(body, "user")
    }

    /// Chat Completions carries each tool result as its own `role: "tool"` message,
    /// keyed by `tool_call_id` (Anthropic nests them in the user turn instead).
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>> {
        let Some(messages) = req.get_mut("messages").and_then(Value::as_array_mut) else {
            return Vec::new();
        };
        let total = messages
            .iter()
            .filter(|message| message["role"] == "user")
            .count();
        let mut seen = 0;
        let mut results = Vec::new();
        for message in messages {
            match message["role"].as_str() {
                Some("user") => {
                    seen += 1;
                    continue;
                }
                Some("tool") => {}
                _ => continue,
            }
            let turn = total - seen;
            let Some(id) = message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            let Some(content) = message.get_mut("content") else {
                continue;
            };
            results.push(ToolResultRef { id, content, turn });
        }
        results
    }

    fn usage_from_body(&self, body: &Value) -> Option<Usage> {
        usage_block(body)
    }

    fn usage_from_sse(&self, event: &Value) -> Option<Usage> {
        usage_block(event)
    }

    /// A streamed Chat Completions response reports `usage` only when the request asked
    /// for it, so a streaming request that has not opted in gets
    /// `stream_options.include_usage = true`. This is the one byte-level change
    /// passthrough mode makes; `[proxy] include_usage = false` turns it off.
    fn prepare_request(&self, body: &mut Value, include_usage: bool) -> bool {
        if !include_usage || body.get("stream") != Some(&Value::Bool(true)) {
            return false;
        }
        let Some(object) = body.as_object_mut() else {
            return false;
        };
        let Some(options) = object
            .entry("stream_options")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        else {
            return false; // present but malformed — never overwrite a caller's value
        };
        if options.contains_key("include_usage") {
            return false; // the caller already chose; respect it either way
        }
        options.insert("include_usage".to_string(), Value::Bool(true));
        true
    }
}

/// `prompt_tokens` counts cached tokens too (Anthropic's `input_tokens` does not), so
/// `input` here is the billed prompt total and `cache_read` is the cached slice of it.
/// There is no cache-creation signal on this wire, so `cache_create` is always 0.
fn usage_block(value: &Value) -> Option<Usage> {
    value
        .get("usage")
        .filter(|usage| usage.is_object())
        .map(|usage| Usage {
            input: int_field(usage, "prompt_tokens"),
            cache_create: 0,
            cache_read: usage
                .get("prompt_tokens_details")
                .map(|details| int_field(details, "cached_tokens"))
                .unwrap_or_default(),
            output: int_field(usage, "completion_tokens"),
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn counts_user_turns_after_each_tool_message() {
        let mut request = json!({"messages":[
            {"role":"user","content":"hi"},
            {"role":"assistant","tool_calls":[{"id":"one"}]},
            {"role":"tool","tool_call_id":"one","content":"old"},
            {"role":"user","content":"more"},
            {"role":"assistant","tool_calls":[{"id":"two"}]},
            {"role":"tool","tool_call_id":"two","content":"live"}
        ]});
        let results = OPENAI_CHAT.tool_results(&mut request);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "one");
        assert_eq!(results[0].turn, 1, "one user turn follows the first result");
        assert_eq!(results[1].turn, 0, "the last result is the live tail");
    }

    #[test]
    fn reads_usage_from_body_and_final_stream_chunk() {
        let usage = json!({"usage":{
            "prompt_tokens":10,"completion_tokens":2,
            "prompt_tokens_details":{"cached_tokens":7}}});
        let expected = Usage {
            input: 10,
            cache_create: 0,
            cache_read: 7,
            output: 2,
        };
        assert_eq!(OPENAI_CHAT.usage_from_body(&usage), Some(expected));
        assert_eq!(OPENAI_CHAT.usage_from_sse(&usage), Some(expected));
        // Every non-final chunk carries `usage: null`.
        assert_eq!(OPENAI_CHAT.usage_from_sse(&json!({"usage":null})), None);
    }

    #[test]
    fn adds_include_usage_only_to_streaming_requests_that_omit_it() {
        let mut streaming = json!({"stream":true});
        assert!(OPENAI_CHAT.prepare_request(&mut streaming, true));
        assert_eq!(streaming["stream_options"]["include_usage"], json!(true));

        let mut already_set = json!({"stream":true,"stream_options":{"include_usage":false}});
        assert!(!OPENAI_CHAT.prepare_request(&mut already_set, true));
        assert_eq!(already_set["stream_options"]["include_usage"], json!(false));

        let mut non_streaming = json!({"model":"gpt-4o"});
        assert!(!OPENAI_CHAT.prepare_request(&mut non_streaming, true));
        assert_eq!(non_streaming, json!({"model":"gpt-4o"}));

        let mut disabled = json!({"stream":true});
        assert!(!OPENAI_CHAT.prepare_request(&mut disabled, false));
        assert_eq!(disabled, json!({"stream":true}), "config can turn it off");
    }
}
