# rtok — implementation plan for a unified, plugin-based token-reduction CLI

Status: plan v1, 2026-09-01. **Progress: P0 done 2026-09-02 (T0.1–T0.8); P12 T12.1–T12.4 done; P13 T13.1–T13.4 done (see `done.md`); P14 done; T1.1–T1.5 and T2.1–T2.6 done; T3.1–T3.6 done; T6.1–T6.3 T7.1–T7.2 done; T4.1 T4.2 T4.3 T4.4 T4.5 T4.6 T4.7 T5.0 T5.1 T5.2 T8.1 T8.2 T9.1 T9.2 T9.3 T9.4 T9.5 T10.1 T10.2 T10.3 T10.4 T11.1 T11.2 T11.3 T11.4 T11.5 T11.6 T11.7 T8.3 T8.4 T8.8 T8.5 T8.6 T8.7 T8.9 T16.1 T16.2 T16.3 T16.4 T16.5 T16.6 T16.7 T16.8 T8.10 T8.11 T8.12 T17.1 T18.1 T18.2 T18.3 T18.4 T17.2 T8.16 T8.17 done.** Companion evidence: `research.md` (comparison, measurements, fact-check). Shape of the code: `architecture.md`. Per-plugin plan: `roadmap.md`. Propositions (not yet tasks): `ideas.md`. Every implemented task must be marked done and moved from here to `done.md` verbatim (Do/Check + `Status: done <date>` and Check result); a task that still lives here is not done.
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

Gate P0: done 2026-09-03 (historical freeze at T0.8) — see `done.md` P0; §6 records later trait extensions.

### P1 — Measure — tasks done, Gate P1 passed 2026-09-03 (see `done.md` P1).

### P2 — Hook surface — tasks done, Gate P2 passed 2026-09-03 (see `done.md` P2).

### P3 — `cmd` plugin (goal: every Bash output archived, filtered, measured)

Gate P3: removed 2026-09-09 — one working day of live traffic plus `expand` rate; not code-closable. Evidence kept: `research.md` §2 (Bash 7.71 M est. tokens baseline). Re-add as a task only with a dated traffic window.

### P4 — `read` plugin + MCP server (goal: replace lean-ctx's 78 tools with 5 and the 3.1 K/turn banner with 0)

Gate P4: removed 2026-09-09 — one day with legacy tools disabled plus `stats` compare; not code-closable. Evidence kept: `research.md` §2 (Read 3.07 M, lean-ctx 0.86 M MCP rows).

### P5 — `proxy` + `archive` (goal: ground-truth usage and cache-safe shrinking of old tool results)

Gate P5: removed 2026-09-09 — 2 d passthrough + 2 d compress with live `usage` rows; the proxy has served no requests here, so `expand` rate and `cache_read` per turn are unmeasurable in code. Evidence kept: replay estimate `research.md` §2 (`archive replay (estimate)` CTT 11.81 G → 8.42 G, −28.7 %, 1 803 candidates). To re-run: point `ANTHROPIC_BASE_URL` at `rtok proxy` (T5.2 `setup --proxy`), two days `passthrough`, two days `--mode compress`, then `rtok stats --cache` and `rtok stats --plugin archive --json` (`expand_rate`).

### P6 — `memory` plugin (goal: one memory instead of two, zero LLM cost)

Gate P6: removed 2026-09-09 — one week with engram + claude-mem disabled plus subjective recall judgement; not code-closable. Evidence kept: `rtok doctor` MCP description rows in `research.md` §2.

### P7 — modes + instruction hygiene

Gate P7: removed 2026-09-09 — A/B `terse` on/off on 6 tasks with pass/fail judgement; not code-closable. Harness kept: T9.1 `rtok bench`.

### P8 — `graph` plugin — tasks done, Gate P8 passed 2026-09-03 (see `done.md` P8).

### P8b — `graph` quality — tasks done, Gate P8b closed 2026-09-09 on the three code clauses (see `done.md` P8b).

### P8c — `graph` on LadybugDB — tasks done; Gate P8c: clause (4) won 2026-09-08, `graph-lbug` stays opt-in (see `done.md` P8c).

### P8d — `graph` freshness · done 2026-09-09 (T8.15–T8.17), Gate P8d passed — see `done.md` P8d.

### P16 — OpenTelemetry export — tasks done, Gate P16 passed 2026-09-07 (see `done.md` P16; backend clause moved to P18).

### P17 — build size — tasks done, Gate P17 passed 2026-09-07 (see `done.md` P17).

### P18 — release — tasks done, v0.0.1 published 2026-09-08 (see `done.md` P18); Gate P18 removed 2026-09-09 (needs a real release run, not code).

### P19 — web dashboard — tasks done, Gate P19 passed 2026-09-09 (see `done.md` P19).

### P9 — A/B bench + migration — tasks done; Gate P9 removed 2026-09-09 (not code-closable). Detail in `migration.md`.

### P10 — other hosts + release — T10.1–T10.6 done 2026-09-09 (D21)

