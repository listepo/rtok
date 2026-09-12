# rtok roadmap — one plan per internal plugin

View of `plan.md` grouped by in-tree plugin (D6). `plan.md` is the source of tasks and Checks; this file is the order to build each plugin. When a task moves to `done.md`, tick it here in the same commit.

**Now:** no open §5 tasks — 166 done, 1 superseded (T10.7 → T10.9); see `done.md` / `plan.md` §5. Remaining work is [Later (v0.2+)](#later-v02) only. Every plugin lane below is implemented; design notes (D15) landed with each first task.

**Sequence if time is short** (`plan.md` §5): P12 → P13 → T14.0 → P1 `measure` → P2 hooks + `inject` → P5 `proxy` passthrough → P3 `cmd` → P4 `read` → P5 `archive` compress → P9 bench. `memory` / `graph` / `guard` / `toon` / P10 / P11 after the core pays for itself; P11 first among those if an OpenAI-API host is in daily use. v0.2+ (LLM compression, embeddings, LSP `graph`, WASM) is [Later](#later-v02); daemon was promoted to P20. Do not start Later while treating v0.1 as unfinished bookkeeping — the numbered tasks are done.

Legend: **blocked by** = tasks that must land first; **gate** = keep-or-revert rule after the plugin is in daily use.

---

## Core (not a plugin — blocks all of them)

| Task | What | Status |
|------|------|--------|
| T0.1–T0.7 | binary, config stub, rusqlite store, trait, estimator, hook types, CI | done 2026-09-01 |
| T0.8 | public plugin API, drop `Kind`, `examples/mcp_tool.rs` | done 2026-09-02 |
| T14.0 | plan template + `plugin_plans` structure test (D15) | done 2026-09-02 |
| Gate P0 | trait shape final; no plugin logic yet | done 2026-09-03 (historical freeze; see `plan.md` §6) |
| P12 T12.1–T12.4 | clap + figment + toml_edit; every flag is a key (D12, D14) | done 2026-09-02 |
| P13 T13.1–T13.4 | Diesel; `calls` / `tokens` / `logs` (D13) | done 2026-09-02 |
| T2.1 | `rtok hook <event>` dispatcher, fail open ≤ 10 ms | done 2026-09-02 |
| T2.2 | latency harness | done 2026-09-02 |
| T2.3 | `rtok agent setup claude` | done 2026-09-02 |
| T4.1 | `rtok mcp` stdio server | done 2026-09-02 |
| T1.4 | `rtok doctor` | done 2026-09-02 |
| T9.3–T9.5, T10.1–T10.4 | replace hooks, README, Cursor/OpenCode/Codex, release | done 2026-09-02 |

---

## `measure`

**Goal.** A baseline you can trust before changing anything. Savings that are not a `Measurement` row do not exist (D3).

**Replaces.** rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard.

**Surfaces.** `rtok stats`, `rtok bench`, proxy `usage`.

**Blocked by.** T0.3 (store). T1.5 needs an API key. T5.5 needs T5.1. T11.6 needs T13.2.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.1 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/measure/PLAN.md`. · done 2026-09-02 |
| 1 | T1.1 | Parse host transcripts (Claude JSONL first) into tool calls, results, usage, turn index. · done 2026-09-02 |
| 2 | T1.2 | `rtok stats`: per-tool sizes, Bash families, MCP groups, **context-token-turns**. · done 2026-09-02 |
| 3 | T1.3 | `--save-baseline` / `--compare`. · done 2026-09-02 |
| 4 | T1.5 | Optional `--calibrate` via `count_tokens`. · done 2026-09-02 |
| 5 | T5.5 | Cache-health from proxy `usage`. · done 2026-09-02 |
| 6 | T9.1 | `rtok bench` A/B harness (shared with P9). · done 2026-09-02 |
| 7 | T11.6 | `usage.api` + per-API stats. · done 2026-09-03 |

**Gate P1.** Baseline saved (`rtok stats --save-baseline before-rtok`); numbers in `research.md` §2.

**Status.** Lane done — T14.1, T1.1–T1.5, T5.5, T9.1, T11.6 done (see `done.md`).

---

## `inject`

**Goal.** Every SessionStart / UserPromptSubmit injection is budgeted and byte-stable.

**Replaces.** caveman shrink-hook, ponytail/caveman modes, lean-ctx banner, engram/claude-mem SessionStart dump.

**Surfaces.** SessionStart, UserPromptSubmit. Other plugins hand `Injection`s to this one; they do not write context themselves.

**Blocked by.** T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.6 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/inject/PLAN.md`. · done 2026-09-02 |
| 1 | T2.4 | Sort by priority, emit until `plugins.inject.budget_tokens` (default 800), drop the rest. Same prefix bytes every turn. · done 2026-09-02 |
| 2 | T7.1 | Modes as markdown (`modes/terse.md`, `modes/yagni.md`), not code. · done 2026-09-02 |

**Gate P2 (shared).** Setup is additive; sessions still work. **Gate P7.** A/B `terse` on/off; keep only if output tokens fall without task failures.

**Status.** Lane done — T14.6, T2.4, T7.1 done (see `done.md`).

---

## `cmd`

**Goal.** Every Bash output archived, filtered, measured; lossless via `expand`.

**Replaces.** rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress.

**Surfaces.** PreToolUse(Bash) → `rtok run`; `rtok expand`; OpenCode `rtok filter --stdin` (T10.2).

**Blocked by.** T0.3. Hook rewrite needs T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.2 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/cmd/PLAN.md`. · done 2026-09-02 |
| 1 | T3.1 | Run via `$SHELL -lc`, archive raw stdout+stderr, pointer trailer when long. · done 2026-09-02 |
| 2 | T3.2 | TOML rule engine (head/tail/dedupe/drop). Never redact. · done 2026-09-02 |
| 3 | T3.3 | Family formatters (cargo, git, pytest/jest, ls/find) + `rules/default.toml`. · done 2026-09-02 |
| 4 | T3.4 | PreToolUse(Bash) rewrite to `rtok run`. · done 2026-09-02 |
| 5 | T3.5 | `rtok expand <id>`. · done 2026-09-02 |
| 6 | T3.6 | `Measurement` per run; `stats --plugin cmd`. · done 2026-09-02 |
| 7 | T10.2 | OpenCode stdin filter. · done 2026-09-02 |

**Gate P3.** One working day vs baseline; keep only if Bash context-token-turns fall and expand rate < 5 %.

**Status.** Lane done — T14.2, T3.1–T3.6, T10.2 done 2026-09-02 (see `done.md` P3).

---

## `read`

**Goal.** Replace lean-ctx’s 78 tools with 5 and the 3.1 K/turn banner with 0.

**Replaces.** lean-ctx ctx_read/search/tree, token-optimizer read_cache/structure_map.

**Surfaces.** MCP `read` / `search` / `tree`; PreToolUse(Read) advice.

**Blocked by.** T4.1 (`rtok mcp`). Advice needs T2.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.3 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/read/PLAN.md`. · done 2026-09-02 |
| 1 | T4.2 | `read` full/lines, size cap, root guard. · done 2026-09-02 |
| 2 | T4.3 | done 2026-09-02 · map/signatures via tree-sitter-tags. |
| 3 | T4.4 | Re-read dedup (sha256 → `unchanged since <id>`). · done 2026-09-02 |
| 4 | T4.5 | `search` + `tree`. · done 2026-09-02 |
| 5 | T4.6 | PreToolUse(Read) advice. · done 2026-09-02 |
| 6 | T4.7 | Register MCP in `setup claude` (core/setup). · done 2026-09-02 |

**Gate P4.** Disable lean-ctx for one day; compare Read/MCP rows and injection tokens vs baseline.

**Status.** Lane done — T14.3, T4.1–T4.7 done 2026-09-02; Gate P4 removed 2026-09-09 (traffic, not code).

---

## `proxy`

**Goal.** Ground-truth `usage` and a cache-safe hop. One proxy, two wires (D11).

**Replaces.** headroom proxy, caveman-proxy.

**Surfaces.** `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`.

**Blocked by.** T0.3, T13.3. OpenAI wires need T5.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.5 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/proxy/PLAN.md`. · done 2026-09-02 |
| 0b | T5.0 | done 2026-09-02 · `httpmock` harness + six `tests/fixtures/proxy/*` wires. |
| 1 | T5.1 | Passthrough + SSE; insert `usage` + `calls`/`call_io`/`tokens`. · done 2026-09-02 |
| 2 | T5.2 | done 2026-09-02 · Lifecycle, health, setup. |
| 3 | T11.1 | `Wire` trait; Anthropic behind it. · done 2026-09-02 |
| 4 | T11.2 | OpenAI Chat Completions. · done 2026-09-02 |
| 5 | T11.3 | OpenAI Responses. · done 2026-09-03 |
| 6 | T11.5 | Codex / OpenCode setup. · done 2026-09-03 |

**Gate P5 (passthrough).** Two days of usage rows. **Gate P11.** Same for one OpenAI-API host.

**Status.** Lane done — T14.5, T5.0–T5.2, T11.1–T11.3, T11.5 done (see `done.md` P5/P11).

---

## `archive`

**Goal.** Shrink old, large `tool_result` blocks without breaking the prompt cache; lossless via `expand`.

**Replaces.** token-optimizer archive_result, headroom CCR, caveman retrieve.

**Surfaces.** Proxy live zone (`Plugin::proxy_filter`); MCP `expand`.

**Blocked by.** T5.1. Cross-wire rewrite needs T11.1–T11.3.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.4 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/archive/PLAN.md`. · done 2026-09-02 |
| 1 | T5.3 | Compress mode: rewrite old/large tool results, keyed by `tool_use_id`, byte-stable. · done 2026-09-02 |
| 2 | T5.4 | `expand` through the proxy; stop rewriting expanded ids. · done 2026-09-02 |
| 3 | T11.4 | Same rewrite on OpenAI wires. · done 2026-09-03 |

**Gate P5 (compress).** Two days compress after passthrough; keep only if context-token-turns fall ≥ 15 % and expand rate < 5 %.

**Status.** Lane done — T14.4, T5.3, T5.4, T11.4 done (see `done.md`).

---

## `memory`

**Goal.** One memory instead of two, zero LLM cost.

**Replaces.** engram, claude-mem.

**Surfaces.** MCP `mem_save` / `search` / `get`; PreCompact checkpoint (T2.5).

**Blocked by.** T0.3 (`notes` + FTS5 exist). Recall needs T2.4. Checkpoint needs T2.4 + T1.1.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.8 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/memory/PLAN.md`. · done 2026-09-02 |
| 1 | T6.1 | Notes API + FTS5 search. · done 2026-09-02 |
| 2 | T2.5 | PreCompact checkpoint → `notes` kind=checkpoint; inject on compact SessionStart. · done 2026-09-02 |
| 3 | T6.2 | SessionStart recall through `inject`. · done 2026-09-02 |
| 4 | T6.3 | Import generic JSONL (not third-party tool formats). · done 2026-09-02 |

**Gate P6.** Disable engram + claude-mem for a week; compare injection and MCP description tokens. Revert if recall is worse.

**Status.** Lane done — T14.8, T6.1–T6.3, T2.5 done 2026-09-02 (see `done.md`).

---

## `graph`

**Goal.** `symbol` / `callers` / `impact` / `outline` from an index rtok builds itself.

**Replaces.** codebase-memory-mcp, code-review-graph, serena, codegraph.

**Surfaces.** MCP tools. Index in SQLite.

**Blocked by.** T4.3 (grammars, done) and T4.5 (`search`, done); T4.1 for MCP.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.9 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/graph/PLAN.md`. · done 2026-09-02 |
| 1 | T8.1 | tree-sitter-tags index (definitions + reference sites). · done 2026-09-02 |
| 2 | T8.2 | MCP `symbol`, `callers`, `outline`; cap + archive id. · done 2026-09-02 |
| 3 | T8.3 | `root` column: two repos in one store stop evicting each other. · done 2026-09-04 |
| 4 | T8.4 | stat gate (mtime + size) before sha256; warm call reads nothing. · done 2026-09-04 |
| 5 | T8.8 | hand-labelled recall on 30 symbols, published in `research.md`. · done 2026-09-04 |
| 6 | T8.5 | `end_line` + `scope`: a caller is the enclosing definition, not a line. · done 2026-09-04 |
| 7 | T8.6 | `symbol` returns the definition body (`plugins.graph.body_lines`). · done 2026-09-04 |
| 8 | T8.7 | `impact(name, depth)` over the scope edges; fourth and last tool. · done 2026-09-04 |
| 9 | T8.9 | contract through `rtok mcp`: four tools byte-exact, two roots, edit, delete. · done 2026-09-04 |
| 10 | T8.10 | `symbol_*` methods move to `src/store/symbols.rs`; one `impl Store` per file is the seam. · done 2026-09-04 |
| 11 | T8.11 | `lbug` behind `graph-lbug`: open `graph.lbdb`, index writes in Cypher. · done 2026-09-04 |
| 12 | T8.12 | `lbug` reads; contract green under the feature. · done 2026-09-04 |
| 13 | T8.13 | `impact` as one query: SQLite `WITH RECURSIVE` vs `lbug` `*1..depth` path. · done 2026-09-08 |
| 14 | T8.14 | P8c bench under both builds; numbers to `research.md`; gate decides, loser deleted. · done 2026-09-08 |
| 15 | T8.15 | `plugins.graph.auto_index` (default on: every call walks) and `plugins.graph.watch` (`off` \| `notify` \ · done 2026-09-08 | `watchman`).|
| 16 | T8.16 | watcher thread inside `rtok mcp` on `notify`: quiet period, then `index::run`; one writer per store under `graph-lbug`. · done 2026-09-08 |
| 17 | T8.17 | `watchman_client` as a second event source behind `graph-watchman`; falls back to `notify`. · done 2026-09-09 |

**Gate P8.** Description-token savings vs the four servers; index this repo in < 2 s.

**Gate P8b.** Graph surface ≤ 150 description tokens (measured 62, 4 tools); warm tool call < 100 ms on a 3 000-file repo (measured 0.053 s); T8.8 definition recall ≥ 0.9 (measured 1.0; reference recall 0.351, `plan.md` §6). Closed 2026-09-09 on the three code clauses (`plan.md` / `done.md` P8b); the P9 task-set comparison is not code-closable.

**Gate P8c.** Contract byte-identical under `default` and `graph-lbug`; hook p95 ≤ 10 ms; warm calls < 100 ms; `impact(4)` on 10 000 edges ≥ 2× faster on `lbug` than the SQLite CTE; clean `just check` ≤ 2× and a reproducible build; sizes published. Loser deleted (D18).

**Gate P8d.** An edit is visible in `symbol` within 1 s with `auto_index = false` and the call reading 0 files; idle `rtok mcp` with the watcher on costs ≤ 50 ms CPU / 60 s and ≤ 2 MB RSS; `watchman` passes the same test and falls back to `notify` without one; hook p95 unchanged; binary bytes published. Watcher stays opt-in if idle cost is lost; `watchman_client` removed if it does not beat `notify`.

**Status.** Lane done — T14.9, T8.1–T8.19 done (P8 / P8b / P8c / P8d); Gate P8 passed 2026-09-03; Gate P8b closed 2026-09-09 on the three code clauses; Gate P8c (2026-09-08) left `graph-lbug` opt-in; Gate P8d passed 2026-09-09. See `done.md` P8*. **P39 (2026-09-12): SQLite only** — LadybugDB (`graph-lbug`, `symbols_lbug.rs`, `lbug` dep) and Grafeo spike (`graph-grafeo`) removed after measurement (Grafeo abandon; Ladybug frozen cost). No live alternate graph backends.

---

## `guard`

**Goal.** Stop identical Read/Bash loops; point at the prior archived result.

**Replaces.** token-optimizer refetch_guard / loop detection.

**Surfaces.** PreToolUse.

**Blocked by.** T2.1, T3.1 (archive ids).

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.7 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/guard/PLAN.md`. · done 2026-09-02 |
| 1 | T2.6 | Deny a Read/Bash that matches one in this session within `plugins.guard.window_turns` (default 8) when an archive id exists; reason names `rtok expand <id>`. Record a measurement. Never deny with no prior archive. · done 2026-09-02 |

**Gate.** Same honesty rule as `cmd`: deny rate is visible in `stats --plugin guard`; expand on a denied id must still work.

**Status.** Lane done — T14.7, T2.6 done 2026-09-02 (see `done.md`).

---

## `toon`

**Goal.** Tabular JSON tool results → TOON, off until a bench says it pays.

**Replaces.** caveman toon / TOON. Encoder written here (D6), not a wrap of another tool.

**Surfaces.** `proxy_filter` on the normalised `Wire` view (D11). Default **off**.

**Blocked by.** T11.1 (`Wire`). Enable for real traffic only after T9.1 can A/B it.

| Order | Task | Plan |
|-------|------|------|
| 0 | T14.10 | **Design first (D15).** Survey ≥ 3 alternatives (≥ 1 outside the retired stack), name what beats them, set the `Target:` this lane's gate must beat → `src/plugins/toon/PLAN.md`. · done 2026-09-02 |
| 1 | T11.7 | Encode tabular JSON arrays/objects when `plugins.toon.enabled`. Deterministic. Measurement per block. Default off → request bytes identical. · done 2026-09-03 |

**Gate P9.** Keep enabled only if cost per passed task falls and pass rate holds.

**Status.** Lane done — T14.10, T11.7 done 2026-09-02 (see `done.md`); still off by default.

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

## Plugin SDK (the contract — not a plugin)

**Replaces.** The in-tree-only `src/plugin.rs` contract: a third party had to depend on the whole `rtok` binary crate to implement one trait.

**Goal.** `rtok-plugin-sdk` on crates.io — the trait, the events, the management surface and the host capabilities, documented and versioned (D25). Every internal plugin implements it, so the SDK is proved by what ships.

**Surfaces.** None of its own. `rtok` re-exports it as `rtok::plugin` and stays the only dispatcher.

**Blocked by.** Nothing — T23.0–T23.6 are all done 2026-09-09. The lane is complete.

| # | Task | What | Complexity |
|---|------|------|------------|
| 0 | T23.0 | **Design first (D15).** Where the crate line goes → `crates/rtok-plugin-sdk/PLAN.md` · done 2026-09-09, the middle line: contract + host capability traits, three dependencies | 3/5 |
| 1 | T23.1 | workspace + crate; the value types move, `rtok::plugin` re-exports · done 2026-09-09 | 3/5 |
| 2 | T23.2 | required methods (`manifest`, `dashboard_page`); catalogue copy moves to its plugin · done 2026-09-09 | 2/5 |
| 3 | T23.3 | host capability traits replace bare `Store` access; `Plugin` and the wire view move into the crate · done 2026-09-09 | 4/5 |
| 4 | T23.4 | the ten plugins move onto the SDK · done 2026-09-09 | 3/5 |
| 5 | T23.5 | crate docs, doctests, one example plugin, `docs/plugin-authoring.md` · done 2026-09-09 | 2/5 |
| 6 | T23.6 | release publishes it (release-plz + `cargo-semver-checks`) · done 2026-09-09 | 2/5 |

**Gate P23.** A crate depending only on the SDK implements a plugin and runs through `Registry::from_plugins`; `rtok stats --json` byte-identical before and after; P17 size gate still passes.

## Dashboard (operator surface — not a plugin)

**Replaces.** Browser view of the same `Store` / `stats` the CLI uses (I-34). Complements ratatui `rtok tui` (P15).

**Goal.** `rtok web` — one process: axum WebSocket API + Slint WASM UI. Same pages as `rtok tui` (D23).

**Surfaces.** `rtok web --host --port` (D20, P19; `rtok dashboard` still runs, hidden and deprecated). Config `[web]`.

| # | Task | What |
|---|------|------|
| 1 | T19.1 | `[web]` host/port, CLI flags · done 2026-09-08 |
| 2 | T19.2 | `/ws` snapshot, per-plugin pages, stats widget · done 2026-09-08 |
| 3 | T19.3 | Slint WASM crate served from the same process · done 2026-09-08 |

## TUI (operator surface — not a plugin)

**Replaces.** token-optimizer dashboard, rtk `gain`, headroom `savings` (I-01 promoted).

**Goal.** `rtok tui` — ratatui dashboard over the same `Store` / `stats` / `doctor` data as the CLI (D17).

**Surfaces.** `rtok tui` (P15, shipped).

| # | Task | What | Complexity |
|---|------|------|------------|
| 0 | T15.0 | one operator model behind `web` and `tui` (D23) · done 2026-09-09 | 2/5 |
| 1 | T15.1 | ratatui + crossterm scaffold, event loop · done 2026-09-09 | 2/5 |
| 2 | T15.2 | header · tabs · footer shell · done 2026-09-09 | 2/5 |
| 3 | T15.3 | Overview tab (CTT, bars, sparkline) · done 2026-09-10 | 3/5 |
| 4 | T15.4 | Plugins tab (toggle enabled) · done 2026-09-10 | 3/5 |
| 5 | T15.5 | Calls tab (P13 rows + detail) · done 2026-09-10 | 3/5 |
| 6 | T15.6 | Doctor tab · done 2026-09-10 | 1/5 |
| 7 | T15.7 | Logs tab · done 2026-09-10 | 2/5 |
| 8 | T15.8–T15.9 | CLI, `[tui]` config · done 2026-09-10 (T15.8); TTY guard · done 2026-09-10 (T15.9) | 2/5 |

**Gate P15.** Overview numbers match `rtok stats --json`; `q` restores terminal.


## OpenTelemetry (export surface — not a plugin)

**Replaces.** Nothing installed exports: token-optimizer, rtk and headroom keep private ledgers. Field reference: OpenLLMetry / Langfuse SDKs (Python-side instrumentation, no hook shape).

**Goal.** Every ledger row is a span, log or sum in any OTLP backend — Jaeger, Grafana, SigNoz, Maple — with nothing on the hook path (D19). Design: `src/otel/PLAN.md`.

**Surfaces.** `rtok otel flush | status`; timers in `proxy` and `mcp`; `Stop` / `SessionEnd` spawn a detached flush.

| # | Task | What |
|---|------|------|
| 1 | T16.1 | `[otel]` keys + `OTEL_EXPORTER_OTLP_*` fallback · done 2026-09-04 |
| 2 | T16.2 | `otel_export` watermarks, row readers, `call_detail` · done 2026-09-04 |
| 3 | T16.3 | OTLP/HTTP JSON encoder, derived ids · done 2026-09-04 |
| 4 | T16.4 | ledger → GenAI semconv spans, events, log records · done 2026-09-04 |
| 5 | T16.5 | `flush` + `rtok otel`, httpmock tests · done 2026-09-04 |
| 6 | T16.6 | proxy / mcp timers, hook spawn · done 2026-09-04 |
| 7 | T16.7 | cumulative sums: tokens, saved, calls · done 2026-09-04 |
| 8 | T16.8 | `docs/otel.md` recipes, live check · done 2026-09-04 |

**Gate P16.** Mock collector sees every row once and never twice; hook p95 ≤ 10 ms with an endpoint set; §2 unchanged. The backend clause moved on 2026-09-07: the real-session trace and the SigNoz / Maple sign-off are Gate P18, a repeatable Jaeger / Grafana check is `ideas.md` I-33.

**Status.** Added 2026-09-04 (D19). T16.1–T16.8 done 2026-09-04, so the phase is implemented; Gate P16 passed 2026-09-07 on tests, latency and the dependency baseline. Jaeger 2.11 and Grafana `otel-lgtm` were each run once against the ledger (`research.md` §2), which found and fixed the 404-stream loop in the exporter; the remaining backend work lives in P18.

---

## Later (v0.2+)

Not v0.1 work. Same plugins, extra backends. Promoted 2026-09-10 from `ideas.md` Later into
`plan.md` phases P28–P33 (Daemon/TUI already P20/P15).

| Plugin / area | v0.2 plan | Ideas / phase |
|---------------|-----------|---------------|
| ~~TUI~~ | Promoted to P15 (D17/D23) — `rtok tui` shipped. | I-01 → P15 |
| `memory` | Optional LLM extractor (claude-mem style) and embeddings beside FTS5. Default off. Progressive disclosure stays. | I-21 → **P28**; I-22 → **P29**; I-13 |
| `graph` | Optional LSP backend (serena-grade) behind `symbol`/`callers`/`outline`; optional embeddings; tags index remains default. Symbol store is SQLite only (P39 closed). | I-24 → **P30**; I-22 → **P29**; I-30 → **P39** (done); I-14, I-15 |
| `archive` / `inject` | Optional LLM compression of old context; optional L0/L1/L2 tiers. Lossless `expand` still required where the source is not regenerable. | I-21 → **P28**; I-25 → **P33** |
| `proxy` | Optional semantic response cache (bifrost-like), off until a false-hit Check is 0 on the P9 set. | I-23 → **P31** |
| ~~core / daemon~~ | Promoted to P20 (D22) — `rtok demon` supervises `proxy`/`mcp`/`dashboard`. | (was mis-tagged I-26; daemon ≠ WASM) → **P20** |
| core | WASM host for out-of-tree plugins (`from_plugins` + `.wasm` example). | I-26 → **P32** |

Design notes: `src/plugins/compress/PLAN.md` (P28, T28.0). Each one carries the `Target:` its gate
must beat, and that line is the gate text below.

**Gate P28 (LLM compression).** `rtok bench` cost per passed task against the v0.1 lossless path must not rise, and `expand` still recovers a non-regenerable original.

