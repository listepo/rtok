//! Claude Code hook surface: `rtok hook <event>`.
//!
//! - [`types`] — stdin/stdout JSON contract (plan T0.6)
//! - dispatcher — plan T2.1

pub mod types;

use crate::config::Config;
use crate::plugin::{Ctx, PreToolDecision};
use crate::plugins::Registry;
use crate::tokens::Class;
use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;
use types::{HookInput, HookOutput, HookSpecificOutput};

/// Fail-open hook entry: always writes JSON and does not return `Err`.
pub fn run(event: &str, mut stdin: impl Read, mut stdout: impl Write, cfg: &Config) {
    let mut buf = Vec::new();
    let _ = stdin.read_to_end(&mut buf);
    let out = panic::catch_unwind(AssertUnwindSafe(|| dispatch_owned(&buf, event, cfg)))
        .unwrap_or_else(|_| b"{}".to_vec());
    let _ = stdout.write_all(&out);
}

fn dispatch_owned(stdin: &[u8], event: &str, cfg: &Config) -> Vec<u8> {
    let mut input: HookInput = match serde_json::from_slice(stdin) {
        Ok(v) => v,
        Err(_) => return b"{}".to_vec(),
    };
    if input.hook_event_name.is_empty() {
        input.hook_event_name = event.to_string();
    }
    let session = if input.session_id.is_empty() {
        "unknown".into()
    } else {
        input.session_id.clone()
    };
    let cx = match Ctx::open(cfg.clone(), session) {
        Ok(cx) => cx,
        Err(_) => return b"{}".to_vec(),
    };
    dispatch(stdin, &input, &cx)
}

pub fn dispatch(stdin: &[u8], input: &HookInput, cx: &Ctx) -> Vec<u8> {
    let start = Instant::now();
    let registry = Registry::new(&cx.config);
    let parent = cx
        .record_call("hook", "hook", Some(&input.hook_event_name))
        .ok();
    let out = match input.hook_event_name.as_str() {
        "PreToolUse" => pre_tool(input, cx, &registry),
        "PostToolUse" => post_tool(input, cx, &registry),
        "SessionStart" | "UserPromptSubmit" => inject_event(input, cx, &registry),
        "PreCompact" => {
            if let Some(ev) = input.pre_compact() {
                for p in registry.enabled() {
                    let _ = panic::catch_unwind(AssertUnwindSafe(|| p.pre_compact(&ev, cx)));
                }
            }
            HookOutput::default()
        }
        _ => HookOutput::default(),
    };
    let bytes = serde_json::to_vec(&out).unwrap_or_else(|_| b"{}".to_vec());
    if let Some(id) = parent {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let _ = cx.store.set_call_ms(id, ms);
        let cap = cx.config.core.call_io_inline_bytes as usize;
        let _ = cx
            .store
            .insert_call_io(id, Some(stdin), Some(&bytes), cap, None);
    }
    bytes
}

fn pre_tool(input: &HookInput, cx: &Ctx, registry: &Registry) -> HookOutput {
    let Some(ev) = input.pre_tool() else {
        return HookOutput::default();
    };
    let mut rewrite: Option<PreToolDecision> = None;
    for p in registry.enabled() {
        plugin_run(cx, p.manifest().id, stdin_est(cx, ev.tool_name));
        let got = panic::catch_unwind(AssertUnwindSafe(|| p.pre_tool(&ev, cx)))
            .ok()
            .flatten();
        match got {
            Some(PreToolDecision::Deny { reason }) => {
                return HookOutput {
                    hook_specific_output: Some(HookSpecificOutput {
                        hook_event_name: "PreToolUse".into(),
                        permission_decision: Some("deny".into()),
                        permission_decision_reason: Some(reason),
                        ..HookSpecificOutput::default()
                    }),
                    ..HookOutput::default()
                };
            }
            Some(r @ PreToolDecision::Rewrite { .. }) => rewrite = Some(r),
            None => {}
        }
    }
    if let Some(PreToolDecision::Rewrite { input, reason }) = rewrite {
        return HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                updated_input: Some(input),
                permission_decision_reason: Some(reason),
                ..HookSpecificOutput::default()
            }),
            ..HookOutput::default()
        };
    }
    HookOutput::default()
}

