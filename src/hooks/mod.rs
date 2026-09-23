//! Claude Code hook surface: `rtok hook <event>`.
//!
//! - [`types`] — stdin/stdout JSON contract (plan T0.6)
//! - dispatcher — plan T2.1

pub mod types;

use crate::config::Config;
use crate::plugin::{Ctx, PreToolDecision, Runtime, SessionStart};
use crate::plugins::Registry;
use crate::tokens::Class;
use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;
use types::{HookInput, HookOutput, HookSpecificOutput};

/// T178: the hook's budget is 10 ms, so it waits 5 ms on another process's lock — one connect,
/// migrations included — and then fails open. Every statement used to wait 1 s.
const LOCK_WAIT: crate::store::LockWait = crate::store::LockWait {
    busy: std::time::Duration::from_millis(5),
    attempts: 1,
    migrate: std::time::Duration::from_millis(5),
};

/// Fail-open hook entry: always writes JSON and does not return `Err`.
/// With `[hook] fail_open = false` (debugging only) errors surface as a panic
/// instead of `{}` — the default `true` keeps the fail-open rule (D1).
pub fn run(event: &str, mut stdin: impl Read, mut stdout: impl Write, cfg: &Config) {
    if cfg.hook.fail_open {
        let mut buf = Vec::new();
        let _ = stdin.read_to_end(&mut buf);
        let out = panic::catch_unwind(AssertUnwindSafe(|| dispatch_owned(&buf, event, cfg)))
            .unwrap_or_else(|_| b"{}".to_vec());
        let _ = stdout.write_all(&out);
    } else {
        let mut buf = Vec::new();
        stdin
            .read_to_end(&mut buf)
            .expect("rtok hook: stdin unreadable (fail_open = false)");
        let out = dispatch_owned_strict(&buf, event, cfg).expect("rtok hook");
        let _ = stdout.write_all(&out);
    }
}

/// Session id: stdin first, then `$<core.session_env>`, else `"unknown"`.
/// An empty `session_env` key disables the env fallback.
fn resolve_session(
    stdin_session: &str,
    session_env_key: &str,
    env: impl Fn(&str) -> Option<String>,
) -> String {
    if !stdin_session.is_empty() {
        return stdin_session.to_string();
    }
    if !session_env_key.is_empty()
        && let Some(v) = env(session_env_key)
    {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return v;
        }
    }
    "unknown".into()
}

/// `Some(message)` when `[hook] max_ms` is non-zero and the event ran over budget.
/// `max_ms = 0` disables the budget. Pure so the slow path stays one `eprintln!`.
fn slow_note(ms: f64, max_ms: u64, event: &str) -> Option<String> {
    if max_ms > 0 && ms > max_ms as f64 {
        Some(format!(
            "hook {event} slow: {ms:.1} ms over max_ms {max_ms} ms"
        ))
    } else {
        None
    }
}

fn dispatch_owned(stdin: &[u8], event: &str, cfg: &Config) -> Vec<u8> {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        dispatch_owned_strict(stdin, event, cfg)
    })) {
        Ok(Ok(out)) => out,
        _ => b"{}".to_vec(),
    }
}

fn dispatch_owned_strict(stdin: &[u8], event: &str, cfg: &Config) -> Result<Vec<u8>, String> {
    let mut input: HookInput =
        serde_json::from_slice(stdin).map_err(|e| format!("hook {event}: bad stdin: {e}"))?;
    // Grok Build sends its own envelope to every hook it runs, including the Claude and Cursor
    // hooks it imports, so its reserved `GROK_HOOK_EVENT` wins over `--host` (plan T98).
    let grok = cfg.hook.host == "grok" || std::env::var_os("GROK_HOOK_EVENT").is_some();
    let copilot = !grok && cfg.hook.host == "copilot";
    let cursor = !grok && cfg.hook.host == "cursor";
    let gemini = !grok && cfg.hook.host == "gemini";
    if grok {
        input.adapt_grok(event);
    } else if cursor {
        input.adapt_cursor(event);
    } else if copilot {
        input.adapt_copilot(event);
    } else if gemini {
        input.adapt_gemini(event);
    } else if cfg.hook.host == "devin" {
        input.adapt_devin(event, std::env::var("DEVIN_PROJECT_DIR").ok());
    } else if input.hook_event_name.is_empty() {
        input.hook_event_name = event.to_string();
    }
    let session = resolve_session(&input.session_id, &cfg.core.session_env, |k| {
        std::env::var(k).ok()
    });
    let mut cx = Runtime::open_with(cfg.clone(), session, LOCK_WAIT)
        .map_err(|e| format!("hook {event}: store open: {e}"))?;
    // SessionStart carries `cwd` like every other event, so the session row is attributed
    // from the first hook of the run rather than whichever call happens to arrive first.
    cx.cwd = input.cwd.clone();
    let out = dispatch(stdin, &input, &cx);
    if copilot {
        let parsed: HookOutput = serde_json::from_slice(&out).unwrap_or_default();
        return Ok(copilot_output(&parsed));
    }
    if cursor {
        let parsed: HookOutput = serde_json::from_slice(&out).unwrap_or_default();
        return Ok(cursor_output(&parsed));
    }
    if gemini {
        let parsed: HookOutput = serde_json::from_slice(&out).unwrap_or_default();
        return Ok(gemini_output(&parsed, &input.hook_event_name));
    }
    Ok(out)
}

