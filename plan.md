# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T44.3 | in progress | P1 | 3 | 0% | Claude Code / Fable 5.1 |
| T44.4 | todo | P1 | 3 | 0% | |
| T44.5 | todo | P1 | 3 | 0% | |
| T45.1 | in progress | P0 | 2 | 0% | OpenCode / Muse Spark 1.3 |
| T45.2 | in progress | P0 | 3 | 0% | OpenCode / Muse Spark 1.3 |
| T45.3 | in progress | P1 | 3 | 0% | OpenCode / Muse Spark 1.3 |
| T45.5 | in progress | P2 | 2 | 0% | OpenCode / Muse Spark 1.3 |
| T45.6 | in progress | P2 | 2 | 0% | OpenCode / Muse Spark 1.3 |

### T44.3. Plugins per app and Claude Desktop

T44.2 gave every app its block (kind and name, app path and version, config files, modules with flag or reason, `already installed`). Two pieces remain. (1) Under the modules, rtok's own plugins grouped the same way — installed / not installed / not supported — derived from each manifest's surfaces and the host's modules (hook → hooks, mcp → mcp, proxy → proxy; the linked plugin stands in for hooks+mcp on Cursor and for cli on pi), disabled plugins marked `(off)`. (2) Claude Desktop joins as a Desktop variant of `claude`: config `claude_desktop_config.json` under `~/Library/Application Support/Claude` (macOS), `%APPDATA%\Claude` (Windows), `~/.config/Claude` (Linux); MCP only, written with the absolute rtok path because the app has no shell PATH; `--replace` and hooks stay CLI-only. Plan: `Agent::plugin_surfaces()`, `agents::plugin_rows` from `Registry::manifests`, a README section per host the parity test also checks, `claude` variants + `support(kind, module)` per kind, README rows `hooks (desktop)` etc.; unit tests for the grouping and the desktop paths; `just check`.

### T44.4. Checks, e2e and platforms

More checks around setup: warn when `rtok` is not on PATH (hooks would fail to spawn), verify after each write that the requested modules read back as installed, and refuse unknown hosts before any backup. Tests: an e2e matrix in `tests/agents_setup.rs` over every host (setup twice → one backup, `already installed`; remove twice → `no changes`; `--dry-run` writes nothing; `agent` alias equals `agents`; `--cli`/`--desktop` filters; Claude Desktop under a temp home), Windows path cases as unit tests, and a `windows-latest` job in `ci.yml` (advisory until green). `just check`.

### T44.5. OpenCode CLI+Desktop MCP and plugin parity with Cursor

OpenCode stays proxy-only while Cursor installs hooks+MCP+plugin; `hosts/opencode/rtok.ts` is copied by hand and `support(mcp/plugin)` is `No`. Done means `rtok agents setup opencode` writes the `mcp` table and links the plugin for CLI and Desktop independently, singleton as in Cursor (linked plugin serves MCP, `mcpServers.rtok` removed), fail open with the ketch install hint when `rtok` is missing.
Plan: `src/agents/opencode/mod.rs` (register/unregister `mcpServers.rtok` via `rtok mcp`, `offer_plugin` link of `hosts/opencode/rtok.ts` into CLI + Desktop plugin dirs, singleton clear, `support`/`installed`/`markers` update), `src/agents/opencode/README.md` (parity table), `hosts/opencode/rtok.ts` (keep `tool.execute.after` bash-only fail-open path). Verify: `mise exec -- cargo test -p rtok --lib agents::opencode` + `just check`.

### T45.1. OTel flush survives a traces error

`flush_into` aborts the whole flush on a non-404 traces POST error (`?`), so logs/metrics stall one extra round. Per-stream isolation: stash the error in `rep.error` and continue; the traces mark stays, other streams advance.
Plan: `src/otel/export.rs` (match per-stream `post` results), `tests/otel.rs` (new `traces_500_still_posts_logs_and_metrics` case). Verify: `mise exec -- cargo test --test otel`.

