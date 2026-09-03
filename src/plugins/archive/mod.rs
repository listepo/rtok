//! `archive` — replace old, large tool-result blocks in the live zone with pointers (plan P5).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.
//!
//! T5.3: in `proxy.mode = "compress"`, a tool-result block is rewritten when it is older
//! than `archive.keep_turns` (a turn = one `user` message, counted from the end) and above
//! `archive.min_tokens`. The original goes to the archive store; the block's `content`
//! becomes a pointer: `[archived <id>: N lines · T tokens · expand(<full id>)]` followed by
//! `head_lines` … `tail_lines` of the original. The decision is persisted per
//! `tool_use_id` before the request is forwarded, so the pointer is byte-identical on every
//! later request (the frozen prefix stays cacheable) and an `expand`ed id (T5.4) is never
//! rewritten again. `system`, `tools` and the last `keep_turns` turns are never touched.
//! T11.4 applies the same rewrite through `Wire::tool_results` for Anthropic Messages,
//! OpenAI Chat Completions, and OpenAI Responses.

use serde_json::Value;

use crate::plugin::{Ctx, Manifest, Measurement, Plugin, Surface};
use crate::proxy::wire::{ToolResultRef, WireRequest};
use crate::tokens::Class;

pub struct Archive;

impl Plugin for Archive {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "archive",
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: true,
        }
    }

    fn proxy_filter(&self, req: &mut WireRequest<'_>, cx: &Ctx) -> Vec<Measurement> {
        if cx.config.proxy.mode != "compress" {
            return Vec::new();
        }
        rewrite(req.tool_results(), cx)
    }
}

/// Rewrite every eligible wire-normalised result; one measurement per rewritten block, all
/// under one `plugin_run` child call. The wire owns the provider-specific request shape.
pub fn rewrite(results: Vec<ToolResultRef<'_>>, cx: &Ctx) -> Vec<Measurement> {
    let keep = cx.config.plugins.archive.keep_turns as usize;
    let mut out = Vec::new();
    for result in results {
        if result.turn < keep {
            continue;
        }
        if let Some(m) = rewrite_block(&result.id, result.content, cx) {
            out.push(m);
        }
    }
    if out.is_empty() {
        return out;
    }
    // Ground truth for `stats`: est. tokens before/after, nested under the API request.
    match cx.record_plugin_run("proxy", "archive") {
        Ok(id) => {
            let before = out.iter().map(|m| i64::from(m.est_before)).sum();
            let after = out.iter().map(|m| i64::from(m.est_after)).sum();
            let _ = cx.record_tokens(id, Some("archive"), "before", "estimate", before);
            let _ = cx.record_tokens(id, Some("archive"), "after", "estimate", after);
            for m in &mut out {
                m.call_id = Some(id);
            }
        }
        Err(e) => cx.log("error", "plugin", "archive", &format!("plugin_run: {e}")),
    }
    out
}

/// Decide for one block: reuse the persisted pointer, skip an expanded or small block, or
/// archive it now. Any store error leaves the block alone (fail open).
fn rewrite_block(tool_use_id: &str, content: &mut Value, cx: &Ctx) -> Option<Measurement> {
    let text = block_text(content)?;
    let a = &cx.config.plugins.archive;
    let (archive_id, pointer) = match cx.store.archive_decision(tool_use_id) {
        Ok(Some(d)) if d.expanded => return None,
        Ok(Some(d)) => (d.archive_id, d.pointer),
        Ok(None) => {
            let est = cx.estimate(&text, Class::Code);
            if est < a.min_tokens {
                return None;
            }
            let dir = &cx.config.core.archive_dir;
            let archive_id = cx
                .store
                .put_archive(&cx.session, text.as_bytes(), dir)
                .map_err(|e| cx.log("error", "plugin", "archive", &format!("put: {e}")))
                .ok()?;
            let pointer = pointer(
                &text,
                &archive_id,
                est,
                a.head_lines as usize,
                a.tail_lines as usize,
            );
            cx.store
                .put_archive_decision(tool_use_id, &archive_id, &cx.session, &pointer)
                .map_err(|e| cx.log("error", "plugin", "archive", &format!("decision: {e}")))
                .ok()?;
            (archive_id, pointer)
        }
        Err(e) => {
            cx.log("error", "plugin", "archive", &format!("decision: {e}"));
            return None;
        }
    };
    let m = Measurement {
        plugin: "archive",
        kind: "pointer",
        before_bytes: text.len() as u64,
        after_bytes: pointer.len() as u64,
        est_before: cx.estimate(&text, Class::Code),
        est_after: cx.estimate(&pointer, Class::Code),
        ref_id: Some(archive_id),
        call_id: None,
    };
    *content = Value::String(pointer);
    Some(m)
}

