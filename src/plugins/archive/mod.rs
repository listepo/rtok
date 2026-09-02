//! `archive` — replace old, large `tool_result` blocks in the live zone with pointers (plan P5).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.
//!
//! T5.3: in `proxy.mode = "compress"`, a `tool_result` block is rewritten when it is older
//! than `archive.keep_turns` (a turn = one `user` message, counted from the end) and above
//! `archive.min_tokens`. The original goes to the archive store; the block's `content`
//! becomes a pointer: `[archived <id>: N lines · T tokens · expand(<full id>)]` followed by
//! `head_lines` … `tail_lines` of the original. The decision is persisted per
//! `tool_use_id` before the request is forwarded, so the pointer is byte-identical on every
//! later request (the frozen prefix stays cacheable) and an `expand`ed id (T5.4) is never
//! rewritten again. `system`, `tools` and the last `keep_turns` turns are never touched.

use serde_json::Value;

use crate::plugin::{Ctx, Manifest, Measurement, MessagesRequest, Plugin, Surface};
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

    fn proxy_filter(&self, req: &mut MessagesRequest, cx: &Ctx) -> Vec<Measurement> {
        if cx.config.proxy.mode != "compress" {
            return Vec::new();
        }
        rewrite(req, cx)
    }
}

/// Rewrite every eligible `tool_result` in `req.messages` in place; one measurement per
/// rewritten block, all under one `plugin_run` child call. Mode-agnostic (the trait method
/// gates on `proxy.mode`); the T5.3 tests call it directly.
pub fn rewrite(req: &mut Value, cx: &Ctx) -> Vec<Measurement> {
    let keep = cx.config.plugins.archive.keep_turns as usize;
    let Some(messages) = req.get_mut("messages").and_then(Value::as_array_mut) else {
        return Vec::new();
    };
    let total = messages.iter().filter(|m| m["role"] == "user").count();
    let mut out = Vec::new();
    let mut turn = 0;
    for msg in messages.iter_mut() {
        if msg["role"] != "user" {
            continue;
        }
        turn += 1;
        if total - turn < keep {
            break; // the live tail: never touched
        }
        let Some(blocks) = msg.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        for block in blocks.iter_mut().filter(|b| b["type"] == "tool_result") {
            if let Some(m) = rewrite_block(block, cx) {
                out.push(m);
            }
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
fn rewrite_block(block: &mut Value, cx: &Ctx) -> Option<Measurement> {
    let tool_use_id = block.get("tool_use_id")?.as_str()?.to_string();
    let text = block_text(block.get("content")?)?;
    let a = &cx.config.plugins.archive;
    let (archive_id, pointer) = match cx.store.archive_decision(&tool_use_id) {
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
                .put_archive_decision(&tool_use_id, &archive_id, &cx.session, &pointer)
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
    block["content"] = Value::String(pointer);
    Some(m)
}

/// The text of a `tool_result` content: a string, or text blocks joined by newlines.
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
    use serde_json::json;

    /// A tool_result far above `min_tokens` (1500 est.) — 400 numbered lines.
    pub fn big(tag: &str) -> String {
        (1..=400)
            .map(|i| format!("{tag} line {i}: some shell output with words"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `turns` user turns; each carries one tool_result for `tu-<turn>` with `big` text.
    pub fn request(turns: usize) -> Value {
        let mut messages = Vec::new();
        for t in 1..=turns {
            messages.push(json!({"role":"user","content":[
                {"type":"tool_result","tool_use_id":format!("tu-{t}"),"content":big(&format!("t{t}"))}]}));
            messages.push(json!({"role":"assistant","content":[
                {"type":"tool_use","id":format!("tu-{}", t+1),"name":"Bash","input":{}}]}));
        }
        json!({"model":"m","max_tokens":8,"system":"sys","tools":[{"name":"Bash"}],"messages":messages})
    }

    fn cx(name: &str) -> Ctx {
        let dir = std::env::temp_dir().join(format!("rtok-archive-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = Ctx::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx.config.proxy.mode = "compress".into();
        cx
    }

    fn contents(v: &Value) -> Vec<String> {
        v["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| m["content"][0]["content"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn only_turns_older_than_keep_turns_are_rewritten() {
        let cx = cx("turns");
        let mut req = request(6);
        let ms = rewrite(&mut req, &cx);
        assert_eq!(ms.len(), 2);
        let c = contents(&req);
        assert!(c[0].starts_with("[archived ") && c[1].starts_with("[archived "));
        assert!(c[0].contains("t1 line 1:") && c[0].contains("t1 line 400"));
        assert!(c[0].contains("… 388 lines …"));
        for (i, t) in c.iter().enumerate().skip(2) {
            assert_eq!(*t, big(&format!("t{}", i + 1)), "turn {} untouched", i + 1);
        }
        assert_eq!(req["system"], "sys");
        assert_eq!(req["tools"][0]["name"], "Bash");
        assert!(ms[0].est_before > ms[0].est_after * 5);
        assert_eq!(cx.store.count_kind("plugin_run").unwrap(), 1);
        assert_eq!(cx.store.count_tokens().unwrap(), 2);
        let id = ms[0].ref_id.as_deref().unwrap();
        assert_eq!(
            cx.store.get_archive(id).unwrap().unwrap(),
            big("t1").into_bytes()
        );
    }

    #[test]
    fn same_request_twice_is_byte_identical_and_prefix_unchanged() {
        let cx = cx("prefix");
        let original = serde_json::to_string(&request(6)).unwrap();
        let mut a = request(6);
        rewrite(&mut a, &cx);
        let mut b = request(6);
        rewrite(&mut b, &cx);
        let a = serde_json::to_string(&a).unwrap();
        assert_eq!(a, serde_json::to_string(&b).unwrap());
        // Everything before the first rewritten block's content is untouched.
        let enc = serde_json::to_string(&big("t1")).unwrap();
        let first = original.find(&enc[1..enc.len() - 1]).unwrap();
        assert!(first > 0);
        assert_eq!(&a[..first], &original[..first]);
    }

    #[test]
    fn small_results_and_expanded_ids_are_left_alone() {
        let cx = cx("skip");
        let mut req = request(6);
        req["messages"][0]["content"][0]["content"] = json!("tiny");
        let ms = rewrite(&mut req, &cx);
        assert_eq!(ms.len(), 1, "only turn 2 is large");
        assert_eq!(contents(&req)[0], "tiny");
        let id = ms[0].ref_id.clone().unwrap();
        assert_eq!(cx.store.mark_expanded(&id).unwrap(), 1);
        let mut again = request(6);
        again["messages"][0]["content"][0]["content"] = json!("tiny");
        assert!(rewrite(&mut again, &cx).is_empty());
        assert_eq!(contents(&again)[1], big("t2"));
        assert_eq!(cx.store.archive_decision_counts().unwrap(), (1, 1));
    }

    /// T5.4 Check: expand → the next request carries the original block again.
    #[test]
    fn expand_freezes_the_id_so_the_original_is_sent_again() {
        let cx = cx("expand");
        let mut req = request(6);
        let ms = rewrite(&mut req, &cx);
        let id = ms[0].ref_id.clone().unwrap();
        let bytes = crate::expand::fetch(&cx, &id).unwrap().unwrap();
        assert_eq!(bytes, big("t1").into_bytes());
        let mut next = request(6);
        let ms2 = rewrite(&mut next, &cx);
        assert_eq!(
            ms2.len(),
            1,
            "turn 2 stays a pointer, turn 1 is original again"
        );
        assert_eq!(contents(&next)[0], big("t1"));
        assert!(contents(&next)[1].starts_with("[archived "));
        assert_eq!(cx.store.archive_decision_counts().unwrap(), (2, 1));
        // One `expand` measurement per freeze; a repeat expand is not a second freeze.
        assert_eq!(cx.store.measurement_count("archive").unwrap(), 1);
        crate::expand::fetch(&cx, &id).unwrap();
        assert_eq!(cx.store.measurement_count("archive").unwrap(), 1);
        assert!(crate::expand::fetch(&cx, "no-such").unwrap().is_none());
    }

    #[test]
    fn passthrough_mode_never_rewrites() {
        let mut cx = cx("mode");
        cx.config.proxy.mode = "passthrough".into();
        let mut req = request(6);
        assert!(Archive.proxy_filter(&mut req, &cx).is_empty());
        assert_eq!(req, request(6));
    }

    #[test]
    fn text_blocks_join_and_images_are_skipped() {
        assert_eq!(
            block_text(&json!([{"type":"text","text":"a"},{"type":"text","text":"b"}])),
            Some("a\nb".into())
        );
        assert_eq!(block_text(&json!([{"type":"image"}])), None);
        assert_eq!(
            pointer("a\nb", "abc", 3, 8, 4),
            "[archived abc: 2 lines · 3 tokens · expand(abc)]\na\nb"
        );
    }
}
