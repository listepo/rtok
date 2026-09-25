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

/// Symbol ranges one pointer may carry (T130.3, creator decision): the newest three.
const MAX_RANGES: usize = 3;

/// One path this session's own traffic named, and the archive id of its last full MCP read
/// when the (unrelated, optional) `read` plugin's cache still has one.
struct Pointer {
    path: String,
    archive_id: Option<String>,
    /// Innermost indexed definitions around what this session read or edited in `path`, newest
    /// first, as `(line, end_line, name)`. Empty without a graph index (T130.3).
    ranges: Vec<(i32, i32, String)>,
}

/// Where one `Read`/`Edit` call landed in its file: a `Read` line window, or an `Edit`'s
/// `new_string`, located in the file only when the index has definitions to widen it to.
enum Touch {
    Lines(i32, i32),
    Text(String),
}

/// Paths this session read or edited, most recent first, each path once (T130).
///
/// Reads `Ledger::recent_hook_inputs` rather than the `read` plugin's dedup cache: that cache
/// is cleared on every `Edit`/`Write` (T4.4), which would drop exactly the paths a spawn brief
/// most wants to keep. `memory` has no Cargo feature dependency on `read` (`memory = []`), so
/// this scans raw hook JSON instead of importing its types.
fn ledger(cx: &Ctx) -> Vec<Pointer> {
    let mut seen = std::collections::HashMap::new();
    let mut touched: Vec<(String, String, Vec<Touch>)> = Vec::new();
    let Ok(bodies) = cx.recent_hook_inputs_for_event("PreToolUse", LEDGER_SCAN_ROWS) else {
        return Vec::new();
    };
    for b in &bodies {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(b) else {
            continue;
        };
        let tool = v.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
        if !matches!(tool, "Read" | "Edit" | "Write") {
            continue;
        }
        let input = v.get("tool_input");
        let Some(path) = input
            .and_then(|i| i.get("file_path").or_else(|| i.get("path")))
            .and_then(|p| p.as_str())
        else {
            continue;
        };
        let at = *seen.entry(path.to_string()).or_insert_with(|| {
            let cwd = v.get("cwd").and_then(|c| c.as_str()).unwrap_or(".");
            touched.push((path.to_string(), cwd.to_string(), Vec::new()));
            touched.len() - 1
        });
        let int = |k: &str| input.and_then(|i| i.get(k)).and_then(|n| n.as_i64());
        let touch = match tool {
            "Read" if int("offset").is_some() || int("limit").is_some() => {
                let from = int("offset").unwrap_or(1).max(1);
                let to = from.saturating_add(int("limit").unwrap_or(2000).max(1) - 1);
                let line = |n: i64| i32::try_from(n).unwrap_or(i32::MAX);
                Some(Touch::Lines(line(from), line(to)))
            }
            "Edit" => input
                .and_then(|i| i.get("new_string"))
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| Touch::Text(s.to_string())),
            _ => None,
        };
        touched[at].2.extend(touch);
    }
    touched
        .into_iter()
        .map(|(path, cwd, touches)| Pointer {
            archive_id: last_full_read_id(cx, &path),
            ranges: symbol_ranges(cx, &cwd, &path, &touches),
            path,
        })
        .collect()
}

