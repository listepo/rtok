# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T48.8 | todo | P2 | 3 | 0% | |
| T49.2 | todo | P2 | 4 | 0% | |
| T50.1 | todo | P2 | 3 | 0% | |
| T50.3 | todo | P3 | 3 | 0% | |
| T51.1 | in progress | P3 | 5 | 5% | OpenCode / Muse Spark 1.3 |
| T52.2 | todo | P3 | 3 | 0% | |
| T52.3 | in progress | P3 | 4 | 10% | OpenCode / Muse Spark 1.3 |
| T53.1 | in progress | P3 | 3 | 10% | OpenCode / Muse Spark 1.3 |
| T53.3 | in progress | P3 | 3 | 0% | OpenCode / Muse Spark 1.3 |
| T53.4 | todo | P3 | 2 | 0% | |
| T55.7 | todo | P3 | 1 | 0% | |
| T56.1 | todo | P2 | 2 | 0% | |
| T56.2 | todo | P2 | 3 | 0% | |
| T56.3 | todo | P2 | 3 | 0% | |
| T56.4 | todo | P3 | 2 | 0% | |

### T48.8. VS Code Copilot Chat host

From I-17. GitHub Copilot Chat in VS Code reads MCP servers from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in the VS Code profile dir, and agent mode may run hooks; T46.4 covered only the Copilot CLI and the desktop app.
Done when the VS Code user dir per OS (Code, Code - Insiders) is resolved, `rtok agents install vscode` registers `servers.rtok`, hooks are added only if VS Code documents a hook file the T46.3 Copilot mapping can serve, remove keeps foreign servers, and the host joins the e2e matrix, config and docs.

### T49.2. Ingest Codex, OpenCode and Cursor session logs

From I-03. `measure` reads Claude Code JSONL only; the other hosts reach the `usage` table only when they go through `rtok proxy`, so their sessions without the proxy are invisible to `stats`, the TUI and the dashboard.
Done when each host's local session store (Codex `~/.codex/sessions/*.jsonl`, OpenCode `opencode.db`, Cursor where it exposes token counts) is read by one reader per host behind the existing `measure` ingest, rows carry the host slug, re-ingest is idempotent, and each reader has a fixture test. A host without token counts is documented as unsupported, not estimated.

### T50.1. More `cmd` filter families

From I-05. `rules/default.toml` covers grep, rg, sed, cat, make, curl, npm, pnpm, node on top of the built-in cargo/git/test/ls rules; python, pytest, pip, go, docker, kubectl, gh and friends pass through unfiltered.
Done when the families are chosen by `rtok discover`-style counts from real transcripts (the evidence goes into `research.md`), each new rule has a fixture with before/after bytes and keeps failures and the `expand <id>` trailer, and `Measurement` rows show the saving per family.

### T50.3. Extra `read` modes

From I-07. `read` has full, lines, map and signatures. The measured Read tail (38–68 K char files) may still be served whole when only imports or code without comments are needed.
Done when a measurement on those files shows which extra mode (imports-only, comments-stripped, or none) saves tokens without losing the answer; each added mode goes through tree-sitter where a grammar exists, falls back to `full`, keeps the read cap and dedup, and has a test per language. If no mode wins, the card closes with the numbers.

### T51.1. Compress JSON and code inside the live zone

From I-09. `archive` rewrites only old `tool_result`s; huge JSON dumps and `data:` blobs in other live-zone fields stay whole every turn.
Done when a proxy-side pass shrinks such payloads losslessly (archived, `expand <id>`), only for content that is byte-stable across turns so the prompt cache holds, with a byte-stability test over a six-turn fixture on both wires and a `Measurement` row. Off by default until a bench shows cost per passed task does not rise.

