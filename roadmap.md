# rtok roadmap — one plan per internal plugin

View of `plan.md` grouped by in-tree plugin (D6). `plan.md` is the source of tasks and Checks; this file is the order to build each plugin. When a task moves to `done.md`, tick it here in the same commit.

**Now (2026-09-18):** every lane below is implemented (v0.1 numbered work incl. T30.2 is in `done.md`); the open rows in `plan.md` are follow-ups per lane, listed here in the order the lane's card gates them. `plan.md` is the source; this list is the grouping.

**Reconciled 2026-09-21 (T126):** every id this table listed (67) has a task heading in `done.md`, except T71.1, dropped under the 1 % gate (`ideas.md` I-71). No lane has an open row; open work is the `plan.md` table.

Remaining approved-not-in-plan work is [Later (v0.2+)](#later-v02).

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
| T2.3 | `rtok agents install claude` | done 2026-09-02 |
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

**Not planned.** General HTTP(S) interception (`HTTPS_PROXY` + a local CA) — T165 measured ≈ 0 % of agent context reachable only that way; re-open condition and privacy rule in `research.md` §21 (creator decision 2026-09-23).

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

Not v0.1 work. Same plugins, extra backends. P28–P33 were promoted from `ideas.md` Later (daemon/TUI already P20/P15). Landed P28–P33 including T30.2 are in `done.md`.

| Plugin / area | v0.2 plan | Ideas / phase |
|---------------|-----------|---------------|
| ~~TUI~~ | Promoted to P15 (D17/D23) — `rtok tui` shipped. | I-01 → P15 |
| `memory` | Optional LLM extractor (claude-mem style) and embeddings beside FTS5. Default off. Progressive disclosure stays. | I-21 → **P28**; I-22 → **P29**; I-13 |
| `graph` | Optional LSP backend (serena-grade) behind `symbol`/`callers`/`outline` — **T30.2 in `done.md`**; optional embeddings (P29 done); tags index remains default. Symbol store is SQLite only (P39 closed). | I-24 → **P30** (T30.2 in plan); I-22 → **P29** (done); I-30 → **P39** (done); I-14, I-15 |
| `archive` / `inject` | Optional LLM compression of old context; optional L0/L1/L2 tiers. Lossless `expand` still required where the source is not regenerable. | I-21 → **P28**; I-25 → **P33** |
| `proxy` | Optional semantic response cache (bifrost-like), off until a false-hit Check is 0 on the P9 set. | I-23 → **P31** |
| ~~core / daemon~~ | Promoted to P20 (D22) — `rtok demon` supervises `proxy`/`mcp`/`dashboard`. | (was mis-tagged I-26; daemon ≠ WASM) → **P20** |
| core | WASM host for out-of-tree plugins (`from_plugins` + `.wasm` example). | I-26 → **P32** |

Design notes: `src/plugins/compress/PLAN.md` (P28, T28.0). Each one carries the `Target:` its gate
must beat, and that line is the gate text below.

**Gate P28 (LLM compression).** `rtok bench` cost per passed task against the v0.1 lossless path must not rise, and `expand` still recovers a non-regenerable original.


# Batch / Flex API pass — implementation plan

> **Canonical copy:** research + T250–T259 checklist now live in [`plan.md`](plan.md)
> (`# Research — Batch / Flex API pass`). This roadmap section remains for navigation;
> prefer editing `plan.md` for task cards.

Implementation plan for the cost levers documented in [`docs/batch-flex.md`](docs/batch-flex.md)
and [`architecture.md`](architecture.md) §12. Design-first branch:
`docs/batch-flex-pass`. Code lives under `src/proxy/` only (hooks/MCP never see
LLM HTTP).

Status: **plan** (no implementation in this document). Ground truth for
“today” is `origin/main` as of the design pass (`bcb03778` docs commit on this
branch; proxy behaviour matches current `src/proxy/`).

## Goals

1. **Batch observe** — when clients already use OpenAI `/v1/batches*` or
   Anthropic `/v1/messages/batches*` through `rtok proxy`, rtok records those
   hops in the ledger and can attribute usage/price once results are fetched.
2. **Flex inject** — optional `[proxy.flex]` so OpenAI sync Chat/Responses
   requests get `service_tier = "flex"` via `prepare` without the agent knowing
   about Flex.
3. **Keep boundaries** — never convert a live sync agent turn into a Batch job;
   never put Batch/Flex logic in hooks or MCP; do not break prompt-cache sticky
   behaviour documented in [`docs/prompt-cache.md`](docs/prompt-cache.md).
4. **Measurable savings** — `rtok stats` / `rtok report` can distinguish sync vs
   Flex vs Batch once observe + Flex pricing rows exist.

Non-goals for this feature (separate tracks):

- Full **model routing** (D9 / `[proxy.routing]` policy + classifier) — stub
  config only until a Check exists; called out as a later stage, not a Blocker
  for Batch/Flex MVP.
- Automatic sync→Batch enqueue, Batch JSONL rewriting, or a new MCP “submit
  batch” tool (optional follow-on, explicit opt-in only).
- Semantic response cache (P31) behaviour beyond a hard skip on Batch paths.

## Scope

| In scope | Out of scope |
|----------|--------------|
| Config parse for `[proxy.batch]` / `[proxy.flex]` (and stub `[proxy.routing]`) | Transparent sync→Batch |
| Path recognition for Batch create/poll/list/cancel/results + related `/v1/files` | Rewriting Batch JSONL member bodies |
| Ledger tags + optional result→`usage` parsing | Anthropic Flex twin (none exists) |
| OpenAI `prepare` Flex injection / force / documented 429 policy | Changing compress/`proxy_filter` semantics for Batch |
| Docs already on this branch stay authoritative; update when behaviour ships | Porting Flex/Batch into host plugins |

Primary modules (today):

- `src/proxy/mod.rs` — `handle` / `shape_request` / fallback / bookkeeping
- `src/proxy/wire.rs` + `openai_chat.rs` / `openai_responses.rs` / `anthropic.rs` —
  exact `Wire::matches` for sync shapes only
- Config load / validate (same path as other `[proxy.*]` tables)
- Store: `calls`, `call_io`, `usage`, `tokens`

## Current baseline (do not regress)

- Batch paths miss every `Wire::matches` → axum **fallback** byte-forward;
  optional `calls` row; **no** compress / prepare / usage parse.
- Client-set `service_tier` on OpenAI sync bodies already forwards.
- `prepare` today shapes OpenAI `stream_options.include_usage` (and related),
  not Flex.
- MCP / Claude hooks never see LLM HTTP.

## Stages

### S0 — Align plan + config surface (docs done on this branch)

**Done when:** `docs/batch-flex.md`, `docs/config.md` stubs, architecture §12,
proxy `AGENTS.md`, and this plan agree on semantics.

**Work:** keep this file updated if decisions change (especially Flex `429`
fallback and Batch `calls.kind` naming).

**Exit:** no code yet; `rtok config validate` still rejects uncommented
`[proxy.batch|flex|routing]` until S1.

### S1 — Config types + validate (no behaviour change)

**Work:**

- Add `ProxyBatch`, `ProxyFlex`, `ProxyRouting` structs with defaults matching
  [`docs/batch-flex.md`](docs/batch-flex.md) (batch observe defaulting carefully:
  `enabled` is effectively always-on via fallback today; prefer `observe` /
  `parse_results` as the real switches).
- Wire into `Config` deserialize + `rtok config validate`.
- Document that unknown-key failure goes away for these tables.

**Tests:** unit parse; golden TOML fixtures; validate accepts commented→live
examples from `docs/config.md`.

**Exit:** config loads; proxy behaviour **unchanged** with defaults.

### S2 — Batch path observe (create / poll / list / cancel)

**Work:**

- Detect Batch path classes (OpenAI + Anthropic tables in
  `docs/batch-flex.md`) without folding them into sync `Wire` adapters.
- Tag `calls` (kind/metadata) so Batch is distinguishable from sync
  `api_request`.
- Keep bodies pass-through; do **not** run compress/`proxy_filter` on Batch
  create payloads or JSONL file bytes.
- Guard: if semantic cache ever runs on the proxy hop, **skip** Batch paths.

**Tests:** proxy integration — POST/GET Batch paths record tagged rows; body
byte-identical; sync wires still match and parse usage.

**Exit:** `rtok stats` can filter or show Batch hop counts (even without
token totals yet).

### S3 — Batch results → usage (opt-in `parse_results`)

**Work:**

- On Anthropic results fetch / OpenAI output file download through the proxy,
  when `parse_results = true`, parse per-line usage into `usage` (or a rollup
  linked to the batch `calls` row).
- Fail-open on parse errors; never block the client download.
- Price rows: use existing provider rates; Batch discount factors as provider
  documents (document any hard-coded assumptions).

**Tests:** fixture JSONL / results streams → expected `usage` rows; disabled
flag → no parse; malformed line → fail-open.

**Exit:** `rtok stats --price` reflects Batch traffic when observe+parse are on.

### S4 — Flex inject via `prepare` (OpenAI wires)

**Work:**

- In OpenAI Chat + Responses `prepare`: if `[proxy.flex] enabled`, set
  `service_tier = "flex"` when omitted; if `force`, overwrite; if client sent
  `default`/`auto` and `force = false`, leave alone.
- Record effective `service_tier` on the `calls` row.
- **429 policy:** implement only after product decision:
  - `fallback = "none"` (default): surface error to client;
  - `fallback = "default"`: one retry without Flex (**confirm before coding** —
    still marked TODO in design docs).
- Add Flex USD/MTok rows under `[stats.prices]` when rates are chosen (dated).

**Tests:** prepare unit tests for omit/force/respect; integration with mock
upstream; 429 behaviour matrix once policy locked.

**Exit:** enabling `[proxy.flex]` changes request JSON only on OpenAI sync
wires; Anthropic unchanged; prompt-cache docs still accurate.

### S5 — Reporting + docs polish

**Work:**

- Breakdown in `rtok report` / stats: sync vs Flex vs Batch.
- Update `docs/batch-flex.md` “planned” → “today” for shipped pieces; keep
  routing as planned.
- Optional: pointer from `next.md` / plan cards when promoting to `plan.md`.

**Exit:** operator can prove savings from rtok ledger without provider console.

### S6 — Model routing stub only (optional track)

**Work:** parse `[proxy.routing]` (`enabled=false`, `sticky`, `default_model`);
no classifier. Sticky upstream flag may land with prompt-cache work (I-84)
independently.

**Exit:** config ready; no model rewrite until D9 Check exists (follow-on
plan, not part of Batch/Flex MVP definition of done).

## Suggested order / sizing

| Stage | Approx. effort | Risk |
|-------|----------------|------|
| S1 Config | 0.5–1 d | Low — validate regressions |
| S2 Batch observe | 1–2 d | Medium — path matching false positives (`/v1/messages` vs `/v1/messages/batches`) |
| S3 Results parse | 1–2 d | Medium — provider result shapes drift |
| S4 Flex prepare | 1–2 d | Medium — 429 fallback / cache interaction |
| S5 Report + docs | 0.5–1 d | Low |
| S6 Routing stub | 0.5 d | Low |

Batch observe (S2) and Flex (S4) can proceed in parallel after S1; S3 depends
on S2.

## Risks

| Risk | Mitigation |
|------|------------|
| Accidental sync→Batch or treating Batch as a Wire | Explicit non-goal; exact path tables; tests that `/v1/messages` still wires and `/v1/messages/batches` does not |
| Compress/`proxy_filter` mutates Batch JSONL | Never attach Batch paths to sync Wire; integration asserts byte identity |
| Flex 429 silent fallback surprises agents | Default `fallback = "none"`; document; require explicit config for retry |
| Prompt-cache miss from Flex/routing churn | Flex only sets `service_tier`; no system/tools rewrite; sticky stays separate |
| Ledger noise / double-count usage | Tag Batch distinctly; parse_results opt-in; fail-open |
| Config validate breaks existing TOMLs | Defaults off; ship behind this branch’s documented keys only |
| Scope creep into D9 routing | S6 stub only; classifier is a separate plan |

## Tests (summary)

- **Unit:** config parse; Batch path classifier; Flex `prepare` field matrix;
  result-line usage parser fixtures.
- **Integration (proxy):** sync wire regression (usage still parsed); Batch
  create/poll tagged + pass-through; Flex enabled request body; semantic-cache
  skip if that code path exists.
- **Manual / live (optional):** one OpenAI Flex call and one small Anthropic
  Message Batch through `rtok proxy` against a throwaway key — document in PR,
  not required for CI.

Reuse existing proxy test harness patterns under `src/proxy/` / `tests/` (same
style as current prepare / wire tests). Prefer named constants over magic
numbers per listepo Rust norms.

## Definition of done (MVP)

MVP = **S1 + S2 + S4 + S5** (S3 strongly recommended for “savings in rtok”;
S6 not required).

- [ ] `[proxy.batch]` / `[proxy.flex]` parse and validate
- [ ] Batch hops tagged in `calls`; bodies unchanged
- [ ] Flex inject/force on OpenAI Chat/Responses via `prepare`; effective tier
      recorded
- [ ] Flex `429` policy implemented as documented (no silent default unless
      configured)
- [ ] Stats/report can separate Flex vs sync (Batch counts at minimum; usage if
      S3 shipped)
- [ ] `docs/batch-flex.md` updated for shipped behaviour; no sync→Batch
- [ ] CI green for new unit/integration tests on Mac; no CloudAgent for listepo
      work

## Related

- [`docs/batch-flex.md`](docs/batch-flex.md) — semantics and endpoint tables
- [`docs/config.md`](docs/config.md) — planned TOML keys
- [`docs/prompt-cache.md`](docs/prompt-cache.md) — sticky vs Batch/Flex
- [`architecture.md`](architecture.md) §12 — pipeline placement
- [`src/plugins/proxy/AGENTS.md`](src/plugins/proxy/AGENTS.md) — agent notes
- `next.md` (PR #266) — token-saving backlog card for Batch/Flex/routing

## Batch / Flex API pass — detailed task cards (T250–T259)

Second pass after merging `batch-flex-plan.md` into this file. Expands each stage into
`plan.md`-style cards grounded in `docs/batch-flex.md`, `architecture.md` §12, and the
real `src/proxy/` + `src/config/` + `src/store/` surfaces as of this branch. Do not treat
the verbatim plan section above as obsolete — these cards are the executable checklist.

**MVP definition of done (from merged plan):** S1 + S2 + S4 + S5 (T251, T252, T254, T255);
S3 (T253) strongly recommended; S6 (T256) stub only. T257–T259 are cross-cutting.

**Global non-goals (every card inherits):** no transparent sync→Batch conversion; no Batch
JSONL member-body rewrite; no Batch/Flex logic in hooks or MCP; Anthropic has no Flex
`service_tier` twin; no CloudAgent for listepo work; do not break prompt-cache sticky
behaviour in `docs/prompt-cache.md`.

**Data-flow baseline (architecture §12 / `src/proxy/mod.rs`):**
`client → axum fallback (`app` → `proxy` → `handle`) → optional `shape_request`
(`record` → `compress` → `rewrite_tools` → `context_edits` → `prepare`) → upstream →
response tee → `record_usage` / `finish` when a `Wire` matches.** Batch paths miss
`wire::for_path` today and stay on the byte-forward arm.

### T250. Epic: Batch / Flex API pass overview

Parent epic for T251–T259. Cost levers: Batch observe + optional result→usage, Flex inject
on OpenAI sync wires via `Wire::prepare_request`, measurable sync vs Flex vs Batch in
`rtok stats` / `rtok report`. Authoritative semantics: [`docs/batch-flex.md`](docs/batch-flex.md).
Pipeline placement: [`architecture.md`](architecture.md) §12. Config stubs already in
[`docs/config.md`](docs/config.md) (`[proxy.batch]` / `[proxy.flex]` / `[proxy.routing]` —
not loaded by the binary yet; `rtok config validate` rejects them as unknown keys).

**Goals**

1. When clients already hit OpenAI `/v1/batches*` or Anthropic `/v1/messages/batches*`
   through `rtok proxy`, tag those hops in `calls` and optionally attribute usage once
   results are fetched (`parse_results`).
2. Optional `[proxy.flex]` so OpenAI Chat/Responses get `service_tier = "flex"` in
   `prepare` without the agent knowing about Flex.
3. Keep boundaries: never sync→Batch; never hooks/MCP; do not regress prompt-cache sticky.
4. `rtok stats` / `rtok report` can distinguish sync vs Flex vs Batch once observe + Flex
   pricing exist.

**Primary modules (touch only these for behaviour)**

| Area | Paths |
|------|--------|
| Proxy HTTP | `src/proxy/mod.rs` (`handle`, `shape_request`, `prepare`, `record`, `record_usage`, `finish`, `cache_response`, `app`) |
| Wires | `src/proxy/wire.rs` (`Wire`, `for_path`, `Usage`, `prepare_request` default), `openai_chat.rs`, `openai_responses.rs`, `anthropic.rs`, `gemini.rs` |
| Semantic cache skip | `src/proxy/semantic_cache.rs` (`eligible`, `build_prompt`) |
| Config | `src/config/mod.rs` (`section! { Proxy {…} }`, nest new sections), `src/config/validate.rs` |
| Store | `src/store/mod.rs` (`insert_call`, `insert_usage`, `insert_call_io`), `src/store/schema.rs` (`calls`, `usage`, `call_io`) |
| Report/stats | `src/report/` (and the `rtok stats` path that reads `usage` + `[stats.prices]`) |
| Docs | `docs/batch-flex.md`, `docs/config.md`, `architecture.md` §12, `src/plugins/proxy/AGENTS.md` |

**`calls` columns today (migrations `0002_schema_v2`, diesel `schema.rs`):**
`id`, `ts`, `session_id`, `host_id`, `provider_id`, `model_id`, `plugin`, `surface`,
`kind`, `parent_id`, `name`, `ms`, `ok`, `error`. Sync proxy hops use
`surface = "proxy"`, `kind = "api_request"`, `name = Some(path)`. **No JSON metadata
column.** Prefer tagging via `kind` / `name` / `call_io.request_json` (T258) over a new
column unless report queries prove otherwise.

**`usage` today (`0005_usage_api` added `api`):**
`id`, `ts`, `session`, `model`, `input`, `cache_create`, `cache_read`, `output`,
`call_id`, `api`.

Plan: keep this epic card as the index; implement only through child cards. Claim children
individually in the summary table.

Check: every child T251–T259 has a card below; `rg '^### T25[0-9]' plan.md` lists ten
headings; no collision with `done.md`; MVP checklist in the merged plan section remains
the acceptance spine.

### T251. S1: Config `[proxy.batch|flex|routing]` structs, defaults, validate

**Stage:** S1 — config only; proxy behaviour unchanged with defaults.

**Files**

- `src/config/mod.rs` — add `section!` blocks (same macro as `ToolsRewrite` / `Proxy`):
  - `ProxyBatch { enabled: bool = true, observe: bool = true, parse_results: bool = false }`
  - `ProxyFlex { enabled: bool = false, force: bool = false, fallback: String = s("none") }`
  - `ProxyRouting { enabled: bool = false, sticky: bool = true, default_model: String = String::new() }`
- Nest on existing `Proxy` section: `batch: ProxyBatch = ProxyBatch::default()`,
  `flex: ProxyFlex = ProxyFlex::default()`, `routing: ProxyRouting = ProxyRouting::default()`.
- `src/config/validate.rs` — accept the new dotted keys; reject
  `proxy.flex.fallback` unless `"none"` | `"default"` (mirror `proxy.mode` style around
  the `"proxy.mode" if !matches!(…)` arm ~line 324).
- `docs/config.md` — flip the “not loaded / validate fails” warnings for these three
  tables once the fields exist (full “planned→today” narrative is T259).

**Every config key**

| Table | Key | Type | Default | Meaning |
|-------|-----|------|---------|---------|
| `[proxy.batch]` | `enabled` | bool | `true` | Master switch (fallback already forwards Batch paths) |
| `[proxy.batch]` | `observe` | bool | `true` | Record distinguishable Batch ledger rows |
| `[proxy.batch]` | `parse_results` | bool | `false` | Parse result streams/files into `usage` (T253) |
| `[proxy.flex]` | `enabled` | bool | `false` | `prepare` may set OpenAI `service_tier = "flex"` when omitted |
| `[proxy.flex]` | `force` | bool | `false` | Overwrite client `service_tier` |
| `[proxy.flex]` | `fallback` | string | `"none"` | `none` \| `default` on Flex 429 (policy in T254) |
| `[proxy.routing]` | `enabled` | bool | `false` | D9 stub only (T256) |
| `[proxy.routing]` | `sticky` | bool | `true` | Prompt-cache affinity flag (I-84); not Batch vs Flex |
| `[proxy.routing]` | `default_model` | string | `""` | Empty = leave client `model` |

**Tests (snake_case)**

- `proxy_batch_flex_routing_defaults_match_docs` — `Config::default()` equals the table above.
- `config_validate_accepts_proxy_batch_flex_routing_tables` — live TOML from `docs/config.md` examples → zero unknown-key errors.
- `config_validate_rejects_proxy_flex_fallback_typo` — `fallback = "retry"` errors.
- `config_deserialize_nested_proxy_batch_observe_false` — nested parse round-trip.
- `rtok_config_validate_trycmd_batch_flex_stub` (trycmd or unit) — previously rejecting fixture now green.

**Risks**

| Risk | Mitigation |
|------|------------|
| Existing user TOMLs that already pasted the stubs start loading unexpected behaviour | Defaults keep observe on but S2/S4 code must no-op until those stages; S1 ships config only |
| `section!` macro surprises on nested structs | Follow `tools_rewrite: ToolsRewrite` precedent on `Proxy` |
| Validate walk misses nested keys | Extend the same dotted-key walker tests that cover `proxy.tools_rewrite.*` |

**Non-goals:** no path classifier, no `prepare` Flex, no report changes.

Check: `cargo nextest run -E 'test(proxy_batch_flex_routing) \| test(config_validate_accepts_proxy_batch)'`; `rtok config validate` on a fixture that includes the three tables; `just check` green; with defaults, a sync `/v1/messages` integration still records `kind = api_request` and parses usage unchanged.

### T252. S2: Batch path classifier + observe tagging (no Wire match, byte-identical)

**Stage:** S2 — observe create/poll/list/cancel (+ files) without folding Batch into sync `Wire` adapters.

**Files**

- New helper module preferred: `src/proxy/batch.rs` (or private fns in `mod.rs`) —
  `BatchClass` enum + `classify(method: &Method, path: &str) -> Option<BatchClass>`.
- `src/proxy/mod.rs` — `handle` / `shape_request` / `record`: when `classify` hits and
  `cfg.proxy.batch.enabled && cfg.proxy.batch.observe`, insert a tagged `calls` row;
  **skip** `compress`, `rewrite_tools`, `context_edits`, and Wire `prepare`; forward
  body bytes unchanged.
- `src/proxy/wire.rs` — do **not** add Batch to `WIRES` / `for_path`; keep
  `Anthropic::matches` as exact `path == "/v1/messages"` (already excludes
  `/v1/messages/batches…`).
- `src/proxy/semantic_cache.rs` — hard skip: if Batch classified, never call
  `eligible` / `lookup` (guard in `handle` before the cache block that currently gates on
  `wire` + `eligible`).
- `src/proxy/openai_chat.rs` / `openai_responses.rs` / `anthropic.rs` / `gemini.rs` —
  regression only (exact `matches` unchanged).

**Path tables (`docs/batch-flex.md`) — classifier must accept**

OpenAI:

| Method | Path pattern | Suggested `calls.kind` |
|--------|--------------|------------------------|
| `POST` | `/v1/batches` | `batch_create` |
| `GET` | `/v1/batches/{id}` | `batch_poll` |
| `GET` | `/v1/batches` | `batch_list` |
| `POST` | `/v1/batches/{id}/cancel` | `batch_cancel` |
| `*` | `/v1/files` and `/v1/files/{id}` (+ `/content`) | `batch_files` (or `files` — document choice; still observe when Batch-related traffic shares the route) |

Anthropic:

| Method | Path pattern | Suggested `calls.kind` |
|--------|--------------|------------------------|
| `POST` | `/v1/messages/batches` | `batch_create` |
| `GET` | `/v1/messages/batches/{id}` | `batch_poll` |
| `GET` | `/v1/messages/batches` | `batch_list` |
| `GET` | `/v1/messages/batches/{id}/results` | `batch_results` |
| `DELETE` | `/v1/messages/batches/{id}` | `batch_delete` |

**Data-flow (Batch create example)**

1. `handle` reads body (`MAX_BODY_BYTES`).
2. `wire::for_path("/v1/batches")` → `None`.
3. `batch::classify(POST, path)` → `Some(BatchCreate { provider: openai })`.
4. If `batch.enabled && batch.observe`: `record`-like insert with
   `surface = "proxy"`, `kind = "batch_create"`, `name = Some(path)` (and provider_id when
   known); **do not** parse as sync chat JSON for compress.
5. Skip semantic cache (no wire / explicit Batch guard).
6. `join_upstream` → OpenAI upstream; forward **byte-identical** request body.
7. Response tee: no `Wire::usage_from_body` (no wire); still set `calls.ms` /
   `call_io` best-effort like other recorded hops.

**False-positive guard:** `/v1/messages` must still match `Anthropic` and run compress/
prepare/usage; `/v1/messages/batches` must **not**.

**Tests**

- `batch_classify_openai_create_poll_list_cancel`
- `batch_classify_anthropic_create_poll_list_results_delete`
- `batch_classify_does_not_match_sync_messages_or_chat_completions`
- `batch_observe_posts_tagged_call_kind_batch_create`
- `batch_observe_body_byte_identical_no_compress`
- `batch_observe_disabled_skips_ledger_still_forwards` (`observe = false` or `enabled = false`)
- `sync_messages_still_wires_and_parses_usage` (regression)
- `semantic_cache_skips_batch_paths_even_when_cache_enabled`

**Risks**

| Risk | Mitigation |
|------|------------|
| `/v1/messages` vs `/v1/messages/batches` confusion | Exact prefix rules + dedicated tests |
| Compress/`proxy_filter` mutates Batch JSONL | Never attach Batch to sync `Wire`; assert byte identity |
| `/v1/files` noise (non-batch file traffic) | Document; optionally require Batch observe only when path is under batches* and treat files as optional observe — record decision in card Check notes |
| Ledger noise | Distinct `kind` values; `observe` switch |

**Non-goals:** result→usage (T253); Flex (T254); sync→Batch.

Check: `cargo nextest run -E 'test(batch_)'`; manual curl through proxy to `/v1/batches` against a mock; `just check`.

### T253. S3: `parse_results` → usage (Anthropic results + OpenAI files); fail-open

**Depends on:** T252. Opt-in via `[proxy.batch] parse_results = true`.

**Files**

- `src/proxy/batch.rs` (or `batch_results.rs`) — parsers:
  - Anthropic `GET …/messages/batches/{id}/results` JSONL / SSE-ish stream → per-line
    usage → `Store::insert_usage` linked to the Batch `calls` row (parent or same hop).
  - OpenAI output file download (`/v1/files/{id}/content`) when content is Batch output
    JSONL → same.
- `src/proxy/mod.rs` — after upstream response for classified `batch_results` /
  `batch_files` download paths, if `parse_results`, spawn fail-open parse (never delay or
  corrupt the client byte stream: parse a tee copy / post-forward buffer like existing
  usage tee patterns in `finish`).
- `src/store/mod.rs` — reuse `insert_usage` / `insert_provider_tokens`; set `usage.api` to
  a stable discriminator (e.g. `"openai_batch"` / `"anthropic_batch"`) distinct from sync
  `"openai"` / `"anthropic"` (`0005_usage_api`).
- Price: existing `[stats.prices]` rows × provider Batch discount factors — **document**
  any hard-coded factor and date it (do not invent silent discounts).

**Fail-open rules**

- Malformed line → log + skip line; continue.
- Parse panic / store error → log; client download already completed or streams unaffected.
- `parse_results = false` → zero `usage` rows from Batch results.

**Tests**

- `parse_anthropic_batch_results_jsonl_inserts_usage_rows`
- `parse_openai_batch_output_file_jsonl_inserts_usage_rows`
- `parse_results_disabled_inserts_no_usage`
- `parse_results_malformed_line_fail_open_skips_line`
- `parse_results_does_not_block_or_alter_response_bytes`

**Risks**

| Risk | Mitigation |
|------|------------|
| Provider result shape drift | Fixtures from current provider docs; fail-open |
| Double-count if create + results both counted | Only `parse_results` writes token `usage`; observe rows stay hop metadata |
| Large JSONL memory | Stream line-by-line; cap like `MAX_BODY_BYTES` policy |

**Migration:** none (T258) — `usage.api` + `call_id` suffice.

Check: `cargo nextest run -E 'test(parse_) \| test(batch_results)'`; `rtok stats --price` on a DB fixture with Batch usage; `just check`.

### T254. S4: Flex `prepare_request` on OpenAI wires; force/omit; 429 policy; tier metadata; prices

**Stage:** S4 — OpenAI only. Anthropic / Gemini unchanged.

**Files**

- `src/proxy/openai_chat.rs` — extend `OpenAiChat::prepare_request` (today only sets
  `stream_options.include_usage` when streaming). After include_usage logic, apply Flex:
  - if `!cfg` passed in: either extend signature
    `prepare_request(&self, body, include_usage, flex: FlexOpts)` **or** read Flex in
    `prepare` wrapper in `mod.rs` before/after `wire.prepare_request` (prefer keeping
    `Wire` free of `Config` — apply Flex in `mod.rs::prepare` for OpenAI wires only, or
    pass a small `PrepareOpts { include_usage, flex_enabled, flex_force }`).
- `src/proxy/openai_responses.rs` — same Flex field rules on Responses bodies.
- `src/proxy/wire.rs` — only if signature of `prepare_request` changes; default remains
  no-op for Anthropic/Gemini.
- `src/proxy/mod.rs` — `prepare`; optional 429 retry arm in `handle` when upstream
  returns 429 and `proxy.flex.fallback == "default"` (**product decision must be locked
  before coding the retry**; default shipped config is `"none"` = surface error).
- `src/proxy/mod.rs` `record` / post-prepare bookkeeping — record effective tier:
  prefer encoding in `calls.name` (e.g. keep path, add nothing if default) **or** rely on
  `call_io.request_json` containing `service_tier` after prepare (T258: **no migration**
  unless report needs a first-class column).
- `src/config/mod.rs` / `docs/config.md` — add dated Flex `[stats.prices."<model>"]`
  rows (or document Flex multiplier) when rates chosen.
- `docs/prompt-cache.md` — confirm Flex only sets `service_tier`, no system/tools rewrite.

**Flex field matrix**

| Client body `service_tier` | `enabled` | `force` | Result |
|----------------------------|-----------|---------|--------|
| omitted | false | * | unchanged |
| omitted | true | false | set `"flex"` |
| `"flex"` | true | false | unchanged |
| `"default"` / `"auto"` / other | true | false | **leave alone** |
| any | true | true | set `"flex"` |
| any | false | true | unchanged (force only applies when enabled — document) |

**429 policy**

- `fallback = "none"` (default): return upstream 429 to client; no silent retry.
- `fallback = "default"`: **one** retry with `service_tier` removed or set to default —
  only after explicit product confirm (still TODO in `docs/batch-flex.md`). Tests must
  cover both once locked.

**Tests**

- `flex_prepare_sets_tier_when_omitted_and_enabled`
- `flex_prepare_force_overwrites_client_tier`
- `flex_prepare_respects_client_default_when_not_force`
- `flex_prepare_noop_when_disabled`
- `flex_prepare_anthropic_body_untouched`
- `flex_prepare_preserves_include_usage_behaviour`
- `flex_429_fallback_none_surfaces_error`
- `flex_429_fallback_default_retries_once_without_flex` (gate on policy lock)
- `flex_effective_tier_visible_in_call_io_or_name`

**Risks**

| Risk | Mitigation |
|------|------------|
| Silent 429 fallback surprises agents | Default `none`; require explicit config |
| Prompt-cache miss from body churn | Only `service_tier` mutation; no tools/system rewrite |
| Breaking `include_usage` prepare | Compose Flex after existing logic; shared tests |

**Non-goals:** Anthropic Flex; model routing (T256); Batch path changes.

Check: `cargo nextest run -E 'test(flex_)'`; integration with mock upstream asserting JSON body; `just check`.

### T255. S5: report/stats breakdown sync vs Flex vs Batch

**Depends on:** T252 (kinds), T254 (tier visibility); T253 improves Batch USD.

**Files**

- Stats/report codepaths that aggregate `calls` / `usage` (locate current
  `rtok stats` / `rtok report` modules under `src/` — extend grouping by `calls.kind`
  and by effective tier from `call_io.request_json` or recorded name convention).
- `docs/batch-flex.md` — operator-facing “how to read the breakdown”.
- Optional web/tui proxy pages only if they already surface usage (do not invent a new
  dashboard epic here).

**Breakdown requirements**

- Counts: sync `api_request` vs `batch_*` kinds.
- Flex: sync OpenAI calls whose effective `service_tier == "flex"` vs not.
- Price: `--price` uses `[stats.prices]` (+ Batch discount docs from T253).

**Tests**

- `stats_breakdown_counts_batch_kinds_separately`
- `stats_breakdown_flags_flex_tier_rows`
- `report_sync_flex_batch_sections_smoke`

Check: fixture DB → `rtok stats` / `rtok report` output contains three buckets; `just check`.

### T256. S6: `[proxy.routing]` stub only (`enabled = false`)

**Files:** `src/config/mod.rs` (already in T251), `src/config/validate.rs`,
`docs/config.md`. **No** model rewrite in `prepare` / `shape_request` until D9 has a
measurement Check (separate plan). Sticky upstream may land with prompt-cache work (I-84)
independently — do not block Batch/Flex MVP.

**Tests:** `routing_stub_deserializes_enabled_false`; `routing_enabled_true_still_does_not_rewrite_model` (explicit no-op assert).

Check: config loads; grep/prepare paths unchanged; `just check`.

### T257. Tests matrix master checklist

Aggregate gate for the pass (run on Mac; no CloudAgent). Every name below must exist
before closing the epic (implementing cards own the code; this card closes when the
matrix is green).

**Unit**

- Config: `proxy_batch_flex_routing_defaults_match_docs`,
  `config_validate_accepts_proxy_batch_flex_routing_tables`,
  `config_validate_rejects_proxy_flex_fallback_typo`
- Classifier: `batch_classify_*` (OpenAI + Anthropic + sync negative)
- Flex prepare: `flex_prepare_*` matrix
- Parsers: `parse_anthropic_batch_results_jsonl_inserts_usage_rows`,
  `parse_openai_batch_output_file_jsonl_inserts_usage_rows`,
  `parse_results_malformed_line_fail_open_skips_line`

**Integration (proxy harness, same style as existing prepare/wire tests under
`src/proxy/` / `tests/`)**

- `sync_messages_still_wires_and_parses_usage`
- `batch_observe_body_byte_identical_no_compress`
- `batch_observe_posts_tagged_call_kind_batch_create`
- `semantic_cache_skips_batch_paths_even_when_cache_enabled`
- `flex_prepare` integration against mock upstream
- `flex_429_*` once policy locked
- `parse_results_does_not_block_or_alter_response_bytes`

**Manual / live (optional, PR notes only):** one OpenAI Flex call + one small Anthropic
Message Batch through `rtok proxy` with a throwaway key.

**Commands:** `cargo nextest run -E 'test(batch_) \| test(flex_) \| test(parse_results) \| test(proxy_batch)'`; `just check`.

### T258. Migrations decision: prefer no schema change

**Verify (already true on this branch):** `calls` has
`id, ts, session_id, host_id, provider_id, model_id, plugin, surface, kind, parent_id,
name, ms, ok, error` — **no** `metadata` / `service_tier` column. `usage` has `api` +
`call_id` (`0005`, `0013` indexes).

**Decision for this pass:** **no migration** by default.

| Need | Store in |
|------|----------|
| Batch vs sync | `calls.kind` (`batch_create`, `batch_poll`, … vs `api_request`) |
| Path | `calls.name` (already `Some(path)` in `record`) |
| Effective Flex tier | `call_io.request_json` after prepare (authoritative body) and/or stats join; optional convention documented in T254 |
| Batch usage source | `usage.api` = `openai_batch` / `anthropic_batch` |

**Only if** report queries cannot filter Flex without JSON extract at unacceptable cost:
propose `0023_calls_service_tier` (`ALTER TABLE calls ADD COLUMN service_tier TEXT`) and
update `src/store/schema.rs` + diesel models — separate mini-PR, not required for MVP.

**Tests if no migration:** schema-drift guard still green; document the decision in
`docs/batch-flex.md`.

Check: `rg 'service_tier' migrations/` empty (unless mini-PR); `just check`.

### T259. Docs flip planned→today + acceptance

**Files:** `docs/batch-flex.md` (planned→today for shipped S1/S2/S4/S5 pieces; keep
routing planned), `docs/config.md` (remove “validate fails” for loaded keys),
`architecture.md` §12 table, `src/plugins/proxy/AGENTS.md`, optional pointer from
`next.md` when promoting.

**Acceptance (MVP = T251+T252+T254+T255; T253 strongly recommended)**

- [ ] `[proxy.batch]` / `[proxy.flex]` parse and validate
- [ ] Batch hops tagged in `calls`; bodies unchanged
- [ ] Flex inject/force on OpenAI Chat/Responses; effective tier recorded without hooks/MCP changes
- [ ] Flex 429 policy as documented (no silent default unless configured)
- [ ] Stats/report separate Flex vs sync (Batch counts at minimum; usage if T253 shipped)
- [ ] `docs/batch-flex.md` updated; explicit **no** sync→Batch
- [ ] CI green for new tests on Mac

**Explicit non-goals restate:** no sync→Batch; hooks/MCP untouched; Anthropic has no Flex;
S6 routing stub only; no local LLM disk inventory in this plan.

Check: docs prose matches behaviour; `just check`; epic T250 can move to `done.md` when
all MVP boxes are ticked.

## Batch / Flex — second-pass gap fill

Appended after T250–T259 to close gaps found by re-reading the live code. Does **not**
replace the verbatim merged plan or the T-cards; implementers must read all three.

### G1. Correct `shape_request` order (fix T250 data-flow shorthand)

Real order in `src/proxy/mod.rs` `shape_request` (as of this branch):

1. `serde_json::from_slice` → `parsed`
2. `record(state, wire, path, parsed, headers, raw)` → `Option<Recorded>`
3. if `state.mode == "compress"` and `wire` is `Some`: `compress(...)` (else body unchanged;
   **unwired paths including Batch never compress**)
4. if `wire` is `Some`: `prepare(state, wire, body)` → `Wire::prepare_request`
5. if `wire` is `Some`: `context_edits` (Anthropic `context_management` only)
6. `rewrite_tools(state, body)` (runs even without a wire today — **Batch observe must
   either skip this for classified Batch paths or prove `tools_rewrite` no-ops on
   non-JSON-chat bodies**; prefer an explicit Batch early-return after `record` that
   returns `(original_bytes, recorded, false)` and skips steps 3–6)

Implication for T252: do not only “skip compress”; also skip `prepare`, `context_edits`,
and `rewrite_tools` for Batch-classified paths so JSONL/`/v1/files` bytes stay identical.

### G2. `plain()` and `dry_run` interactions

- `ProxyState::plain()` (`src/proxy/mod.rs`): when true (`proxy.enabled` / `core.enabled`
  false), `handle` forwards byte-identical with **no** `shape_request` / record / cache.
  Batch observe must not invent bookkeeping in plain mode.
- `[proxy] dry_run` (config field on `Proxy`): confirm current behaviour before Batch/Flex
  (existing proxy semantics); Flex inject and Batch parse must honour the same dry-run
  contract as sync traffic — document in T254/T253 Checks once read.

### G3. Exact `WIRES` set (do not extend for Batch)

`wire::for_path` (`src/proxy/wire.rs`):

```text
WIRES = [&ANTHROPIC, &OPENAI_CHAT, &OPENAI_RESPONSES, &GEMINI]
```

API constants: `API_ANTHROPIC`, `API_OPENAI_CHAT`, `API_OPENAI_RESPONSES`, `API_GEMINI`.
Gemini (`src/proxy/gemini.rs`) has no Batch/Flex role in this pass — classifier must not
treat `:generateContent` as Batch; Flex must not touch Gemini bodies.

Exact sync matches today:

| Wire | `matches` |
|------|-----------|
| `Anthropic` | `path == "/v1/messages"` |
| `OpenAiChat` | `path == "/v1/chat/completions"` |
| `OpenAiResponses` | (Responses path — keep as in `openai_responses.rs`) |
| `Gemini` | generate/streamGenerate actions only |

### G4. Stats / report concrete modules (refine T255)

Not only `src/report/`:

| Surface | Path |
|---------|------|
| `rtok stats` | `src/cli.rs` → `crate::web::model::stats_report` (`src/web/model.rs`) + `src/measure/stats` |
| `rtok report` | `src/report/mod.rs` `document`, renderers `markdown.rs` / `html.rs` / `pdf.rs` |
| Prices | `[stats.prices]` / `ModelPrice` in `src/config/mod.rs`; `--price` via `stats_flags` in `cli.rs` |

T255 should extend `stats_report` grouping and, if the markdown report’s Calls section is
the operator-facing breakdown, add a sync/Flex/Batch subsection in `src/report/markdown.rs`
fed by model structs — prefer one shared aggregation helper to avoid three total formulas
(see historical T207 lesson in `done.md`).

### G5. `insert_call` / `Recorded` / `parent_id`

- `Store::insert_call(session, surface, kind, host_id, provider_id, model_id, plugin, name)`
  — proxy sync uses `surface="proxy"`, `kind="api_request"`, `plugin` often via other
  helpers; `record` passes `plugin` as implicit through surface and `name=Some(path)`.
- `Store::set_call_parent(id, parent)` exists — T253 may parent result-parse usage’s
  `call_id` under the `batch_create` row when the batch id is known from the path.
- `Recorded { session, model, call_id }` in `mod.rs` — Batch observe can reuse or mirror.

### G6. Semantic cache config path

Cache is `[plugins.proxy.semantic_cache]` (`SemanticCache` on plugins proxy section),
**not** under `[proxy.]`. Guard in `handle` uses `state.cfg.plugins.proxy.semantic_cache`.
T252 skip must use that path.

### G7. OpenAI Responses `matches` + prepare today

Confirm in `openai_responses.rs`: `matches` path string and whether `prepare_request`
already mutates the body (include_usage analogue). Flex composition must not drop that
behaviour (T254 test `flex_prepare_preserves_include_usage_behaviour` covers Chat;
add `flex_prepare_preserves_responses_prepare_behaviour` if Responses has its own prepare).

### G8. Additional test names discovered in gap pass

- `batch_shape_request_skips_rewrite_tools_and_context_edits`
- `batch_observe_noop_in_plain_mode`
- `batch_classify_rejects_gemini_generate_content`
- `flex_prepare_preserves_responses_prepare_behaviour`
- `stats_report_includes_sync_flex_batch_buckets` (wire to `web::model::stats_report`)
- `report_markdown_mentions_batch_or_flex_when_present`

### G9. Risks added

| Risk | Mitigation |
|------|------------|
| `rewrite_tools` runs without a wire and could alter Batch JSON | Batch early-return in `shape_request` (G1) |
| Stats totals diverge across cli/report/web | One aggregation helper (G4); regression test |
| Flex prepare signature fight with `Wire` trait | Prefer `PrepareOpts` in `mod.rs::prepare` without forcing Config into every wire |
| `usage.api` new discriminators break dashboards that assume only anthropic/openai | Document; update any match/`GROUP BY api` readers |

### G10. Acceptance command pack (copy into PR)

```bash
just check
cargo nextest run -E 'test(batch_) | test(flex_) | test(parse_results) | test(proxy_batch_flex)'
rg -n 'proxy\.(batch|flex|routing)' src/config/mod.rs
rg -n 'batch_create|service_tier' src/proxy/
# plain-mode smoke: with proxy.enabled=false, Batch POST must not insert calls
```