### T45.2. Proxy cache hits and errors keep usage rows

A semantic-cache hit writes measurement + `call_io` but no `usage`/`tokens` row and drops request bytes; upstream errors and non-2xx/no-usage responses also leave no row. Every request owes one usage row (T5.1).
Plan: `src/proxy/mod.rs` (insert usage + provider tokens on cache hit with request bytes; minimal row on error), `tests/proxy.rs` (new cases). Verify: `mise exec -- cargo test --test proxy`.

### T45.3. Archive decisions scoped per session

`archive_decisions` PK is bare `tool_use_id` while reads/writes scope `(session, tool_use_id)`: a repeated id in a second session hits `INSERT OR IGNORE` and is never persisted; `mark_expanded` is global so expanding in session A freezes B.
Plan: new migration `migrations/0014.sql` (composite PK, never edit applied ones), `src/store/mod.rs`, `src/expand.rs`. Verify: `mise exec -- cargo test --lib store::` + expand tests.

### T45.5. Gates cover tests, webui and examples

`.jscpd.json` scans only three src dirs, blind to `tests/`/`crates/rtok-webui` where the known mirrors live; `examples/mcp_tool.rs` teaches zero-`Measurement` plugins against D3.
Plan: `.jscpd.json` (extend scope, keep gate green), `examples/mcp_tool.rs` (record + assert one row like `hello_plugin`), `tests/trycmd/*` (one more surface snapshot). Verify: `mise exec -- jscpd` + scoped cargo tests.

### T45.6. Extra hook/expand/guard/read/toon coverage

Nine new cases in `tests/extra_cover.rs` (new file, no existing file touched): hook fail-open on garbage/empty stdin, `expand` unknown-id with `--lines`, 11-row `plugins` listing, PreToolUse rewrite + deny-wins merge, guard deny naming an expandable id, read cap marker within `max_chars`, toon comma-cell round-trip.
Plan: verify `mise exec -- cargo test --test extra_cover` green (done 9/9), fix the `collapsible_if` lint at `src/hooks/mod.rs:47` left by T45.4, then commit the single new file. Verify: scoped tests + clippy on the new test target.

## Reference

Historical phase notes (P0–P39) live in `done.md`. Companion evidence: `research.md`, `architecture.md`. Per-plugin plan: `roadmap.md`. Unapproved propositions: `ideas.md`.

Claim a `todo` row before work: set Status to `in progress` and Agent to `Provider / model`. Before stopping unfinished work, set Status to `todo` and clear Agent. When the Check passes, move the task entirely to `done.md` (Do/Check + Check result) and drop it from this table, its card, and `todo.md`.

### Decisions (read before any task)

