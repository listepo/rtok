//! PreToolUse(Read) advice (plan T4.6): deny native Read of large files not just edited.

use rtok_plugin_sdk::{Ctx, PreToolDecision, PreToolUse};

const REASON: &str =
    "use rtok read(mode=map) first; native Read allowed for files you are about to edit";

pub fn pre_tool(ev: &PreToolUse<'_>, cx: &Ctx) -> Option<PreToolDecision> {
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    if !cfg.advice || ev.tool_name != "Read" {
        return None;
    }
    let path = super::path_arg(ev.tool_input)?;
    let len = std::fs::metadata(path).ok()?.len();
    if len <= cfg.native_max_bytes {
        return None;
    }
    if recently_edited(cx, path) {
        if cfg.delta {
            if let Some(id) = last_read_id(cx, path) {
                return Some(PreToolDecision::Deny {
                    reason: format!(
                        "file changed since last read; use rtok read(mode=diff) vs {id:.8}"
                    ),
                });
            }
        }
        return None;
    }
    Some(PreToolDecision::Deny {
        reason: REASON.into(),
    })
}

/// Archive id of the last MCP `read` of this path (`mode=full`), if still cached.
fn last_read_id(cx: &Ctx, path: &str) -> Option<String> {
    let abs = dunce::canonicalize(path).ok()?;
    let key = super::cache::key(abs.to_string_lossy().as_ref(), "full", None);
    cx.get_read_cache(&key).ok().flatten()?.0
}

/// The edit window: the last 5 finished tool calls (PostToolUse rows).
const WINDOW_TOOL_CALLS: usize = 5;
/// Hook rows read to find them. The window used to be the last 10 rows of any event, but
/// prompts, session starts, compactions and every PreToolUse write rows too, so an edit two
/// tool calls back could already be out of it.
const SCAN_ROWS: i64 = 50;

