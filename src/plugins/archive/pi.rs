//! T70.2: the pi `context` carrier for the `archive` live zone.
//!
//! pi's `context` event fires before every LLM call with the full message
//! array (`event.messages`, a deep copy) and accepts `{ messages }` back as
//! what is sent — verified against the pi 0.85.1 extensions docs
//! (https://pi.dev/docs/latest/extensions) and a real session file under
//! `~/.pi/agent/sessions/`: results are `role: "toolResult"` messages keyed by
//! a camelCase `toolCallId`, with text-block `content`. Because the handler
//! re-runs before every call, the rewrite must be idempotent: `archive`
//! persists each pointer decision in the store, so replaying the same array
//! yields byte-identical output, and a newly-aged block is the only change on
//! the next call.
//!
//! [`rewrite_stdin`] is the `rtok archive rewrite --stdin` entry point the pi
//! extension shells out to. It is a second carrier of the SAME live-zone
//! function the proxy filter calls (`rewrite` + `rewrite_blobs` over
//! `ToolResultRef` / `BlobRef`); there is no second implementation.

use anyhow::Result;
use serde_json::Value;

use rtok_plugin_sdk::{BlobRef, Ctx, ToolResultRef};

/// pi session messages → wire-normalised tool results. A message is a result
/// when its role is `toolResult`; the id is the camelCase `toolCallId` pi
/// carries, the payload is its `content` (a string or text-block array,
/// rewritten in place by `rewrite`). Turns are counted the way the proxy
/// wires count them: user messages, from the end.
pub fn tool_results(messages: &mut Value) -> Vec<ToolResultRef<'_>> {
    let Some((entries, total)) = split(messages) else {
        return Vec::new();
    };
    let mut seen = 0;
    let mut out = Vec::new();
    for message in entries {
        if message["role"] == "user" {
            seen += 1;
            continue;
        }
        if message["role"] != "toolResult" {
            continue;
        }
        let turn = total - seen;
        let Some(id) = message
            .get("toolCallId")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(content) = message.get_mut("content") else {
            continue;
        };
        out.push(ToolResultRef { id, content, turn });
    }
    out
}

/// Shrinkable non-result payloads (T51.1 `live_blobs`): user message text,
/// whole strings and `text` parts. The `live_blobs` gate lives in
/// `rewrite_blobs`, same as the proxy carrier.
pub fn live_blobs(messages: &mut Value) -> Vec<BlobRef<'_>> {
    let Some((entries, total)) = split(messages) else {
        return Vec::new();
    };
    let mut seen = 0;
    let mut out = Vec::new();
    for message in entries {
        if message["role"] != "user" {
            continue;
        }
        seen += 1;
        let turn = total - seen;
        if message.get("content").is_some_and(Value::is_string) {
            let content = message.get_mut("content").expect("string checked above");
            out.push(BlobRef { content, turn });
        } else if let Some(parts) = message
            .get_mut("content")
            .and_then(Value::as_array_mut)
        {
            for part in parts {
                if part["type"].as_str() == Some("text")
                    && let Some(content) = part.get_mut("text")
                {
                    out.push(BlobRef { content, turn });
                }
            }
        }
    }
    out
}

/// The message array and how many of its entries are a user turn, or `None`
/// when `messages` is not an array. Shared by both views so turn counting
/// stays one rule.
fn split(messages: &mut Value) -> Option<(&mut Vec<Value>, usize)> {
    let entries = messages.as_array_mut()?;
    let total = entries.iter().filter(|entry| entry["role"] == "user").count();
    Some((entries, total))
}