Execution plan (OpenCode / Muse Spark 1.3; scope from I-09: NON-`tool_result` content — nested JSON dumps + `data:` blobs in user content blocks; plain live-tail text/code stays (model is working with it; last-2-turns rule); Responses/Gemini deferred with default-empty, card's "both wires" = Anthropic + Chat):
1. SDK `wire.rs` (additive, defaulted): `BlobRef { content, turn }` (no provider id — keyed by content hash) + `ToolResults::live_blobs(req)` (empty default) + `WireRequest::live_blobs()` passthrough.
2. `Anthropic`/`OpenAiChat` `impl ToolResults`: override `live_blobs` — user blocks only, SKIP `tool_result` blocks (archive owns them), yield text/image-document base64 + big-JSON text strings with turn counting mirrored from `tool_results`.
3. `archive/mod.rs`: `rewrite_blobs()` — `[plugins.archive] live_blobs = false` gate (default off: zero behavior change); eligible turn >= 2 (proxy invariant, not keep_turns); candidate = `data:`/base64 or JSON-parseable text over `min_tokens`; key `blob:{sha256}`, `pointer()` reuse with kind `live_blob`, expanded-skip, shared `record_run` tail with `rewrite()`.
4. Config key in mod.rs + default.toml + docs/config.md + config-show snapshot (env free, no flag).
5. `tests/proxy.rs`: six-turn inline requests (stable JSON blob in user turns) on Anthropic + Chat, compress mode — two identical POSTs byte-identical upstream; eligible turns pointered, turns 0-1 untouched, archive rows 0, `live_blob` Measurements present, `rtok expand <id>` recovers the original.
6. Verify: fmt, clippy, targeted nextest, build-min, jscpd. ~10 files — deviation noted.

### T52.2. More grammars and compressed index payloads

From I-16. Tags cover Rust, TS, JS, Python, Dart, C and Go. Java, Kotlin, Swift, C#, Ruby and PHP repos get no `symbol`/`outline`, and large indexes store plain text.
Done when each added grammar is an optional feature (dependency reasons in the commit, creator approval for new crates) with a fixture test, and index payload compression is added only if a large repo's `rtok.db` size is measured before and after.

### T52.3. Ranked repo map at SessionStart

From I-28 (aider repo map). The most-referenced definitions could orient the model at session start.
Done when a P7-style A/B shows the map lowers cost per passed task; the map is ranked by reference count from `symbols`, fits a share of the D5 budget alongside `memory`, is byte-stable across turns, and is off by default until that A/B passes.

Execution plan (OpenCode / Muse Spark 1.3): `bench` shells to `claude -p` (LLM-gated), so no A/B pass is obtainable in-task → implement OFF BY DEFAULT, record the outcome (stays off). One key `plugins.graph.map_tokens = 0` (0 = off; nonzero = token cap, the D5-budget share next to `memory.recall_tokens`); no indexing on the hook path (map reads existing rows only, empty index → no injection). `Store::symbol_top_refs(root, limit)`: names with ref counts + one def site, ORDER BY refs DESC, name ASC (byte-stable). `Graph::session_start` offers priority-1 `repo map` lines trimmed to the cap. Files: `src/plugins/graph/mod.rs`, `src/store/symbols.rs`, `src/config/mod.rs` + `config/default.toml` + `docs/config.md` (D12). Tests: ranked order, byte-stability, cap trim, off-by-default (no injection at 0), SessionStart hook e2e on/off. Measure map tokens on this repo for the record. Verify in isolation (main red on concurrent WIP).

### T53.1. Coaching nudges under an A/B

From I-18. Short nudges ("do not re-read", "use expand") may cut waste, but they are re-read every turn and dilute instructions.
Done when an opt-in `inject` nudge set exists as data (D7), stays inside the D5 budget and byte-stable, and a P7-style A/B on the bench shows it does not raise cost per passed task; without that result it stays off.

Execution plan (T53.1, OpenCode / Muse Spark 1.3):
1. `modes/nudges.md` (new, data per D7): re-read/expand/outline-first/search-before-Grep nudges, ≤250 tokens like terse/yagni.
2. `src/plugins/inject/mod.rs`: `NUDGES` const + `builtin("nudges")` arm (same resolution as terse/yagni; opt-in via modes list, default off); test: ≤250 tok, SessionStart-once + byte-stable, absent from UserPromptSubmit.
3. Evidence: hook SessionStart bytes on/off (measured), dry `rtok bench` both ways (pass parity; zeros without RTOK_BENCH_LIVE), recorded in `research.md`; live cost gate stays open → default off. No live bench (needs API spend + approval — not run).
4. Verify in isolated worktree: fmt, clippy `-D warnings`, nextest (inject, hook e2e).

### T53.3. Hook start without Security.framework

From I-32. On macOS the one binary links Security.framework and CoreFoundation for reqwest's platform verifier, costing about 1.3–1.5 ms of dyld time per hook spawn, as much as the hook's own work.
Done when the creator picks the trade-off (webpki roots with `use_preconfigured_tls` and dead-stripped dylibs, versus a second tiny hook binary), the choice is recorded as a decision, and the hook p95 before/after is measured and stored in `research.md`. Corporate CA support must be documented either way.

Execution plan (OpenCode / Muse Spark 1.3; decision as given: webpki + `use_preconfigured_tls`, single binary): verified in reqwest 0.13.4 source that feature removal cannot work — `rustls_platform_verifier::Verifier::new` is referenced ungated under `__rustls`, and 0.13 has no webpki-roots feature, so `__rustls`-only does not compile; the `rustls` feature stays and the verifier becomes never-called-but-linked (otool decides the fact). 1) `Cargo.toml` + `rustls 0.23` + `webpki-roots 1` + `rustls-pemfile 2` (one-line reason in commit), `toolchain.md` + workspace `rust.md` rows; 2) `src/tls.rs`: `preconfigured()` builds `rustls::ClientConfig` (aws-lc-rs provider) from Mozilla roots, extended with `SSL_CERT_FILE` PEM bundle when set (fail closed with context — curl parity); unit tests (roots non-empty, bundle loads, missing/empty errors); 3) `src/proxy/mod.rs` + `src/otel/export.rs`: both client builders add `.use_preconfigured_tls(...)` (one shared helper, no duplication); 4) `research.md` T53.3 section: decision record + otool before/after + release hook p95 before/after via `tests/latency.rs` serialized + binary size; 5) `docs/config.md` [proxy] corporate-CA paragraph. Verify in isolation worktree (HEAD + own files): fmt, clippy `-D warnings`, nextest (tls/proxy/otel scope); release latency + otool before/after. Commit own hunks only.

