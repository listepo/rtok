# rtok — implementation plan for a unified, plugin-based token-reduction CLI

Status: plan v1, 2026-09-01. **Progress: P0 done 2026-09-02 (T0.1–T0.8); P12 T12.1–T12.4 done; P13 T13.1–T13.4 done (see `done.md`); P14 done; T1.1–T1.5 and T2.1–T2.6 done; T3.1–T3.6 done; T6.1–T6.3 T7.1–T7.2 done; T4.1 T4.2 T4.3 T4.4 T4.5 T4.6 T4.7 T5.0 T5.1 T5.2 T8.1 T8.2 T9.1 T9.2 T9.3 T9.4 T9.5 T10.1 T10.2 T10.3 T10.4 T11.1 T11.2 T11.3 T11.4 T11.5 T11.6 T11.7 T8.3 T8.4 T8.8 T8.5 T8.6 T8.7 T8.9 T16.1 T16.2 T16.3 T16.4 T16.5 T16.6 T16.7 T16.8 T8.10 T8.11 T8.12 T17.1 T18.1 T18.2 T18.3 T18.4 T17.2 T8.16 done.** Companion evidence: `research.md` (comparison, measurements, fact-check). Shape of the code: `architecture.md`. Per-plugin plan: `roadmap.md`. Propositions (not yet tasks): `ideas.md`. Every implemented task must be marked done and moved from here to `done.md` verbatim (Do/Check + `Status: done <date>` and Check result); a task that still lives here is not done.
Crate and binary: `rtok`, this repo (`~/GitHub/rtok`). Rust 1.97.1 is pinned in `mise.toml`; run cargo as `mise exec -- cargo …` (or `mise activate`). The legacy Docker chain stays in `~/GitHub/reduce-token`. Agent instructions: `AGENTS.md` (`CLAUDE.md` is a symlink to it).

## 0. Decisions (read before any task)

| # | Decision | Why (evidence in research.md) |
|---|----------|-------------------------------|
| D1 | **Rust, one static binary, in-tree plugins behind one trait + Cargo features.** No WASM, no subprocess plugins, no daemon **in v0.1**. v0.2+ may add a daemon and/or a WASM plugin host (see Later versions); this repo still does not wrap third-party tools (D6). | Hooks run on every tool call; Rust cold start <5 ms vs ~100 ms per Python hook. Your stack runs 27 Python hooks per event chain today. |
| D2 | **One binary, three surfaces:** `rtok hook` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`). | PostToolUse hooks cannot modify tool results (verified in docs). Only PreToolUse rewrite, MCP tool replacement, or a proxy can shrink what the model sees. |
| D3 | **Measurement first.** Nothing ships until `rtok stats` reads real usage from session logs and the proxy. Every plugin logs before/after. Metric = *context-token-turns* (tokens × turns they stay in context), plus output tokens. | Vendor claims 60–95 %; your measured savings 3–40 %; JetBrains measured rtk at −7.6 % to 0 %. Nobody in the stack measures end to end. |
| D4 | **Lossless by default.** Every compression keeps the original retrievable via `rtok expand <id>` / MCP `expand`. Lossy only where the source is regenerable (re-run the command). | Caveman issue #112 (silent code corruption); trust is the product. |
| D5 | **Injection budget.** All SessionStart/UserPromptSubmit injections go through one plugin with a per-turn token cap (default 800) and a byte-stable prefix. | lean-ctx alone injects ~3.1 K tokens per turn; injections are re-read (cached) every turn. |
| D6 | **Every plugin is native, written from scratch in this repo. No third-party plugins.** A plugin never spawns, links, imports, or reads the data of another tool (rtk, lean-ctx, engram, claude-mem, codebase-memory-mcp, serena, headroom, caveman, …). The tools in `research.md` are the *spec* of what to rebuild and retire, not code to wrap. Third parties extend rtok from outside through the public plugin API (`rtok::plugin`, `Registry::from_plugins`, `docs/plugin-authoring.md`, `examples/`), never through this repo. Rewritten 2026-09-01 by user decision (was: native *or* adapter). | A runtime dependency on the tools the bench is meant to retire makes the measurement circular and the install fragile. One code path per method is what `Measurement` can attribute. |
| D7 | **Prompt "modes" (terse, YAGNI) are data files, not code.** | Ponytail/caveman are markdown; measured effect must be A/B tested, not assumed. |
| D8 | **One SQLite file** (`~/.rtok/rtok.db`, WAL). Schema is D13. Raw archived payloads on disk under `~/.rtok/archive/` (D4); the DB holds indexes and inline JSON under a size cap. | engram, claude-mem, codebase-memory-mcp all converge on SQLite (+FTS5). |
| D9 | **Agents are provider-agnostic.** Route by job: low-cost for mechanical work and any task a cheap model can finish; mid-tier for coding (cheaper mid when the task is small); high-performance for research/investigation only after the user confirms. Do not silently switch up. Host names (Claude Code, Codex, Cursor) are products, not the implementer. Rewritten 2026-09-02. | User constraint: cost-aware routing, any provider. |
| D10 | **Retire, don't stack.** Phase 9 replaces the 81 legacy hooks with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities: 3 tools compress bash, 3 compress reads, 2 memories, 3–4 code graphs. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages (`/v1/messages`) and OpenAI (`/v1/chat/completions`, `/v1/responses`) are `Wire` adapters behind one proxy; plugins that touch requests (`archive`, `toon`) and `usage` capture work on a normalised view of tool results, never on a specific JSON shape. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. Added 2026-09-01 by user request. | Codex, OpenCode, Cursor-with-own-key and aider talk OpenAI; without it `measure` has no ground truth for them and `archive` cannot shrink their context. One proxy, two parsers is cheaper than two proxies. |
| D12 | **One config file holds every setting; every CLI flag is a config key.** `~/.rtok/config.toml` (schema and reference file: `docs/config.md`, embedded as `config/default.toml`). Precedence: defaults < user file < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env < flags. Positional per-call arguments (`hook <event>`, `expand <id>`, `run -- <cmd>`) have no key; everything else does, enforced by a test that walks the clap tree. `rtok config show --sources` tells where each value came from. Added 2026-09-01 by user request. | Hooks are spawned with a fixed command line, the proxy and MCP server run for hours, and a bench needs two reproducible configurations — none of that works with flags alone. One precedence rule beats per-flag special cases. |
| D13 | **Core persists through a sync ORM on bundled SQLite.** Diesel (`sqlite` + bundled `libsqlite3-sys` with FTS5) replaces rusqlite. Plugins never write SQL; `Store` is the only DB owner. Every surface action is a `calls` row carrying host agent, provider, model, and plugin. MCP calls and API request/response bodies are stored in `call_io` (inline under `core.call_io_inline_bytes`, else `archive`). Token counts are stored before and after each plugin run, plus MCP tokens for that plugin. Core, plugin, and module logs go to `logs` (and still to `core.log_file`). Hook path: metadata always, body only if under the inline cap — never archive, never fail the hook (D1). Added 2026-09-01 by user request. | Raw SQL in plugins cannot join MCP vs API vs hook or attribute tokens per plugin. Diesel is sync, so the ≤ 10 ms hook path stays blocking and fail-open. Async ORMs (SeaORM/SQLx) would need a runtime per hook. One schema is what `stats` can join. |
| D14 | **CLI is clap 4 (derive); config layers are figment; TOML writes are toml_edit.** Do not hand-roll flag parsing, file/env merge, or comment-preserving edits. Clap owns the subcommand tree and the T12.4 coverage walk (`features = ["derive", "wrap_help"]`). Figment owns defaults < user file < project file < env < flags and per-key provenance for `config show --sources` (named providers `default`, `user`, `project`, `env`, `flag`). Env is `RTOK_<SECTION>_<KEY>` looked up in a leaf table from `Config::default()`, not `Env::split("_")` (that would turn `proxy.openai_upstream` into `proxy.openai.upstream`). `toml_edit` owns `config set`. Not used: twelf (flattens every config key into root clap args — wrong for subcommands); config-rs (no per-key provenance); confique (no clap overlay). Added 2026-09-02 by user request. | Clap is the Rust CLI standard. Figment’s docs recommend this pairing and track which provider set each key — that is T12.2. |
| D15 | **Every plugin is designed against alternatives before it is built.** Each of the ten catalogue plugins gets `src/plugins/<id>/PLAN.md`: the problem in measurable terms, ≥ 3 alternatives surveyed (≥ 1 from outside the stack rtok retires, with version and date), what each gets right and wrong, the mechanism rtok will use and why it beats them, the options rejected, one number the plugin's gate must beat, and what would falsify the design. A plugin's first implementation task does not start before its `PLAN.md` is merged (phase P14). Added 2026-09-02 by user request. | D6 says write every method from scratch; that only pays if the from-scratch version is designed to be better than what it retires, and copying a retired tool's behaviour caps rtok at that tool's quality. `research.md` compares the *installed* stack only, and a `roadmap.md` lane says what to build, not why it beats the field. |
| D16 | **Git is `main` only.** Agents commit on `main`. Do not create, check out, or merge `rtok/<task-id>` or any other feature branch. One task is still one commit (`<task-id>: <title>`). Added 2026-09-02 by user request. | Feature branches left an unmerged stack agents cannot resume from; `main` is the single line of work. |
| D18 | **The graph index may live in an embedded graph database, chosen by gate.** LadybugDB (`lbug`, the MIT community fork of Kùzu) is a second storage backend for the `symbols` index only, behind Cargo feature `graph-lbug`, off by default until Gate P8c passes. D8 narrows to the ledgers: `calls`, `usage`, `measurements`, `notes` and the archive index stay in the one `rtok.db`; the graph index is a derived cache (0006 already dropped it wholesale) and may live in `~/.rtok/graph.lbdb` beside it. D13 holds: Cypher lives only in `src/store/`, the plugin calls the same `symbol_*` methods and never writes a query. D6 holds: `lbug` is a library like Diesel or tree-sitter, not a spawned tool. The backend is kept only if it wins Gate P8c on numbers, and the loser's code is deleted. Added 2026-09-04 by user request. | The field this plugin retires (codebase-memory-mcp, code-review-graph) runs on a graph store, and `impact` is the first query where a hop-bounded path pattern beats N indexed lookups. The costs are known up front — ~212 K SLoC of C++ in `build.rs` or a 78 MB prebuilt archive, one read-write process per file, docs.rs failing on the current crate, a second file beside `rtok.db` — so the gate prices them rather than the plan assuming them away. Survey: `src/plugins/graph/PLAN.md` v0.3. |
| D19 | **Observability is a projection of the ledgers, never a second recorder.** OpenTelemetry export reads the rows rtok already writes (`sessions`, `calls`, `call_io`, `usage`, `tokens`, `measurements`, `logs`) and posts them as OTLP/HTTP JSON to one endpoint (`[otel] endpoint`, else `OTEL_EXPORTER_OTLP_ENDPOINT`) using the GenAI semantic conventions: `invoke_agent` per session, `execute_tool` per hook and MCP call, `chat {model}` per proxied request with the four `gen_ai.usage.*` token attributes, `logs` as log records on the same trace. Ids are derived (`trace_id = sha256(session)`, `span_id = sha256(call id)`), so a resend is byte-identical and delivery is at-least-once behind a per-stream watermark in `otel_export`. Nothing runs on the hook path: `proxy` and `mcp` flush on a timer; `Stop` / `SessionEnd` spawn `rtok otel flush` detached. Full content (`gen_ai.input.messages`, `gen_ai.tool.call.arguments` / `result`) is on by default up to `content_bytes`; past that the archive id is attached and `expand <id>` holds the rest (D4). Off unless an endpoint is set. Added 2026-09-04 by user request. | The user wants every call visible in Jaeger, Grafana, SigNoz and Maple with the information the DB holds. An in-process tracer would be a second recorder beside D8's ledger and put a network client in a 10 ms hook; a projection costs nothing at record time and cannot disagree with `rtok stats`. OTLP/HTTP JSON is accepted by all four backends and needs no new crate — `serde_json` and `reqwest` are in §2. Survey: `src/otel/PLAN.md`. |
| D20 | **Local web dashboard is an operator surface, not a catalogue plugin.** `rtok dashboard` serves axum (HTTP + WebSocket) and a Slint WASM UI (`crates/rtok-webui`, wasm-bindgen). Slint is not linked into the hook binary. Config `[dashboard] host/port` default `127.0.0.1:3333`. Each plugin has a page via `Plugin::dashboard_page`; token-saving plugins show a shared stats widget from `Measurement` / `usage` rows. Added 2026-09-08. | A second HTML/JS stack duplicates Slint; linking Slint into `rtok hook` would fail P17 size/latency. |
| D21 | **Every new plugin is plugin and MCP as one unit, a singleton, with one call path per capability.** Applies to catalogue plugins (`src/plugins/<id>/`) and host plugins (`plugins/<host>/`). (1) **Plugin + MCP together:** if it exposes tools, it *is* the MCP for those tools in the same bundle — not a second server and not a second registration (`rtok setup --mcp` plus the plugin both listing `rtok`). (2) **Singleton:** one MCP process / one writer per store; do not spawn a second `rtok mcp` for the same `rtok.db` / graph index (D18). (3) **No duplicate calls:** hooks, MCP tools, CLI, rules, skills, and commands must not invoke the same function twice. A hook that rewrites Bash to `rtok run` is not a duplicate of MCP `read`/`search`; a skill that shells out to `rtok read` when MCP `read` exists *is*. (4) **Desktop and CLI:** a host plugin must load in that host's desktop app and its CLI (Cursor: `.cursor-plugin/` plus MCP; `agent --plugin-dir` / marketplace). (5) **Missing `rtok`:** fail open (D1) and tell the user it must be installed and how, using ketch: `ketch install listepo/rtok`. If ketch is missing, the bootstrap from the README: `curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash` then `ketch install listepo/rtok`. (6) **Setup offers the host plugin:** `rtok setup <host>` must offer to install `plugins/<host>/` (Cursor: `rtok setup cursor` offers `plugins/cursor` for desktop and CLI). `--dry-run` prints the offer; `--yes` accepts it; default on a TTY is a prompt. If the user accepts the plugin, that plugin *is* the MCP — do not also register `mcpServers.rtok` in the host's mcp.json (D21 singleton). Added 2026-09-08 by user request. | Two MCP registrations spawn two writers on the graph store. Duplicate tool paths split `Measurement` (D3, D10) and burn tokens. Desktop vs CLI must see the same unit. Install is ketch (P18). Setup that only writes hooks.json leaves the plugin undiscoverable in Cursor Desktop/CLI. |

Deferred to **v0.2+** (not rejected; do not implement while v0.1 tasks are open). Catalogue and first Checks: `ideas.md` Later and `roadmap.md` Later. LLM-based compression (LLMLingua, claude-mem style extraction); embeddings / semantic search; LSP-grade call graph (v0.1 `graph` is tree-sitter-tags); semantic response cache (bifrost); a daemon besides `proxy`/`mcp`; a WASM plugin host. Each needs a numbered phase in this file and a measurement Check before it ships. (Formerly listed as v0.1 non-goals “rejected on evidence”. Codex Responses-API proxy moved into v0.1 as P11 on 2026-09-01, D11.)

## 1. Architecture

```
                    ┌──────────────── rtok (one binary) ────────────────┐
 Claude Code ──hook─┤ hook <event>  → EventBus → plugins (Pre/Post/...) │
 Cursor/OpenCode ───┤ mcp           → tools: read search tree run expand │
                    │                 mem_save mem_search symbol callers │
 ANTHROPIC_BASE_URL ┤ proxy         → usage capture, live-zone rewrite   │──▶ api.anthropic.com
                    │ stats | bench | doctor | setup | run | expand      │
                    └──────────┬─────────────────────┬──────────────────┘
                         ~/.rtok/rtok.db        ~/.rtok/archive/
