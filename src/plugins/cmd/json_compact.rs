//! JSON output compaction for `cmd` (T65.2).

use serde_json::Value;

/// Compact JSON `output` when it parses; otherwise return `None`.
pub fn compact(output: &str, json_items: u32, json_string: u32) -> Option<String> {
    let trimmed = output.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return None;
    }
    let v: Value = serde_json::from_str(trimmed).ok()?;
    let lines = render_value(&v, json_items as usize, json_string as usize);
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

fn render_value(v: &Value, json_items: usize, json_string: usize) -> Vec<String> {
    match v {
        Value::Object(map) => map
            .iter()
            .filter_map(|(k, val)| {
                if is_empty(val) {
                    return None;
                }
                Some(format!("{k}: {}", render_scalar(val, json_items, json_string)))
            })
            .collect(),
        Value::Array(arr) => {
            if arr.is_empty() {
                return vec!["[]".into()];
            }
            let mut lines = Vec::new();
            for (i, item) in arr.iter().take(json_items).enumerate() {
                lines.push(format!("[{i}]: {}", render_scalar(item, json_items, json_string)));
            }
            if arr.len() > json_items {
                lines.push(format!("… +{} more", arr.len() - json_items));
            }
            lines
        }
        other => vec![render_scalar(other, json_items, json_string)],
    }
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

fn render_scalar(v: &Value, json_items: usize, json_string: usize) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => truncate_str(s, json_string),
        Value::Array(arr) => {
            if arr.len() <= json_items {
                format!("[{}]", arr.len())
            } else {
                format!("[{} items, … +{} more]", json_items, arr.len() - json_items)
            }
        }
        Value::Object(o) => format!("{{{}}}", o.len()),
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return format!("\"{s}\"");
    }
    let n = s.chars().count();
    let head: String = s.chars().take(max).collect();
    format!("\"{head}…\" ({n} chars)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_null_fields_and_truncates_strings() {
        let raw = r#"{"title":"hello","empty":null,"body":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#;
        let out = compact(raw, 20, 40).unwrap();
        assert!(!out.contains("empty"));
        assert!(out.contains("40 chars") || out.contains('…'));
    }

    #[test]
    fn array_cap_shows_more() {
        let raw = r#"[1,2,3,4,5,6,7,8,9,10,11,12]"#;
        let out = compact(raw, 3, 200).unwrap();
        assert!(out.contains("+9 more"));
    }

    #[test]
    fn plain_text_is_untouched() {
        assert!(compact("not json\n", 20, 200).is_none());
    }
}
