//! Anthropic Messages wire (`POST /v1/messages`, plan T11.1).

use serde_json::Value;

use super::wire::{ToolResultRef, ToolResults, Usage, UsageFields, Wire, find_usage, turn_setup};

pub static ANTHROPIC: Anthropic = Anthropic;

pub struct Anthropic;

impl ToolResults for Anthropic {
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>> {
        let Some((messages, total)) = turn_setup(req, "messages") else {
            return Vec::new();
        };
        let mut seen = 0;
        let mut results = Vec::new();
        for message in messages {
            if message["role"] != "user" {
                continue;
            }
            seen += 1;
            let turn = total - seen;
            let Some(blocks) = message.get_mut("content").and_then(Value::as_array_mut) else {
                continue;
            };
            for block in blocks {
                if block["type"] != "tool_result" {
                    continue;
                }
                let Some(id) = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    continue;
                };
                let Some(content) = block.get_mut("content") else {
                    continue;
                };
                results.push(ToolResultRef { id, content, turn });
            }
        }
        results
    }
}

impl Wire for Anthropic {
    fn matches(&self, path: &str) -> bool {
        path == "/v1/messages"
    }

    fn provider(&self) -> &'static str {
        "anthropic"
    }

    fn session_id<'a>(&self, body: &'a Value) -> Option<&'a str> {
        body.get("metadata")
            .and_then(|metadata| metadata.get("user_id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
    }

    fn usage_from_body(&self, body: &Value) -> Option<Usage> {
        usage_block(body)
    }

    fn usage_from_sse(&self, event: &Value) -> Option<Usage> {
        usage_block(event)
    }
}

fn usage_block(value: &Value) -> Option<Usage> {
    find_usage(
        value,
        &UsageFields {
            alt_parent: Some("message"),
            input: "input_tokens",
            output: "output_tokens",
            cache_create: Some("cache_creation_input_tokens"),
            cache_read: "cache_read_input_tokens",
            cache_read_details: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extracts_only_old_anthropic_tool_results() {
        let mut request = json!({"messages":[
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"one","content":"old"}]},
            {"role":"assistant","content":"ok"},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"two","content":"live"}]}
        ]});
        let results = ANTHROPIC.tool_results(&mut request);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "one");
        assert_eq!(results[0].turn, 1);
        assert_eq!(results[1].turn, 0);
    }

    #[test]
    fn reads_body_and_sse_usage() {
        assert_eq!(
            ANTHROPIC.usage_from_body(&json!({"usage":{"input_tokens":3,"output_tokens":2}})),
            Some(Usage {
                input: 3,
                cache_create: 0,
                cache_read: 0,
                output: 2
            })
        );
        assert_eq!(
            ANTHROPIC.usage_from_sse(&json!({"message":{"usage":{"cache_read_input_tokens":4}}})),
            Some(Usage {
                input: 0,
                cache_create: 0,
                cache_read: 4,
                output: 0
            })
        );
    }
}