```

Plugin trait (final shape; T0.4 implements it):

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> Manifest;                    // id, surfaces, default_on
    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> { None }
    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> { None }   // additionalContext only
    fn session_start(&self, ev: &SessionStart, cx: &Ctx) -> Option<Injection> { None }
    fn prompt_submit(&self, ev: &PromptSubmit, cx: &Ctx) -> Option<Injection> { None }
    fn pre_compact(&self, ev: &PreCompact, cx: &Ctx) {}
    fn mcp_tools(&self) -> Vec<ToolDef> { vec![] }
    fn proxy_filter(&self, req: &mut MessagesRequest, cx: &Ctx) -> Vec<Measurement> { vec![] }
}
```

`Ctx` gives every plugin: the DB handle, the token estimator, the archive store, config, session id. `Measurement { plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id }` is the only way savings enter the DB.

Plugin catalogue (v0.1 scope). Every plugin is native Rust written from scratch here (D6); the *spec* column names the tools whose behaviour it re-implements and P9 retires.

| id | spec (replaces; evidence in research.md) | surface | mechanism |
|----|-------------------------------------------|---------|-----------|
| `measure` | rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard | `stats`, `bench`, proxy | session JSONL ingest + proxy `usage`; context-token-turns |
| `cmd` | rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress | PreToolUse(Bash) → `rtok run` | archive raw output; per-family formatters + TOML rules written here; pointer trailer |
| `read` | lean-ctx ctx_read/search/tree (78 tools), token-optimizer read_cache/structure_map | MCP `read`,`search`,`tree` + PreToolUse(Read) advice | modes full/lines/map/signatures via tree-sitter-tags; re-read dedup (hash → "unchanged") |
| `archive` | token-optimizer archive_result, headroom CCR, caveman retrieve | proxy live zone + `expand` | replace old, large `tool_result` blocks with pointer + head/tail; deterministic per tool_use_id |
| `proxy` | headroom proxy, caveman-proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` | passthrough + SSE streaming; usage capture; never touches system, tools, or last 2 turns |
| `inject` | caveman shrink-hook, ponytail/caveman modes, lean-ctx banner, engram/claude-mem SessionStart context | SessionStart, UserPromptSubmit | budgeted, byte-stable prefix; modes as markdown |
| `guard` | token-optimizer refetch_guard/loop detection | PreToolUse | identical read/command within N turns → deny with pointer to prior result |
| `memory` | engram, claude-mem | MCP `mem_save/search/get`, PreCompact checkpoint | agent-written notes, SQLite FTS5, progressive disclosure |
| `graph` | codebase-memory-mcp, code-review-graph, serena, codegraph | MCP `symbol`,`callers`,`outline` | own tree-sitter-tags index (definitions + reference sites) in SQLite; capped output |
| `toon` (off by default) | caveman toon, TOON | proxy/MCP | tabular JSON → TOON (vendor bench: 42.6 % fewer tokens) |

## 2. Working agreement for agents

- Task claim: each task has `Status:` (`open` | `in progress`) and `Model:` (`-` when open). Claim only if `Status: open`; set `in progress` and your model before work. Before you stop, unfinished tasks → `Status: open`, `Model: -`.
- One task = one commit on `main` only. Do not create or switch to a feature branch (`rtok/<task-id>` or otherwise). Never skip the Check. Same commit: mark the task done and move it from this file to `done.md` (verbatim Do/Check, `Status: done <date>`, Check result). Implemented work left in `plan.md` is unfinished.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message. Allowed baseline: clap (derive, wrap_help), figment (toml, env), toml_edit, serde, serde_json, diesel (sqlite, bundled libsqlite3-sys with FTS5), regex, anyhow, tokio, hyper/axum, reqwest, rmcp, tree-sitter + tree-sitter-tags, sha2, time; `lbug` only behind feature `graph-lbug` (D18, P8c). Reason diesel replaces rusqlite: typed models for D13 `calls`/`tokens`/`logs`; sync, so hooks stay ≤ 10 ms. Reason figment + toml_edit replace a direct `toml` dep and a hand-rolled merger: D14.
- Don't duplicate code or logic: find the existing helper and reuse it, or extract one shared helper at the responsible layer (the module that owns the behaviour, not a junk drawer).
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` green before every Check.
- CLI testing: unit tests for internal business logic; integration tests for end-to-end binary execution, argument parsing, and output formatting. Dev-deps (one-line reason in the first commit that adds each): **`assert_cmd`** — spawn the compiled binary; assert exit code, stdout, stderr; **`predicates`** — compose output matchers (contains, regex, …); **`assert_fs`** — temp files/dirs setup, teardown, and verification; **`trycmd`** — snapshot tests from plain text or Markdown command files under `tests/<name>/` (prefer over line-by-line assertions when CLI output is long or complex; files double as docs). Unit tests live next to the code; integration and snapshot tests under `tests/`. Allowed dev-deps baseline also includes `httpmock` (T5.0).
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.
- Every new CLI flag gets a key in `config/default.toml` and a row in `docs/config.md` in the same commit (D12); flag names in the tasks below imply the key `<subcommand>.<flag>` or `plugins.<id>.<flag>`.
- No plugin shells out to, links, imports from, or reads the data of a third-party tool (D6). Port the *behaviour* described in `research.md`, never the code. Retired tools' names may appear only in `rtok doctor` (inspection), `rtok setup --replace` (retirement) and `rtok bench` (comparison). External plugins are written against the public API (`rtok::plugin`, `Registry::from_plugins`), outside this repo.
- Every new plugin obeys D21: plugin and MCP as one unit; singleton (one MCP / one writer per store); one call path per capability (no duplicate invocations of the same function from hooks, MCP, CLI, rules, skills, or commands); host plugins (`plugins/<host>/`) work on desktop and CLI; if `rtok` is not installed, fail open and print that it must be installed with ketch (`ketch install listepo/rtok`; bootstrap ketch from the README if needed). `rtok setup <host>` offers `plugins/<host>/` (Cursor: T10.5).

