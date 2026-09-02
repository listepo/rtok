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
        if ev.source == "compact" {
            super::checkpoint::offer(cx)
        } else {
            None
        }
    }

    fn pre_compact(&self, ev: &PreCompact, cx: &Ctx) {
        let _ = super::checkpoint::save(ev.transcript_path, cx);
    }
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
}