| # | Decision | Why (evidence in `research.md`) |
| --- | --- | --- |
| D1 | **Rust, one static binary, in-tree plugins behind one trait + Cargo features.** No WASM, no subprocess plugins, no daemon **on the hook path in v0.1** (`rtok demon` supervises the long-running surfaces only, D22). A later WASM plugin host is P32 (landed); this repo still does not wrap third-party tools (D6). | Hooks run on every tool call; Rust cold start vs Python hooks. |
| D2 | **One binary, three surfaces:** `rtok hook` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`). | PostToolUse hooks cannot modify tool results. Only PreToolUse rewrite, MCP tool replacement, or a proxy can shrink what the model sees. |
| D3 | **Measurement first.** Nothing ships until `rtok stats` reads real usage from session logs and the proxy. Every plugin logs before/after. Metric = *context-token-turns* (tokens × turns they stay in context), plus output tokens. | Vendor claims vs measured savings; nobody in the stack measures end to end. |
| D4 | **Lossless by default.** Every compression keeps the original retrievable via `rtok expand <id>` / MCP `expand`. Lossy only where the source is regenerable (re-run the command). | Trust is the product. |
| D5 | **Injection budget.** All SessionStart/UserPromptSubmit injections go through one plugin with a per-turn token cap (default 800) and a byte-stable prefix. | Injections are re-read (cached) every turn. |
| D6 | **Every plugin is native, written from scratch in this repo. No third-party plugins.** A plugin never spawns, links, imports, or reads the data of another tool. Third parties extend rtok from outside through the public plugin API (`rtok-plugin-sdk`, `Registry::from_plugins`, `docs/plugin-authoring.md`), never through this repo. | One code path per method is what `Measurement` can attribute. |
| D7 | **Prompt "modes" (terse, YAGNI) are data files, not code.** | Measured effect must be A/B tested, not assumed. |
| D8 | **One SQLite file** (`~/.rtok/rtok.db`, WAL). Schema is D13. Raw archived payloads on disk under `~/.rtok/archive/` (D4); the DB holds indexes and inline JSON under a size cap. | Field tools converge on SQLite (+FTS5). |
| D9 | **Agents are provider-agnostic.** Route by job: low-cost for mechanical work; mid-tier for coding; high-performance for research only after the user confirms. Host names are products, not the implementer. | User constraint: cost-aware routing, any provider. |
| D10 | **Retire, don't stack.** Phase 9 replaces the legacy hook pile with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities across bash/read/memory/graph tools. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages and OpenAI Chat Completions / Responses are `Wire` adapters behind one proxy. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. | One proxy, two parsers is cheaper than two proxies. |
| D12 | **One config file holds every setting; every CLI flag is a config key.** Precedence: defaults < user file < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env < flags. `rtok config show --sources` tells where each value came from. | Hooks, long-running servers, and benches need one precedence rule. |
| D13 | **Core persists through a sync ORM on bundled SQLite.** Diesel (`sqlite` + bundled `libsqlite3-sys` with FTS5). Plugins never write SQL; `Store` is the only DB owner. Hook path: metadata always, body only if under the inline cap — never archive, never fail the hook (D1). | Diesel is sync, so the ≤ 10 ms hook path stays blocking and fail-open. |
| D14 | **CLI is clap 4 (derive); config layers are figment; TOML writes are toml_edit.** Env is `RTOK_<SECTION>_<KEY>` looked up in a leaf table from `Config::default()`. | Clap owns the subcommand tree; Figment tracks per-key provenance. |
| D15 | **Every plugin is designed against alternatives before it is built.** Each catalogue plugin has `src/plugins/<id>/PLAN.md`. | From-scratch only pays if the design beats what it retires. |
| D16 | **Git is `main` only.** Agents commit on `main`. Do not create feature branches. One task is still one commit (`<task-id>: <title>`). | `main` is the single line of work. |
| D18 | **The graph index lives in SQLite with the ledgers (D8).** LadybugDB and Grafeo were gated, frozen, then removed (P39). No live `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` feature flags. SQL for symbols lives only in `src/store/symbols.rs`. D6 holds: no spawned graph tool. | Both graph-store candidates were priced and deleted per the gate. Survey archive: `src/plugins/graph/PLAN.md`. |
| D19 | **Observability is a projection of the ledgers, never a second recorder.** OpenTelemetry export reads existing rows and posts OTLP/HTTP JSON. Nothing runs on the hook path. | Delivery is at-least-once behind a per-stream watermark. |
| D20 | **Local web UI is an operator surface, not a catalogue plugin.** `rtok web` serves axum + a Slint WASM UI (`crates/rtok-webui`). Slint is not linked into the hook binary. | Linking Slint into `rtok hook` would fail the size/latency gate. |
| D21 | **Every new plugin is plugin and MCP as one unit, a singleton, with one call path per capability.** Host plugins load in that host's desktop app and its CLI. Missing `rtok`: fail open and print that it must be installed with ketch. | Duplicate MCP processes and duplicate call paths break D18 and measurement. |
| D22 | **`rtok demon` supervises rtok's own long-running surfaces**, not a catalogue plugin. Allow-list names (`proxy`, `mcp`, `dashboard`). Nothing in it is on the hook path. | The proxy is the wire hop; when it dies every host silently loses it. |
| D23 | **`rtok tui` and `rtok web` are two renderings of one operator model.** A page that exists on one surface and not the other is a defect. | Two independently built surfaces drift. |
| D24 | **`rtok report` renders; it never computes a number of its own.** It reads the D23 operator model. Recommendations are rules over those rows, never an LLM call. | A saving that is not a `Measurement` row does not exist (D3). |
| D25 | **The plugin contract is `rtok-plugin-sdk`.** Required methods are explicit; event methods keep no-op defaults. `rtok` is the only dispatcher. | One contract, no in-tree shortcut. |
| D26 | **One log with two readers:** a rotating text file and the `logs` table OTel exports. Bounded (`max_bytes` 1 MiB, `files` 5). | An unbounded log on a long-running proxy is a disk-full bug. |
| D27 | **Anything a command prints, or the store keeps, is a page on `rtok web` and `rtok tui`.** Writing commands stay CLI-only. | The two surfaces plus CLI must not disagree about what a session is. |
| D28 | **The agent-host contract is `rtok-agent-sdk`.** Installers go through it; host-specific code stays in `src/setup/<host>.rs`. | One write cycle, one plugin-offer body. |

### Architecture

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

`Ctx` gives every plugin: the DB handle, the token estimator, the archive store, config, session id. `Measurement { plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id }` is the only way savings enter the DB.

Plugin catalogue (v0.1). Every plugin is native Rust written from scratch here (D6).

| id | spec (replaces; evidence in research.md) | surface | mechanism |
| --- | --- | --- | --- |
| `measure` | rtk gain, headroom savings, lean-ctx gain | `stats`, `bench`, proxy | session JSONL ingest + proxy `usage`; context-token-turns |
| `cmd` | rtk hook, lean-ctx ctx_shell | PreToolUse(Bash) → `rtok run` | archive raw output; per-family formatters + TOML rules; pointer trailer |
| `read` | lean-ctx ctx_read/search/tree | MCP `read`,`search`,`tree` + PreToolUse(Read) | modes via tree-sitter-tags; re-read dedup |
| `archive` | token-optimizer archive_result, headroom CCR | proxy live zone + `expand` | replace old large `tool_result` with pointer + head/tail |
| `proxy` | headroom proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` | passthrough + SSE; usage capture; never touches system, tools, or last 2 turns |
| `inject` | caveman shrink-hook, ponytail/caveman modes | SessionStart, UserPromptSubmit | budgeted, byte-stable prefix; modes as markdown |
| `guard` | token-optimizer refetch_guard | PreToolUse | identical read/command within N turns → deny |
| `memory` | engram, claude-mem | MCP `mem_save/search/get` | agent-written notes, SQLite FTS5 |
| `graph` | codebase-memory-mcp, serena, codegraph | MCP `symbol`,`callers`,`outline` | tree-sitter-tags index in SQLite; optional LSP is T30.2 |
| `toon` (off by default) | caveman toon, TOON | proxy/MCP | tabular JSON → TOON |

### Working agreement

- Graph backends: do not claim tasks that reintroduce `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` / `graph-grafeo` / cmake-for-liblbug. Symbol index is SQLite only.
- One task = one commit on `main` only. Never skip the Check.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message.
- Don't duplicate code or logic: find the existing helper and reuse it, or extract one shared helper at the responsible layer.
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run` green before every Check.
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.
- Every new CLI flag gets a key in `config/default.toml` and a row in `docs/config.md` in the same commit (D12).
- No plugin shells out to, links, imports from, or reads the data of a third-party tool (D6).
- Every new plugin obeys D21.