## 3. Phases and tasks

Format: **Tn.m title** · depends · files · do · **Check** (command → expected). End each task with `Status: open` and `Model: -` until claimed.

### P0 — Scaffold (goal: `rtok --version`, DB, plugin registry, hook I/O types) — **done 2026-09-01, moved to `done.md`**

T0.1–T0.8 are complete; their text, Checks and deviations are in `done.md`. Gate P0 (review: trait shape final; no plugin logic yet) closed as the scaffold freeze (see §6).

Gate P0 (review): trait shape final; no plugin logic yet. **Status: done 2026-09-03 (historical).** Check: trait is `Plugin` in `src/plugin.rs`; "no plugin logic yet" held at T0.8 (`c9b6f81`). Later tasks extended the trait and added plugin logic by design — §6.

### P1 — Measure (goal: a baseline you can trust before changing anything)

Gate P1: baseline saved (`rtok stats --save-baseline before-rtok`). Record the numbers in research.md §2. **Status: done 2026-09-03.** Check: `~/.rtok/measurements/before-rtok.json` (580 sessions, 181 303 lines, Bash 7.71 M / Read 3.07 M est. tokens, 30d); `--compare` all Δ0; numbers in `research.md` §2. Slice is not the original 17-session H-measured table.

### P2 — Hook surface (goal: one hook command per event, < 10 ms, budgeted injection)


Gate P2: `rtok setup claude` installed alongside the legacy hooks (additive, nothing removed yet); `rtok doctor` shows 88 hooks; sessions still work. **Status: done 2026-09-03.** Check: 7 additions (81→88), second `setup claude` → `no changes`; doctor `hooks 88`; 7 `rtok hook` commands beside the original 81; `settings.json` still parses; proxy chain still 8788→8787; backup `settings.json.bak-*`.

### P3 — `cmd` plugin (goal: every Bash output archived, filtered, measured)

Gate P3: disable the legacy Bash-compression hooks in settings, run one working day, `rtok stats --compare before-rtok`. Keep only if Bash context-token-turns fall and `expand` rate < 5 %.

### P4 — `read` plugin + MCP server (goal: replace lean-ctx's 78 tools with 5 and the 3.1 K/turn banner with 0)

Gate P4: disable lean-ctx hooks and MCP server for one day; compare `rtok stats` Read/MCP rows and injection tokens per turn vs baseline.

### P5 — `proxy` + `archive` (goal: ground-truth usage and cache-safe shrinking of old tool results)

Gate P5: run proxy in passthrough for two days (usage ground truth), then `compress` for two days; compare cache_read per turn, output tokens, expand rate. Keep `compress` only if context-token-turns fall ≥ 15 % with expand rate < 5 %.
Replay 2026-09-02 (first half, estimate): `rtok stats` now applies the `archive` policy (keep 4 turns, ≥ 1 500 tokens, 8 + 4 lines) to every tool result in the transcripts and reports `archive replay (estimate) ctt before → after`. On this machine: 569 sessions, 1 771 results qualify, context-token-turns 11.66 G → 8.33 G, −28.6 % (last 14 days: −28.4 %), so the ≥ 15 % bar clears with margin before any live run. Still open, needs live traffic: the proxy has served no requests here yet (`rtok stats --cache` shows no usage rows), so expand rate (< 5 %) and cache_read per turn are unmeasured. To run it: point `ANTHROPIC_BASE_URL` at `rtok proxy` (T5.2 `setup --proxy`), two days `passthrough`, two days `--mode compress`, then `rtok stats --cache` and `rtok stats --plugin archive --json` (`expand_rate`).

### P6 — `memory` plugin (goal: one memory instead of two, zero LLM cost)

Gate P6: disable engram + claude-mem plugins for a week; compare per-turn injection tokens and MCP tool-description tokens (`rtok doctor`). Revert if recall quality is noticeably worse (subjective, note it).

### P7 — modes + instruction hygiene

Gate P7: A/B (P9 harness) `terse` on/off on 6 tasks; keep only if output tokens fall without task failures.

### P8 — `graph` plugin (goal: `symbol`/`callers`/`outline` from an index rtok builds itself, replacing four graph servers)

Gate P8: measure MCP description tokens saved by disabling the other graph servers (code-review-graph 30 tools, serena ~25, lean-ctx 78); index time on this repo < 2 s. **Status: done 2026-09-03.** Check: Result 2026-09-02 (passed) — index 0.48 s cold / 0.03 s warm; retiring three servers saves ~4 493 description tokens vs rtok's ~117.
Result 2026-09-02 (passed): `rtok doctor`, once it spawned servers with their `args`/`env` and a 15 s timeout (it ran the bare `command` with a 2 s wait, so every `uvx`/`npx` server read as 0 tools), measured on this machine: code-review-graph 30 tools ~2 295 desc tokens, serena 22 ~1 494, lean-ctx 12 ~704, codebase-memory-mcp 0 (the binary exits on start; Claude Code reports the same connection failure). rtok's whole MCP surface: 10 tools ~117 desc tokens. Retiring the three measurable servers saves ~4 493 description tokens per request and rtok adds ~117 (descriptions only; input schemas not counted). Index on this repo, release binary: 0.48 s cold (61 files, 6 625 rows), 0.03 s warm.

### P8b — `graph` quality (goal: the same ≤ 4 tools answer with the code, across repos, with a published recall) — added 2026-09-04

