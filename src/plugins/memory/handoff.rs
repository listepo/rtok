//! Off-by-default sub-agent handoff stub (plan T59.6) and the spawn-brief digest (T130).

use rtok_plugin_sdk::{Class, Ctx, Measurement};

const SHARE: &str = "23 K of 2.83 M (0.8%)";

pub fn handoff(cx: &Ctx, budget_tokens: u32) -> String {
    let cfg = cx.plugin_config::<crate::config::Memory>("memory");
    let text = if !cfg.handoff {
        format!(
            "handoff disabled: Agent/Task results were {SHARE} on the measured workload; \
             enable [plugins.memory] handoff after sub-agents exceed 5 %."
        )
    } else {
        build_brief(cx, budget_tokens, "")
            .unwrap_or_else(|| "handoff: nothing read or edited yet.".into())
    };
    let before = 0u64;
    let after = text.len() as u64;
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "handoff",
        before_bytes: before,
        after_bytes: after,
        est_before: 0,
        est_after: cx.estimate(&text, Class::Prose),
        ref_id: Some(SHARE.into()),
        call_id: None,
    });
    text
}

pub fn handoff_tool() -> rtok_plugin_sdk::ToolDef {
    rtok_plugin_sdk::ToolDef {
        name: "handoff",
        description: "Budgeted session digest for sub-agents (off by default).",
        input_schema: serde_json::json!({
            "type":"object",
            "properties":{"budget_tokens":{"type":"integer"}},
            "required":["budget_tokens"]
        }),
    }
}

/// Fixed instructions on every spawn brief (T130): how to use a pointer without paying for
/// the whole file. Byte-stable, so it never busts the prompt cache on its own.
const INSTRUCTIONS: &str = "Read a slice with rtok read(mode=lines, range=a-b) before the whole \
file.\nGet the archived body with rtok expand <id>; answer with path:line citations.";

/// `PreToolUse` rows scanned for a `Read|Edit|Write` when building the ledger (T130).
/// T202: `hook_event_name` lives in `calls.name` (`record_call`, `src/hooks/mod.rs`), so the
/// query itself now returns only `PreToolUse` rows (`recent_hook_inputs_for_event`) instead
/// of the last 200 rows of *any* event with the filter applied afterwards in Rust — 100 keeps
/// today's reach: every tool call writes a Pre and a Post row, so 200 rows of any event held
/// about 100 `PreToolUse` rows.
const LEDGER_SCAN_ROWS: i64 = 100;

/// One path this session's own traffic named, and the archive id of its last full MCP read
/// when the (unrelated, optional) `read` plugin's cache still has one.
struct Pointer {
    path: String,
    archive_id: Option<String>,
}

/// Paths this session read or edited, most recent first, each path once (T130).
///
/// Reads `Ledger::recent_hook_inputs` rather than the `read` plugin's dedup cache: that cache
/// is cleared on every `Edit`/`Write` (T4.4), which would drop exactly the paths a spawn brief
/// most wants to keep. `memory` has no Cargo feature dependency on `read` (`memory = []`), so
/// this scans raw hook JSON instead of importing its types.
fn ledger(cx: &Ctx) -> Vec<Pointer> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let Ok(bodies) = cx.recent_hook_inputs_for_event("PreToolUse", LEDGER_SCAN_ROWS) else {
        return out;
    };
    for b in &bodies {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(b) else {
            continue;
        };
        let tool = v.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
        if !matches!(tool, "Read" | "Edit" | "Write") {
            continue;
        }
        let Some(path) = v
            .get("tool_input")
            .and_then(|i| i.get("file_path").or_else(|| i.get("path")))
            .and_then(|p| p.as_str())
        else {
            continue;
        };
        if !seen.insert(path.to_string()) {
            continue;
        }
        out.push(Pointer {
            archive_id: last_full_read_id(cx, path),
            path: path.to_string(),
        });
    }
    out
}

/// Archive id of `path`'s last full MCP read, if the `read` plugin's cache still has one.
/// Mirrors `plugins::read::cache::key(path, "full", None)` without depending on that
/// (optional) feature — see [`ledger`].
fn last_full_read_id(cx: &Ctx, path: &str) -> Option<String> {
    let abs = dunce::canonicalize(path).ok()?;
    let key = format!("{}\tfull\t", abs.to_string_lossy());
    cx.get_read_cache(&key).ok().flatten()?.0
}

