//! Gemini wire (`POST …/models/{model}:generateContent` and `:streamGenerateContent`,
//! plan T51.3).
//!
//! Shapes follow ai.google.dev (REST `v1beta`): the request carries history as
//! `contents[{role: "user" | "model", parts}]`, tool results as user-role parts
//! holding `functionResponse: {name, response}` (no stable call id — archive
//! decisions key on the function `name`), and usage as top-level
//! `usageMetadata` in the body or the final stream chunk.

use serde_json::Value;

use super::wire::{ToolResultRef, ToolResults, Usage, UsageFields, Wire, find_usage, turn_setup};

pub static GEMINI: Gemini = Gemini;

pub struct Gemini;

impl ToolResults for Gemini {
    /// Gemini roles are `user` / `model` (never `assistant`); a tool result is a
    /// user-role part whose `functionResponse` names the call. The rewritten payload
    /// is `response` only, so the function name stays visible to the model.
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>> {
        let Some((contents, total)) = turn_setup(req, "contents") else {
            return Vec::new();
        };
        let mut seen = 0;
        let mut results = Vec::new();
        for content in contents {
            if content["role"] != "user" {
                continue;
            }
            seen += 1;
            let turn = total - seen;
            let Some(parts) = content.get_mut("parts").and_then(Value::as_array_mut) else {
                continue;
            };
            for part in parts {
                if part.get("functionResponse").is_none() {
                    continue;
                }
                let Some(id) = part["functionResponse"]
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    continue;
                };
                let Some(content) = part
                    .get_mut("functionResponse")
                    .and_then(|r| r.get_mut("response"))
                else {
                    continue;
                };
                results.push(ToolResultRef { id, content, turn });
            }
        }
        results
    }
}

impl Wire for Gemini {
    fn matches(&self, path: &str) -> bool {
        path.ends_with(":generateContent") || path.ends_with(":streamGenerateContent")
    }

    fn provider(&self) -> &'static str {
        super::wire::GEMINI_PROVIDER
    }

    /// The model travels in the path (`/v1beta/models/{model}:generateContent`);
    /// the body carries none, so the default body lookup would record no model.
    fn model(&self, path: &str, _body: Option<&Value>) -> Option<String> {
        let (model, _action) = path.rsplit('/').next()?.split_once(':')?;
        (!model.is_empty()).then(|| model.to_string())
    }

    fn usage_from_body(&self, body: &Value) -> Option<Usage> {
        // Non-SSE `streamGenerateContent` answers with an array of response objects;
        // the totals ride on the last one carrying `usageMetadata`.
        if let Some(items) = body.as_array() {
            return items.iter().rev().find_map(usage_block);
        }
        usage_block(body)
    }

    fn usage_from_sse(&self, event: &Value) -> Option<Usage> {
        usage_block(event)
    }

    /// `promptTokenCount` already contains the cached slice (like both OpenAI wires),
    /// so the ledger total is input + output — adding `cache_read` again would charge
    /// the same tokens twice.
    fn provider_total(&self, usage: Usage) -> i64 {
        usage.input.saturating_add(usage.output)
    }
}

/// `cachedContentTokenCount` is the cached slice of the prompt (implicit or explicit
/// caching); there is no cache-creation signal on this wire, so `cache_create` is 0.
fn usage_block(value: &Value) -> Option<Usage> {
    find_usage(
        value,
        &UsageFields {
            alt_parent: None,
            container: "usageMetadata",
            input: "promptTokenCount",
            output: "candidatesTokenCount",
            cache_create: None,
            cache_read: "cachedContentTokenCount",
            cache_read_details: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn matches_generate_and_stream_actions_only() {
        assert!(GEMINI.matches("/v1beta/models/gemini-2.0-flash:generateContent"));
        assert!(GEMINI.matches("/v1/models/gemini-2.0-flash:streamGenerateContent"));
        assert!(!GEMINI.matches("/v1/messages"));
        assert!(!GEMINI.matches("/v1/chat/completions"));
        assert!(!GEMINI.matches("/v1/responses"));
        assert!(!GEMINI.matches("/v1beta/models"));
    }

    #[test]
    fn model_comes_from_the_path() {
        assert_eq!(
            GEMINI.model("/v1beta/models/gemini-2.0-flash:generateContent", None),
            Some("gemini-2.0-flash".to_string())
        );
        assert_eq!(
            GEMINI.model(
                "/v1beta/models/gemini-2.0-flash:streamGenerateContent",
                Some(&json!({}))
            ),
            Some("gemini-2.0-flash".to_string())
        );
        assert_eq!(GEMINI.model("/v1beta/models", None), None);
    }

    #[test]
    fn keys_function_responses_by_name_and_counts_user_turns() {
        let mut request = json!({"contents":[
            {"role":"user","parts":[{"text":"hi"}]},
            {"role":"model","parts":[{"functionCall":{"name":"read","args":{}}}]},
            {"role":"user","parts":[{"functionResponse":{"name":"read","response":"old"}}]},
            {"role":"model","parts":[{"functionCall":{"name":"run","args":{}}}]},
            {"role":"user","parts":[{"functionResponse":{"name":"run","response":"live"}}]}
        ]});
        let results = GEMINI.tool_results(&mut request);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "read");
        assert_eq!(results[0].turn, 1, "one user turn follows the first result");
        assert_eq!(results[1].turn, 0, "the last result is the live tail");
        assert_eq!(results[0].content, &json!("old"));
    }

    #[test]
    fn response_without_a_name_is_not_a_result() {
        let mut request = json!({"contents":[
            {"role":"user","parts":[{"functionResponse":{"response":"x"}}]}
        ]});
        assert!(GEMINI.tool_results(&mut request).is_empty());
    }

    #[test]
    fn reads_usage_from_body_array_and_sse() {
        let usage = json!({"usageMetadata":{
            "promptTokenCount":12,"cachedContentTokenCount":7,
            "candidatesTokenCount":3,"totalTokenCount":15}});
        let expected = Usage {
            input: 12,
            cache_create: 0,
            cache_read: 7,
            output: 3,
        };
        assert_eq!(GEMINI.usage_from_body(&usage), Some(expected));
        assert_eq!(GEMINI.usage_from_sse(&usage), Some(expected));
        // Non-SSE streams answer an array; totals ride on the last usage carrier.
        let array = json!([{"candidates":[]}, usage]);
        assert_eq!(GEMINI.usage_from_body(&array), Some(expected));
        assert_eq!(GEMINI.usage_from_body(&json!([{"candidates":[]}])), None);
        // `promptTokenCount` already contains the cached slice: 12 + 3, not 12 + 7 + 3.
        assert_eq!(GEMINI.provider_total(expected), 15);
    }
}
