//! OpenAI Responses wire (`POST /v1/responses`, plan T11.3).

use serde_json::Value;

use super::wire::{ToolResultRef, Usage, Wire, int_field, str_field};

pub static OPENAI_RESPONSES: OpenAiResponses = OpenAiResponses;

pub struct OpenAiResponses;

impl Wire for OpenAiResponses {
    fn matches(&self, path: &str) -> bool {
        path == "/v1/responses"
    }

    fn provider(&self) -> &'static str {
        "openai"
    }

    fn session_id<'a>(&self, body: &'a Value) -> Option<&'a str> {
        str_field(body, "user")
    }

    /// Responses carries tool results as `input[]` items of type `function_call_output`,
    /// keyed by `call_id`, with the payload in `output` (Chat Completions uses whole
    /// `role: "tool"` messages instead).
    ///
    /// `previous_response_id` means the history lives on the server and `input` holds only
    /// the new turn, so there is nothing old to rewrite: yield none and record usage only.
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>> {
        if req
            .get("previous_response_id")
            .is_some_and(|id| !id.is_null())
        {
            return Vec::new();
        }
        let Some(input) = req.get_mut("input").and_then(Value::as_array_mut) else {
            return Vec::new();
        };
        let total = input.iter().filter(|item| item["role"] == "user").count();
        let mut seen = 0;
        let mut results = Vec::new();
        for item in input {
            if item["role"] == "user" {
                seen += 1;
                continue;
            }
            if item["type"] != "function_call_output" {
                continue;
            }
            let turn = total - seen;
            let Some(id) = item
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            let Some(content) = item.get_mut("output") else {
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
        (event.get("type").and_then(Value::as_str) == Some("response.completed"))
            .then(|| usage_block(event))
            .flatten()
    }
}

/// `input_tokens` counts cached tokens too, so `cache_read` is the cached slice of it.
/// A streamed response reports usage once, on the final `response.completed` event, which
/// nests it under `response`. There is no cache-creation signal on this wire.
fn usage_block(value: &Value) -> Option<Usage> {
    value
        .get("usage")
        .or_else(|| value.get("response").and_then(|resp| resp.get("usage")))
        .filter(|usage| usage.is_object())
        .map(|usage| Usage {
            input: int_field(usage, "input_tokens"),
            cache_create: 0,
            cache_read: usage
                .get("input_tokens_details")
                .map(|details| int_field(details, "cached_tokens"))
                .unwrap_or_default(),
            output: int_field(usage, "output_tokens"),
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn keys_function_call_outputs_by_call_id_and_counts_later_user_turns() {
        let mut request = json!({"input":[
            {"role":"user","content":"one"},
            {"type":"function_call_output","call_id":"a","output":"old"},
            {"role":"user","content":"two"},
            {"type":"function_call_output","call_id":"b","output":"live"}
        ]});
        let results = OPENAI_RESPONSES.tool_results(&mut request);
        assert_eq!(results.len(), 2);
        assert_eq!((results[0].id.as_str(), results[0].turn), ("a", 1));
        assert_eq!((results[1].id.as_str(), results[1].turn), ("b", 0));
    }

    #[test]
    fn yields_nothing_when_history_is_server_side() {
        let mut request = json!({"previous_response_id":"resp_1","input":[
            {"type":"function_call_output","call_id":"a","output":"x"}
        ]});
        assert!(OPENAI_RESPONSES.tool_results(&mut request).is_empty());
    }

    #[test]
    fn reads_usage_from_body_and_from_response_completed() {
        let expected = Usage {
            input: 10,
            cache_create: 0,
            cache_read: 7,
            output: 2,
        };
        let usage = json!({"input_tokens":10,"input_tokens_details":{"cached_tokens":7},
                           "output_tokens":2});
        assert_eq!(
            OPENAI_RESPONSES.usage_from_body(&json!({"usage": usage})),
            Some(expected)
        );
        assert_eq!(
            OPENAI_RESPONSES
                .usage_from_sse(&json!({"type":"response.completed","response":{"usage": usage}})),
            Some(expected)
        );
        assert_eq!(
            OPENAI_RESPONSES.usage_from_sse(&json!({"type":"response.created","response":{}})),
            None
        );
        assert_eq!(
            OPENAI_RESPONSES
                .usage_from_sse(&json!({"type":"response.created","response":{"usage": usage}})),
            None
        );
    }
}