/// `rtok archive rewrite --stdin` (T70.2): apply the one live-zone rewrite to
/// a pi message array. Input bytes that are not a JSON array, and arrays where
/// nothing is eligible, come back verbatim — the caller compares against its
/// input to detect "no change" (and pi keeps the exact same array). Any store
/// error leaves the array alone (fail open, D1).
pub fn rewrite_stdin(input: &[u8], cx: &Ctx) -> Result<Vec<u8>> {
    let Ok(mut messages) = serde_json::from_slice::<Value>(input) else {
        return Ok(input.to_vec());
    };
    if !messages.is_array()
        || !cx
            .plugin_config::<crate::config::Archive>("archive")
            .enabled
    {
        return Ok(input.to_vec());
    }
    let mut ms = super::rewrite(tool_results(&mut messages), cx);
    ms.extend(super::rewrite_blobs(live_blobs(&mut messages), cx));
    if ms.is_empty() {
        return Ok(input.to_vec());
    }
    for m in &ms {
        if let Err(e) = cx.record(m) {
            cx.log("error", "plugin", "archive", &format!("measurement: {e}"));
        }
    }
    Ok(serde_json::to_vec(&messages)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx(name: &str) -> crate::plugin::Runtime {
        let dir = std::env::temp_dir().join(format!("rtok-pi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = crate::plugin::Runtime::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx
    }

    fn big(tag: &str) -> String {
        (1..=400)
            .map(|i| format!("{tag} line {i}: some shell output with words"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A pi session array: user/assistant/toolResult triples, newest last,
    /// exactly the shape of a real `~/.pi/agent/sessions/**.jsonl` message.
    fn pi_array(tags: &[&str]) -> Value {
        let mut messages = Vec::new();
        for (i, tag) in tags.iter().enumerate() {
            messages.push(serde_json::json!({
                "role": "user",
                "content": [{"type": "text", "text": format!("prompt {i}")}]
            }));
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": [{"type": "text", "text": "working…"}]
            }));
            messages.push(serde_json::json!({
                "role": "toolResult",
                "toolCallId": format!("call_{tag}"),
                "toolName": "bash",
                "content": [{"type": "text", "text": big(tag)}],
                "isError": false
            }));
        }
        Value::Array(messages)
    }

    #[test]
    fn pi_view_yields_tool_results_with_user_turn_distance() {
        let mut messages = pi_array(&["a", "b", "c"]);
        let refs = tool_results(&mut messages);
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].id, "call_a");
        assert_eq!(refs[0].turn, 2, "oldest result sits two user turns back");
        assert_eq!(refs[2].turn, 0, "newest result is this turn");
        assert_eq!(refs[0].content[0]["text"].as_str().unwrap(), big("a"));
    }

    #[test]
    fn non_arrays_and_quiet_arrays_echo_the_input_bytes() {
        let cx = cx("echo");
        let ctx = Ctx::new(&cx);
        let junk = b"not json at all";
        assert_eq!(rewrite_stdin(junk, &ctx).unwrap(), junk);
        let object = b"{\"role\":\"user\"}";
        assert_eq!(rewrite_stdin(object, &ctx).unwrap(), object);
        // A three-turn array with the default keep_turns = 4: nothing outside
        // the live zone, so the bytes come back untouched.
        let quiet = serde_json::to_vec(&pi_array(&["a", "b", "c"])).unwrap();
        assert_eq!(rewrite_stdin(&quiet, &ctx).unwrap(), quiet);
    }

    /// The card's Check: three replays byte-identical; a fourth call with one
    /// new turn changes only the newly-aged block.
    #[test]
    fn rewrite_is_byte_stable_and_ages_one_block_per_turn() {
        let cx = cx("stable");
        let ctx = Ctx::new(&cx);
        let input = serde_json::to_vec(&pi_array(&["a", "b", "c", "d", "e"])).unwrap();
        let out1 = rewrite_stdin(&input, &ctx).unwrap();
        assert_ne!(out1, input, "the oldest result is outside keep_turns = 4");
        let out2 = rewrite_stdin(&out1, &ctx).unwrap();
        let out3 = rewrite_stdin(&out2, &ctx).unwrap();
        assert_eq!(out1, out2, "replay 2 byte-identical");
        assert_eq!(out2, out3, "replay 3 byte-identical");

        // One new turn arrives: only the newly-aged block (b) may change.
        let mut grown: Value = serde_json::from_slice(&out3).unwrap();
        grown.as_array_mut().unwrap().extend([
            serde_json::json!({"role": "user", "content": [{"type": "text", "text": "prompt 5"}]}),
            serde_json::json!({"role": "assistant", "content": [{"type": "text", "text": "…"}]}),
            serde_json::json!({"role": "toolResult", "toolCallId": "call_f", "toolName": "bash",
                "content": [{"type": "text", "text": big("f")}], "isError": false}),
        ]);
        let out4 = rewrite_stdin(&serde_json::to_vec(&grown).unwrap(), &ctx).unwrap();
        let before: Value = serde_json::from_slice(&out3).unwrap();
        let after: Value = serde_json::from_slice(&out4).unwrap();
        let diff = |a: &Value, b: &Value| {
            a.as_array()
                .unwrap()
                .iter()
                .zip(b.as_array().unwrap())
                .enumerate()
                .filter(|(_, (x, y))| x != y)
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        let changed = diff(&before, &after);
        let a_pos = before
            .as_array()
            .unwrap()
            .iter()
            .position(|m| m["toolCallId"] == "call_a")
            .unwrap();
        let b_pos = before
            .as_array()
            .unwrap()
            .iter()
            .position(|m| m["toolCallId"] == "call_b")
            .unwrap();
        assert_eq!(changed, vec![b_pos], "only the newly-aged block changed");
        assert!(after[a_pos]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("[archived "));
        // Lossless: expand recovers the original bytes of every pointer.
        for tag in ["a", "b"] {
            let pos = after
                .as_array()
                .unwrap()
                .iter()
                .position(|m| m["toolCallId"] == format!("call_{tag}"))
                .unwrap();
            let text = after[pos]["content"][0]["text"].as_str().unwrap();
            let id = text
                .split("expand(")
                .nth(1)
                .unwrap()
                .split(')')
                .next()
                .unwrap();
            let back = ctx.get_archive(id).unwrap().expect("archived payload");
            assert_eq!(String::from_utf8(back).unwrap(), big(tag));
        }
        assert_eq!(
            cx.store.measurement_count("archive").unwrap(),
            2,
            "one measurement row per rewritten block, replays add none"
        );
    }

    /// The extension's own contract: when the array is small enough that
    /// nothing is eligible, the output IS the input (cheap no-change compare).
    #[test]
    fn small_results_stay_whole() {
        let cx = cx("small");
        let ctx = Ctx::new(&cx);
        let mut messages = pi_array(&["a"]);
        messages[2]["content"] = serde_json::json!([{"type": "text", "text": "tiny"}]);
        let input = serde_json::to_vec(&messages).unwrap();
        assert_eq!(rewrite_stdin(&input, &ctx).unwrap(), input);
        assert_eq!(cx.store.measurement_count("archive").unwrap(), 0);
    }
}