fn post_tool(input: &HookInput, cx: &Ctx, registry: &Registry) -> HookOutput {
    let Some(ev) = input.post_tool() else {
        return HookOutput::default();
    };
    let mut parts = Vec::new();
    for p in registry.enabled() {
        plugin_run(cx, p.manifest().id, stdin_est(cx, ev.tool_name));
        if let Ok(Some(s)) = panic::catch_unwind(AssertUnwindSafe(|| p.post_tool(&ev, cx))) {
            parts.push(s);
        }
    }
    let text = cap_budget(cx, &parts.join("\n"));
    if text.is_empty() {
        return HookOutput::default();
    }
    HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            hook_event_name: "PostToolUse".into(),
            additional_context: Some(text),
            ..HookSpecificOutput::default()
        }),
        ..HookOutput::default()
    }
}

fn inject_event(input: &HookInput, cx: &Ctx, registry: &Registry) -> HookOutput {
    let mut inj = Vec::new();
    for p in registry.enabled() {
        plugin_run(cx, p.manifest().id, 0);
        let one = panic::catch_unwind(AssertUnwindSafe(|| {
            if let Some(ev) = input.session_start() {
                p.session_start(&ev, cx)
            } else if let Some(ev) = input.prompt_submit() {
                p.prompt_submit(&ev, cx)
            } else {
                None
            }
        }))
        .ok()
        .flatten();
        if let Some(i) = one {
            inj.push(i);
        }
    }
    inj.sort_by_key(|b| std::cmp::Reverse(b.priority));
    let mut parts = Vec::new();
    let mut used = 0u32;
    let budget = cx.config.plugins.inject.budget_tokens;
    for i in inj {
        let t = cx.estimate(&i.text, Class::Prose);
        if used + t > budget {
            continue;
        }
        used += t;
        parts.push(i.text);
    }
    let text = parts.join("\n");
    if text.is_empty() {
        return HookOutput::default();
    }
    HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            hook_event_name: input.hook_event_name.clone(),
            additional_context: Some(text),
            ..HookSpecificOutput::default()
        }),
        ..HookOutput::default()
    }
}

fn plugin_run(cx: &Ctx, id: &str, tokens: i64) {
    if let Ok(call_id) = cx.store.insert_call(
        &cx.session,
        "hook",
        "plugin_run",
        None,
        None,
        None,
        Some(id),
        Some(id),
    ) {
        let _ = cx.record_tokens(call_id, Some(id), "before", "estimator", tokens);
        let _ = cx.record_tokens(call_id, Some(id), "after", "estimator", tokens);
    }
}

fn stdin_est(cx: &Ctx, name: &str) -> i64 {
    i64::from(cx.estimate(name, Class::Code))
}

fn cap_budget(cx: &Ctx, text: &str) -> String {
    let budget = cx.config.plugins.inject.budget_tokens;
    if cx.estimate(text, Class::Prose) <= budget {
        return text.to_string();
    }
    let mut out = String::new();
    for line in text.lines() {
        let cand = if out.is_empty() {
            line.to_string()
        } else {
            format!("{out}\n{line}")
        };
        if cx.estimate(&cand, Class::Prose) > budget {
            break;
        }
        out = cand;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Ctx;

    #[test]
    fn fixture_pre_tool_is_valid_json() {
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let cx = Ctx::in_memory("b1e2c3d4-0000-4000-8000-000000000001").unwrap();
        let input: HookInput = serde_json::from_str(raw).unwrap();
        let out = dispatch(raw.as_bytes(), &input, &cx);
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v.is_object());
        assert!(cx.store.count_kind("hook").unwrap() >= 1);
    }

    #[test]
    fn malformed_stdin_is_empty_object() {
        let cfg = Config::default();
        let mut out = Vec::new();
        run("PreToolUse", b"not-json".as_slice(), &mut out, &cfg);
        assert_eq!(out, b"{}");
    }
}