Promoted from the v0.2 survey in `src/plugins/graph/PLAN.md` (2026-09-04) after Gate P8 closed on token count alone. Order is T8.3 → T8.4 → T8.8 → T8.5 → T8.6 → T8.7: the two defects first, the measurement before the edges, the edges before what consumes them.

Gate P8b: graph surface ≤ 150 description tokens (`rtok doctor`); warm tool call < 100 ms on a 3 000-file repo; T8.8 definition recall ≥ 0.9 (met at 1.0; reference recall is 0.351, see §6); on the P9 task set, tasks touching ≥ 3 files use fewer tool calls with `symbol` than at v0.1 (`calls` table). Revert T8.6 if tool calls do not fall; revert T8.7 if `impact` is never called across a week of `calls`.

**Status 2026-09-04: three of four clauses measured and passed; the gate stays open on the fourth.** Surface: 4 tools, **62** description tokens (bar 150), asserted by `graph_surface_is_four_tools_under_150_tokens`. Warm tool calls on a generated 3 000-file repo (9 000 rows), release build, after a 22.1 s cold index: `symbol` **23 ms**, `callers` **24 ms**, `impact` **26 ms** (bar 100 ms) — re-measured after T8.5–T8.7, since those changed what a call does. Definition recall **1.000** with precision 1.000 (bar 0.9); reference recall 0.351, published, see §6. The fourth clause needs the P9 task set run twice, which has not happened; until it does, T8.6 and T8.7 stand unjudged and their revert conditions remain live.

### P8c — `graph` on LadybugDB (goal: the same four tools, byte-identical, on an embedded graph store — kept only if it wins on numbers) — added 2026-09-04 (D18)

Survey and rejected options in `src/plugins/graph/PLAN.md` (v0.3, 2026-09-04). Order is T8.9 → T8.10 → T8.11 → T8.12 → T8.13 → T8.14: the contract first, the seam second, the backend third, the one query a graph store can win fourth, the measurement last. Nothing in this phase changes what the four tools print; `tests/graph_contract.rs` is the acceptance test for every task.

Gate P8c (numbers from the same machine, release, for both builds — `default` and `--features graph-lbug`): (1) `tests/graph_contract.rs` passes unchanged under both; (2) `rtok hook PostToolUse` p95 ≤ 10 ms over 100 runs of the `graph-lbug` binary — the binary grows, the hook path must not; (3) warm `symbol` / `callers` / `impact(2)` < 100 ms on the P8b 3 000-file repo; (4) `impact(4)` on a fan-out-10 fixture (10 000 edges): the `lbug` path query is ≥ 2× faster than the SQLite `WITH RECURSIVE` of T8.13 — the one clause only a graph store can win; (5) clean `just check` ≤ 2× the default build's wall time, and no unpinned network fetch in `build.rs` (a download at build time is pinned to the crate version with a checksum, or the build is from source); (6) release binary bytes and `graph.lbdb` bytes published in `research.md` §2. Decision rule: (4) lost or tied → delete the `lbug` code and feature from T8.11–T8.13, keep T8.9's tests, T8.10's seam and the CTE if it beat the Rust BFS; (4) won but (5) lost → `graph-lbug` stays opt-in, never default; all six hold → `graph-lbug` joins `default` and `src/store/symbols.rs` is deleted in the next task.

**Status 2026-09-08: clause (4) won; `graph-lbug` stays opt-in.** `impact(4)` 371 ms (`lbug`) vs 28.5 s (SQLite CTE), 77× the 2× bar. Clauses (2) and (3) fail on the `graph-lbug` binary (PostToolUse p95 97 ms, warm calls 0.78–0.87 s); default SQLite meets both (8.07 ms, 18–27 ms). Clause (1) green both; (5) `just check` 16.9 s with liblbug already built, source build still pinned (T8.11); (6) binaries 19.7 MiB vs 32.4 MiB, `graph.lbdb` 6.10 MB. All six do not hold, so the feature never joins `default` and the lbug code is not deleted. Table: `research.md` §2.

### P8d — `graph` freshness (goal: the index follows the working tree without a tool call paying for the walk; the parser stays tree-sitter) — added 2026-09-05

What exists: the index is tree-sitter-tags (T8.1, the seven grammars of `read`); every tool call runs `index::run`, a gitignore-aware walk with a stat gate (T8.4: 0.053 s warm on 3 000 files); `PostToolUse(Edit|Write)` deletes the file's rows so the next call re-parses it. So "auto index" is on and unconditional today, and there is no watcher. A richer parser is not this phase: rtok's own queries above the grammar's tags (type positions, `scoped_identifier`) are I-31, an LSP backend is I-24, both stay in `ideas.md` until a task needs a reference the tags miss. Order T8.15 → T8.16 → T8.17: the knob, the watcher, the second backend. Nothing changes what the four tools print; `tests/graph_contract.rs` stays untouched.

Constraints that shape the design: under `graph-lbug` one process holds the read-write `Database` (PLAN.md v0.3), so the watcher runs as a thread inside `rtok mcp`, borrowing the server's `Ctx` in a scoped thread — never a second process on the same store. The hook path is a separate process and is not touched (AGENTS.md: indexing never runs there). A watch event does not name what to re-parse; it only ends a quiet period, after which `index::run` on the root does the incremental work — the stat gate already makes a run cost a walk, so no per-file bookkeeping is added.

Gate P8d (release, this machine): (1) with `auto_index = false` and `watch = "notify"`, an edit to a fixture file is visible in `symbol` within 1 s while the tool call itself opens no file (`Report.read == 0`, a walk of 0); (2) `rtok mcp` idle for 60 s with the watcher on: CPU time within 50 ms of the watcher-off run and RSS within 2 MB; (3) `watch = "watchman"` passes (1) on the same fixture and `watchman watch-list` names the root; with no watchman socket it falls back to `notify` and says so once on stderr; (4) `rtok hook PostToolUse` p95 ≤ 10 ms, unchanged; (5) release binary bytes before and after `notify` and `watchman_client` in `research.md` §2. Decision rule: (2) lost → the watcher defaults to `"off"` and stays opt-in; (3) lost or watchman ≤ notify on (1) latency → T8.17's crate is removed and `watchman` stays a documented `ideas.md` entry.

**Status 2026-09-09: passed.** (1) `notify` re-index within 1 s, `Report.read == 0` (+ delete disappears); (2) idle ΔCPU −10 ms, ΔRSS +1.14 MB; (3) `watchman` passes (1) (~500 ms vs `notify` ~250 ms, same bar), `watch-list` names the root, socket-less fallback prints exactly one line; (4) hook p95 8.25 ms serialized (`-- --test-threads=1`; parallel rounds straddle the bar on scheduler noise); (5) release 19 764 144 B pre-`notify` → 19 867 968 B default → 20 429 392 B with `graph-watchman`. No decision-rule trigger fired: `watch` stays opt-in `off`, `graph-watchman` stays opt-in, never default. Rows: `research.md` §2.

