# rtok ideas

Parking lot for **propositions**: improvements or missing features that alternative tools
already do (or claim to do), but that are **not** a task in `plan.md` yet.

This file is not a backlog to implement. Agents implement only numbered tasks in `plan.md`
(`AGENTS.md`). An idea moves the other way: write it here → if evidence appears, promote it
to `plan.md` with a Check → then it may appear on `roadmap.md`.

Evidence lives in `research.md`. v0.1-out-of-scope items go to [Later (v0.2+)](#later-v02), not Rejected. Rejected is only for ideas that will never ship.

## How to add an idea

1. One row or one `I-NNN` section. Name the **source tool** and the **rtok plugin/area**.
2. Say what is missing relative to that tool, not a redesign of rtok.
3. Link the research row or a URL. No task, no Check, no branch.

**Promote:** add a task to `plan.md` with a Check; tick the idea `promoted <task-id>`;
mention it in `plan.md` §6.

**Defer:** move the row to [Later (v0.2+)](#later-v02) with a target version.
**Reject:** only if it will never ship; one line of evidence.

---

## Open

Inspired by the comparison matrix (`research.md` §4) and stack gaps (`research.md` §5)
that v0.1 does not schedule.

### Measure and doctor

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-01 | token-optimizer dashboard; rtk `gain`; headroom `savings` | TUI (`measure`) | **promoted P15** — ratatui `rtok tui` (not HTML); per-plugin, per-day CTT dashboard. | T1.2 done; promoted 2026-09-02 → P15 T15.1–T15.9, D17. |
| I-02 | research.md §8 | `measure` | `rtok stats --price` with per-model input/cache/output rates (Fable/Mythos 0.025× cache read). | Open question; T1.2 has no price table. |
| I-03 | Cursor, Codex, aider transcripts | `measure` | Ingest host logs beyond Claude Code JSONL (Cursor, Codex, OpenCode). | T1.1 is Claude JSONL first. Other hosts get `usage` via the proxy (P11). |
| I-04 | Anthropic deferred tools / `ENABLE_TOOL_SEARCH` | `doctor` | Detect whether `ANTHROPIC_BASE_URL` disabled MCP tool search; report how to keep deferred tools. | research.md §8; T1.4 can grow a Check later. |

### `cmd`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-05 | rtk ~80 filters; token-optimizer 111 bash compressors; lean-ctx 95+ shell patterns | `cmd` | Broader family coverage (sed, grep, cat, pnpm, python — the bulk of your Bash tokens in research.md §2) beyond the T3.3 starter set. | T3.3 lists cargo/git/test/ls. Extra families are data in `rules/default.toml`, not a new plugin. |
| I-06 | rtk TOML custom filters | `cmd` | Documented user filter API (`rules/*.toml` drop-in) matching rtk’s extension model, written here (D6). | T3.2 is the engine; a public schema + examples can wait until the engine exists. |

### `read`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-07 | lean-ctx 10 read modes | `read` | Extra modes beyond full/lines/map/signatures (e.g. imports-only, comments-stripped) if T4.3 does not cover the measured Read tail (38–68 K char files). | T4.2–T4.3 are four modes. Add a mode only with a Check on those files. |
| I-08 | lean-ctx deny Grep/Glob | `read` / `guard` | PreToolUse deny of native Grep/Glob with a pointer to MCP `search`/`tree`. | lean-ctx does this to force its 78 tools; rtok wants fewer tools. Only promote if `doctor` shows Grep/Glob dominating Read-class tokens after T4.5. |

### `archive` / `proxy`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-09 | headroom JSON crusher + code compressor | `archive` or new plugin | Compress JSON/code *inside* the live zone that is not a `tool_result` (nested dumps, huge `data:` blobs). | P5 only rewrites old `tool_result`s (cache-safe). Other fields risk cache busts (issue #81967). Needs a byte-stability Check. |
| I-10 | Anthropic context editing (`clear_tool_uses_*`, `compact_*`) | `proxy` | Optionally emit native context-editing instead of (or with) rtok rewrite, so the platform does the shrink. | v0.1 aligns with caching and does not fight the platform; a plugin that *sets* those headers is extra surface. |
| I-11 | Gemini / other wires | `proxy` | Third `Wire` (e.g. Gemini). | architecture.md §9: new `src/proxy/<name>.rs`. Not scheduled until an OpenAI-API host is measured (P11). |
| I-12 | headroom wrap CLI | `proxy` | `rtok wrap -- <agent>` that sets `ANTHROPIC_BASE_URL`/`OPENAI_BASE_URL` for one process. | T5.2 is lifecycle/setup; wrap is sugar on top. |

### `memory` / `graph`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-13 | claude-mem progressive disclosure; engram 18-tool banner | `memory` | Titles → ids → bodies strictly; never inject bodies at SessionStart (research.md §6 #7). | T6.2 already goes through `inject`. Spell the ladder as a Check if T6.2 is too loose. |
| I-14 | codebase-memory-mcp Cypher-like queries | `graph` | A small query language over the tags index (beyond `symbol`/`callers`/`outline`). | T8.2 is three tools. Cypher is a fourth surface; promote only if those three miss a measured query. |
| I-15 | code-review-graph impact radius | `graph` | `impact(path)` / changed-symbol fan-out for reviews. | Same as I-14: extra MCP tool, extra description tokens. |
| I-16 | codebase-memory-mcp 162 langs, LZ4 blobs | `graph` | More grammars + compressed index payloads. | T8.1 is tree-sitter-tags on this repo first; langs are data. |

### Hosts and product

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-17 | aider, Windsurf, Zed, Copilot Chat | `setup` | Installers beyond Claude / Cursor / OpenCode / Codex (T10.1–T10.3). | P10 is those three + release. New host = new `src/setup/<host>.rs` after T10.1 pattern. |
| I-18 | token-optimizer coaching / quality nudges | `inject` | Prompt nudges (“don’t re-read”, “use expand”). | Nudges are re-read every turn (D5). Lean-ctx’s 3.1 K banner is the cautionary tale. Promote only with a P7-style A/B. |
| I-19 | `log`/`tracing` in every alternative CLI | core | Dedicated `tracing` logger: levels (`error`–`trace`), `core.log_file`, no stderr on the hook path; `Ctx::log` stays the DB path (D13). | Config already has `log_level` / `log_file`; P13 writes the `logs` table. File+level subscriber is not a numbered task. |
| I-20 | clap ecosystem | CLI | Shell completions (`clap_complete`) and a man page (`clap_mangen`). | D14 is clap + figment. Completions are polish after T12.4. |

---

## Later (v0.2+)

Scheduled for a higher version, **not rejected**. Do not implement while v0.1 tasks are open.
When v0.1 is done, promote each ID to a numbered phase in `plan.md` (Later versions table)
and a row on `roadmap.md` Later. v0.1 `graph` stays tags-only; v0.1 hooks stay process-per-event.

| ID | Inspired by | Area | Proposition | Why v0.2+, not v0.1 |
|----|-------------|------|-------------|---------------------|
| I-21 | LLMLingua-2, claude-mem extraction | `compress` / `memory` | LLM-based compression and/or observation extraction. Default off. | Costs tokens; quality risk on code. Ship only if a bench beats v0.1 lossless. |
| I-22 | code-review-graph embeddings, mem0 | `memory`, `graph` | Optional embeddings / semantic search beside FTS5. | No measured need in current sessions; FTS5 is enough for v0.1. |
| I-23 | bifrost | `proxy` | Semantic response cache (similarity threshold). Opt-in. | Agent contexts rarely repeat; a hit can be a wrong answer. Needs a false-hit Check. |
| I-24 | serena | `graph` | LSP-grade / type-resolved backend behind the same MCP tools. | v0.1 tags index covers `symbol`/`callers`/`outline`; LSP is the precision ceiling. |
| I-25 | OpenViking L0/L1/L2 | `archive` / `inject` | Tiered session context loading. | Needs a model path and an AGPL license call-out; unmeasured vs v0.1 archive. |
| I-26 | (architecture) | core | WASM plugin host for out-of-tree plugins. | D1 v0.1 is in-tree + `from_plugins`. WASM is how third parties ship without linking. D6 still: this repo does not vendor those plugins. |
| I-27 | (architecture) | core | Optional daemon besides `proxy`/`mcp`. | v0.1 hooks are short-lived and fail open in ≤ 10 ms; a daemon must not become a single point of failure. |

---

## Rejected

Nothing permanently rejected. Scope by version (Open / Later), do not discard.

---

## Promoted

| I-01 | P15 T15.1–T15.9 | ratatui `rtok tui` dashboard (D17) | 2026-09-02 |

| ID | Became | Date |
|----|--------|------|
| (OpenAI Responses / Codex proxy) | D11 / P11 | 2026-09-01 |
| (config file for every flag) | D12 / P12 | 2026-09-01 |
| (ORM + action store) | D13 / P13 | 2026-09-01 |
| (`guard` / `toon` numbered tasks) | T2.6 / T11.7 | 2026-09-02 |