/// Gemini CLI (https://geminicli.com/docs/hooks/reference/, fetched 2026-09-22) reads a
/// top-level `{decision: "deny", reason}` to block a call (nothing nests under
/// `permissionDecision`) and `{hookSpecificOutput: {tool_input}}` to rewrite one; after a tool
/// it reads `{hookSpecificOutput: {additionalContext}}`, the key Claude uses too. `{}` stays.
pub fn gemini_output(out: &HookOutput, event: &str) -> Vec<u8> {
    let empty = || b"{}".to_vec();
    let Some(h) = &out.hook_specific_output else {
        return empty();
    };
    if event == "PreToolUse" {
        if h.permission_decision.as_deref() == Some("deny") {
            let mut o = serde_json::Map::new();
            o.insert("decision".into(), "deny".into());
            if let Some(r) = &h.permission_decision_reason {
                o.insert("reason".into(), r.as_str().into());
            }
            return serde_json::to_vec(&serde_json::Value::Object(o)).unwrap_or_else(|_| empty());
        }
        if let Some(input) = &h.updated_input {
            let v = serde_json::json!({"hookSpecificOutput": {"tool_input": input}});
            return serde_json::to_vec(&v).unwrap_or_else(|_| empty());
        }
        return empty();
    }
    if let Some(ctx) = &h.additional_context {
        let v = serde_json::json!({"hookSpecificOutput": {"additionalContext": ctx}});
        return serde_json::to_vec(&v).unwrap_or_else(|_| empty());
    }
    empty()
}

/// GitHub Copilot CLI reads a flat object: `{permissionDecision, permissionDecisionReason,
/// modifiedArgs}` on preToolUse, `{additionalContext}` after a tool; nothing nests under
/// `hookSpecificOutput`. A Claude `decision: block` becomes `deny`. `{}` stays `{}`.
pub fn copilot_output(out: &HookOutput) -> Vec<u8> {
    let mut o = serde_json::Map::new();
    let mut put = |k: &str, v: serde_json::Value| {
        o.insert(k.to_string(), v);
    };
    if let Some(h) = &out.hook_specific_output {
        if let Some(d) = &h.permission_decision {
            put("permissionDecision", d.as_str().into());
        }
        if let Some(r) = &h.permission_decision_reason {
            put("permissionDecisionReason", r.as_str().into());
        }
        if let Some(u) = &h.updated_input {
            put("modifiedArgs", u.clone());
        }
        if let Some(c) = &h.additional_context {
            put("additionalContext", c.as_str().into());
        }
    }
    if out.decision.as_deref() == Some("block") && !o.contains_key("permissionDecision") {
        o.insert("permissionDecision".into(), "deny".into());
        if let Some(r) = &out.reason {
            o.insert("permissionDecisionReason".into(), r.as_str().into());
        }
    }
    serde_json::to_vec(&serde_json::Value::Object(o)).unwrap_or_else(|_| b"{}".to_vec())
}

