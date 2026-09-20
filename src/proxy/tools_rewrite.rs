//! Opt-in `tools[]` description rewrite (T59.5). Off: the request is untouched.

use serde_json::Value;

use crate::config::{Estimator, ToolsRewrite};
use crate::tokens::{self, Class};

/// Before/after description bytes and estimates for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Delta {
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub est_before: u32,
    pub est_after: u32,
    pub changed: bool,
}

/// Truncate descriptions and drop allow/deny names. `None` if `tools[]` is absent.
/// Never writes `input_schema` / `parameters`. Unknown tool objects are left as-is.
pub(super) fn rewrite(body: &mut Value, cfg: &ToolsRewrite, est: &Estimator) -> Option<Delta> {
    if !cfg.enabled {
        return None;
    }
    let tools = body.get_mut("tools")?.as_array_mut()?;
    let mut before_bytes = 0u64;
    let mut after_bytes = 0u64;
    let mut est_before = 0u32;
    let mut est_after = 0u32;
    let mut changed = false;
    let mut i = 0;
    while i < tools.len() {
        let Some(name) = tool_name(&tools[i]).map(str::to_string) else {
            i += 1;
            continue;
        };
        let drop = cfg.deny.iter().any(|d| d == &name)
            || (!cfg.allow.is_empty() && !cfg.allow.iter().any(|a| a == &name));
        let (b, eb) = desc_stats(&tools[i], est);
        before_bytes += b;
        est_before += eb;
        if drop {
            tools.remove(i);
            changed = true;
            continue;
        }
        if cfg.max_description_tokens > 0
            && let Some(desc) = tool_desc_mut(&mut tools[i])
        {
            let trimmed = truncate_at_sentence(desc, cfg.max_description_tokens, est);
            if trimmed != *desc {
                *desc = trimmed;
                changed = true;
            }
        }
        let (a, ea) = desc_stats(&tools[i], est);
        after_bytes += a;
        est_after += ea;
        i += 1;
    }
    Some(Delta {
        before_bytes,
        after_bytes,
        est_before,
        est_after,
        changed,
    })
}

fn tool_name(tool: &Value) -> Option<&str> {
    tool.get("name")
        .and_then(Value::as_str)
        .or_else(|| tool.pointer("/function/name").and_then(Value::as_str))
}

fn tool_desc_mut(tool: &mut Value) -> Option<&mut String> {
    let path = if matches!(tool.get("description"), Some(Value::String(_))) {
        "/description"
    } else {
        "/function/description"
    };
    match tool.pointer_mut(path) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

fn desc_stats(tool: &Value, est: &Estimator) -> (u64, u32) {
    let d = tool
        .get("description")
        .and_then(Value::as_str)
        .or_else(|| {
            tool.pointer("/function/description")
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    (d.len() as u64, tokens::estimate(d, Class::Prose, est))
}

fn truncate_at_sentence(text: &str, max_tokens: u32, est: &Estimator) -> String {
    if tokens::estimate(text, Class::Prose, est) <= max_tokens {
        return text.to_string();
    }
    let mut out = String::new();
    for sentence in sentences(text) {
        let mut cand = out.clone();
        cand.push_str(sentence);
        if tokens::estimate(cand.trim(), Class::Prose, est) <= max_tokens {
            out = cand;
        } else if out.is_empty() {
            return prefix_tokens(text, max_tokens, est);
        } else {
            break;
        }
    }
    out.trim().to_string()
}

fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if matches!(b[i], b'.' | b'!' | b'?')
            && (i + 1 == b.len() || b[i + 1].is_ascii_whitespace())
        {
            let end = i + 1;
            out.push(&text[start..end]);
            start = end;
        }
        i += 1;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

fn prefix_tokens(text: &str, max_tokens: u32, est: &Estimator) -> String {
    let mut prefix = String::new();
    for c in text.chars() {
        prefix.push(c);
        if tokens::estimate(&prefix, Class::Prose, est) > max_tokens {
            prefix.pop();
            break;
        }
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(max: u32, allow: &[&str], deny: &[&str]) -> ToolsRewrite {
        ToolsRewrite {
            enabled: true,
            max_description_tokens: max,
            allow: allow.iter().map(|s| (*s).to_string()).collect(),
            deny: deny.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn est() -> Estimator {
        Estimator::default()
    }

    #[test]
    fn anthropic_truncates_description_not_schema() {
        let schema = json!({"type": "object", "properties": {"command": {"type": "string"}}});
        let mut body = json!({
            "tools": [{
                "name": "Bash",
                "description": "Short one. This second sentence is far too long to keep under the cap.",
                "input_schema": schema,
            }],
            "messages": [{"role": "user", "content": "hi"}],
        });
        let d = rewrite(&mut body, &cfg(5, &[], &[]), &est()).unwrap();
        assert!(d.changed && d.after_bytes < d.before_bytes);
        assert_eq!(body["tools"][0]["description"], "Short one.");
        assert_eq!(body["tools"][0]["input_schema"], schema);
        assert_eq!(body["messages"][0]["content"], "hi");
        let again = body.clone();
        rewrite(&mut body, &cfg(5, &[], &[]), &est()).unwrap();
        assert_eq!(body, again);
    }

    #[test]
    fn openai_chat_truncates_function_description_not_parameters() {
        let params = json!({"type": "object", "properties": {"q": {"type": "string"}}});
        let mut body = json!({
            "tools": [{
                "type": "function",
                "function": {
                    "name": "search",
                    "description": "Short one. This second sentence is far too long to keep under the cap.",
                    "parameters": params,
                }
            }],
        });
        let d = rewrite(&mut body, &cfg(5, &[], &[]), &est()).unwrap();
        assert!(d.changed);
        assert_eq!(body["tools"][0]["function"]["description"], "Short one.");
        assert_eq!(body["tools"][0]["function"]["parameters"], params);
    }

    #[test]
    fn deny_drops_tools_but_not_a_later_call() {
        let mut body = json!({
            "tools": [
                {"name": "keep", "description": "Stay.", "input_schema": {"type": "object"}},
                {"name": "drop_me", "description": "Gone.", "input_schema": {"type": "object"}},
            ],
            "messages": [{
                "role": "assistant",
                "content": [{"type": "tool_use", "id": "t1", "name": "drop_me", "input": {}}],
            }],
        });
        let d = rewrite(&mut body, &cfg(0, &[], &["drop_me"]), &est()).unwrap();
        assert!(d.changed);
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["name"], "keep");
        assert_eq!(body["messages"][0]["content"][0]["name"], "drop_me");
    }

    #[test]
    fn off_or_missing_tools_is_a_no_op() {
        let mut body = json!({"tools": [{"name": "Bash", "description": "Hi there."}]});
        let mut off = cfg(5, &[], &[]);
        off.enabled = false;
        assert!(rewrite(&mut body, &off, &est()).is_none());
        assert_eq!(body["tools"][0]["description"], "Hi there.");
        let mut empty = json!({"messages": []});
        assert!(rewrite(&mut empty, &cfg(5, &[], &[]), &est()).is_none());
    }
}