### T53.4. `just otel-check` against real backends

From I-33. OTel export is gated by mock collectors; the Jaeger 2.11 and Grafana `otel-lgtm` recipes in `docs/otel.md` were checked by hand once.
Done when `just otel-check` starts both containers on shifted ports, flushes a copy of a fixture ledger, and asserts through their APIs: Jaeger has `execute_tool` spans for `service=rtok`, Tempo answers the trace id, Prometheus has `rtok_calls_total`; it skips with a clear message when Docker is missing, and it stays out of `just check`.

### T55.7. Stats `strip_prefix_cd` and quoted paths

From review 2026-09-17. `src/measure/stats.rs` `strip_prefix_cd` splits the path on the first whitespace, so `cd 'My Documents' && git status` / `cd "C:\Program Files\…" && …` does not strip cleanly and family bucketing mis-attributes. `strip_prefix_env` already understands quotes; `guard::strip_cd_and` finds `&&` and is fine.
Done when `strip_prefix_cd` accepts single- and double-quoted path segments (malformed quotes fail-open), with unit tests for spaced quoted paths.

### T56.1. Test VFS helper and convention

**All tests must prefer a virtual filesystem** over host `TempDir` / raw `std::fs` as the primary approach. Goal: unit tests run against an in-memory FS so they do not depend on real disk layout, and Windows/macOS path quirks (case fold, spaced profiles) can be simulated. `src/testutil.rs` now ships a thin `Vfs` (path → bytes).
Done when this decision is recorded (D29), AGENTS.md notes the rule, `Vfs` covers write/read/len/paths, and at least one read/cmd/graph unit test uses it with no host temp dir.

### T56.2. Migrate read/search unit tests to VFS

Hottest filesystem tests first: `display_rel` / search size-gate logic should use `Vfs` or pure `Path` values. WalkBuilder-backed integration may stay on disk until a walk adapter exists (T56.4).
Done when the pure path and size-gate tests need no host temp dir, and remaining disk tests are listed as follow-ups.

### T56.3. Migrate cmd/setup path tests to VFS

Quoting tests are already pure strings; setup/agent install tests that write hook files should use `Vfs` (or a directory trait) where practical.
Done when new file-touching unit tests in setup/agents use `Vfs` (or document why a disk fixture remains), and one legacy test is migrated as a template.

### T56.4. Optional walk/VFS adapter for search/tree

If search/tree keep needing real walks, introduce a narrow trait (metadata + read bytes + list dir) with a `Vfs` impl so oversized-file and relative-path tests run without host disk.
Done when search/tree unit tests for the size-cap and relative-path cases can run against `Vfs`, or the card closes with a measured reason to keep WalkBuilder-on-disk.


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
| D29 | **Unit tests prefer a virtual filesystem (`testutil::Vfs`) over host TempDir/std::fs.** Pure path/content/size logic must not require real disk; Windows/macOS quirks are simulated in Vfs. Migrate hottest suites first (read/search/cmd/setup) as T56.x — not a big-bang rewrite of e2e. | Hermetic tests; reproducible CI; path-case and spaced-path bugs (T55) need a simulated FS. |

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
---

## Review 2026-09-17 — bug hunt (post #37–#47)

Scope: `origin/main` after cross-platform agent fixes #37–#47. Local WIP from other agents was stashed (`preserve-other-agents-wip-before-docs-review-bugs-plan`) and not reviewed. No code fixes in this pass — findings tracked as T55.x.

### Blockers

None for the macOS/Linux happy path on current main. Windows correctness gaps below are P1.

### Should fix

1. ~~**T55.1**~~ — done in #49 (`display_rel` case-insensitive strip).
2. ~~**T55.2**~~ — done in #49 (`never_wrap` case-insensitive stem).
3. ~~**T55.3**~~ — done in #49 (`file_uri` percent-encoding).
4. ~~**T55.4**~~ — done in #49 (host-shell-safe wrap / cmd quoting).
5. ~~**T55.5**~~ — done in #49 (`search_max_bytes`).
6. ~~**T55.6**~~ — done in #49 (`call` in mcp.cmd).

### Nits

1. **T55.7 — `strip_prefix_cd` and quoted spaced paths** (still open).
2. **`expand::parse_range` when start > line count** — empty slice quietly; optional clamp.
3. **`guard::strip_wrap`** — updated in #49 for PowerShell `''`.

### Residual still open (called out before)

- T55.1–T55.6 closed by #49.
- PATH / single-quote parsers — rtok: T55.7; larger residual in ketch.
- Test VFS migration — D29 / T56.x (`Vfs` helper in #49).
- T48.3 still todo: Cursor plugin mcp.json still bare `rtok mcp`.

### Out of scope this pass

Concurrent agent WIP on local `main` (stashed as `preserve-other-agents-wip-before-docs-review-bugs-plan`). Half-finished T48–T53 cards on that WIP were not judged as shipped bugs.