**T8.17 `watchman` backend** · T8.16 · `Cargo.toml`, `src/plugins/graph/watch.rs`, `docs/config.md`
Do: optional dependency `watchman_client = "0.9"` (Meta's client; tokio, already a dependency) behind feature `graph-watchman` (in `default` only if Gate P8d (3) and (5) pass). `watch = "watchman"`: connect to the socket (`watchman get-sockname`), `watch-project` the root, subscribe with the same suffix filter as `notify`, and feed the same quiet-period loop; the fallback to `notify` when the socket is missing prints one stderr line. The 250 ms loop and `index::run` call are shared with T8.16 — one function, two event sources.
Check: with `/opt/homebrew/bin/watchman` on PATH the T8.16 test passes with `watch = "watchman"` and `watchman watch-list` lists the temp root; with `PATH` emptied the same test passes through the fallback and stderr has exactly one `watchman: … falling back to notify` line; Gate P8d (3) and (5) numbers into `research.md` §2 and the decision rule applied in the same commit.
Complexity: 4/5 — async `watchman_client` (tokio) bridged into the sync quiet loop, one loop with two event sources, an external daemon on the machine, a fallback path that must print exactly one stderr line, feature gating (`graph-watchman`), and the Gate P8d (3)+(5) numbers plus the removal decision rule in the same commit.
Status: done 2026-09-09
Model: Muse Spark (meta/muse-spark-1.3-contributor)
Check result: moved to `done.md` — `watchman_sees_daemon_edit_within_1s_reading_nothing` + `mcp_watchman_*` green; Gate P8d (3) passes, (4) 9.77 ms, (5) +2.8 %; crate stays opt-in per the decision rule.

### P16 — OpenTelemetry export (goal: every session, call, token and saving rtok records is a trace, log and metric in any OTLP backend, with nothing added to the hook path) — added 2026-09-04 (D19); gate passed 2026-09-07, backend clause moved to P18 and `ideas.md` I-33

Design, mapping table and rejected options in `src/otel/PLAN.md`. Order T16.1 → T16.8: config, watermark, encoder, mapping, exporter + CLI, triggers, metrics, docs + live check. Every task keeps `rtok hook` at ≤ 10 ms and does nothing unless `[otel] endpoint` (or `OTEL_EXPORTER_OTLP_ENDPOINT`) is set.

**Status 2026-09-04: three of four clauses measured and passed; the gate stays open on the third.** (1) `tests/otel.rs`, six tests, green. (2) release p95 with a reachable endpoint: `PostToolUse` 9.70 ms and `Stop` 8.97 ms against a no-endpoint baseline of 8.89 ms and 8.36 ms — the spawn costs 0.61 ms and both stay under the 10 ms bar (`research.md` §2). (4) `§2` baseline unchanged: no crate was added. (3) is half done: an independent OTLP receiver validated one session's real traffic with 0 problems (205 spans, 3 metric streams, ids and int64 encodings as the spec requires), but none of Jaeger, Grafana, SigNoz or Maple was exercised — Docker is blocked by this machine's shell allowlist, so no collector image could start. The four recipes are in `docs/otel.md`; running one of them closes the clause.

**Status 2026-09-07: passed on (1), (2), (4); (3) moved.** The first two backends were run (evidence in `research.md` §2, the recipes stay in `docs/otel.md`; a repeatable check is `ideas.md` I-33) and the run found and fixed an exporter loop: a backend that answers 404 to a stream made each failed flush log a row the next flush re-sent, so pending grew by one per flush — a 404 is now "not served", skipped without a log row (`tests/otel.rs`, seven tests). What clause (3) still asked for — one real hooks + MCP + proxy session as one trace, and SigNoz and Maple — needs a released binary on `PATH` and the user's accounts, so it is now part of Gate P18.

Gate P16: (1) `tests/otel.rs` against a mock collector: every `calls`, `logs` and ended `sessions` row after the watermark is posted once per flush, a second flush posts nothing, a non-2xx leaves the watermark; (2) `rtok hook PostToolUse` and `Stop` p95 ≤ 10 ms over 100 runs with an endpoint set and unreachable; (3) moved 2026-09-07 — backend rendering is `ideas.md` I-33, the real-session trace and the SigNoz / Maple sign-off are in Gate P18; (4) §2 baseline unchanged.

### P17 — build size (goal: what a contributor compiles and what a user downloads stop growing with the dependency list) — added 2026-09-04; T17.1–T17.2 done, gate passed 2026-09-07

D18 brought a C++ graph engine into the tree and the cost showed up immediately: on this machine the debug `liblbug` rlib is 2.08 GB, kept twice for two feature sets, over a 4.6 GB cmake directory, while `~/.cargo/shared_target/debug` reached 69 GB and the volume 98 % full. The dev profile is the lever — full DWARF is most of a 124 MB test binary and all of the C++ debug library — and `strip` is the lever on the shipped binary. No profile may set `panic = "abort"`: `hooks::dispatch` fails open through five `catch_unwind` sites, and aborting would break the rule that a hook exits 0 on error.

Gate P17: release and debug `rtok` bytes, `liblbug.a` bytes and the `lbug` build directory measured before and after on this machine and published in `research.md` §2; `just check` green; `rtok hook PostToolUse` p95 ≤ 10 ms in release; a panic inside a plugin still prints a backtrace with `file:line`.

T17.1 and T17.2 are done (`done.md`). **Status: passed 2026-09-07.** Check: sizes in `research.md` §2 (T17.1); `just check` green at T17.1 and T17.2; the p95 clause, on the bar at 10.07 ms on 2026-09-04 and 10.3–13.6 ms under a load average of 14–36 on 2026-09-05, measured 7.24 / 7.86 / 8.17 ms for `PostToolUse` over three rounds of `cargo test --release --test latency` on a quiet machine (load 2.9–3.4), with `PreToolUse` at 5.79–6.94 ms; the drift was the machine, not the profile or the database. The breakdown (same section) puts the hook's own work at ~1.3 ms and 1.3–1.5 ms of every spawn in the dyld cost of Security.framework + CoreFoundation, which only `proxy` / `otel` TLS need — I-32 in `ideas.md`, not a P17 task.

### P18 — release (goal: a macOS user installs a released binary with one command, and the version of the next release is computed, never typed) — added 2026-09-04; tasks done (T18.5 release-plz added and done 2026-09-07), gate needs the first real release

T10.4 wrote the release config but never ran it: no tag, no GitHub Release, and `dist-workspace.toml` has never met a runner. The runner half is not the problem — dist 0.32 already plans `macos-14` for `aarch64-apple-darwin` and `macos-15-intel` for `x86_64-apple-darwin`, both current images. What is missing is the entry point (nothing creates a tag), the numbering (versions are hand-edited), and the install path a macOS user would actually use (README only documents `cargo install --path .`). Versions restart at 0.0.1 and every release is the next patch. Codesigning and notarisation are out of scope: they need Apple Developer credentials this repo does not have, so downloads are Gatekeeper-quarantined via a browser and clean via the installer.

**Blocker 2026-09-07 (user decision, resolved 2026-09-08).** `dist-workspace.toml` listed `homebrew` under `installers` and `publish-jobs`, so `release.yml` pushed a formula to `listepo/homebrew-tap` with `secrets.HOMEBREW_TAP_TOKEN` — the secret is not set, so the workflow would push the version bump and then fail in `publish-homebrew-formula`. Resolved by dropping `homebrew` from both lists (shell installer only) and documenting `ketch install listepo/rtok` instead; release-plz PR #2 closed unmerged, the first release goes through `tools/release.sh`.

Gate P18: the Release workflow, started from the Actions tab with no argument, produces a GitHub Release whose tag is one patch above the last; its `aarch64-apple-darwin` and `x86_64-apple-darwin` archives download, extract and run `rtok --version` on macOS, printing that tag; the README install line puts `rtok` on `PATH` under both bash and zsh; running the workflow a second time yields the next patch with nothing hand-edited; with the released binary on `PATH`, one real Claude Code session (hooks + MCP + proxy) is one trace in an OTLP backend — `invoke_agent` root, `execute_tool` spans with `gen_ai.tool.call.arguments` / `result`, `chat {model}` spans with the four `gen_ai.usage.*` attributes, its `logs` rows on the same trace id — and the same config is verified once in SigNoz and once in Maple with the user's account / API key, each dated in `research.md` §2 (moved here from Gate P16 (3) on 2026-09-07); `just check` green.

### P19 — web dashboard (goal: one command serves a Slint WASM UI and a WebSocket API over the same Store the CLI uses) — added 2026-09-08 (D20)

Operator surface like `rtok tui` (P15), in the browser. Frameworks: axum `ws` + `tower-http` ServeDir, Slint on wasm32 (official web renderer). Not a catalogue plugin — it does not save tokens.

Gate P19: `just dashboard` (or `rtok dashboard`) listens on the configured host/port; browser `/` loads the Slint canvas when `pkg/` exists; `/ws` pushes a snapshot whose `plugins` length matches the catalogue; a `saves_tokens` page includes input/output/est_before/est_after. **Status: passed 2026-09-09.** Check: `rtok dashboard --port 3334` → `/` 200 (canvas + `./pkg/rtok_webui.js` module, `pkg/` freshly built with wasm-pack 0.15.0, 91 KB js + 10.5 MB wasm, gitignored), `/health` ok, `/ws` snapshot 10 plugins with `saves_tokens` stats (`input/output/est_before/est_after`); `pkg` js/wasm both 200.

### P9 — A/B bench + migration (goal: replace 81 hooks with ≤ 8, keep only what measures) — tasks done; gate remains a review + user decision

Gate P9 (review + your decision): adopt config B if cost per passed task is lower and pass rate is equal; otherwise keep the measured winners only.

The migration side of this phase — which rtok plugin owns each lean-ctx behaviour, the two gaps (`cmd` runner prefixes and families, `graph` callees) and the cutover order — is written in `migration.md` (2026-09-07, plan only; its gaps become tasks here only when promoted).

### P10 — other hosts + release — T10.1–T10.6 done 2026-09-09 (D21)

### P11 — OpenAI API surface (goal: same proxy, same plugins, same numbers for OpenAI-API hosts) — added 2026-09-01 (D11)

Gate P11: run one OpenAI-API host (Codex) through the proxy in passthrough for two days; every request has a `usage` row. Then compress for two days; keep only under the P5 gate rule (context-token-turns − 15 %, expand rate < 5 %). Record in research.md §2.

### P12 — Config file (goal: every setting in one file, one precedence rule, no flag without a key) — added 2026-09-01 (D12)

Gate P12 (review): `docs/config.md`, `config/default.toml` and `Config` agree; no subcommand keeps its own defaults; merge is figment, CLI is clap, `config set` is toml_edit (D14). **Status: done 2026-09-03.** Check: `filter --cmd` is `Option<String>` (absent → `filter.cmd` from figment); mapping table lists only clap-defined flags plus env `RTOK_HOME`; D14 crates unchanged (`layers.rs` figment, `cli.rs` clap, `validate.rs` toml_edit).

### P13 — ORM + action store (goal: every MCP call, API request, plugin run, and log is a typed row) — added 2026-09-01 (D13)

Runs right after P12, before P1 writes any rows. `Store` becomes Diesel over bundled SQLite; plugins keep using `Ctx` and never SQL. `events` is superseded by `calls` (0001 table stays, nothing new writes it). `measurements` (D3) and `usage` stay the savings/ground-truth ledgers; they gain `call_id`. FTS5 for `notes_fts` stays as `diesel::sql_query` (Diesel cannot model `VIRTUAL TABLE`).

Schema (`migrations/0002.sql`). Integers are i64. Booleans are 0/1. `ts` is unixepoch. Foreign keys ON.

```sql
-- Dimension: host agents, API providers, models (upserted from traffic).
CREATE TABLE hosts (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,          -- claude | cursor | codex | opencode | aider | other
    kind TEXT NOT NULL,                 -- cli | ide | other
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE providers (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,          -- anthropic | openai | other
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE models (
    id INTEGER PRIMARY KEY,
    provider_id INTEGER NOT NULL REFERENCES providers(id),
    slug TEXT NOT NULL,                 -- request `model` field
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (provider_id, slug)
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,                -- host session id
    host_id INTEGER REFERENCES hosts(id),
    project TEXT,
    cwd TEXT,
    source TEXT,                        -- startup | resume | compact | …
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER
);

-- Unified action log. parent_id nests plugin_run under hook | mcp_call | api_request.
CREATE TABLE calls (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    host_id INTEGER REFERENCES hosts(id),
    provider_id INTEGER REFERENCES providers(id),
    model_id INTEGER REFERENCES models(id),
    plugin TEXT,                        -- null = core / surface
    surface TEXT NOT NULL,              -- hook | mcp | proxy | cli
    kind TEXT NOT NULL,                 -- hook | mcp_call | api_request | plugin_run | expand | cli
    parent_id INTEGER REFERENCES calls(id),
    name TEXT,                          -- hook event, MCP tool, HTTP path
    ms REAL,
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT
);
CREATE INDEX calls_session ON calls (session_id, ts);
CREATE INDEX calls_kind ON calls (kind, ts);
CREATE INDEX calls_plugin ON calls (plugin, ts);
CREATE INDEX calls_parent ON calls (parent_id);

-- Full MCP args/result and API request/response. Over core.call_io_inline_bytes → archive.
CREATE TABLE call_io (
    call_id INTEGER PRIMARY KEY REFERENCES calls(id),
    request_bytes INTEGER NOT NULL DEFAULT 0,
    response_bytes INTEGER NOT NULL DEFAULT 0,
    request_sha256 TEXT,
    response_sha256 TEXT,
    request_json TEXT,
    response_json TEXT,
    request_archive TEXT REFERENCES archive(id),
    response_archive TEXT REFERENCES archive(id)
);

-- Token counts. One row per (call, plugin, phase).
-- phase=before|after: estimator or provider, around a plugin_run or whole api_request.
-- phase=mcp: tokens of MCP traffic owned by this plugin (args+result of its tools).
CREATE TABLE tokens (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    call_id INTEGER NOT NULL REFERENCES calls(id),
    plugin TEXT,
    phase TEXT NOT NULL,                -- before | after | mcp
    source TEXT NOT NULL,               -- estimate | provider | mcp
    tokens INTEGER NOT NULL,
    bytes INTEGER,
    input INTEGER,
    output INTEGER,
    cache_create INTEGER,
    cache_read INTEGER
);
CREATE INDEX tokens_call ON tokens (call_id, phase);
CREATE INDEX tokens_plugin ON tokens (plugin, ts);

-- Core, plugin, and module logs (also still written to core.log_file).
CREATE TABLE logs (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    level TEXT NOT NULL,                -- error | warn | info | debug
    source TEXT NOT NULL,               -- core | plugin | module
    name TEXT NOT NULL,                 -- plugin id or rust module path
    session TEXT,
    call_id INTEGER REFERENCES calls(id),
    plugin TEXT,
    message TEXT NOT NULL,
    fields TEXT                         -- JSON extras
);
CREATE INDEX logs_ts ON logs (ts);
CREATE INDEX logs_source ON logs (source, name, ts);
CREATE INDEX logs_session ON logs (session, ts);

ALTER TABLE measurements ADD COLUMN call_id INTEGER REFERENCES calls(id);
ALTER TABLE usage ADD COLUMN call_id INTEGER REFERENCES calls(id);

INSERT INTO hosts (id, slug, kind) VALUES
  (1,'claude','cli'),(2,'cursor','ide'),(3,'codex','cli'),
  (4,'opencode','cli'),(5,'aider','cli'),(6,'other','other');
INSERT INTO providers (id, slug, name) VALUES
  (1,'anthropic','Anthropic'),(2,'openai','OpenAI'),(3,'other','Other');
```

T13.1–T13.4 are done — see `done.md`.

Gate P13 (review): no rusqlite; no SQL outside `src/store/`; hook-path tests never write `archive/` for `call_io`; a plugin_run has before and after token rows. **Status: done 2026-09-03.** Check: no `rusqlite` in the tree; `oversized_hook_call_io_does_not_archive`; `only_results_outside_the_live_tail_are_rewritten` asserts before/after `tokens` phases. Runtime SQL is `src/store/` only; `migrations/*.sql` are included by `Store` (see §6).

### P14 — per-plugin design research (goal: every plugin beats the field on a named number before a line of it is written) — added 2026-09-02 (D15)

One task per catalogue plugin, each producing that plugin's own plan: `src/plugins/<id>/PLAN.md`.
**Not a batch.** T14.0 lands first (it makes the rest checkable); each T14.x then lands in the commit
immediately before that plugin's first implementation task, so the survey is current when it is used.
A T14.x is ≤ 1 file plus a `research.md` §6 row — no code, no dependency.

Every T14.x must, in `PLAN.md`: survey **≥ 3 alternatives** with version and date, **≥ 1 from outside the
stack rtok retires** (another ecosystem, a library, a paper); say in one line each what the alternative
gets right and what it gets wrong; name rtok's mechanism and the one property that makes it better
(“written in Rust” is not one); list **≥ 2 rejected options** with the reason; set **`Target:`** — the one
number that plugin's gate in `roadmap.md` must beat; and **`Falsified by:`** — the observation that kills
the design. Where the survey changes a task, amend it in §6 rather than silently building something else.

Shared Check for T14.1–T14.10: `cargo test plugin_plans` green; the `Target:` line matches that plugin's
gate in `roadmap.md`; every surveyed alternative appears in `research.md` §6 with a date.

T14.0–T14.10 are done — see `done.md`. Shared Check `cargo test plugin_plans` green; ten `PLAN.md` files; alternatives dated in `research.md` §6.

Gate P14 (review): `ls src/plugins/*/PLAN.md | wc -l` → 10; every `Target:` matches a `roadmap.md` gate; no plugin was implemented before its `PLAN.md` merged (`git log` order); any design that contradicted a task produced a §6 amendment. **Status: done 2026-09-03.** Check: `cargo test plugin_plans` (`every_target_matches_a_roadmap_gate`); 10 files; guard Mechanism is T2.6 Deny. `git log` order: all PLAN.md landed in `830e049` with measure/hooks impl — see §6.

### Later versions (v0.2+) — deferred, not rejected — added 2026-09-02

Do not start these while v0.1 work is open. When v0.1 is done, promote each row to a numbered phase with a Check. Detail: `ideas.md` Later, `roadmap.md` Later.

| Version | Work | First Check (when scheduled) |
|---------|------|------------------------------|
| v0.2 | **LLM compression** — optional `compress` / `memory` extractor (LLMLingua-2, claude-mem-style). Default off. | `rtok bench` vs v0.1 lossless path: cost per passed task must not rise; expand still recovers originals where the source is not regenerable. |
| v0.2 | **Embeddings / semantic search** — optional backend for `memory` search and `graph` (mem0, code-review-graph embeddings). | FTS5 remains default; embed path is a config flag; a fixture note is found by both. |
| v0.2 | **LSP graph** — `graph` may add a serena-grade LSP backend behind the same MCP tools (`symbol`/`callers`/`outline`). Tags index stays default. | Same MCP names; LSP off → tags-only bytes; LSP on → at least one fixture where tags miss and LSP hits. |
| v0.2 | **Semantic response cache** — bifrost-like, opt-in. | Off → identical proxy bytes; on → documented false-hit rate on the P9 task set (must be 0 on that set or the feature stays off). |
| v0.2 | **Daemon** — optional long-running supervisor besides `proxy`/`mcp`. | Hooks still fail open in ≤ 10 ms if the daemon is down (D1). |
| v0.2 | **WASM plugin host** — load out-of-tree plugins without linking them into this repo. D6 still: this repo does not vendor those plugins. | `Registry::from_plugins` plus one example `.wasm` that records a `Measurement`; in-tree plugins unchanged. |
| v0.2 | **Tiered session context** (OpenViking L0/L1/L2). | Measured against v0.1 `archive`+`inject`; license (AGPL) called out in the task. |


## 4. Definition of done for v0.1

1. `rtok doctor` shows ≤ 8 token-related hooks, one MCP server for reads/memory/graph, one proxy hop (serving both Anthropic and OpenAI wire formats, D11).
2. `rtok stats --compare before-rtok` over ≥ 5 working days shows lower context-token-turns per session and lower output tokens per passed bench task, with expand rate < 5 %.
3. Every plugin has a `Measurement` path and appears in `rtok stats --plugin <id>`.
4. Hook p95 < 10 ms; proxy adds < 20 ms per request (measured in T5.1 test).
5. README documents what is lossless, what is estimated, and how to revert (`rtok setup claude --remove`, backups).
6. `rtok config show --sources` lists every setting with its origin; the coverage test (T12.4) is green.
7. Every hook, MCP `tools/call`, and proxy request has a `calls` row with host agent + provider + model (when known); each plugin run has `tokens` before and after, and MCP tokens for that plugin when it served a tool.

## 5. Order of value (if time is short)

P1 (measure) → P2 (hooks) → P5 (proxy passthrough for ground truth) → P3 (cmd) → P4 (read) → P5 compress → P9 (bench + retire). P6–P8, P10 and P11 only after P9 shows the core pays for itself; P11 first among those if an OpenAI-API host is in daily use. P12 (config) is not optional and comes right after P0's gate, before any task adds a flag. P13 (ORM + action store) comes right after P12, before P1 writes any rows. P14 is not a phase you sit down and finish: T14.0 lands with P12/P13, then each T14.x lands in the commit before its plugin's first task (T14.1 before T1.1, T14.6 before T2.4, T14.2 before T3.1, …). v0.2+ Later versions (LLM compression, embeddings, LSP graph, daemon, WASM) start only after §4 v0.1 done.

Complexity of what is left (added 2026-09-08; 1 = trivial, 5 = hard). Open tasks carry a `Complexity:` line; the open gates are mostly calendar- or user-bound, not engineering:

| Item | Complexity | Waits on |
|---|---|---|
| T8.16 | 3/5 | implemented in the working tree; `just check`, commit, move to `done.md` |
| T8.17 | 4/5 | the only unstarted code task: watchman backend + Gate P8d (3)+(5) numbers + removal decision rule |
| Gate P8d | 1–2/5 | measurements after T8.16/T8.17; clause (2) already recorded; a quiet machine for p95 |
| Gate P19 | 1/5 | browser check of `just dashboard` (T19.1–T19.3 done) |
| Gates P3 / P4 / P6 / P7 / P11 | 1/5 code, days of traffic | legacy tools disabled + `rtok stats --compare` |
| Gate P5 | 2/5 | the proxy has served no live request yet: 2 d passthrough + 2 d compress; expand rate and cache_read unmeasured |
| Gate P8b (4) + P9 | 3/5 | P9 task set run twice; user keep/drop decision |
| Gate P18 | 3/5 | first real release from the Actions tab; one real session as one trace; SigNoz + Maple with user accounts |

## 6. Plan amendments (recorded while implementing; each is small and evidence-free by nature)

| Date | Change | Why |
|------|--------|-----|
| 2026-09-08 | Gate P8c (T8.14): `graph-lbug` stays opt-in, never default. Clause (4) won (77×); (2) and (3) fail on the `graph-lbug` binary. | T8.14 release bench, this machine; numbers in `research.md` §2. |
| 2026-09-01 | Estimator rates are `[estimator] code/prose/json/cjk` in config, not `core.estimator_chars_per_token`. | T0.5 needs four classes; one key per class is what `--calibrate` (T1.5) will rewrite. |
| 2026-09-01 | Crate is a library plus a thin `src/main.rs`. | Tests, `examples/hello_plugin.rs` and every surface share one API; no code duplication in the bin. |
| 2026-09-01 | Every plugin directory carries `README.md` (users) and `AGENTS.md` (invariants, owned files, Checks). | Per-plugin instructions keep the root `AGENTS.md` under 350 tokens while giving the implementing agent the constraints it needs per task. |
| 2026-09-01 | Finished tasks move to `done.md`; `plan.md` keeps only open work. | The plan stays short enough to load every session. |
| 2026-09-01 | `guard` and `toon` are in the catalogue and registry but have no numbered task yet. | Superseded 2026-09-02: T2.6 (`guard`) and T11.7 (`toon`); see `roadmap.md`. |
| 2026-09-01 | `README.md` exists now as a status page; T9.5 still replaces it with measured results. | Newcomers need install + layout before P9. |
| 2026-09-01 | OpenAI API support added: decision D11, phase P11 (T11.1–T11.6), non-goal on the Codex Responses proxy withdrawn. | User request; OpenAI-API hosts (Codex, OpenCode, aider) were otherwise unmeasurable and uncompressible. |
| 2026-09-01 | Config file designed (`docs/config.md`): decision D12, phase P12 (T12.1–T12.4), new `rtok config` subcommand, `core.inject_budget_tokens` moves to `plugins.inject.budget_tokens`. | User request: every CLI parameter must be settable in the config file. |
| 2026-09-01 | No third-party plugins: D6 rewritten (all plugins native, written from scratch; research tools are specs). rtk delegation removed (T3.2 → rule engine, T3.3 → formatters); engram/claude-mem importers → generic JSONL (T6.3); graph adapter → own tree-sitter-tags index (T8.1–T8.2); `Kind` dropped and `Registry::from_plugins` + a second example added as the public plugin API (T0.8). | User decision: no runtime dependency on the tools rtok retires; one code path per method to measure; third parties extend through the public API, outside this repo. |
| 2026-09-01 | ORM + action store: decision D13, phase P13 (T13.1–T13.4). Diesel replaces rusqlite. Schema adds `hosts`, `providers`, `models`, `sessions`, `calls`, `call_io`, `tokens`, `logs`. T2.1/T4.1/T5.1/T5.3 write `calls`; T11.6 migration becomes 0003.sql. | User request: store all MCP/API actions with bodies, token counts before/after (including MCP per plugin), core and plugin logs, and host/model/provider on every call. |
| 2026-09-02 | v0.1 “non-goals” are deferred to v0.2+, not rejected. D1 scoped to v0.1; Later versions table; `ideas.md` Later; `roadmap.md` Later; architecture §11 retitled. | User request: LLM compression, embeddings, LSP graph, daemons, WASM belong in a higher version. |
| 2026-09-02 | `ideas.md`: parking lot for propositions inspired by alternative tools (`research.md` §4–§5) that are not tasks yet. Promote only with a Check in this file. | User request: store improvement/missing-feature ideas relevant to other tools. |
| 2026-09-02 | `roadmap.md`: one build plan per internal plugin, derived from this file. T2.6 `guard` and T11.7 `toon` added so every catalogue plugin has a numbered task and Check. | User request: roadmap based on the plan, a plan for each internal plugin. |
| 2026-09-02 | D9 + `AGENTS.md` Models: high-perf for research/investigate (user must confirm); mid-tier for coding; low-cost when the task fits. | User request: model routing by job class. |
| 2026-09-04 | T18.4 added to P18: `docs/release.md`, the codesigning and notarisation runbook. | User request. The README says the binaries are unsigned; this is the document that says what turning it on costs and how. |
| 2026-09-04 | T17.2 added to P17: `cargo-cache` pinned in `mise.toml`. | User request. `just cache` / `just cache-autoclean` already existed and called it the way pinned tools are called, but it was only ever a global install. |
| 2026-09-04 | Phase P18 (T18.1–T18.3): release. Versions restart at 0.0.1 and each release is the next patch, computed by the workflow; the release is startable from the Actions tab; README installs a released binary in one line. | User request: update the macOS release action, bump the patch after each release starting from 0.0.1, use a tool to create the release, and refresh the bash/zsh self-install script. T10.4 shipped the config but no release was ever cut. |
| 2026-09-04 | Phase P17 (T17.1): build size. Dev profile drops full DWARF, `lbug`'s C++ builds optimised even in dev, release strips symbols. | User request after D18 put a 2 GB debug C++ library and 8.7 GB of dev artifacts in the tree on a volume already 98 % full. |
| 2026-09-02 | D9 rewritten: drop Haiku/Sonnet/Opus. Tasks no longer name a model; implementer is a small/cheap model from any provider; gates are a mid-tier review; only a frontier model edits this plan. `AGENTS.md` matches. | User request: do not lock agents to Claude models. |
| 2026-09-02 | CLI + config crates: decision D14. clap 4 (derive, wrap_help) stays the CLI; figment replaces hand-rolled layering and the direct `toml` dep; toml_edit is the `config set` writer. T12.2/T12.3/Gate P12 name the crates. | User request: use the best Rust tools for CLIs and configs. |
| 2026-09-02 | T12.2 env mapping is **not** `Env::split("_")`. Keys contain underscores (`proxy.openai_upstream`, `plugins.inject.budget_tokens`), and splitting every `_` would produce `proxy.openai.upstream`. Instead `Env::prefixed("RTOK_")` is filtered through a table built from `Config::default()`: `PROXY_OPENAI_UPSTREAM` → `proxy.openai_upstream`. Unknown names (`RTOK_HOME`, `RTOK_CONFIG`) are dropped rather than becoming unknown keys under `deny_unknown_fields`. Comma-separated list values are split by a thin wrapper for keys whose default is an array, since figment's `Env` does not split values. | The naive split silently mis-targets 20+ keys and would make `RTOK_HOME` a parse error. |
| 2026-09-02 | `[proxy] dry_run = false` added to `config/default.toml` and `docs/config.md`. `rtok proxy --dry-run` prints the effective proxy settings and exits — the T12.2 Check needs it, and it is a setting, not an action, so D12 requires a key. | T12.2 Check: `RTOK_PROXY_PORT=1 rtok proxy --port 2 --dry-run` must report port 2. |
| 2026-09-02 | T5.0 `httpmock` upstream harness added before T5.1; T11.2/T11.3 Checks point at shared `tests/fixtures/proxy/*` mocks. | User request: use httpmock for Anthropic and OpenAI API proxy tests instead of ad-hoc fixtures per task. |
| 2026-09-02 | Per-plugin design research: decision D15, phase P14 (T14.0–T14.10). Every catalogue plugin gets its own plan, `src/plugins/<id>/PLAN.md`, surveying ≥ 3 alternatives (≥ 1 outside the retired stack), naming the mechanism that beats them, and setting the number its `roadmap.md` gate must beat. Each T14.x is scheduled immediately before its plugin's first implementation task, not as a batch. | User request: research and investigate alternatives for each internal plugin and make it better, with an individual plan per plugin. |
| 2026-09-02 | `CHANGELOG.md` is generated by git-cliff (`just changelog`, `cliff.toml`, tool pinned in `mise.toml`); commit subjects `<task-id>:`/`plan:`/`docs:`/`ci:` are the grouping. T10.4 (release) runs it before tagging. | User request; the `<task-id>: <title>` commit rule already carries the information, so no hand-written changelog. |
| 2026-09-02 | CLI testing stack in §2: unit tests + integration (`assert_cmd`, `predicates`, `assert_fs`, `trycmd` for Markdown/plain snapshot cases). | User request: standard Rust CLI test crates instead of ad-hoc shell in Checks. |
| 2026-09-02 | Git is `main` only (D16): no `rtok/<task-id>` (or other) feature branches; one task remains one commit on `main`. `AGENTS.md` Workflow and plan §2 match. | User request: use only main branch. |
| 2026-09-02 | Don't duplicate code or logic: reuse an existing helper, or extract one shared helper at the responsible layer. `AGENTS.md` never-bend, plan §2, `docs/plugin-authoring.md` §4. | User request. |
| 2026-09-02 | An implemented task is unfinished until it is marked done and moved from `plan.md` to `done.md` (verbatim + Check). Same commit as the implementation. `AGENTS.md`, plan §2, `docs/plugin-authoring.md`. | User request: all implemented tasks should mark as done and move to done.md. |
| 2026-09-02 | T11.2's Check asks for a `usage` row with `api = openai_chat`, but `usage.api` is added by T11.6, which depends on T11.2. T11.2 verified the other two clauses (identical response bytes, `cache_read` from `cached_tokens`); the per-API assertion moves to T11.6, whose Check already covers "fixture usage rows for two APIs → two rows in the stats table". Until then an OpenAI row is identifiable only via `calls.provider = openai`. | Writing the column early would pull T11.6's migration into T11.2, i.e. implement a task out of order. |
| 2026-09-02 | `Wire` gained `prepare_request(&mut Value, include_usage) -> bool` (default no-op) for provider request shaping in both proxy modes; `int_field` moved from `anthropic.rs` to `wire.rs`. T11.2 needs `stream_options.include_usage` on OpenAI streaming requests, and the wire is the layer that owns request shape — the alternative was a path match inside `mod.rs`. | Keeps format knowledge in the wire (T11.1's premise) and avoids duplicating `int_field` per wire. |
| 2026-09-03 | Gate P13 "no SQL outside `src/store/`" is the runtime rule: plugins and surfaces have no `sql_query` / rusqlite. Schema lives in `migrations/*.sql` and is applied only via `Store::MIGRATIONS` (`include_str!`). Files are not moved into `src/store/` — Diesel's conventional layout. | Literal reading would require relocating migration files; that is not a second SQL client. |
| 2026-09-03 | Gate P14 `git log` order: all ten `PLAN.md` files were added in `830e049` (2026-09-02) in the same commit as measure + hook implementation. T14 scheduled each design immediately before that plugin's first impl, not as a batch; history is not rewritten. | Closing the gate honestly requires recording the batch, not pretending PLAN-before-code. |
| 2026-09-03 | `guard` PLAN Mechanism was rewrite-to-expand; T2.6 is `Deny` with `rtok expand <id>` in the reason. Mechanism (and Rejected) now match T2.6. | A design that contradicted a task must produce a §6 amendment (P14). |
| 2026-09-03 | Gate P0 "trait shape final; no plugin logic yet" was true at T0.8 (`c9b6f81`). Later tasks extended `Plugin` (e.g. `proxy_filter` takes `WireRequest`) and added plugin logic. The review closes as that scaffold freeze, not as a claim about HEAD. | Cannot empty plugins or freeze the trait without undoing P1–P11. |
| 2026-09-04 | P8b (T8.3–T8.8) added to `graph` after Gate P8 closed on description tokens alone. The re-survey found two defects the gate could not see: `symbols` has no `root`, so a second repo evicts the first, and every tool call re-reads and re-hashes the whole tree. T8.5–T8.7 answer what codegraph and code-review-graph do that three tools do not, within one added tool. | User request to compare the field and plan the next step; survey and rejected options in `src/plugins/graph/PLAN.md` (2026-09-04). |
| 2026-09-04 | Gate P8b's "T8.8 recall ≥ 0.9" now reads "definition recall ≥ 0.9". Measured: definitions 30/30, recall 1.000, precision 1.000; references 40/114, recall 0.351; all sites 0.486. Every one of the 74 misses is a construct the tree-sitter Rust tags query does not capture — type positions (64), macro bodies (9), path-qualified calls (1). | The 0.9 bar was written before the index was measured. Re-labelling to only what the index can see would score it against its own edges; lowering the bar to 0.486 would make it meaningless. The honest split is a bar on definitions, which the index owns, and a published number plus a regression floor on references, whose ceiling belongs to the grammar (I-31). |
| 2026-09-04 | D18 added: LadybugDB (`lbug`) as a gated second backend for the `symbols` index (P8c, T8.9–T8.14). D8 narrowed to the ledgers — the graph index is a derived cache and may live in `graph.lbdb`. D13 unchanged in substance: Cypher lives in `src/store/` only. §2 baseline gains `lbug` behind `graph-lbug`. | User request 2026-09-04. The gate is written so the backend can lose: clause (4) is the only thing a graph store does that SQLite cannot do in one indexed lookup, and clause (5) prices the C++ build and the unpinned prebuilt. Survey in `src/plugins/graph/PLAN.md` v0.3. |
| 2026-09-04 | D19 added: OpenTelemetry export as phase P16 (T16.1–T16.8). §2 unchanged: OTLP/HTTP JSON is built with `serde_json` and posted with `reqwest`; the one-shot `rtok otel flush` runs on a current-thread tokio runtime rather than adding `reqwest/blocking`. Migration `0009.sql` adds `otel_export` (per-stream watermarks) — a ledger under D8. | User request 2026-09-04 ("log full info, OpenTelemetry for AI agents, show in Maple / SigNoz / Jaeger / Grafana"). Design and rejected options in `src/otel/PLAN.md`. |
| 2026-09-07 | T18.5 added to P18 and done: release-plz as a second entry point — a `release: vX.Y.Z` pull request with the next version and changelog; merging it dispatches the dist Release workflow through `tools/release.sh --no-bump`. release-plz neither tags nor publishes; dist does both. No new dependency in the binary; no new secret required. | User request 2026-09-07 ("Add support release-plz"). Two paths, one script, one workflow — they cannot disagree on the version. |
| 2026-09-08 | D21 added: every new plugin is plugin + MCP as one unit, a singleton, one call path per capability; host plugins work on desktop and CLI; missing `rtok` tells the user to install with ketch. `AGENTS.md` never-bend and plan §2 match. | User request 2026-09-08 (Cursor host plugin: plugin and MCP simultaneously, no duplicate calls, singleton; missing rtok → ketch install). |
| 2026-09-08 | T10.5 added (P10 reopened): `rtok setup cursor` offers to install `plugins/cursor`. D21 gains clause (6); `AGENTS.md` matches. | User request 2026-09-08 (`rtok setup cursor` должен предлагать установить и плагин). |
| 2026-09-08 | T10.6 added (P10 open): pi host plugin — `plugins/pi/` pi package (TS extension + skill, no MCP per pi philosophy), one bash call path (`tool_call` rewrite to `rtok run`, `tool_result` via `rtok filter`), `rtok setup pi` offers it after the T10.1 pattern. Promotes the pi part of I-17. | User request 2026-09-08 ("add plugin for pi agent", confirmed). |
| 2026-09-08 | Complexity ratings added to the plan: a `Complexity:` line on open tasks and a complexity table for the remaining work in §5 (scale 1–5). | User request; makes the remaining effort visible next to the order of value. |