/// The text of a tool-result content: a string, or text blocks joined by newlines.
/// `None` when any part is not text (images stay as they are).
fn block_text(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => {
            let mut out = Vec::with_capacity(parts.len());
            for p in parts {
                if p["type"] != "text" {
                    return None;
                }
                out.push(p["text"].as_str()?);
            }
            Some(out.join("\n"))
        }
        _ => None,
    }
}

fn pointer(text: &str, id: &str, est: u32, head: usize, tail: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();
    let short = &id[..id.len().min(12)];
    let mut s = format!("[archived {short}: {n} lines · {est} tokens · expand({id})]");
    if n <= head + tail {
        for l in &lines {
            s.push('\n');
            s.push_str(l);
        }
        return s;
    }
    for l in &lines[..head] {
        s.push('\n');
        s.push_str(l);
    }
    s.push_str(&format!("\n… {} lines …", n - head - tail));
    for l in &lines[n - tail..] {
        s.push('\n');
        s.push_str(l);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(tag: &str) -> String {
        (1..=400)
            .map(|i| format!("{tag} line {i}: some shell output with words"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn cx(name: &str) -> Ctx {
        let dir = std::env::temp_dir().join(format!("rtok-archive-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = Ctx::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx.config.proxy.mode = "compress".into();
        cx
    }

    fn refs<'a>(values: &'a mut [Value]) -> Vec<ToolResultRef<'a>> {
        let total = values.len();
        values
            .iter_mut()
            .enumerate()
            .map(|(index, content)| ToolResultRef {
                id: format!("tu-{}", index + 1),
                content,
                turn: total - index - 1,
            })
            .collect()
    }

    #[test]
    fn only_results_outside_the_live_tail_are_rewritten() {
        let cx = cx("turns");
        let mut values: Vec<Value> = (1..=6)
            .map(|n| Value::String(big(&format!("t{n}"))))
            .collect();
        let ms = rewrite(refs(&mut values), &cx);
        assert_eq!(ms.len(), 2);
        let first = values[0].as_str().unwrap();
        assert!(first.starts_with("[archived ") && first.contains("t1 line 400"));
        for (index, content) in values.iter().enumerate().skip(2) {
            assert_eq!(content, &Value::String(big(&format!("t{}", index + 1))));
        }
        assert_eq!(cx.store.count_kind("plugin_run").unwrap(), 1);
        let ids = cx.store.call_ids_of_kind("plugin_run").unwrap();
        assert_eq!(ids.len(), 1);
        let phases = cx.store.token_phases(ids[0]).unwrap();
        assert!(phases.iter().any(|p| p == "before"), "{phases:?}");
        assert!(phases.iter().any(|p| p == "after"), "{phases:?}");
    }

    #[test]
    fn decisions_are_deterministic_and_expanded_ids_stay_original() {
        let cx = cx("repeat");
        let mut first = vec![
            Value::String(big("one")),
            Value::String(big("two")),
            Value::String(big("three")),
            Value::String(big("four")),
            Value::String(big("five")),
            Value::String(big("six")),
        ];
        let first_ms = rewrite(refs(&mut first), &cx);
        let archive_id = first_ms[0].ref_id.clone().unwrap();
        let first_body = first.clone();
        let mut second = vec![
            Value::String(big("one")),
            Value::String(big("two")),
            Value::String(big("three")),
            Value::String(big("four")),
            Value::String(big("five")),
            Value::String(big("six")),
        ];
        rewrite(refs(&mut second), &cx);
        assert_eq!(first_body, second);
        assert_eq!(cx.store.mark_expanded(&archive_id).unwrap(), 1);
        let mut third = vec![
            Value::String(big("one")),
            Value::String(big("two")),
            Value::String(big("three")),
            Value::String(big("four")),
            Value::String(big("five")),
            Value::String(big("six")),
        ];
        assert_eq!(rewrite(refs(&mut third), &cx).len(), 1);
        assert_eq!(third[0], Value::String(big("one")));
    }

    #[test]
    fn text_blocks_join_and_images_are_skipped() {
        assert_eq!(
            block_text(&serde_json::json!([{"type":"text","text":"a"},{"type":"text","text":"b"}])),
            Some("a\nb".into())
        );
        assert_eq!(block_text(&serde_json::json!([{"type":"image"}])), None);
        assert_eq!(
            pointer("a\nb", "abc", 3, 8, 4),
            "[archived abc: 2 lines · 3 tokens · expand(abc)]\na\nb"
        );
    }

    fn six_turn_wires() -> [(&'static str, &'static dyn crate::proxy::wire::Wire); 3] {
        use crate::proxy::anthropic::ANTHROPIC;
        use crate::proxy::openai_chat::OPENAI_CHAT;
        use crate::proxy::openai_responses::OPENAI_RESPONSES;
        [
            ("anthropic_messages_6turns.json", &ANTHROPIC),
            ("openai_chat_6turns.json", &OPENAI_CHAT),
            ("openai_responses_6turns.json", &OPENAI_RESPONSES),
        ]
    }

    fn load_six_turn(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/proxy/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&std::fs::read(&path).expect(name)).expect(name)
    }

    fn result_texts(wire: &dyn crate::proxy::wire::Wire, req: &mut Value) -> Vec<String> {
        wire.tool_results(req)
            .into_iter()
            .map(|result| result.content.as_str().expect("string payload").to_string())
            .collect()
    }

    #[test]
    fn six_turn_fixtures_archive_only_turns_1_and_2_on_every_wire() {
        for (name, wire) in six_turn_wires() {
            let cx = cx(name);
            let original = load_six_turn(name);
            let original_bytes = serde_json::to_vec(&original).unwrap();
            let mut first = original.clone();
            let ms = rewrite(wire.tool_results(&mut first), &cx);
            assert_eq!(ms.len(), 2, "{name}");
            let rewritten = serde_json::to_vec(&first).unwrap();
            let pos = rewritten
                .windows(10)
                .position(|window| window == b"[archived ")
                .expect(name);
            assert_eq!(&original_bytes[..pos], &rewritten[..pos], "{name} prefix");
            let mut second = original.clone();
            rewrite(wire.tool_results(&mut second), &cx);
            assert_eq!(first, second, "{name} deterministic");
            let texts = result_texts(wire, &mut first);
            assert_eq!(texts.len(), 6, "{name}");
            assert!(
                texts[0].starts_with("[archived ") && texts[1].starts_with("[archived "),
                "{name}"
            );
            for (index, text) in texts.iter().enumerate().skip(2) {
                assert!(
                    text.starts_with(&format!("t{} line 1:", index + 1)),
                    "{name} turn {}",
                    index + 1
                );
            }
            let archive_id = ms[0].ref_id.clone().unwrap();
            assert_eq!(cx.store.mark_expanded(&archive_id).unwrap(), 1);
            let mut expanded = original.clone();
            assert_eq!(rewrite(wire.tool_results(&mut expanded), &cx).len(), 1);
            let live = result_texts(wire, &mut expanded);
            assert!(live[0].starts_with("t1 line 1:"), "{name} expand");
            assert!(live[1].starts_with("[archived "), "{name} turn 2 stays");
        }
    }
}