pub fn dispatch(stdin: &[u8], input: &HookInput, cx: &Runtime) -> Vec<u8> {
    let start = Instant::now();
    let registry = Registry::new(&cx.config);
    let parent = match cx.record_call("hook", "hook", Some(&input.hook_event_name)) {
        Ok(id) => Some(id),
        // T178: another process held the writer lock past `LOCK_WAIT`. Every later write would
        // wait again, so pass the input through unchanged and record nothing.
        Err(e) if crate::store::is_locked(&e) => {
            eprintln!("rtok: hook {} skipped: store locked", input.hook_event_name);
            return b"{}".to_vec();
        }
        Err(_) => None,
    };
    let out = match input.hook_event_name.as_str() {
        "PreToolUse" => pre_tool(input, cx, &registry),
        "PostToolUse" => post_tool(input, cx, &registry),
        "AfterMCPExecution" => after_mcp(input, cx),
        "SessionStart" | "UserPromptSubmit" | "PostCompact" | "SubagentStart" => {
            inject_event(input, cx, &registry)
        }
        "PreCompact" => {
            if let Some(ev) = input.pre_compact() {
                for p in registry.enabled() {
                    let _ =
                        panic::catch_unwind(AssertUnwindSafe(|| p.pre_compact(&ev, &Ctx::new(cx))));
                }
            }
            HookOutput::default()
        }
        "SessionEnd" => {
            #[cfg(feature = "inject")]
            {
                let path = input.transcript_path.as_deref().unwrap_or("");
                let _ = panic::catch_unwind(AssertUnwindSafe(|| {
                    let _ = crate::plugins::checkpoint::save_session(path, &Ctx::new(cx));
                }));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = cx.store.end_session(&cx.session, now);
            HookOutput::default()
        }
        _ => HookOutput::default(),
    };
    let bytes = serde_json::to_vec(&out).unwrap_or_else(|_| b"{}".to_vec());
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    if let Some(note) = slow_note(ms, cx.config.hook.max_ms, &input.hook_event_name) {
        eprintln!("rtok: {note}");
    }
    if let Some(id) = parent {
        let _ = cx.store.set_call_ms(id, ms);
        let cap = cx.config.core.call_io_inline_bytes as usize;
        let _ = cx
            .store
            .insert_call_io(id, Some(stdin), Some(&bytes), cap, None);
    }
    if matches!(input.hook_event_name.as_str(), "Stop" | "SessionEnd") {
        crate::otel::export::spawn_child(cx);
    }
    bytes
}

#[cfg(not(feature = "cmd"))]
fn after_mcp(_input: &HookInput, _cx: &Runtime) -> HookOutput {
    HookOutput::default()
}

#[cfg(feature = "cmd")]
fn after_mcp(input: &HookInput, cx: &Runtime) -> HookOutput {
    if input.hook_event_name != "AfterMCPExecution" {
        return HookOutput::default();
    }
    let server = input.mcp_server_name().unwrap_or("");
    if server.eq_ignore_ascii_case("rtok") {
        return HookOutput::default();
    }
    let tool = input.tool_name.as_deref().unwrap_or("mcp");
    let raw = input
        .tool_response
        .as_ref()
        .and_then(|v| v.as_str())
        .or_else(|| input.extra.get("result_json").and_then(|v| v.as_str()))
        .unwrap_or("");
    if raw.is_empty() {
        return HookOutput::default();
    }
    let modified = shorten_mcp_result(cx, server, tool, raw);
    modified
        .map(|m| HookOutput {
            updated_mcp_tool_output: Some(serde_json::json!({"modified": m})),
            ..HookOutput::default()
        })
        .unwrap_or_default()
}

#[cfg(feature = "cmd")]
fn shorten_mcp_result(
    cx: &Runtime,
    _server: &str,
    _tool: &str,
    result_json: &str,
) -> Option<String> {
    use crate::plugins::cmd::rules::{self, Settings};
    use serde_json::Value;
    let mut v: Value = serde_json::from_str(result_json).ok()?;
    let text = mcp_result_text(&v)?;
    let max = cx.config.mcp.max_result_chars as usize;
    if text.chars().count() <= max {
        return None;
    }
    let id = rtok_plugin_sdk::Archive::put_archive(cx, text.as_bytes()).ok()?;
    let settings = Settings::from_config(&cx.config);
    let rule = settings.pick("mcp");
    let cut = rules::apply(&settings, &text, 0, &rule, &id);
    let printed = format!("{cut}\n[rtok {id} · expand: rtok expand {id}]");
    set_mcp_result_text(&mut v, printed);
    serde_json::to_string(&v).ok()
}

#[cfg(feature = "cmd")]
fn mcp_result_text(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for block in arr {
            if let Some(t) = block.get("text").and_then(|x| x.as_str()) {
                parts.push(t);
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
    }
    None
}

#[cfg(feature = "cmd")]
fn set_mcp_result_text(v: &mut serde_json::Value, text: String) {
    if v.is_string() {
        *v = serde_json::Value::String(text);
        return;
    }
    if let Some(arr) = v.get_mut("content").and_then(|c| c.as_array_mut())
        && let Some(first) = arr.first_mut()
        && first.get("text").is_some()
    {
        first["text"] = serde_json::Value::String(text);
    }
}

fn pre_tool(input: &HookInput, cx: &Runtime, registry: &Registry) -> HookOutput {
    let Some(ev) = input.pre_tool() else {
        return HookOutput::default();
    };
    let mut rewrite: Option<PreToolDecision> = None;
    for p in registry.enabled() {
        let got = panic::catch_unwind(AssertUnwindSafe(|| {
            p.pre_tool(&ev, &Ctx::with_agent(cx, input.agent_id.as_deref()))
        }))
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

fn post_tool(input: &HookInput, cx: &Runtime, registry: &Registry) -> HookOutput {
    let Some(ev) = input.post_tool() else {
        return HookOutput::default();
    };
    let mut parts = Vec::new();
    for p in registry.enabled() {
        if let Ok(Some(s)) = panic::catch_unwind(AssertUnwindSafe(|| {
            p.post_tool(&ev, &Ctx::with_agent(cx, input.agent_id.as_deref()))
        })) {
            parts.push(s);
        }
    }
    let text = cap_budget(cx, &parts.join("\n"));
    let updated = cursor_mcp_output(input, cx);
    if text.is_empty() && updated.is_none() {
        return HookOutput::default();
    }
    HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            hook_event_name: "PostToolUse".into(),
            additional_context: (!text.is_empty()).then_some(text),
            updated_mcp_tool_output: updated,
            ..HookSpecificOutput::default()
        }),
        ..HookOutput::default()
    }
}

/// Cursor reads a flat object: `{updated_mcp_tool_output}` after an MCP tool,
/// `{additional_context}` on session/prompt hooks. Anything else (a guard deny, a
/// shell hook with no replacement) keeps the Claude `hookSpecificOutput` shape.
pub fn cursor_output(out: &HookOutput) -> Vec<u8> {
    let nested = || serde_json::to_vec(out).unwrap_or_else(|_| b"{}".to_vec());
    let Some(h) = &out.hook_specific_output else {
        return nested();
    };
    let mut o = serde_json::Map::new();
    if let Some(updated) = &h.updated_mcp_tool_output {
        o.insert("updated_mcp_tool_output".into(), updated.clone());
    }
    if let Some(c) = &h.additional_context {
        o.insert("additional_context".into(), c.as_str().into());
    }
    if o.is_empty() {
        return nested();
    }
    serde_json::to_vec(&serde_json::Value::Object(o)).unwrap_or_else(|_| b"{}".to_vec())
}

fn is_rtok_mcp(input: &HookInput) -> bool {
    if input
        .extra
        .get("mcp_server_name")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("rtok"))
    {
        return true;
    }
    let raw = input.tool_name.as_deref().unwrap_or("");
    raw.strip_prefix("MCP:").unwrap_or(raw) == "expand"
}

