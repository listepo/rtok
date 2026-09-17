# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T48.5 | in progress | P2 | 3 | 0% | OpenCode / Muse Spark |
| T48.6 | todo | P2 | 3 | 0% | |
| T48.8 | todo | P2 | 3 | 0% | |
| T49.1 | todo | P2 | 3 | 0% | |
| T49.2 | todo | P2 | 4 | 0% | |
| T50.1 | todo | P2 | 3 | 0% | |
| T50.2 | todo | P3 | 2 | 0% | |
| T50.3 | todo | P3 | 3 | 0% | |
| T50.4 | todo | P3 | 2 | 0% | |
| T51.1 | todo | P3 | 5 | 0% | |
| T51.3 | todo | P3 | 4 | 0% | |
| T52.1 | todo | P3 | 3 | 0% | |
| T52.2 | todo | P3 | 3 | 0% | |
| T52.3 | todo | P3 | 4 | 0% | |
| T53.1 | todo | P3 | 3 | 0% | |
| T53.2 | todo | P3 | 1 | 0% | |
| T53.3 | todo | P3 | 3 | 0% | |
| T53.4 | todo | P3 | 2 | 0% | |

### T48.5. Windsurf host

From I-17. Windsurf (Codeium) stores MCP servers in `~/.codeium/windsurf/mcp_config.json` and ships Cascade hooks; neither is installed by rtok today.
Done when the current Windsurf docs are verified and linked, `rtok agents install windsurf` registers `rtok mcp` and, if the hook protocol can carry `rtok hook` (or a mapped `--host windsurf` payload like T46.3), the hooks; remove keeps foreign entries; the host joins the install/remove/list e2e matrix, config, docs and `README.md` host lists.

Execution plan (OpenCode / Muse Spark): docs verified 2026-09-17 — MCP https://docs.windsurf.com/windsurf/cascade/mcp (`~/.codeium/windsurf/mcp_config.json`, `mcpServers.<name>` `{command, args}`, no `type` for stdio), hooks https://docs.windsurf.com/windsurf/cascade/hooks (`~/.codeium/windsurf/hooks.json`, 12 Cascade events as `agent_action_name`/`tool_info` stdin — not Claude-shaped, and no `--host windsurf` mapping exists, so hooks stay `no` with that reason, like T46.3 scoped the mapping to its own task). Files: `src/agents/windsurf/{mod.rs,README.md}` (new; MCP-only via SDK `register_server`/`unregister_server`, single Desktop variant), `src/agents/mod.rs` registry + HOSTS + `host("windsurf").is_none` fix, `src/config/mod.rs` (`[setup.windsurf] config_path`, finish expand, path leaves), `config/default.toml`, `docs/config.md`, `src/cli.rs` host help (2 lines), `README.md` host list, `site/content/docs/commands.md`, `tests/trycmd/config-show.stdout`, `tests/agents_install.rs` matrix + `notahost` unknown-host rename, `tests/agent_remove.rs` windsurf test, `tests/common/agents.rs` write_cfg. Unit tests in mod.rs: dry_run names file/touches nothing, apply idempotent, remove keeps foreign, second remove no changes. Verify: `mise exec -- cargo fmt/clippy/nextest` for agents/config/e2e scope (full `just check` may fail on concurrent agents' uncommitted work — report, don't fix).

### T48.6. Zed host

From I-17. Zed configures MCP as `context_servers` in `~/.config/zed/settings.json` (JSON with comments) and has no shell hook events; its agent can also use external agents over ACP.
Done when `rtok agents install zed` adds `context_servers.rtok` without destroying comments or foreign servers, the support table says hooks/proxy/plugin `no` with the reason, remove restores the file, and the host joins the e2e matrix, config and docs.

### T48.8. VS Code Copilot Chat host

From I-17. GitHub Copilot Chat in VS Code reads MCP servers from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in the VS Code profile dir, and agent mode may run hooks; T46.4 covered only the Copilot CLI and the desktop app.
Done when the VS Code user dir per OS (Code, Code - Insiders) is resolved, `rtok agents install vscode` registers `servers.rtok`, hooks are added only if VS Code documents a hook file the T46.3 Copilot mapping can serve, remove keeps foreign servers, and the host joins the e2e matrix, config and docs.

### T49.1. `rtok stats --price`

From I-02. `rtok stats` reports tokens but not money, so a saving cannot be compared with a model's cost; cache reads are priced very differently from input (research.md §8).
Done when a price table (per model: input, cache write, cache read, output per MTok) lives in config with defaults that cite a dated source, `rtok stats --price` adds cost columns and a saved-cost total computed from the same `usage` rows, unknown models show `-` instead of a guess, and a trycmd snapshot plus a unit test on the arithmetic cover it.

### T49.2. Ingest Codex, OpenCode and Cursor session logs

From I-03. `measure` reads Claude Code JSONL only; the other hosts reach the `usage` table only when they go through `rtok proxy`, so their sessions without the proxy are invisible to `stats`, the TUI and the dashboard.
Done when each host's local session store (Codex `~/.codex/sessions/*.jsonl`, OpenCode `opencode.db`, Cursor where it exposes token counts) is read by one reader per host behind the existing `measure` ingest, rows carry the host slug, re-ingest is idempotent, and each reader has a fixture test. A host without token counts is documented as unsupported, not estimated.

### T50.1. More `cmd` filter families

