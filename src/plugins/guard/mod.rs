//! Deny duplicate Read/Bash when a prior archive id exists (plan T2.6).

use crate::plugin::{
    Ctx, Manifest, Measurement, Plugin, PostToolUse, PreToolDecision, PreToolUse, Surface,
};
use serde_json::Value;

pub struct Guard;

impl Plugin for Guard {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "guard",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        let key = cache_key(ev.tool_name, ev.tool_input)?;
        let (id, ts) = cx.store.get_read_cache(&cx.session, &key).ok().flatten()?;
        let id = id?;
        let n = cx.store.calls_since(&cx.session, ts).unwrap_or(0);
        if n > i64::from(cx.config.plugins.guard.window_turns) {
            return None;
        }
        let reason = format!("duplicate; rtok expand {id}");
        let _ = cx.record(&Measurement {
            plugin: "guard",
            kind: "guard",
            before_bytes: 0,
            after_bytes: 0,
            est_before: 0,
            est_after: 0,
            ref_id: Some(id.clone()),
            call_id: None,
        });
        Some(PreToolDecision::Deny { reason })
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        let key = cache_key(ev.tool_name, ev.tool_input)?;
        let body = payload(ev.tool_response);
        let id = cx
            .store
            .put_archive(&cx.session, &body, &cx.config.core.archive_dir)
            .ok()?;
        let _ = cx.store.put_read_cache(&cx.session, &key, &id, Some(&id));
        None
    }
}

fn cache_key(tool: &str, input: &Value) -> Option<String> {
    match tool {
        "Read" => {
            let p = input.get("file_path")?.as_str()?.trim();
            (!p.is_empty()).then(|| format!("read:{p}"))
        }
        "Bash" => {
            let c = input.get("command")?.as_str()?;
            Some(format!("bash:{}", norm_cmd(c)))
        }
        _ => None,
    }
}

fn norm_cmd(s: &str) -> String {
    let mut t = collapse(s);
    while let Some(rest) = strip_cd_and(&t) {
        t = rest;
    }
    t
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_cd_and(s: &str) -> Option<String> {
    let s = s.strip_prefix("cd ")?;
    let i = s.find("&&")?;
    Some(collapse(&s[i + 2..]))
}

fn payload(v: &Value) -> Vec<u8> {
    if let Some(s) = v.as_str() {
        return s.as_bytes().to_vec();
    }
    for k in ["stdout", "content"] {
        if let Some(s) = v.get(k).and_then(Value::as_str) {
            return s.as_bytes().to_vec();
        }
    }
    serde_json::to_vec(v).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::plugin::Ctx;
    use serde_json::json;

    fn setup() -> Ctx {
        let dir = std::env::temp_dir().join("rtok-t26-two-identical-read");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.core.db_path = dir.join("db");
        cfg.core.archive_dir = dir.join("ar");
        Ctx::open(cfg, "t26").unwrap()
    }

    #[test]
    fn two_identical_reads_second_denies_naming_archive() {
        let cx = setup();
        let g = Guard;
        let path = json!({"file_path": "/Users/dev/proj/src/main.rs"});
        let other = json!({"file_path": "/Users/dev/proj/src/lib.rs"});
        let read = PreToolUse {
            tool_name: "Read",
            tool_input: &path,
        };
        assert!(g.pre_tool(&read, &cx).is_none());
        let resp = json!({"content": "fn main() {}"});
        let post = PostToolUse {
            tool_name: "Read",
            tool_input: &path,
            tool_response: &resp,
        };
        assert!(g.post_tool(&post, &cx).is_none());
        match g.pre_tool(&read, &cx) {
            Some(PreToolDecision::Deny { reason }) => {
                assert!(reason.contains("rtok expand "), "{reason}");
                let id = reason.rsplit(' ').next().unwrap();
                assert!(reason.contains(id));
            }
            other => panic!("{other:?}"),
        }
        let diff = PreToolUse {
            tool_name: "Read",
            tool_input: &other,
        };
        assert!(g.pre_tool(&diff, &cx).is_none());
        assert!(cx.store.measurement_count("guard").unwrap() >= 1);
    }
}