fn mcp_result(response: &serde_json::Value) -> Option<serde_json::Value> {
    if response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .is_some()
    {
        return Some(response.clone());
    }
    if response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(serde_json::Value::as_array)
        .is_some()
    {
        return Some(response["result"].clone());
    }
    None
}

fn cursor_mcp_output(input: &HookInput, cx: &Runtime) -> Option<serde_json::Value> {
    if cx.config.hook.host != "cursor" || is_rtok_mcp(input) {
        return None;
    }
    // `shorten_result` rewrites it in place; without the `cmd` plugin nothing does.
    #[cfg_attr(not(feature = "cmd"), allow(unused_mut))]
    let mut result = mcp_result(input.tool_response.as_ref()?)?;
    #[cfg(feature = "cmd")]
    {
        let settings = crate::plugins::cmd::rules::Settings::from_config(&cx.config);
        let server = input
            .extra
            .get("mcp_server_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mcp");
        let raw = input.tool_name.as_deref().unwrap_or("tool");
        let tool = raw.strip_prefix("MCP:").unwrap_or(raw);
        if crate::mcp::wrap::shorten_result(
            cx,
            &settings,
            server,
            tool,
            &mut result,
            "archive",
            "mcp",
        ) {
            return Some(result);
        }
    }
    #[cfg(not(feature = "cmd"))]
    let _ = result;
    None
}

