# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T57.1 | todo | P3 | 3 | 0% | |
| T59.5 | todo | P3 | 3 | 0% | |
| T61.2 | todo | P3 | 3 | 0% | |



### T57.1. Flag-aware `guard` read-only classes

From I-38 (promoted 2026-09-17). `guard::read_only` decides which Bash calls get a dedup key from a fixed stem list (`ls cat head tail grep rg find tree wc` plus `git status|log|diff|show|branch`). It ignores flags, redirections and pipes, so it errs both ways:
- **Writers keyed as read-only** (correctness): `find . -name x -delete`, `cat a > b`, `grep x > out`, `ls | xargs rm`, `tail -f log` are keyed, so they never reach the "mutating Bash clears every `bash` key" arm; a following repeat of `ls` or `cat b` is denied with a stale archive (fail-open violation, same family as T55.8).
- **Repeats never keyed** (missed savings): `sed -n 1,40p f`, `jq . f`, `awk '{print $1}' f`, `git rev-parse HEAD`, `cargo metadata`, `wc -l` under a pipe.
Done when:
1. Evidence first: stem and flag counts over real transcripts (`[stats] transcripts_dir`, the `measure::stats::collect` path `doctor` already uses) for Bash calls that repeat inside `window_turns`, recorded in `research.md` with the date and command; stems are added or removed only with a count behind them.
2. `read_only` becomes flag-aware: a command is keyed only if its first stem is read-only **and** it has no writer marker — `>` / `>>` redirection, `| tee`, a pipe into a non-read-only stem, `find … -delete` / `-exec`, `sed -i` / `--in-place`, `tail -f`. Any command with a writer marker takes the mutating path and clears the `bash` keys. New read-only stems come from step 1 (expected: `sed` without `-i`, `jq`, `awk`, `git rev-parse`, `cargo metadata`). Parsing stays first-word + marker scan; no shell grammar (`cmd/AGENTS.md`).
3. Unit tests in `src/plugins/guard/mod.rs`: `sed -n` keyed and `sed -i` mutating; `find -delete` mutating; `cat a > b` mutating; `tail -f` never keyed; `cat a | grep b` keyed; `ls | xargs rm` mutating; and the false-deny Check: `ls` → `find . -delete` → `ls` is allowed.
4. `guard` deny Measurements (`kind = guard`) on the hook e2e fixture before and after, so the change in deny count is a measured row, not a claim. Off-by-default is not needed: the change only removes wrong denies and adds keyed repeats that already carry a retrievable archive.
Depends on T55.8 and T55.9 (guard key ownership and cwd) landing first, so the tests do not pin two behaviors at once.


### T59.5. Byte-stable `tools[]` description rewrite in the proxy

From I-45 (Portkey / LiteLLM "tool description compression + allowlist", 18–28 % claimed, unverified). Redundant on Claude Code with Tool Search deferral (`doctor` flags `mcp_tool_search_disabled`); a host without deferral pays every schema on every turn at cache-read price.
Done when:
1. Evidence: `doctor` already prices descriptions per server; a `stats` row shows description tokens × turns per session for a host without deferral, recorded in `research.md`. Below 3 % of session input, the card closes with the number.
2. Proxy option `proxy.tools_rewrite = { max_description_tokens = N, allow = [..], deny = [..] }`, off by default: descriptions truncated at a sentence boundary to N tokens (the tokenizer `measure` uses), tools outside `allow` or inside `deny` dropped from `tools[]`; the rewrite is deterministic so the cached prefix changes once per session, and `input_schema` is never touched.
3. `Measurement { plugin = "proxy", kind = "tools_rewrite" }` per request with before/after description bytes; wire tests for Anthropic and OpenAI Chat request shapes; a tool the model then calls that was dropped by `deny` is forwarded unchanged (the proxy never blocks a call).



### T61.2. Archive skill bodies outside the live zone