/// The shared digest builder (T130): a budgeted list of pointers into this session's own
/// ledger, ranked with the paths `hint` names first — the `SubagentStart` spawn brief and the
/// `handoff` MCP tool are two surfaces of one digest (D21). `None` when the ledger is empty
/// (no offering, no `Measurement` noise, matching `PromptSubmit`'s convention).
pub fn build_brief(cx: &Ctx, budget_tokens: u32, hint: &str) -> Option<String> {
    let mut pointers = ledger(cx);
    if pointers.is_empty() {
        return None;
    }
    pointers.sort_by_key(|p| !hint.contains(p.path.as_str()));
    let lines: Vec<String> = pointers
        .iter()
        .map(|p| match &p.archive_id {
            Some(id) => format!("{} — rtok expand {id}", p.path),
            None => p.path.clone(),
        })
        .collect();
    let full = format!("{}\n{INSTRUCTIONS}", lines.join("\n"));
    let archived = cx.put_archive(full.as_bytes()).ok();
    let trailer = archived
        .as_ref()
        .map(|id| format!("\n[rtok {id} · expand: rtok expand {id}]"))
        .unwrap_or_default();
    let room = budget_tokens.saturating_sub(cx.estimate(&trailer, Class::Prose));
    let capped = crate::plugin::fit_budget(cx, &full, Class::Prose, room);
    let text = format!("{capped}{trailer}");
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "brief",
        before_bytes: 0,
        after_bytes: text.len() as u64,
        est_before: 0,
        est_after: cx.estimate(&text, Class::Prose),
        ref_id: archived,
        call_id: None,
    });
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;

    #[test]
    fn stub_records_measurement_when_off() {
        let cx = crate::plugin::Runtime::in_memory("t596").unwrap();
        let ctx = Ctx::new(&cx);
        let out = handoff(&ctx, 800);
        assert!(out.contains("disabled"));
        assert!(cx.store.measurement_count("memory").unwrap() >= 1);
    }

    /// T130 review fix: the enabled branch must actually call `build_brief` — the shared
    /// digest builder behind both the `handoff` MCP tool and the `SubagentStart` hook —
    /// instead of a placeholder string.
    #[test]
    fn handoff_enabled_shares_the_spawn_brief_builder() {
        let mut cx = crate::plugin::Runtime::in_memory("t596-on").unwrap();
        cx.config.plugins.memory.handoff = true;
        touch(&cx, "Read", "/repo/a.rs");
        let ctx = Ctx::new(&cx);
        let out = handoff(&ctx, 300);
        assert_eq!(out, build_brief(&ctx, 300, "").unwrap(), "{out}");
        assert!(out.contains("/repo/a.rs"), "{out}");
    }

    /// Enabled but nothing read or edited yet: a fallback line, not the "not yet measured"
    /// placeholder `build_brief` replaced.
    #[test]
    fn handoff_enabled_with_an_empty_ledger_falls_back() {
        let mut cx = crate::plugin::Runtime::in_memory("t596-on-empty").unwrap();
        cx.config.plugins.memory.handoff = true;
        let ctx = Ctx::new(&cx);
        let out = handoff(&ctx, 300);
        assert_eq!(out, "handoff: nothing read or edited yet.");
    }

    /// Fabricates a `PreToolUse(<tool>)` row in the hook window `ledger()` scans (T130), the
    /// same shape `hooks::dispatch` archives on every real call — including `calls.name`
    /// (T202 filters `ledger()`'s query on it, same as real dispatch does).
    fn touch(cx: &crate::plugin::Runtime, tool: &str, path: &str) {
        let stdin = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": tool,
            "tool_input": {"file_path": path},
        });
        let id = cx.record_call("hook", "hook", Some("PreToolUse")).unwrap();
        cx.store
            .insert_call_io(
                id,
                Some(&serde_json::to_vec(&stdin).unwrap()),
                None,
                65536,
                None,
            )
            .unwrap();
    }

    #[test]
    fn empty_ledger_offers_nothing() {
        let cx = crate::plugin::Runtime::in_memory("t130-empty").unwrap();
        assert!(build_brief(&Ctx::new(&cx), 300, "").is_none());
    }

    #[test]
    fn brief_lists_touched_paths_and_carries_its_own_expand_id() {
        let cx = crate::plugin::Runtime::in_memory("t130-brief").unwrap();
        touch(&cx, "Read", "/repo/a.rs");
        touch(&cx, "Edit", "/repo/b.rs");
        let text = build_brief(&Ctx::new(&cx), 300, "").unwrap();
        assert!(text.contains("/repo/a.rs"), "{text}");
        assert!(text.contains("/repo/b.rs"), "{text}");
        assert!(text.contains("rtok expand"), "{text}");
        assert!(
            cx.store.measurement_count("memory").unwrap() >= 1,
            "a brief that fires must leave a Measurement row (kind=brief)"
        );
    }

    #[test]
    fn paths_named_in_the_hint_sort_first() {
        let cx = crate::plugin::Runtime::in_memory("t130-hint").unwrap();
        touch(&cx, "Read", "/repo/a.rs");
        touch(&cx, "Read", "/repo/b.rs");
        let text = build_brief(&Ctx::new(&cx), 300, "look at /repo/b.rs please").unwrap();
        assert!(
            text.find("/repo/b.rs").unwrap() < text.find("/repo/a.rs").unwrap(),
            "{text}"
        );
    }

    #[test]
    fn brief_stays_under_budget_and_is_byte_stable() {
        let cx = crate::plugin::Runtime::in_memory("t130-budget").unwrap();
        for n in 0..20 {
            touch(&cx, "Read", &format!("/repo/file_{n}.rs"));
        }
        let ctx = Ctx::new(&cx);
        let budget = 40u32;
        let first = build_brief(&ctx, budget, "").unwrap();
        assert!(ctx.estimate(&first, Class::Prose) <= budget, "{first}");
        let second = build_brief(&ctx, budget, "").unwrap();
        assert_eq!(
            first, second,
            "unchanged ledger must produce identical bytes"
        );
    }
}
