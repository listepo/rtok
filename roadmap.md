# rtok roadmap — one plan per internal plugin

View of `plan.md` grouped by in-tree plugin (D6). `plan.md` is the source of tasks and Checks; this file is the order to build each plugin. When a task moves to `done.md`, tick it here in the same commit.

**Now:** T1.1 (JSONL parser). P12, P13, and P14 (T14.0–T14.10 design notes) are done 2026-09-02. Every plugin below is **manifest only** until its first task, and every lane now starts at row 0 with its design note (D15) — survey the alternatives first, then build something better, never a Rust copy of the tool being retired.

**Sequence if time is short** (`plan.md` §5): P12 → P13 → T14.0 → P1 `measure` → P2 hooks + `inject` → P5 `proxy` passthrough → P3 `cmd` → P4 `read` → P5 `archive` compress → P9 bench. `memory` / `graph` / `guard` / `toon` / P10 / P11 after the core pays for itself; P11 first among those if an OpenAI-API host is in daily use. v0.2+ (LLM compression, embeddings, LSP `graph`, daemon, WASM) is [Later](#later-v02); do not start it in v0.1.

Legend: **blocked by** = tasks that must land first; **gate** = keep-or-revert rule after the plugin is in daily use.

---

## Core (not a plugin — blocks all of them)

| Task | What | Status |
|------|------|--------|
| T0.1–T0.7 | binary, config stub, rusqlite store, trait, estimator, hook types, CI | done 2026-09-01 |
| T0.8 | public plugin API, drop `Kind`, `examples/mcp_tool.rs` | done 2026-09-02 |
| T14.0 | plan template + `plugin_plans` structure test (D15) | done 2026-09-02 |
| Gate P0 | trait shape final; no plugin logic yet | open |
| P12 T12.1–T12.4 | clap + figment + toml_edit; every flag is a key (D12, D14) | done 2026-09-02 |
| P13 T13.1–T13.4 | Diesel; `calls` / `tokens` / `logs` (D13) | done 2026-09-02 |
| T2.1 | `rtok hook <event>` dispatcher, fail open ≤ 10 ms | done 2026-09-02 |
| T2.2 | latency harness | open |
| T2.3 | `rtok setup claude` | open |
| T4.1 | `rtok mcp` stdio server | done 2026-09-02 |
| T1.4 | `rtok doctor` | open |
| T9.3–T9.5, T10.1–T10.4 | replace hooks, README, Cursor/OpenCode/Codex, release | T9.3 done 2026-09-02 |

---

## `measure`

**Goal.** A baseline you can trust before changing anything. Savings that are not a `Measurement` row do not exist (D3).

**Replaces.** rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard.

**Surfaces.** `rtok stats`, `rtok bench`, proxy `usage`.

**Blocked by.** T0.3 (store). T1.5 needs an API key. T5.5 needs T5.1. T11.6 needs T13.2.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.1 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/measure/PLAN.md`. |
| 1 | T1.1 | Parse host transcripts (Claude JSONL first) into tool calls, results, usage, turn index. · done 2026-09-02 |
| 2 | T1.2 | `rtok stats`: per-tool sizes, Bash families, MCP groups, **context-token-turns**. · done 2026-09-02 |
| 3 | T1.3 | `--save-baseline` / `--compare`. · done 2026-09-02 |
| 4 | T1.5 | Optional `--calibrate` via `count_tokens`. |
| 5 | T5.5 | Cache-health from proxy `usage`. |
| 6 | T9.1 | `rtok bench` A/B harness (shared with P9). |
| 7 | T11.6 | `usage.api` + per-API stats. |

**Gate P1.** Baseline saved (`rtok stats --save-baseline before-rtok`); numbers in `research.md` §2.

**Status.** Manifest only. First task: T1.1. T14.1 done 2026-09-02.

---

## `inject`

**Goal.** Every SessionStart / UserPromptSubmit injection is budgeted and byte-stable.

**Replaces.** caveman shrink-hook, ponytail/caveman modes, lean-ctx banner, engram/claude-mem SessionStart dump.

**Surfaces.** SessionStart, UserPromptSubmit. Other plugins hand `Injection`s to this one; they do not write context themselves.

**Blocked by.** T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.6 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/inject/PLAN.md`. |
| 1 | T2.4 | Sort by priority, emit until `plugins.inject.budget_tokens` (default 800), drop the rest. Same prefix bytes every turn. |
| 2 | T7.1 | Modes as markdown (`modes/terse.md`, `modes/yagni.md`), not code. |

**Gate P2 (shared).** Setup is additive; sessions still work. **Gate P7.** A/B `terse` on/off; keep only if output tokens fall without task failures.

**Status.** Manifest only. First task: T2.4. T14.6 done 2026-09-02.

---

## `cmd`

**Goal.** Every Bash output archived, filtered, measured; lossless via `expand`.

**Replaces.** rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress.

**Surfaces.** PreToolUse(Bash) → `rtok run`; `rtok expand`; OpenCode `rtok filter --stdin` (T10.2).

**Blocked by.** T0.3. Hook rewrite needs T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.2 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/cmd/PLAN.md`. |
| 1 | T3.1 | Run via `$SHELL -lc`, archive raw stdout+stderr, pointer trailer when long. |
| 2 | T3.2 | TOML rule engine (head/tail/dedupe/drop). Never redact. |
| 3 | T3.3 | Family formatters (cargo, git, pytest/jest, ls/find) + `rules/default.toml`. |
| 4 | T3.4 | PreToolUse(Bash) rewrite to `rtok run`. |
| 5 | T3.5 | `rtok expand <id>`. |
| 6 | T3.6 | `Measurement` per run; `stats --plugin cmd`. |
| 7 | T10.2 | OpenCode stdin filter. |

**Gate P3.** One working day vs baseline; keep only if Bash context-token-turns fall and expand rate < 5 %.

**Status.** Manifest only. First task: T3.1. T14.2 done 2026-09-02.

---

## `read`

**Goal.** Replace lean-ctx’s 78 tools with 5 and the 3.1 K/turn banner with 0.

**Replaces.** lean-ctx ctx_read/search/tree, token-optimizer read_cache/structure_map.

**Surfaces.** MCP `read` / `search` / `tree`; PreToolUse(Read) advice.

**Blocked by.** T4.1 (`rtok mcp`). Advice needs T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.3 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/read/PLAN.md`. |
| 1 | T4.2 | `read` full/lines, size cap, root guard. |
| 2 | T4.3 | map/signatures via tree-sitter-tags. |
| 3 | T4.4 | Re-read dedup (sha256 → `unchanged since <id>`). |
| 4 | T4.5 | `search` + `tree`. |
| 5 | T4.6 | PreToolUse(Read) advice. |
| 6 | T4.7 | Register MCP in `setup claude` (core/setup). |

**Gate P4.** Disable lean-ctx for one day; compare Read/MCP rows and injection tokens vs baseline.

**Status.** T4.2 done 2026-09-02. Next: T4.3–T4.6. T14.3 done 2026-09-02.

---

## `proxy`

**Goal.** Ground-truth `usage` and a cache-safe hop. One proxy, two wires (D11).

**Replaces.** headroom proxy, caveman-proxy.

**Surfaces.** `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`.

**Blocked by.** T0.3, T13.3. OpenAI wires need T5.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.5 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/proxy/PLAN.md`. |
| 0b | T5.0 | done 2026-09-02 · `httpmock` harness + six `tests/fixtures/proxy/*` wires. |
| 1 | T5.1 | Passthrough + SSE; insert `usage` + `calls`/`call_io`/`tokens`. |
| 2 | T5.2 | Lifecycle, health, setup. |
| 3 | T11.1 | `Wire` trait; Anthropic behind it. |
| 4 | T11.2 | OpenAI Chat Completions. |
| 5 | T11.3 | OpenAI Responses. |
| 6 | T11.5 | Codex / OpenCode setup. |

**Gate P5 (passthrough).** Two days of usage rows. **Gate P11.** Same for one OpenAI-API host.

**Status.** Manifest only. First task: T5.1. T14.5 done 2026-09-02.

---

## `archive`

**Goal.** Shrink old, large `tool_result` blocks without breaking the prompt cache; lossless via `expand`.

**Replaces.** token-optimizer archive_result, headroom CCR, caveman retrieve.

**Surfaces.** Proxy live zone (`Plugin::proxy_filter`); MCP `expand`.

**Blocked by.** T5.1. Cross-wire rewrite needs T11.1–T11.3.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.4 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/archive/PLAN.md`. |
| 1 | T5.3 | Compress mode: rewrite old/large tool results, keyed by `tool_use_id`, byte-stable. |
| 2 | T5.4 | `expand` through the proxy; stop rewriting expanded ids. |
| 3 | T11.4 | Same rewrite on OpenAI wires. |

**Gate P5 (compress).** Two days compress after passthrough; keep only if context-token-turns fall ≥ 15 % and expand rate < 5 %.

**Status.** Manifest only. First task: T5.3. T14.4 done 2026-09-02.

---

## `memory`

**Goal.** One memory instead of two, zero LLM cost.

**Replaces.** engram, claude-mem.

**Surfaces.** MCP `mem_save` / `search` / `get`; PreCompact checkpoint (T2.5).

**Blocked by.** T0.3 (`notes` + FTS5 exist). Recall needs T2.4. Checkpoint needs T2.4 + T1.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.8 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/memory/PLAN.md`. |
| 1 | T6.1 | Notes API + FTS5 search. |
| 2 | T2.5 | PreCompact checkpoint → `notes` kind=checkpoint; inject on compact SessionStart. |
| 3 | T6.2 | SessionStart recall through `inject`. |
| 4 | T6.3 | Import generic JSONL (not third-party tool formats). |

**Gate P6.** Disable engram + claude-mem for a week; compare injection and MCP description tokens. Revert if recall is worse.

**Status.** Manifest only. Schema exists since T0.3. First task: T6.1. T14.8 done 2026-09-02.

---

## `graph`

**Goal.** `symbol` / `callers` / `outline` from an index rtok builds itself.

**Replaces.** codebase-memory-mcp, code-review-graph, serena, codegraph.

**Surfaces.** MCP tools. Index in SQLite.

**Blocked by.** T4.3 (grammars) and T4.5 (`search`); T4.1 for MCP.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.9 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/graph/PLAN.md`. |
| 1 | T8.1 | tree-sitter-tags index (definitions + reference sites). |
| 2 | T8.2 | MCP `symbol`, `callers`, `outline`; cap + archive id. |

**Gate P8.** Description-token savings vs the four servers; index this repo in < 2 s.

**Status.** Manifest only. First task: T8.1. T14.9 done 2026-09-02.

---

## `guard`

**Goal.** Stop identical Read/Bash loops; point at the prior archived result.

**Replaces.** token-optimizer refetch_guard / loop detection.

**Surfaces.** PreToolUse.

**Blocked by.** T2.1, T3.1 (archive ids).

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.7 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/guard/PLAN.md`. |
| 1 | T2.6 | Deny a Read/Bash that matches one in this session within `plugins.guard.window_turns` (default 8) when an archive id exists; reason names `rtok expand <id>`. Record a measurement. Never deny with no prior archive. |

**Gate.** Same honesty rule as `cmd`: deny rate is visible in `stats --plugin guard`; expand on a denied id must still work.

**Status.** Manifest only. First task: T2.6 (added 2026-09-02). T14.7 done 2026-09-02.

---

## `toon`

**Goal.** Tabular JSON tool results → TOON, off until a bench says it pays.

**Replaces.** caveman toon / TOON. Encoder written here (D6), not a wrap of another tool.

**Surfaces.** `proxy_filter` on the normalised `Wire` view (D11). Default **off**.

**Blocked by.** T11.1 (`Wire`). Enable for real traffic only after T9.1 can A/B it.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.10 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/toon/PLAN.md`. |
| 1 | T11.7 | Encode tabular JSON arrays/objects when `plugins.toon.enabled`. Deterministic. Measurement per block. Default off → request bytes identical. |

**Gate P9.** Keep enabled only if cost per passed task falls and pass rate holds.

**Status.** Manifest only. Off by default. First task: T11.7 (added 2026-09-02). T14.10 done 2026-09-02.

---

## Dependency sketch

```
P12 config ──► P13 store ──► T2.1 hook ──► inject T2.4 ──► memory T6.x / T2.5
                 │                    └──────────────────► cmd T3.x ──► guard T2.6
                 │                                         └──────────► T10.2
                 ├──► measure T1.x
                 └──► T4.1 mcp ──► read T4.x ──► graph T8.x
                 └──► proxy T5.1 ──► archive T5.3 ──► Wire T11.x ──► toon T11.7
                                   └────────────────► T5.5 / T11.6 (measure)
```

P9 bench (`measure` T9.1) is the keep-or-drop gate for every plugin that claims a saving.

---

## TUI (operator surface — not a plugin)

**Replaces.** token-optimizer dashboard, rtk `gain`, headroom `savings` (I-01 promoted).

**Goal.** `rtok tui` — ratatui dashboard over the same `Store` / `stats` / `doctor` data as the CLI (D17).

**Surfaces.** `rtok tui` only (v0.2+).

| # | Task | What |
|---|------|------|
| 1 | T15.1 | ratatui + crossterm scaffold, event loop |
| 2 | T15.2 | header · tabs · footer shell |
| 3 | T15.3 | Overview tab (CTT, bars, sparkline) |
| 4 | T15.4 | Plugins tab (toggle enabled) |
| 5 | T15.5 | Calls tab (P13 rows + detail) |
| 6 | T15.6 | Doctor tab |
| 7 | T15.7 | Logs tab |
| 8 | T15.8–T15.9 | CLI, `[tui]` config, TTY guard |

**Gate P15.** Overview numbers match `rtok stats --json`; `q` restores terminal.


## Later (v0.2+)

Not v0.1 work. Same plugins, extra backends. Promote from `ideas.md` Later when v0.1 is done
(`plan.md` Later versions).

| Plugin / area | v0.2 plan | Ideas |
| TUI | `rtok tui` ratatui dashboard: Overview, Plugins, Calls, Doctor, Logs (P15). | I-01 |
|---------------|-----------|-------|
| `memory` | Optional LLM extractor (claude-mem style) and embeddings beside FTS5. Default off. Progressive disclosure stays. | I-21, I-22, I-13 |
| `graph` | Optional LSP backend (serena-grade) behind `symbol`/`callers`/`outline`; optional embeddings; tags index remains default. | I-24, I-22, I-14, I-15 |
| `archive` / `inject` | Optional LLM compression of old context; optional L0/L1/L2 tiers. Lossless `expand` still required where the source is not regenerable. | I-21, I-25 |
| `proxy` | Optional semantic response cache (bifrost-like), off until a false-hit Check is 0 on the P9 set. | I-23 |
| core | Optional daemon (hooks still fail open if it is down). WASM host for out-of-tree plugins (`from_plugins` + `.wasm` example). | I-26, I-27 |

