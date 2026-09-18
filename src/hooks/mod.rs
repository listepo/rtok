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
    let copilot = cfg.hook.host == "copilot";
    if cfg.hook.host == "cursor" {
        input.adapt_cursor(event);
    } else if copilot {
        input.adapt_copilot(event);
    } else if input.hook_event_name.is_empty() {
        input.hook_event_name = event.to_string();
    }
    let session = resolve_session(&input.session_id, &cfg.core.session_env, |k| {
        std::env::var(k).ok()
    });
    let mut cx = Runtime::open(cfg.clone(), session)
        .map_err(|e| format!("hook {event}: store open: {e}"))?;
    // SessionStart carries `cwd` like every other event, so the session row is attributed
    // from the first hook of the run rather than whichever call happens to arrive first.
    cx.cwd = input.cwd.clone();
    let out = dispatch(stdin, &input, &cx);
    if copilot {
        let parsed: HookOutput = serde_json::from_slice(&out).unwrap_or_default();
        return Ok(copilot_output(&parsed));
    }
    Ok(out)
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
    let parent = cx
        .record_call("hook", "hook", Some(&input.hook_event_name))
        .ok();
    let out = match input.hook_event_name.as_str() {
        "PreToolUse" => pre_tool(input, cx, &registry),
        "PostToolUse" => post_tool(input, cx, &registry),
        "SessionStart" | "UserPromptSubmit" | "PostCompact" => inject_event(input, cx, &registry),
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

fn pre_tool(input: &HookInput, cx: &Runtime, registry: &Registry) -> HookOutput {
    let Some(ev) = input.pre_tool() else {
        return HookOutput::default();
    };
    let mut rewrite: Option<PreToolDecision> = None;
    for p in registry.enabled() {
        let got = panic::catch_unwind(AssertUnwindSafe(|| p.pre_tool(&ev, &Ctx::new(cx))))
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
        if let Ok(Some(s)) =
            panic::catch_unwind(AssertUnwindSafe(|| p.post_tool(&ev, &Ctx::new(cx))))
        {
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

    #[test]
    fn copilot_output_shapes_pre_post_block_and_empty() {
        let json = |b: Vec<u8>| serde_json::from_slice::<serde_json::Value>(&b).unwrap();
        assert_eq!(
            json(copilot_output(&HookOutput::default())),
            serde_json::json!({})
        );

        let pre = HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                permission_decision: Some("allow".into()),
                permission_decision_reason: Some("rtok".into()),
                updated_input: Some(serde_json::json!({"command": "rtok cmd -- git status"})),
                additional_context: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            json(copilot_output(&pre)),
            serde_json::json!({
                "permissionDecision": "allow",
                "permissionDecisionReason": "rtok",
                "modifiedArgs": {"command": "rtok cmd -- git status"}
            })
        );

        let post = HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                hook_event_name: "PostToolUse".into(),
                additional_context: Some("ctx".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
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
}
