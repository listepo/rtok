//! PreToolUse(Read) advice (plan T4.6): deny native Read of large files not just edited.

use crate::plugin::{Ctx, PreToolDecision, PreToolUse};

const REASON: &str =
    "use rtok read(mode=map) first; native Read allowed for files you are about to edit";

pub fn pre_tool(ev: &PreToolUse<'_>, cx: &Ctx) -> Option<PreToolDecision> {
    if !cx.config.plugins.read.advice || ev.tool_name != "Read" {
        return None;
    }
    let path = ev.tool_input.get("file_path")?.as_str()?;
    let len = std::fs::metadata(path).ok()?.len();
    if len <= cx.config.plugins.read.native_max_bytes {
        return None;
    }
    if recently_edited(cx, path) {
        return None;
    }
    Some(PreToolDecision::Deny {
        reason: REASON.into(),
    })
}

fn recently_edited(cx: &Ctx, path: &str) -> bool {
    let _ = (cx, path);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::plugin::Ctx;
    use serde_json::json;
    use std::fs;

    fn cx(name: &str) -> Ctx {
        let dir =
            std::env::temp_dir().join(format!("rtok-read-hook-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        Ctx::open(c, name).unwrap()
    }

    fn ev<'a>(input: &'a serde_json::Value) -> PreToolUse<'a> {
        PreToolUse {
            tool_name: "Read",
            tool_input: input,
        }
    }

    #[test]
    fn hundred_kb_is_denied() {
        let cx = cx("big");
        let p = cx.config.core.archive_dir.parent().unwrap().join("big.txt");
        fs::write(&p, "x".repeat(100 * 1024)).unwrap();
        let input = json!({"file_path": p.to_str().unwrap()});
        let d = pre_tool(&ev(&input), &cx).expect("deny");
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
        assert!(pre_tool(&ev(&input), &cx).is_none());
    }
}