From I-05. `rules/default.toml` covers grep, rg, sed, cat, make, curl, npm, pnpm, node on top of the built-in cargo/git/test/ls rules; python, pytest, pip, go, docker, kubectl, gh and friends pass through unfiltered.
Done when the families are chosen by `rtok discover`-style counts from real transcripts (the evidence goes into `research.md`), each new rule has a fixture with before/after bytes and keeps failures and the `expand <id>` trailer, and `Measurement` rows show the saving per family.

### T50.2. User filter drop-in directory and schema

From I-06. Users can already override rules through one user rules file, but there is no `rules.d/*.toml` drop-in, no published schema and no example, so writing a filter means reading `src/plugins/cmd/rules.rs`.
Done when every `*.toml` in a configured rules dir is merged after the defaults in name order, a malformed file is reported by `rtok config validate` and skipped at runtime (fail open), `docs/cmd-rules.md` documents every field with a worked example (and a site row), and tests cover merge order and a broken file.

### T50.3. Extra `read` modes

From I-07. `read` has full, lines, map and signatures. The measured Read tail (38–68 K char files) may still be served whole when only imports or code without comments are needed.
Done when a measurement on those files shows which extra mode (imports-only, comments-stripped, or none) saves tokens without losing the answer; each added mode goes through tree-sitter where a grammar exists, falls back to `full`, keeps the read cap and dedup, and has a test per language. If no mode wins, the card closes with the numbers.

### T50.4. Optional deny of native Grep and Glob

From I-08. lean-ctx denies the host's Grep/Glob to force its own tools; rtok's MCP `search`/`tree` are cheaper but the model still reaches for the native tools.
Done when `rtok doctor` reports the share of Read-class tokens spent in Grep/Glob, and an opt-in `guard` rule (off by default) denies them in PreToolUse with a pointer to `search`/`tree`, fails open when the MCP server is not installed, and is covered by hook tests. Default stays off unless the doctor numbers justify it.

### T51.1. Compress JSON and code inside the live zone

From I-09. `archive` rewrites only old `tool_result`s; huge JSON dumps and `data:` blobs in other live-zone fields stay whole every turn.
Done when a proxy-side pass shrinks such payloads losslessly (archived, `expand <id>`), only for content that is byte-stable across turns so the prompt cache holds, with a byte-stability test over a six-turn fixture on both wires and a `Measurement` row. Off by default until a bench shows cost per passed task does not rise.



### T51.3. Gemini wire in the proxy

From I-11. The proxy speaks Anthropic Messages and OpenAI Chat/Responses; Gemini `generateContent` / `streamGenerateContent` hosts cannot use rtok's proxy.
Done when `src/proxy/gemini.rs` implements the `Wire` adapter (usage, cached tokens, streaming passthrough byte-identical), routes by path, records usage rows like the other wires, and has body and stream tests against a mock upstream.

### T52.1. Query language over the graph index

From I-14. `graph` answers `symbol`, `callers`, `impact` and `outline`; composite questions (callers of X inside path Y of kind Z) take several calls.
Done when a measured transcript shows such chains, and a small filter syntax on an existing tool (not a fourth tool, to keep description tokens flat) answers them from the `symbols` edges with tests; otherwise the card closes with the evidence.

### T52.2. More grammars and compressed index payloads

From I-16. Tags cover Rust, TS, JS, Python, Dart, C and Go. Java, Kotlin, Swift, C#, Ruby and PHP repos get no `symbol`/`outline`, and large indexes store plain text.
Done when each added grammar is an optional feature (dependency reasons in the commit, creator approval for new crates) with a fixture test, and index payload compression is added only if a large repo's `rtok.db` size is measured before and after.

### T52.3. Ranked repo map at SessionStart

From I-28 (aider repo map). The most-referenced definitions could orient the model at session start.
Done when a P7-style A/B shows the map lowers cost per passed task; the map is ranked by reference count from `symbols`, fits a share of the D5 budget alongside `memory`, is byte-stable across turns, and is off by default until that A/B passes.

### T53.1. Coaching nudges under an A/B

From I-18. Short nudges ("do not re-read", "use expand") may cut waste, but they are re-read every turn and dilute instructions.
Done when an opt-in `inject` nudge set exists as data (D7), stays inside the D5 budget and byte-stable, and a P7-style A/B on the bench shows it does not raise cost per passed task; without that result it stays off.

### T53.2. Shell completions and man page

From I-20. `rtok` has a large clap surface but no completions or man page.
Done when `rtok completions <shell>` prints bash/zsh/fish/powershell completions and `rtok man` (or a build step) produces the man page, a trycmd snapshot covers one shell, and the README shows installation. Needs creator approval for `clap_complete` and `clap_mangen` before code.

### T53.3. Hook start without Security.framework

From I-32. On macOS the one binary links Security.framework and CoreFoundation for reqwest's platform verifier, costing about 1.3–1.5 ms of dyld time per hook spawn, as much as the hook's own work.
Done when the creator picks the trade-off (webpki roots with `use_preconfigured_tls` and dead-stripped dylibs, versus a second tiny hook binary), the choice is recorded as a decision, and the hook p95 before/after is measured and stored in `research.md`. Corporate CA support must be documented either way.

### T53.4. `just otel-check` against real backends

From I-33. OTel export is gated by mock collectors; the Jaeger 2.11 and Grafana `otel-lgtm` recipes in `docs/otel.md` were checked by hand once.
Done when `just otel-check` starts both containers on shifted ports, flushes a copy of a fixture ledger, and asserts through their APIs: Jaeger has `execute_tool` spans for `service=rtok`, Tempo answers the trace id, Prometheus has `rtok_calls_total`; it skips with a clear message when Docker is missing, and it stays out of `just check`.

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