From I-51 (`research.md` §10.7). A skill body is re-sent in every later request of its session; the `archive` plugin already replaces old tool results with byte-stable pointers, keyed by `tool_use_id`, but a skill body is a user text block, not a tool result, so it is never touched.
Gated on T61.1: proceeds only when the `resident` column shows skill bodies ≥ 2 % of input tokens over a 30-day window on this machine; otherwise the card leaves the plan for `ideas.md` with the number.
Done when the wire normaliser yields a `SkillRef { id: <tool_use_id of the preceding "Launching skill" result>, name, content, turn }` for a user text block that starts with `Base directory for this skill:` right after that result; `archive::rewrite` treats it like a result outside `keep_turns` (archive once, pointer `[archived <id>: skill <name> · N lines · expand(<id>)]`, byte-identical on every later request, `Measurement { plugin = "archive", kind = "skill" }`); `expand <id>` returns the body; a proxy test replays a 3-turn fixture and asserts the pointer appears on turn `keep_turns + 1` and the body never re-archives; off switch `[plugins.archive] skills = true` documented next to `live_blobs`.







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
| D30 | **HTTPS uses webpki Mozilla roots (`use_preconfigured_tls`); one binary.** Corporate CAs via `SSL_CERT_FILE` (curl parity, fail closed). reqwest 0.13 `rustls` still links `rustls-platform-verifier`; `otool` showed Security.framework still present (T53.3). A second hook binary was rejected. | I-32: 1.3–1.5 ms dyld; dropping the `rustls` feature does not compile. |

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

1. ~~**T55.7**~~ — done (`skip_word`; quoted `cd` paths bucket by family).
2. **`expand::parse_range` when start > line count** — empty slice quietly; optional clamp.
3. **`guard::strip_wrap`** — updated in #49 for PowerShell `''`.
4. **T55.8 / T55.9 / T55.10** — filed from the T55.7 code read: guard `read:` keys survive a mutating Bash, guard Bash key cwd-blind, three copies of `cmd_stem`.

### Residual still open (called out before)

- T55.1–T55.6 closed by #49.
- PATH / single-quote parsers — rtok: T55.7 closed; larger residual in ketch.
- Guard false denies — T55.8 (P2), T55.9; flag-aware read-only classes promoted from I-38 as T57.1.
- Test VFS migration — D29 / T56.x (`Vfs` helper in #49).
- T48.3 still todo: Cursor plugin mcp.json still bare `rtok mcp`.

### Second pass (2026-09-17, later the same day)

Scope: hook dispatcher/types, guard, cmd (hook/run/rules/formatters), read (mod/cache/hook/search), archive, proxy (mod/wire/anthropic/openai_chat/semantic_cache), expand, store (archive/decisions/read_cache paths), mcp session handling. Four findings reproduced with a scratch integration test (written, run, deleted — `cargo nextest run --test zz_review_repro` → 4 failed exactly as predicted); two filed from code read. No code fixes in this pass — findings tracked as T55.11–T55.16, propositions as I-39/I-40.

- ~~**T55.11 (P2)**~~ — done: `expand` never froze the owning session's pointer (every expand caller runs under session `expand` / `mcp-<pid>`, decisions belong to the proxy session; expand rate stayed 0, toon attribution dead). Reproduced.
- **T55.12 (P2)** — Windows `wrap_quote` `''` quoting is wrong under Git Bash (Claude Code's Windows shell): apostrophes silently dropped from the rewritten command. Code read.
- ~~**T55.13 (P3)**~~ — done: Copilot `Read` inputs use `path`; `guard::cache_key` and `read::hook` match `file_path` only → dedup and advice never fire on that host. Reproduced.
- ~~**T55.14 (P3)**~~ — done: the semantic-cache key ignored tool_result text; two requests differing only in tool results hashed identically and were both eligible under defaults. Reproduced.
- **T55.15 (P3)** — `live_blobs` overwrites image/document `source.data` / `image_url.url` with pointer text → invalid request when the flag is on. Reproduced.
- ~~**T55.16 (P3)**~~ — done: the guard deny read the full archived body (and estimated over it) on the ≤ 10 ms PreToolUse path. Code read.

### Out of scope this pass

Concurrent agent WIP on local `main` (stashed as `preserve-other-agents-wip-before-docs-review-bugs-plan`). Half-finished T48–T53 cards on that WIP were not judged as shipped bugs.


---

## Note 2026-09-17 — testing library candidates

Shared catalog: [`listepo/rust.md`](../../rust.md) → *Testing candidates*.
Catalog only — no blanket `Cargo.toml` adds (Working agreement: justify each dep).

Fits for rtok (1–3):

1. `vfs` (crates.io) — evaluate against in-house T56 `src/testutil.rs` `Vfs`
   before adopting; mandate stays "prefer VFS over host TempDir".
2. `mockall` — trait mocks for provider / plugin host seams when hand fakes
   get noisy (`httpmock` stays for HTTP).
3. `tokio-test` — async unit helpers beyond `#[tokio::test]` where time/task
   control matters.

Already covered: `assert_cmd`, `divan`, `httpmock`, `insta`, `rstest`,
`trycmd`, `similar`. Skip `test-case` / `expect-test` / `mockito` duplicates;
`testcontainers` / `bolero`/`honggfuzz` only if a measured e2e/fuzz gap appears.
