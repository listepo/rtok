//! `toon` — tabular JSON → TOON encoding (vendor bench: −42.6 % tokens). Off by default
//! until A/B measured.
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here. Proxy rewrite respects the
//! same live-zone / `archive.keep_turns` rule as `archive` (recent turns stay intact).

use serde_json::Value;

use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Manifest, Measurement, Plugin, Surface, ToolResultRef, WireRequest,
};

pub struct Toon;

impl Plugin for Toon {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "toon",
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: false,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "TOON",
            "Compact tabular JSON. Off by default until it beats the corpus.",
            true,
        )
    }

    fn proxy_filter(&self, req: &mut WireRequest<'_>, cx: &Ctx) -> Vec<Measurement> {
        if !cx.plugin_config::<crate::config::Toon>("toon").enabled {
            return Vec::new();
        }
        rewrite(req.tool_results(), cx)
    }
}

fn rewrite(results: Vec<ToolResultRef<'_>>, cx: &Ctx) -> Vec<Measurement> {
    let min_rows = cx.plugin_config::<crate::config::Toon>("toon").min_rows as usize;
    // Same live-zone rule as `archive`: never touch the last `keep_turns` turns.
    let keep = cx
        .plugin_config::<crate::config::Archive>("archive")
        .keep_turns as usize;
    let mut out = Vec::new();
    for result in results {
        if result.turn < keep {
            continue;
        }
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
        .put_archive(&bytes)
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

/// Only arrays of uniform scalar objects encode; shared with the `--ai` report
/// rendering (T22.4), which shapes the same tables for a model. `pub(crate)` so the
/// encoder has one home (D6: one code path per method).
pub(crate) fn tabular_keys(value: &Value, min_rows: usize) -> Option<Vec<String>> {
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

/// TOON block for uniform rows under `keys`, in the caller's column order.
/// `pub(crate)` with [`tabular_keys`]: the `--ai` rendering dogfoods this encoder
/// instead of growing a second table syntax.
pub(crate) fn encode(rows: &[Value], keys: &[String]) -> String {
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
    use rtok_plugin_sdk::WireRequest;
    use serde_json::json;

    fn cx(name: &str, enabled: bool, min_rows: u32) -> crate::plugin::Runtime {
        let dir = std::env::temp_dir().join(format!("rtok-toon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = crate::plugin::Runtime::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx.config.plugins.toon.enabled = enabled;
        cx.config.plugins.toon.min_rows = min_rows;
        // Encoding unit tests use a single turn; live-zone coverage sets keep_turns itself.
        cx.config.plugins.archive.keep_turns = 0;
        cx
    }

    fn tool_req(table: Value) -> Value {
        let content = serde_json::to_string_pretty(&table).unwrap();
        json!({"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content": content}]}]})
    }

    /// Six user turns, each with the same tabular tool result. Turns are counted from
    /// the end (`turn` = users after this one), matching the Anthropic wire.
    fn six_turn_req(table: Value) -> Value {
        let content = serde_json::to_string_pretty(&table).unwrap();
        let mut messages = Vec::new();
        for i in 1..=6 {
            messages.push(json!({
                "role": "user",
                "content": [{"type":"tool_result","tool_use_id": format!("t{i}"), "content": content}]
            }));
            if i < 6 {
                messages.push(json!({"role":"assistant","content":"ok"}));
            }
        }
        json!({"messages": messages})
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
        assert!(filter(&mut body, &Ctx::new(&cx)).is_empty());
        assert_eq!(body, original);
    }

    #[test]
    fn encodes_3x4_table_and_decode_recovers_keys() {
        let cx = cx("enc", true, 3);
        let mut body = tool_req(rows_3x4());
        let ms = filter(&mut body, &Ctx::new(&cx));
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

    #[test]
    fn live_zone_turns_are_untouched() {
        let mut cx = cx("live", true, 3);
        cx.config.plugins.archive.keep_turns = 4;
        let table = rows_3x4();
        let mut body = six_turn_req(table.clone());
        let original = body.clone();
        let ms = filter(&mut body, &Ctx::new(&cx));
        // keep_turns=4 → only turns with turn>=4 rewrite (first two of six).
        assert_eq!(ms.len(), 2, "only older-than-live-zone results encode");
        let texts: Vec<_> = (0..6)
            .map(|i| {
                // message index: user, assistant, user, ... → user at 2*i
                body["messages"][2 * i]["content"][0]["content"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert!(
            texts[0].starts_with("[toon "),
            "turn1 should encode: {}",
            texts[0]
        );
        assert!(
            texts[1].starts_with("[toon "),
            "turn2 should encode: {}",
            texts[1]
        );
        let pretty = serde_json::to_string_pretty(&table).unwrap();
        for t in &texts[2..] {
            assert_eq!(t, &pretty, "live-zone turn must stay original");
        }
        // Prefix before first rewrite stays byte-stable vs a fresh filter of the original.
        let mut again = original.clone();
        filter(&mut again, &Ctx::new(&cx));
        assert_eq!(body, again);
    }
}
