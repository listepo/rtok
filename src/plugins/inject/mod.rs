//! Budgeted SessionStart / UserPromptSubmit injection (plan T2.4, D5).

use crate::plugin::{
    Ctx, Injection, Manifest, Measurement, Plugin, PreCompact, SessionStart, Surface,
};
use crate::tokens::Class;

/// Catalogue plugin `inject`.
pub struct Inject;

impl Plugin for Inject {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "inject",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }

    fn session_start(&self, ev: &SessionStart, cx: &Ctx) -> Option<Injection> {
        let mut text = String::new();
        let mut priority = 5u8;
        if ev.source == "compact"
            && let Some(c) = super::checkpoint::offer(cx)
        {
            text.push_str(&c.text);
            priority = 9;
        }
        if let Some(m) = modes_text(cx) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&m);
        }
        if text.is_empty() {
            None
        } else {
            Some(Injection {
                plugin: "inject",
                text,
                priority,
            })
        }
    }

    fn pre_compact(&self, ev: &PreCompact, cx: &Ctx) {
        let _ = super::checkpoint::save(ev.transcript_path, cx);
    }
}

const TERSE: &str = include_str!("../../../modes/terse.md");
const YAGNI: &str = include_str!("../../../modes/yagni.md");

fn builtin(name: &str) -> Option<&'static str> {
    match name {
        "terse" => Some(TERSE),
        "yagni" => Some(YAGNI),
        _ => None,
    }
}

fn modes_text(cx: &Ctx) -> Option<String> {
    let names = if !cx.config.plugins.inject.modes.is_empty() {
        cx.config.plugins.inject.modes.as_slice()
    } else {
        cx.config.setup.modes.as_slice()
    };
    if names.is_empty() {
        return None;
    }
    let dir = &cx.config.plugins.inject.modes_dir;
    let mut text = String::new();
    for name in names {
        let path = dir.join(format!("{name}.md"));
        let Some(body) = std::fs::read_to_string(&path)
            .ok()
            .or_else(|| builtin(name).map(str::to_string))
        else {
            continue;
        };
        text.push_str(&body);
        if !body.ends_with('\n') {
            text.push('\n');
        }
    }
    Some(text)
}

/// Pick injections in priority order until the budget. A candidate that starts
/// under budget is emitted even if it overshoots (T2.4 Check: 500+500 at 800).
pub fn apply(cx: &Ctx, mut offered: Vec<Injection>) -> String {
    offered.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.plugin.cmp(b.plugin)));
    let budget = cx.config.plugins.inject.budget_tokens;
    let mut parts = Vec::new();
    let mut dropped = Vec::new();
    let mut used = 0u32;
    let mut before = 0u32;
    let mut before_bytes = 0u64;
    for i in &offered {
        let t = cx.estimate(&i.text, Class::Prose);
        before += t;
        before_bytes += i.text.len() as u64;
        if used >= budget {
            dropped.push(format!("dropped:{}:{t}", i.plugin));
            continue;
        }
        used += t;
        parts.push(i.text.as_str());
    }
    let text = parts.join("\n");
    let after = cx.estimate(&text, Class::Prose);
    let _ = cx.record(&Measurement {
        plugin: "inject",
        kind: "inject",
        before_bytes,
        after_bytes: text.len() as u64,
        est_before: before,
        est_after: after,
        ref_id: dropped.first().cloned(),
        call_id: None,
    });
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(tokens: u32, cx: &Ctx) -> String {
        let rate = cx.config.estimator.prose.max(0.1);
        let mut s = "x".repeat((tokens as f32 * rate) as usize);
        while cx.estimate(&s, Class::Prose) < tokens {
            s.push('x');
        }
        while cx.estimate(&s, Class::Prose) > tokens && !s.is_empty() {
            s.pop();
        }
        s
    }

    #[test]
    fn three_500_budget_800_drops_one_and_is_byte_stable() {
        let cx = Ctx::in_memory("inject-t24").unwrap();
        let text = blob(500, &cx);
        assert_eq!(cx.estimate(&text, Class::Prose), 500);
        let offered = vec![
            Injection {
                plugin: "a",
                text: text.clone(),
                priority: 3,
            },
            Injection {
                plugin: "b",
                text: text.clone(),
                priority: 2,
            },
            Injection {
                plugin: "c",
                text: text.clone(),
                priority: 1,
            },
        ];
        let once = apply(&cx, offered.clone());
        let twice = apply(&cx, offered);
        assert_eq!(once, twice);
        assert_eq!(once.matches('\n').count(), 1, "two emitted → one separator");
        assert_eq!(once, format!("{text}\n{text}"));
        assert_eq!(cx.store.measurement_count("inject").unwrap(), 2);
    }

    #[test]
    fn session_start_has_mode_once_prompt_submit_does_not() {
        let dir = std::env::temp_dir().join("rtok-t71-modes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.plugins.inject.modes = vec!["terse".into(), "yagni".into()];
        let start = serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": "t71",
            "source": "startup"
        });
        let mut out = Vec::new();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut out, &cfg);
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let text = v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(text.contains("# terse"), "{text}");
        assert!(text.contains("# yagni"), "{text}");
        assert_eq!(text.matches("# terse").count(), 1);
        let cx = Ctx::in_memory("t71").unwrap();
        assert!(cx.estimate(TERSE, Class::Prose) <= 250);
        assert!(cx.estimate(YAGNI, Class::Prose) <= 250);
        out.clear();
        let prompt = serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "t71",
            "prompt": "hi"
        });
        crate::hooks::run(
            "UserPromptSubmit",
            prompt.to_string().as_bytes(),
            &mut out,
            &cfg,
        );
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let ctx = v
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        assert!(!ctx.contains("# terse"), "{ctx}");
        assert!(!ctx.contains("# yagni"), "{ctx}");
    }
}