/// Widens each touch to the innermost indexed definition that encloses it (T130.3), newest
/// first, deduped, at most [`MAX_RANGES`]. The index is keyed like `graph::index::canon`
/// (canonical root, `/`-separated relative path) — mirrored here, as `last_full_read_id`
/// mirrors the read cache, so `memory = []` needs no `graph` feature. The file is read only
/// when the index has definitions for it and an `Edit` must be located.
fn symbol_ranges(cx: &Ctx, cwd: &str, path: &str, touches: &[Touch]) -> Vec<(i32, i32, String)> {
    let canon = |p: &std::path::Path| {
        dunce::canonicalize(p)
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
    };
    if touches.is_empty() {
        return Vec::new();
    }
    let abs = std::path::Path::new(cwd).join(path);
    let (root, file) = (canon(std::path::Path::new(cwd)), canon(&abs));
    let Some(rel) = file.strip_prefix(&root).and_then(|r| r.strip_prefix('/')) else {
        return Vec::new();
    };
    let defs = cx.symbol_file_defs(&root, rel).unwrap_or_default();
    if defs.is_empty() {
        return Vec::new();
    }
    let text = touches
        .iter()
        .any(|t| matches!(t, Touch::Text(_)))
        .then(|| std::fs::read_to_string(&abs).ok())
        .flatten();
    let mut out = Vec::new();
    for t in touches {
        let (from, to) = match t {
            Touch::Lines(a, b) => (*a, *b),
            Touch::Text(s) => {
                let Some(at) = text
                    .as_deref()
                    .and_then(|x| x.find(s.as_str()).map(|i| (x, i)))
                else {
                    continue;
                };
                let from = at.0[..at.1].matches('\n').count() as i32 + 1;
                (
                    from,
                    from + s.trim_end_matches('\n').matches('\n').count() as i32,
                )
            }
        };
        let Some((name, _, line, end)) = defs
            .iter()
            .filter(|d| d.2 <= from && d.3 >= to)
            .min_by_key(|d| d.3 - d.2)
        else {
            continue;
        };
        let range = (*line, *end, name.clone());
        if !out.contains(&range) {
            out.push(range);
        }
        if out.len() == MAX_RANGES {
            break;
        }
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
    let digest = |ranged: bool| {
        let lines: Vec<String> = pointers
            .iter()
            .map(|p| {
                let mut line = p.path.clone();
                if ranged && !p.ranges.is_empty() {
                    let spans: Vec<String> = p
                        .ranges
                        .iter()
                        .map(|(a, b, n)| format!("{a}-{b} {n}"))
                        .collect();
                    line = format!("{line}:{}", spans.join(", "));
                }
                match &p.archive_id {
                    Some(id) => format!("{line} — rtok expand {id}"),
                    None => line,
                }
            })
            .collect();
        format!("{}\n{INSTRUCTIONS}", lines.join("\n"))
    };
    let full = digest(true);
    let archived = cx.put_archive(full.as_bytes()).ok();
    let trailer = archived
        .as_ref()
        .map(|id| format!("\n[rtok {id} · expand: rtok expand {id}]"))
        .unwrap_or_default();
    let room = budget_tokens.saturating_sub(cx.estimate(&trailer, Class::Prose));
    // T130.3: ranges go before any pointer does; the archived body keeps them.
    let shown = if cx.estimate(&full, Class::Prose) <= room {
        full
    } else {
        digest(false)
    };
    let capped = crate::plugin::fit_budget(cx, &shown, Class::Prose, room);
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

    /// T131: `measure::subagents::has_spawn_brief` detects a fired brief by this substring
    /// of `INSTRUCTIONS`, mirrored there as `SPAWN_BRIEF_MARKER` rather than imported (a
    /// hook-side marker check pulling in the whole digest builder would be backwards). This
    /// pins the one thing that link needs: rewording `INSTRUCTIONS` so the marker no longer
    /// matches must fail this test, not silently zero `rtok stats`' "with brief" split.
    #[test]
    fn instructions_still_carry_the_spawn_brief_marker() {
        assert!(INSTRUCTIONS.contains(crate::measure::subagents::SPAWN_BRIEF_MARKER));
    }

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
        touch_at(cx, ".", tool, serde_json::json!({"file_path": path}));
    }

    /// [`touch`] with a session `cwd` and the whole `tool_input` (T130.3 ranges).
    fn touch_at(cx: &crate::plugin::Runtime, cwd: &str, tool: &str, input: serde_json::Value) {
        let stdin = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "cwd": cwd,
            "tool_name": tool,
            "tool_input": input,
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

    /// T130.3: an indexed file gets the innermost definitions its `Read` window and `Edit`
    /// landed in, newest first; a file the index lacks gets a bare path; an over-budget brief
    /// drops the ranges before any pointer.
    #[cfg(feature = "graph")]
    #[test]
    fn pointers_carry_enclosing_symbol_ranges_when_indexed() {
        let (cx, dir) = crate::testutil::runtime("t130-3-ranges");
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let src =
            "fn alpha() {\n    let x = 1;\n    let y = 2;\n}\n\nfn beta() {\n    let z = 3;\n}\n";
        std::fs::write(repo.join("lib.rs"), src).unwrap();
        std::fs::write(repo.join("notes.txt"), "a\nb\nc\n").unwrap();
        crate::plugins::graph::index::run(&Ctx::new(&cx), &repo, false).unwrap();
        let cwd = repo.to_str().unwrap();
        let lib = repo.join("lib.rs").to_string_lossy().into_owned();
        let notes = repo.join("notes.txt").to_string_lossy().into_owned();
        touch_at(
            &cx,
            cwd,
            "Read",
            serde_json::json!({"file_path": notes, "offset": 2, "limit": 1}),
        );
        touch_at(
            &cx,
            cwd,
            "Read",
            serde_json::json!({"file_path": lib, "offset": 2, "limit": 2}),
        );
        let edit =
            serde_json::json!({"file_path": lib, "old_string": "1", "new_string": "let z = 3;"});
        touch_at(&cx, cwd, "Edit", edit);
        let ctx = Ctx::new(&cx);
        let text = build_brief(&ctx, 300, "").unwrap();
        assert!(
            text.contains(&format!("{lib}:6-8 beta, 1-4 alpha")),
            "{text}"
        );
        assert!(
            text.lines().any(|l| l == notes),
            "unindexed: bare path\n{text}"
        );

        let budget = ctx.estimate(&text, Class::Prose) - 2;
        let tight = build_brief(&ctx, budget, "").unwrap();
        assert!(
            tight.contains(&lib) && !tight.contains("1-4 alpha"),
            "{tight}"
        );
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
