//! PreToolUse(Read) advice (plan T4.6): deny native Read of large files not just edited.

use rtok_plugin_sdk::{Ctx, PreToolDecision, PreToolUse};

const REASON: &str =
    "use rtok read(mode=map) first; native Read allowed for files you are about to edit";

pub fn pre_tool(ev: &PreToolUse<'_>, cx: &Ctx) -> Option<PreToolDecision> {
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    if !cfg.advice || ev.tool_name != "Read" {
        return None;
    }
    let path = ev.tool_input.get("file_path")?.as_str()?;
    let len = std::fs::metadata(path).ok()?.len();
    if len <= cfg.native_max_bytes {
        return None;
    }
    if recently_edited(cx, path) {
        return None;
    }
    Some(PreToolDecision::Deny {
        reason: REASON.into(),
    })
}

/// How many recent hook calls form the "last 5 turns" window: every tool call
/// emits PreToolUse + PostToolUse, so 5 turns ≈ 10 hook rows.
const WINDOW_CALLS: i64 = 10;

/// True when a PostToolUse(Edit|Write) for `path` sits in the window.
/// Fail open: any store error allows the Read (unmodified input, D1).
fn recently_edited(cx: &Ctx, path: &str) -> bool {
    let bodies = match cx.recent_hook_inputs(WINDOW_CALLS) {
        Ok(b) => b,
        Err(_) => return true,
    };
    bodies.iter().any(|b| edits_path(b, path))
}

fn edits_path(stdin: &str, path: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(stdin) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if v.get("hook_event_name").and_then(|e| e.as_str()) != Some("PostToolUse") {
        return false;
    }
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
}
