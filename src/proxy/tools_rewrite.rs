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
