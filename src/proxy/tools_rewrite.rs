//! Byte-stable `tools[]` description rewrite (plan T59.5).

use rtok_plugin_sdk::{Class, Measurement};
use serde_json::{Value, json};

use crate::config::{Estimator, ToolsRewrite};
use crate::tokens;

/// Rewrite `tools[]` when `[proxy.tools_rewrite]` is armed (`max_description_tokens > 0`).
pub fn rewrite(body: &mut Value, cfg: &ToolsRewrite, est: &Estimator) -> Option<Measurement> {
    let max = cfg.max_description_tokens;
    if max == 0 {
        return None;
    }
    let tools = body.get_mut("tools")?.as_array_mut()?;
    let mut before_bytes = 0u64;
    let mut after_bytes = 0u64;
    let mut kept = Vec::with_capacity(tools.len());
    for mut tool in tools.drain(..) {
        let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
        if !cfg.allow.is_empty() && !cfg.allow.iter().any(|a| a == name) {
            continue;
        }
        if cfg.deny.iter().any(|d| d == name) {
            continue;
        }
        if let Some(desc) = tool.get("description").and_then(Value::as_str) {
            before_bytes += desc.len() as u64;
            let short = truncate_description(desc, max, est);
            after_bytes += short.len() as u64;
            tool["description"] = json!(short);
        }
        if let Some(func) = tool.get_mut("function") {
            if let Some(desc) = func.get("description").and_then(Value::as_str) {
                before_bytes += desc.len() as u64;
                let short = truncate_description(desc, max, est);
                after_bytes += short.len() as u64;
                func["description"] = json!(short);
            }
        }
        kept.push(tool);
    }
    *tools = kept;
    if before_bytes == 0 && after_bytes == 0 {
        return None;
    }
    Some(Measurement {
        plugin: "proxy",
        kind: "tools_rewrite",
        before_bytes,
        after_bytes,
        est_before: tokens::estimate(&"x".repeat(before_bytes as usize), Class::Prose, est),
        est_after: tokens::estimate(&"x".repeat(after_bytes as usize), Class::Prose, est),
        ref_id: None,
        call_id: None,
    })
}

fn truncate_description(text: &str, max_tokens: u32, est: &Estimator) -> String {
    if tokens::estimate(text, Class::Prose, est) <= max_tokens {
        return text.to_string();
    }
    let mut best = String::new();
    for (i, _) in text.match_indices(['.', '!', '?']) {
        let cand = text[..=i].trim();
        if !cand.is_empty() && tokens::estimate(cand, Class::Prose, est) <= max_tokens {
            best = cand.to_string();
        }
    }
    if !best.is_empty() {
        return best;
    }
    let mut s = text.to_string();
    while !s.is_empty() && tokens::estimate(&s, Class::Prose, est) > max_tokens {
        s.pop();
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn truncates_at_sentence_boundary_and_drops_deny() {
        let mut cfg = Config::default();
        cfg.proxy.tools_rewrite.max_description_tokens = 5;
        cfg.proxy.tools_rewrite.deny = vec!["secret".into()];
        let long = "First sentence is here. Second sentence adds more detail about the tool.";
        let mut body = json!({
            "tools": [
                {"name": "read", "description": long},
                {"name": "secret", "description": "must drop"}
            ]
        });
        let m = rewrite(&mut body, &cfg.proxy.tools_rewrite, &cfg.estimator).unwrap();
        assert!(m.before_bytes > m.after_bytes);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "read");
    }
}