fn inject_event(input: &HookInput, cx: &Runtime, registry: &Registry) -> HookOutput {
    let mut inj = Vec::new();
    for p in registry.enabled() {
        let one = panic::catch_unwind(AssertUnwindSafe(|| {
            if let Some(ev) = input.session_start() {
                p.session_start(&ev, &Ctx::new(cx))
            } else if let Some(ev) = input.prompt_submit() {
                p.prompt_submit(&ev, &Ctx::new(cx))
            } else if input.hook_event_name == "PostCompact" {
                p.session_start(&SessionStart { source: "compact" }, &Ctx::new(cx))
            } else if let Some(ev) = input.subagent_start() {
                // The parent's own ledger, not the new subagent's (T130): it has none yet.
                p.subagent_start(&ev, &Ctx::new(cx))
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
    // No offerings → no Measurement noise (D3): UserPromptSubmit usually has none.
    if inj.is_empty() {
        return HookOutput::default();
    }
    #[cfg(feature = "inject")]
    let text = crate::plugins::inject::apply(&Ctx::new(cx), inj);
    #[cfg(not(feature = "inject"))]
    let text = {
        let _ = inj;
        String::new()
    };
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

fn cap_budget(cx: &Runtime, text: &str) -> String {
    let budget = cx.config.plugins.inject.budget_tokens;
    if cx.estimate(text, Class::Prose) <= budget {
        return text.to_string();
    }
    let mut out = String::new();
    let mut rest = text.lines();
    while let Some(line) = rest.next() {
        let cand = if out.is_empty() {
            line.to_string()
        } else {
            format!("{out}\n{line}")
        };
        if cx.estimate(&cand, Class::Prose) > budget {
            let dropped = std::iter::once(line)
                .chain(rest)
                .collect::<Vec<_>>()
                .join("\n");
            let marker = format!("dropped:post_tool:{}", cx.estimate(&dropped, Class::Prose));
            if out.is_empty() {
                // Estimates round up per part, so a prefix that fits the room left after
                // `\n{marker}` keeps the whole line under budget.
                let room = budget.saturating_sub(cx.estimate(&format!("\n{marker}"), Class::Prose));
                let prefix = crate::plugin::fit_budget(&Ctx::new(cx), line, Class::Prose, room);
                if !prefix.is_empty() {
                    return format!("{prefix}\n{marker}");
                }
                return if cx.estimate(&marker, Class::Prose) <= budget {
                    marker
                } else {
                    String::new()
                };
            }
            let with = format!("{out}\n{marker}");
            return if cx.estimate(&with, Class::Prose) <= budget {
                with
            } else {
                out
            };
        }
        out = cand;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{post_out, pre_out};

    /// Shared by every per-host `_output` test below: hook stdout bytes back to `Value`.
    fn json(bytes: Vec<u8>) -> serde_json::Value {
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn copilot_output_shapes_pre_post_block_and_empty() {
        assert_eq!(
            json(copilot_output(&HookOutput::default())),
            serde_json::json!({})
        );

        let pre = pre_out(
            Some("allow"),
            Some("rtok"),
            Some(serde_json::json!({"command": "rtok cmd -- git status"})),
        );
        assert_eq!(
            json(copilot_output(&pre)),
            serde_json::json!({
                "permissionDecision": "allow",
                "permissionDecisionReason": "rtok",
                "modifiedArgs": {"command": "rtok cmd -- git status"}
            })
        );

        let post = post_out("ctx");
        assert_eq!(
            json(copilot_output(&post)),
            serde_json::json!({"additionalContext": "ctx"})
        );

        let block = HookOutput {
            decision: Some("block".into()),
            reason: Some("guard".into()),
            ..Default::default()
        };
        assert_eq!(
            json(copilot_output(&block)),
            serde_json::json!({"permissionDecision": "deny", "permissionDecisionReason": "guard"})
        );
    }
    #[test]
    fn gemini_output_shapes_deny_rewrite_context_and_empty() {
        assert_eq!(
            json(gemini_output(&HookOutput::default(), "PreToolUse")),
            serde_json::json!({})
        );

        let deny = pre_out(Some("deny"), Some("dup"), None);
        assert_eq!(
            json(gemini_output(&deny, "PreToolUse")),
            serde_json::json!({"decision": "deny", "reason": "dup"})
        );

        let rewrite = pre_out(
            None,
            None,
            Some(serde_json::json!({"command": "rtok cmd -- git status"})),
        );
        assert_eq!(
            json(gemini_output(&rewrite, "PreToolUse")),
            serde_json::json!({"hookSpecificOutput": {"tool_input": {"command": "rtok cmd -- git status"}}})
        );

        let post = post_out("ctx");
        assert_eq!(
            json(gemini_output(&post, "PostToolUse")),
            serde_json::json!({"hookSpecificOutput": {"additionalContext": "ctx"}})
        );
    }

    use crate::plugin::Runtime;

    #[test]
    fn fixture_pre_tool_is_valid_json() {
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let cx = Runtime::in_memory("b1e2c3d4-0000-4000-8000-000000000001").unwrap();
        let input: HookInput = serde_json::from_str(raw).unwrap();
        let out = dispatch(raw.as_bytes(), &input, &cx);
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v.is_object());
        assert!(cx.store.count_kind("hook").unwrap() >= 1);
    }

    #[test]
    fn oversized_hook_call_io_does_not_archive() {
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let mut v: serde_json::Value = serde_json::from_str(raw).unwrap();
        v["pad"] = serde_json::Value::String("x".repeat(70_000));
        let stdin = serde_json::to_vec(&v).unwrap();
        assert!(stdin.len() > 65_536);
        let input: HookInput = serde_json::from_slice(&stdin).unwrap();
        let cx = Runtime::in_memory("b1e2c3d4-0000-4000-8000-000000000002").unwrap();
        let _ = dispatch(&stdin, &input, &cx);
        let ids = cx.store.call_ids_of_kind("hook").unwrap();
        assert!(!ids.is_empty());
        let (req, res) = cx.store.call_io_archives(ids[0]).unwrap();
        assert!(req.is_none(), "{req:?}");
        assert!(res.is_none(), "{res:?}");
    }

    #[test]
    fn malformed_stdin_is_empty_object() {
        let cfg = Config::default();
        let mut out = Vec::new();
        run("PreToolUse", b"not-json".as_slice(), &mut out, &cfg);
        assert_eq!(out, b"{}");
    }

    /// T45.4: `core.session_env` resolves the session when stdin has none.
    #[test]
    fn resolve_session_prefers_stdin_then_env() {
        let env = |_: &str| Some("env-sess".to_string());
        assert_eq!(resolve_session("stdin-sess", "ANY_KEY", env), "stdin-sess");
        assert_eq!(resolve_session("", "ANY_KEY", env), "env-sess");
        assert_eq!(
            resolve_session("", "ANY_KEY", |_| Some("  padded  ".to_string())),
            "padded"
        );
        assert_eq!(resolve_session("", "", env), "unknown");
        assert_eq!(resolve_session("", "ANY_KEY", |_| None), "unknown");
        assert_eq!(
            resolve_session("", "ANY_KEY", |_| Some("   ".to_string())),
            "unknown"
        );
    }

    /// T45.4: `[hook] max_ms` fires only when non-zero and exceeded.
    #[test]
    fn slow_note_fires_only_over_budget() {
        assert!(slow_note(12.0, 10, "PreToolUse").is_some());
        assert_eq!(slow_note(9.9, 10, "PreToolUse"), None);
        assert_eq!(slow_note(500.0, 0, "PreToolUse"), None);
    }

    /// T45.4: `fail_open = false` surfaces a bad payload instead of `{}`.
    #[test]
    #[should_panic(expected = "bad stdin")]
    fn strict_path_panics_on_malformed_stdin() {
        let cfg = Config {
            hook: crate::config::Hook {
                fail_open: false,
                ..Default::default()
            },
            ..Config::default()
        };
        let mut out = Vec::new();
        run("PreToolUse", b"not-json".as_slice(), &mut out, &cfg);
    }

    /// T45.4: the strict path runs a valid event (temp store, default env key unset).
    #[test]
    fn strict_path_runs_valid_input() {
        let dir = std::env::temp_dir().join(format!("rtok-hooks-t454-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.core.archive_dir = dir.join("archive");
        cfg.hook.fail_open = false;
        // Hermetic regardless of ambient env: the fixture carries its own
        // session id, and the fallback key is a probe nothing else sets.
        cfg.core.session_env = "RTOK_T454_PROBE_SESSION".into();
        // The fixture carries its own session id, so the `session_env`
        // fallback is not exercised here; the probe key only proves the
        // lookup misses hermetically. No env mutation needed.
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let mut out = Vec::new();
        run("PreToolUse", raw.as_bytes(), &mut out, &cfg);
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v.is_object());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T25.0 Check: a hook run leaves a `sessions` row with non-NULL `host_id` and
    /// `project` — resolved from `[hook] host` and the event's own `cwd`, not left `None`.
    #[test]
    fn hook_run_attributes_the_session() {
        let dir = std::env::temp_dir().join(format!("rtok-hooks-t25-{}", std::process::id()));
        let repo = dir.join("myproj");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let mut v: serde_json::Value = serde_json::from_str(raw).unwrap();
        v["cwd"] = serde_json::Value::String(repo.display().to_string());
        v["session_id"] = serde_json::Value::String("b1e2c3d4-0000-4000-8000-0000000000aa".into());
        let stdin = serde_json::to_vec(&v).unwrap();
        let input: HookInput = serde_json::from_slice(&stdin).unwrap();
        let mut cx = Runtime::in_memory(input.session_id.clone()).unwrap();
        cx.cwd = input.cwd.clone();
        let _ = dispatch(&stdin, &input, &cx);
        let (slug, project, cwd) = cx.store.session_row(&cx.session).unwrap().unwrap();
        assert_eq!(
            slug.as_deref(),
            Some("claude"),
            "default [hook] host resolves"
        );
        assert_eq!(project.as_deref(), Some("myproj"), "git root basename");
        assert_eq!(cwd.as_deref(), Some(repo.display().to_string()).as_deref());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T25.0 Check: `pi` records as `pi`, not `other` — 0010.sql seeds the slug
    /// `rtok agents install pi` installs but 0002.sql's original list never had.
    #[test]
    fn pi_host_resolves_to_pi_not_other() {
        let dir = std::env::temp_dir().join(format!("rtok-hooks-t25-pi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = Config::default();
        cfg.hook.host = "pi".into();
        cfg.core.db_path = dir.join("rtok.db");
        let raw = include_str!("../../tests/fixtures/hooks/pre_tool_bash.json");
        let input: HookInput = serde_json::from_str(raw).unwrap();
        let cx = Runtime::open(cfg, input.session_id.clone()).unwrap();
        let _ = dispatch(raw.as_bytes(), &input, &cx);
        let (slug, ..) = cx.store.session_row(&cx.session).unwrap().unwrap();
        assert_eq!(slug.as_deref(), Some("pi"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_budget_marks_drop_when_first_line_exceeds() {
        let mut cx = Runtime::in_memory("cap-first").unwrap();
        cx.config.plugins.inject.budget_tokens = 20;
        let huge = "word ".repeat(400);
        let out = cap_budget(&cx, huge.trim_end());
        assert!(!out.is_empty(), "must not swallow the whole payload");
        assert!(
            out.lines().any(|l| l.starts_with("dropped:post_tool:")),
            "{out}"
        );
        assert!(cx.estimate(&out, Class::Prose) <= 20, "{out}");
    }

    #[test]
    fn cap_budget_keeps_fitting_lines_and_names_the_rest() {
        let mut cx = Runtime::in_memory("cap-rest").unwrap();
        cx.config.plugins.inject.budget_tokens = 30;
        let small = "ok";
        let huge = "word ".repeat(400);
        let text = format!("{small}\n{}", huge.trim_end());
        let out = cap_budget(&cx, &text);
        assert!(out.starts_with("ok\n"), "{out}");
        assert!(
            out.lines().any(|l| l.starts_with("dropped:post_tool:")),
            "{out}"
        );
        assert!(cx.estimate(&out, Class::Prose) <= 30, "{out}");
    }

    #[test]
    fn cursor_session_start_injects_flat_and_stable() {
        let dir = std::env::temp_dir().join(format!("rtok-cursor-ss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.hook.host = "cursor".into();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.core.archive_dir = dir.join("archive");
        cfg.plugins.inject.modes = vec!["nudges".into()];
        let raw = serde_json::json!({
            "hook_event_name": "sessionStart",
            "conversation_id": "sess-cur",
            "cwd": dir.display().to_string(),
            "source": "startup"
        });
        let stdin = serde_json::to_vec(&raw).unwrap();
        let mut out1 = Vec::new();
        run("SessionStart", stdin.as_slice(), &mut out1, &cfg);
        let mut out2 = Vec::new();
        run("SessionStart", stdin.as_slice(), &mut out2, &cfg);
        assert_eq!(out1, out2, "byte-stable");
        let v: serde_json::Value = serde_json::from_slice(&out1).unwrap();
        assert!(v.get("hookSpecificOutput").is_none(), "{v}");
        assert!(v.get("additional_context").is_some(), "{v}");
        let mut claude = cfg.clone();
        claude.hook.host = "claude".into();
        claude.plugins.inject.modes = vec!["nudges".into()];
        let mut claude_out = Vec::new();
        run(
            "SessionStart",
            include_str!("../../tests/fixtures/hooks/session_start.json").as_bytes(),
            &mut claude_out,
            &claude,
        );
        let cv: serde_json::Value = serde_json::from_slice(&claude_out).unwrap();
        let cursor_ctx = v["additional_context"].as_str().unwrap_or("");
        let claude_ctx = cv["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");
        assert_eq!(cursor_ctx, claude_ctx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn cursor_cfg(dir: &std::path::Path) -> Config {
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c
    }

    /// T98: a Grok PreToolUse on `run_terminal_command` is rewritten and answered in Claude's
    /// shape, which Grok reads unchanged.
    #[cfg(feature = "cmd")]
    #[test]
    fn grok_pre_tool_use_rewrites_the_terminal_command() {
        let dir = std::env::temp_dir().join(format!("rtok-hook-grok-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut cfg = cursor_cfg(&dir);
        cfg.hook.host = "grok".into();
        let stdin = serde_json::to_vec(&serde_json::json!({
            "hookEventName": "pre_tool_use",
            "hook_event_name": "PreToolUse",
            "sessionId": "grok-1",
            "cwd": dir.to_string_lossy(),
            "toolName": "run_terminal_command",
            "toolInput": {"command": "git status"}
        }))
        .unwrap();
        let out = dispatch_owned_strict(&stdin, "PreToolUse", &cfg).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let hso = &v["hookSpecificOutput"];
        assert_eq!(hso["hookEventName"], "PreToolUse", "{v}");
        let cmd = hso["updatedInput"]["command"].as_str().unwrap_or("");
        assert!(cmd.contains("git status") && cmd != "git status", "{v}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn mcp_stdin(server: &str, tool: &str, text: &str) -> Vec<u8> {
        let result = serde_json::json!({"content":[{"type":"text","text": text}]});
        serde_json::to_vec(&serde_json::json!({
            "hook_event_name": "postToolUse",
            "tool_name": tool,
            "tool_input": {},
            "tool_output": result.to_string(),
            "conversation_id": "s-mcp",
            "mcp_server_name": server
        }))
        .unwrap()
    }

    #[test]
    fn cursor_output_emits_snake_case_mcp_replacement() {
        assert_eq!(
            json(cursor_output(&HookOutput::default())),
            serde_json::json!({})
        );
        let out = HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                hook_event_name: "PostToolUse".into(),
                updated_mcp_tool_output: Some(serde_json::json!({"content":[]})),
                additional_context: Some("note".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let v = json(cursor_output(&out));
        assert_eq!(
            v["updated_mcp_tool_output"]["content"],
            serde_json::json!([])
        );
        assert_eq!(v["additional_context"], "note");
        assert!(v.get("hookSpecificOutput").is_none());
    }

    #[test]
    fn cursor_mcp_post_tool_use_shortens_only_foreign_long_results() {
        let dir = std::env::temp_dir().join(format!(
            "rtok-hook-mcp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let cfg = cursor_cfg(&dir);
        let long: String = (1..=200).map(|i| format!("line {i}\n")).collect();
        let out = dispatch_owned_strict(
            &mcp_stdin("linear", "MCP:list_issues", &long),
            "PostToolUse",
            &cfg,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let printed = v["updated_mcp_tool_output"]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert!(printed.contains("expand: rtok expand "), "{v}");
        assert!(printed.len() < long.len(), "{printed}");
        let store = crate::store::Store::open(&cfg.core.db_path).unwrap();
        let rows = store.list_measurements("archive").unwrap();
        assert_eq!(
            rows.iter().filter(|r| r.kind == "mcp").count(),
            1,
            "{rows:?}"
        );

        let small = dispatch_owned_strict(
            &mcp_stdin("linear", "MCP:list_issues", "ok\n"),
            "PostToolUse",
            &cfg,
        )
        .unwrap();
        assert_eq!(small, b"{}");

        let own =
            dispatch_owned_strict(&mcp_stdin("rtok", "MCP:search", &long), "PostToolUse", &cfg)
                .unwrap();
        assert_eq!(own, b"{}", "rtok MCP results must not be rewritten");
        assert_eq!(
            store
                .list_measurements("archive")
                .unwrap()
                .iter()
                .filter(|r| r.kind == "mcp")
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