**T10.7 `setup --remove` strips MCP** · T10.1 · `src/cli.rs`, `src/setup/claude.rs`, `src/setup/cursor.rs`
Do: `setup claude/cursor --remove` also removes `mcpServers.rtok` via the shared `unregister_stdio_mcp` helper (foreign servers kept); cursor keeps unlinking the plugin, and `--dry-run --remove` previews without touching the FS.
Check: unit `unregister_strips_only_rtok_and_keeps_foreign` (both hosts); temp-HOME apply (`--mcp` / `--yes`) → `--remove` → second `--remove` is `no changes`, foreign entries kept; `just check` green.
Complexity: 1/5 — two call sites plus one shared helper, no new flags, no config keys.
Status: in progress
Model: Muse Spark (meta/muse-spark)

### P11 — OpenAI API surface (goal: same proxy, same plugins, same numbers for OpenAI-API hosts) — added 2026-09-01 (D11)

Gate P11: removed 2026-09-09 — one OpenAI-API host through the proxy 2 d passthrough + 2 d compress; needs live Codex traffic, not code-closable. Re-add with a dated traffic window; record in `research.md` §2.

### P12 — Config file — tasks done, Gate P12 passed 2026-09-03 (see `done.md` P12).

### P13 — ORM + action store — tasks done, Gate P13 passed 2026-09-03 (see `done.md` P13).

### P14 — per-plugin design research — tasks done, Gate P14 passed 2026-09-03 (see `done.md` P14).

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


## 4. Definition of done for v0.1 (code-closable only; traffic/user-gated rows removed 2026-09-09, see §6)

1. `rtok doctor` shows ≤ 8 token-related hooks, one MCP server for reads/memory/graph, one proxy hop (serving both Anthropic and OpenAI wire formats, D11).
2. Every plugin has a `Measurement` path and appears in `rtok stats --plugin <id>`.
3. Hook p95 < 10 ms; proxy adds < 20 ms per request (measured in T5.1 test).
4. README documents what is lossless, what is estimated, and how to revert (`rtok setup claude --remove`, backups).
5. `rtok config show --sources` lists every setting with its origin; the coverage test (T12.4) is green.
6. Every hook, MCP `tools/call`, and proxy request has a `calls` row with host agent + provider + model (when known); each plugin run has `tokens` before and after, and MCP tokens for that plugin when it served a tool.

Removed 2026-09-09 (needs days of live traffic, not code): old row 2 — `rtok stats --compare before-rtok` over ≥ 5 working days with lower context-token-turns per session, lower output tokens per passed bench task, expand rate < 5 %. Re-add with a dated traffic window.

## 5. Order of value (if time is short)

P1 (measure) → P2 (hooks) → P5 (proxy passthrough for ground truth) → P3 (cmd) → P4 (read) → P5 compress → P9 (bench + retire). P6–P8, P10 and P11 only after P9 shows the core pays for itself; P11 first among those if an OpenAI-API host is in daily use. P12 (config) is not optional and comes right after P0's gate, before any task adds a flag. P13 (ORM + action store) comes right after P12, before P1 writes any rows. P14 is not a phase you sit down and finish: T14.0 lands with P12/P13, then each T14.x lands in the commit before its plugin's first task (T14.1 before T1.1, T14.6 before T2.4, T14.2 before T3.1, …). v0.2+ Later versions (LLM compression, embeddings, LSP graph, daemon, WASM) start only after §4 v0.1 done.

Complexity of what is left (added 2026-09-08; 1 = trivial, 5 = hard). Pruned 2026-09-09: every remaining item below is code- or measurement-closable; traffic/user-gated gates (P3/P4/P5/P6/P7/P9/P11/P18, P8b fourth clause) were removed, see §6.

| Item | Complexity | Waits on |
|---|---|---|
All code-closable gates passed (P8d, P19); the table is retired 2026-09-09 — nothing code-closable is left open. Traffic/user-gated gates were removed above, see §6.

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
| 2026-09-09 | T18.6 added to P18 and done: the release runs only on a green `just check`. A reusable `verify.yml` (the `ci.yml` matrix) gates the dispatch in `bump.yml` and in release-plz's `release` job, and `release-plz.yml` now fails loudly without `RELEASE_PLZ_TOKEN` instead of opening a pull request that starts no CI. Not a dist `plan-jobs` entry: `host` treats a skipped `build-local-artifacts` as fine, so a red gate there would publish a Release with no binaries. | User request 2026-09-09 (set the release up along the lines of `../ketch`, after a beta-readiness check). ketch runs the same gate inside its release and refuses to run without the token; v0.0.1 shipped with neither. |
| 2026-09-09 | Removed what only traffic/user action can close: Gates P3, P4, P5, P6, P7, P9, P11, P18 and the P8b fourth clause (P9 task-set comparison); §4 row 2 (≥ 5 working days `stats --compare`). Each removal keeps its evidence pointer (`research.md` §2) and its re-run recipe, so re-adding needs only a dated window, not rediscovery. §4 renumbered; §5 table pruned to code-closable items. | User request ("убрать только то, что кодом не закрывается"). A plan change per AGENTS.md; no code, no task, no gate result touched. |