/// True when a PostToolUse(Edit|Write) for `path` sits in the window.
/// Fail open: a store error allows the Read (unmodified input, D1), and so does a hook
/// body the store could not keep (empty string) — the window cannot rule out an edit of
/// this file, and denying a Read of a file the agent just wrote is the worse error.
fn recently_edited(cx: &Ctx, path: &str) -> bool {
    let bodies = match cx.recent_hook_inputs(SCAN_ROWS) {
        Ok(b) => b,
        Err(_) => return true,
    };
    let mut tool_calls = 0;
    for b in &bodies {
        if b.is_empty() {
            return true;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(b) else {
            continue;
        };
        if v.get("hook_event_name").and_then(|e| e.as_str()) != Some("PostToolUse") {
            continue;
        }
        if edits_path(&v, path) {
            return true;
        }
        tool_calls += 1;
        if tool_calls == WINDOW_TOOL_CALLS {
            break;
        }
    }
    false
}

fn edits_path(v: &serde_json::Value, path: &str) -> bool {
    let tool = v.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
    if tool != "Edit" && tool != "Write" {
        return false;
    }
    let edited = v
        .get("tool_input")
        .and_then(|i| i.get("file_path").or_else(|| i.get("path")))
        .and_then(|p| p.as_str())
        .unwrap_or("");
    same_path(edited, path)
}

/// Exact match, or relative-vs-absolute (`src/main.rs` vs `/repo/src/main.rs`).
/// Component-aware: `/repo/main.rs` must not match `ain.rs`.
fn same_path(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let a_path = std::path::Path::new(a);
    let b_path = std::path::Path::new(b);
    a_path.ends_with(b_path) || b_path.ends_with(a_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;
    use serde_json::json;
    use std::fs;

    fn cx(name: &str) -> crate::plugin::Runtime {
        crate::testutil::runtime(name).0
    }

    fn ev<'a>(input: &'a serde_json::Value) -> PreToolUse<'a> {
        PreToolUse {
            tool_name: "Read",
            tool_input: input,
        }
    }

    #[test]
    fn edited_file_reads_on() {
        let cx = cx("edit");
        let dir = cx.config.core.archive_dir.parent().unwrap().to_path_buf();
        let p = dir.join("edit.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let path = p.to_str().unwrap();
        let edit = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Edit",
            "tool_input": {"file_path": path},
        });
        let id = cx.record_call("hook", "hook", None).unwrap();
        cx.store
            .insert_call_io(
                id,
                Some(&serde_json::to_vec(&edit).unwrap()),
                None,
                65536,
                None,
            )
            .unwrap();
        let input = json!({"file_path": path});
        assert!(pre_tool(&ev(&input), &Ctx::new(&cx)).is_none());
    }

    #[test]
    fn edited_reread_advises_diff_when_cached() {
        let cx = cx("delta-advice");
        let dir = cx.config.core.archive_dir.parent().unwrap().to_path_buf();
        let p = dir.join("delta.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let path = p.to_str().unwrap();
        let abs = dunce::canonicalize(&p).unwrap();
        let key = crate::plugins::read::cache::key(abs.to_str().unwrap(), "full", None);
        let ctx = Ctx::new(&cx);
        let id = ctx.put_archive(b"previous body").unwrap();
        ctx.put_read_cache(&key, "h", Some(&id)).unwrap();
        let edit = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Edit",
            "tool_input": {"file_path": path},
        });
        let hid = cx.record_call("hook", "hook", None).unwrap();
        cx.store
            .insert_call_io(
                hid,
                Some(&serde_json::to_vec(&edit).unwrap()),
                None,
                65536,
                None,
            )
            .unwrap();
        let input = json!({"file_path": path});
        match pre_tool(&ev(&input), &Ctx::new(&cx)) {
            Some(PreToolDecision::Deny { reason }) => {
                assert!(reason.contains("mode=diff"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }

    fn hook_row(cx: &crate::plugin::Runtime, body: serde_json::Value) {
        let id = cx.record_call("hook", "hook", None).unwrap();
        let bytes = serde_json::to_vec(&body).unwrap();
        cx.store
            .insert_call_io(id, Some(&bytes), None, 65536, None)
            .unwrap();
    }

    /// Non-tool rows (prompts, PreToolUse) do not push an edit out of the window; the
    /// sixth finished tool call after it does.
    #[test]
    fn the_window_counts_tool_calls_not_hook_rows() {
        let cx = cx("window");
        let p = cx.config.core.archive_dir.parent().unwrap().join("w.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let path = p.to_str().unwrap();
        hook_row(
            &cx,
            json!({"hook_event_name": "PostToolUse", "tool_name": "Write", "tool_input": {"file_path": path}}),
        );
        for _ in 0..12 {
            hook_row(&cx, json!({"hook_event_name": "UserPromptSubmit"}));
        }
        let input = json!({"file_path": path});
        assert!(
            pre_tool(&ev(&input), &Ctx::new(&cx)).is_none(),
            "still in window"
        );
        for _ in 0..WINDOW_TOOL_CALLS {
            hook_row(
                &cx,
                json!({"hook_event_name": "PreToolUse", "tool_name": "Bash"}),
            );
            hook_row(
                &cx,
                json!({"hook_event_name": "PostToolUse", "tool_name": "Bash"}),
            );
        }
        assert!(pre_tool(&ev(&input), &Ctx::new(&cx)).is_some(), "aged out");
    }

    #[test]
    fn hundred_kb_is_denied() {
        let cx = cx("big");
        let p = cx.config.core.archive_dir.parent().unwrap().join("big.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let input = json!({"file_path": p.to_str().unwrap()});
        let d = pre_tool(&ev(&input), &Ctx::new(&cx)).expect("deny");
        match d {
            PreToolDecision::Deny { reason } => assert!(reason.contains("rtok read"), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    /// T55.13: Copilot's adapted `Read` carries `path`, not `file_path`; the large-file
    /// advice must still deny.
    #[test]
    fn copilot_path_key_gets_the_read_advice() {
        let cx = cx("copilot-path");
        let dir = cx.config.core.archive_dir.parent().unwrap().to_path_buf();
        let p = dir.join("copilot-big.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let input = json!({"path": p.to_str().unwrap()});
        match pre_tool(&ev(&input), &Ctx::new(&cx)) {
            Some(PreToolDecision::Deny { reason }) => {
                assert!(reason.contains("rtok read"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn two_kb_is_silent() {
        let cx = cx("small");
        let p = cx
            .config
            .core
            .archive_dir
            .parent()
            .unwrap()
            .join("small.txt");
        fs::write(&p, "x".repeat(2048)).unwrap();
        let input = json!({"file_path": p.to_str().unwrap()});
        assert!(pre_tool(&ev(&input), &Ctx::new(&cx)).is_none());
    }

    #[test]
    fn same_path_rejects_suffix_false_positive() {
        assert!(!same_path("/repo/main.rs", "ain.rs"));
        assert!(same_path("/repo/src/main.rs", "src/main.rs"));
        assert!(same_path("src/main.rs", "/repo/src/main.rs"));
    }

    /// A hook body the store could not keep (above `core.call_io_inline_bytes`, no archive
    /// dir) comes back empty. The window must fail open there: a big `Write` followed by a
    /// native `Read` of the same file used to be denied because the write was invisible.
    #[test]
    fn an_unreadable_hook_body_allows_the_read() {
        let cx = cx("elided");
        let dir = cx.config.core.archive_dir.parent().unwrap().to_path_buf();
        let p = dir.join("elided-big.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let big = "x".repeat(70 * 1024);
        let stdin = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": p.to_str().unwrap()},
            "pad": big,
        });
        let id = cx.record_call("hook", "hook", None).unwrap();
        cx.store
            .insert_call_io(
                id,
                Some(&serde_json::to_vec(&stdin).unwrap()),
                None,
                65536,
                None,
            )
            .unwrap();
        assert!(
            cx.store.recent_hook_inputs(&cx.session, 10).unwrap()[0].is_empty(),
            "the body is elided, not stored"
        );
        let input = json!({"file_path": p.to_str().unwrap()});
        assert!(
            pre_tool(&ev(&input), &Ctx::new(&cx)).is_none(),
            "an unreadable window must not deny a file this session may have written"
        );
    }
}
