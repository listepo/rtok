//! `toon` — tabular JSON → TOON encoding (vendor bench: −42.6 % tokens). Off by default
//! until A/B measured.
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use serde_json::Value;

use crate::plugin::{Ctx, Manifest, Measurement, Plugin, Surface};
use crate::proxy::wire::{ToolResultRef, WireRequest};
use crate::tokens::Class;

pub struct Toon;

impl Plugin for Toon {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "toon",
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: false,
        }
    }

    fn proxy_filter(&self, req: &mut WireRequest<'_>, cx: &Ctx) -> Vec<Measurement> {
        if !cx.config.plugins.toon.enabled {
            return Vec::new();
        }
        rewrite(req.tool_results(), cx)
    }
}

fn rewrite(results: Vec<ToolResultRef<'_>>, cx: &Ctx) -> Vec<Measurement> {
    let min_rows = cx.config.plugins.toon.min_rows as usize;
    let mut out = Vec::new();
    for result in results {
        if let Some(m) = rewrite_block(result.content, cx, min_rows) {
            out.push(m);
        }
    }
    out
}

fn rewrite_block(content: &mut Value, cx: &Ctx, min_rows: usize) -> Option<Measurement> {
    let table = match content {
        Value::Array(_) => content.clone(),
        Value::String(s) => serde_json::from_str(s).ok().filter(Value::is_array)?,
        _ => return None,
    };
    let keys = tabular_keys(&table, min_rows)?;
    let rows = table.as_array()?;
    let bytes = serde_json::to_vec(&table).ok()?;
    let archive_id = cx
        .store
        .put_archive(&cx.session, &bytes, &cx.config.core.archive_dir)
        .map_err(|e| cx.log("error", "plugin", "toon", &format!("put: {e}")))
        .ok()?;
    let encoded = encode(rows, &keys);
    let replacement = format!("[toon {archive_id}]\n{encoded}");
    let orig = match content {
        Value::String(s) => s.clone(),
        _ => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let m = Measurement {
        plugin: "toon",
        kind: "encode",
        before_bytes: orig.len() as u64,
        after_bytes: replacement.len() as u64,
        est_before: cx.estimate(&orig, Class::Code),
        est_after: cx.estimate(&replacement, Class::Code),
        ref_id: Some(archive_id),
        call_id: None,
    };
    *content = Value::String(replacement);
    Some(m)
}

fn tabular_keys(value: &Value, min_rows: usize) -> Option<Vec<String>> {
    let arr = value.as_array()?;
    if arr.len() < min_rows.max(1) {
        return None;
    }
    let first = arr[0].as_object()?;
    if first.len() < 3 {
        return None;
    }
    let keys: Vec<String> = first.keys().cloned().collect();
    for item in arr {
        let obj = item.as_object()?;
        if obj.len() != keys.len() {
            return None;
        }
        for k in &keys {
            match obj.get(k)? {
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
                _ => return None,
            }
        }
    }
    Some(keys)
}

fn encode(rows: &[Value], keys: &[String]) -> String {
    let mut out = format!("[{}]{{{}}}:", rows.len(), keys.join(","));
    for row in rows {
        out.push_str("\n  ");
        for (i, k) in keys.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&encode_cell(row.get(k).unwrap_or(&Value::Null)));
        }
    }
    out
}

fn encode_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s)
            if s.contains([',', '{', '}', '\n']) || s.starts_with(' ') || s.ends_with(' ') =>
        {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

#[allow(dead_code)]
fn decode(toon: &str) -> Option<Value> {
    let (header, body) = toon.split_once('\n')?;
    let s = header.trim().strip_suffix(':')?;
    let brace = s.find('{')?;
    if !s.starts_with('[') || !s[..brace].ends_with(']') || !s.ends_with('}') {
        return None;
    }
    let keys: Vec<String> = s[brace + 1..s.len() - 1]
        .split(',')
        .map(str::to_string)
        .collect();
    let mut rows = Vec::new();
    for line in body.lines() {
        let cells: Vec<Value> = line.trim_start().split(',').map(decode_cell).collect();
        if cells.len() != keys.len() {
            return None;
        }
        let mut obj = serde_json::Map::new();
        for (k, v) in keys.iter().zip(cells) {
            obj.insert(k.clone(), v);
        }
        rows.push(Value::Object(obj));
    }
    Some(Value::Array(rows))
}

#[allow(dead_code)]
fn decode_cell(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Value::String(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    match serde_json::from_str(s) {
        Ok(v @ (Value::Number(_) | Value::Bool(_))) => v,
        _ => Value::String(s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::anthropic::ANTHROPIC;
    use crate::proxy::wire::WireRequest;
    use serde_json::json;

    fn cx(name: &str, enabled: bool, min_rows: u32) -> Ctx {
        let dir = std::env::temp_dir().join(format!("rtok-toon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = Ctx::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx.config.plugins.toon.enabled = enabled;
        cx.config.plugins.toon.min_rows = min_rows;
        cx
    }

    fn tool_req(table: Value) -> Value {
        let content = serde_json::to_string_pretty(&table).unwrap();
        json!({"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content": content}]}]})
    }

    fn rows_3x4() -> Value {
        json!([
            {"a": 11, "b": "alpha-value-one", "c": 33, "d": "delta-value-one"},
            {"a": 44, "b": "beta-value-twoo", "c": 66, "d": "delta-value-two"},
            {"a": 77, "b": "gamma-value-thr", "c": 99, "d": "delta-value-tre"},
        ])
    }

    fn filter(body: &mut Value, cx: &Ctx) -> Vec<Measurement> {
        Toon.proxy_filter(&mut WireRequest::new(&ANTHROPIC, body), cx)
    }

    #[test]
    fn default_off_leaves_bytes_identical() {
        let cx = cx("off", false, 3);
        let mut body = tool_req(json!([
            {"a": 1, "b": 2, "c": 3},
            {"a": 4, "b": 5, "c": 6},
            {"a": 7, "b": 8, "c": 9},
            {"a": 10, "b": 11, "c": 12},
        ]));
        let original = body.clone();
        assert!(filter(&mut body, &cx).is_empty());
        assert_eq!(body, original);
    }

    #[test]
    fn encodes_3x4_table_and_decode_recovers_keys() {
        let cx = cx("enc", true, 3);
        let mut body = tool_req(rows_3x4());
        let ms = filter(&mut body, &cx);
        assert_eq!(ms.len(), 1);
        assert!(ms[0].after_bytes < ms[0].before_bytes);
        let text = body["messages"][0]["content"][0]["content"]
            .as_str()
            .unwrap();
        let decoded = decode(text.split_once('\n').unwrap().1).unwrap();
        let keys: Vec<_> = decoded[0].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["a", "b", "c", "d"]);
    }

    #[test]
    fn round_trip_values() {
        let table = rows_3x4();
        let keys = tabular_keys(&table, 3).unwrap();
        let encoded = encode(table.as_array().unwrap(), &keys);
        assert_eq!(decode(&encoded).unwrap(), table);
    }
}
