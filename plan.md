# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T73 | in progress | P1 | 2 | 0% | Cursor / grok 4.6 |
| T74 | todo | P2 | 1 | — | — |
| T75 | todo | P1 | 2 | — | — |
| T76 | todo | P2 | 2 | — | — |

### T73. Cycle demon surfaces around a binary replace

Creator 2026-09-19. `ketch upgrade` kills PIDs holding the binary but does not write the demon stop marker, so the supervisor can respawn mid-replace and keep SQLite (`rtok.db` WAL) locked. `rtok-update` does not stop anything. HTTP+WS are `web`; MCP is `mcp`; SQLite is released when those processes exit.

**Plan.** Hidden `rtok demon upgrade`: snapshot kernel-live services (`rows` flock), `stop` them (marker + wait), run `ketch upgrade rtok --yes` (or `rtok-update`, or `RTOK_UPDATE_CMD` in tests), `start` the same set even if replace failed. Reuse `stop`/`start`. Do not revive T40 `demon update`. Spawn the supervisor from the on-disk path when `current_exe` is gone after replace.

Check: `tests/demon.rs` — live mcp is down during the replace command (no state file), up afterwards; a failing replace still leaves mcp running; `just check`.

**Evidence (2026-09-20).** The Check's first clause is live: one full gate failed `upgrade_stops_before_replace_and_starts_after` — the replace stub's guard `[ ! -f $RTOK_HOME/demon/mcp.json ] || exit 2` fired (`tests/demon.rs:197`, `update command failed (exit status: 2)`), i.e. the update command ran while the mcp state file still existed. Six other runs the same day passed (two local full suites, PR-CI ×2, push-CI ×2, release verify ×2). Harden the stop-wait: wait for the state file to be gone and re-check immediately before invoking the update command, not just for the stop marker.

### T74. Make the two load-sensitive gate tests deterministic

Two tests fail a full `just check` under parallel CPU load and pass standalone, so a green gate still rerolls dice:
- `tui::app::tests::space_toggles_the_selected_plugin_through_config_set` — nextest `terminate-after = 3` killed it at 180 s once on 2026-09-20 (full gate on a loaded machine); the immediately following full run and every scoped run passed. The key-injection → frame-assert waits carry no internal deadline, so contention turns into a suite-level timeout.
- `otel::hooks_stay_fast_with_an_unreachable_endpoint` — latency budget; failed two gates on 2026-09-17, passed standalone every time.

(Related but different, fixed 2026-09-20: `rtok::cli_trycmd cli` "panics" after every release bump — `tests/trycmd/version.stdout` / `man.stdout` pinned the literal version. Now wildcarded `rtok [..] ([..])` / `v[..] ([..])`, so a bump can't break the gate again.)

Done when both tests bound their own waiting (deadline + tolerant retry to that deadline in the tui TestBackend loop and in the otel latency assert) so a loaded runner slows them instead of failing them — no `--test-threads` masking: the point is the wait, not the machine. Check: two full suites running concurrently on one busy machine — zero flakes across three runs.

### T75. `agents uninstall` leaves the host marked installed (green check stuck)

Creator 2026-09-21. After `rtok agents uninstall <host>` (reported with a "cloud" plugin uninstall), the host/plugin is either not actually removed or the UI still shows it as installed — the green checkmark stays on.

**Repro.** Run `rtok agents uninstall <host>` (example path: uninstall involving a "cloud" plugin / host install). Open the agents/plugins UI (TUI or web) and look at that row.

**Expected.** The host/plugin is uninstalled: hooks/MCP/proxy/plugin link gone, and the green installed checkmark is cleared.

**Actual.** The entry still looks installed — green checkmark stuck — and/or the uninstall did not take effect on disk.

**Plan.** Trace `AgentCmd::Uninstall` → `setup_host(..., SetupArgs::removing)` and whatever feeds the agents/plugins list `enabled` / installed mark. Make uninstall write the same source the UI reads (config + on-disk host files), then refresh or re-read so the checkmark clears. Add a regression test: uninstall → list/UI snapshot shows not installed.

Check: uninstall a previously installed host; UI checkmark off; `rtok agents list` / info agree; `just check`.


### T76. Offer to restart the host after `agents install` / `uninstall`

Creator 2026-09-21. After `rtok agents install <host>` or `rtok agents uninstall <host>` finishes (hooks/MCP/proxy/plugin link already written or removed), ask whether to restart that application or agent. Yes → stop it, then start it again so the new config is live. No → leave the process alone and exit.

**Timeout logic (no baked-in default wait).** Config key `restart_prompt_timeout_seconds` (nesting to match existing config style) **defaults to `0`**. **`0` means "not set"** — **no timeout** — wait **indefinitely** for the user's answer. A timeout applies **only** when the user explicitly sets a **positive** number in config; then **no response within that time = No** (do **not** restart the agent). There is **no** 30-second or 60-second default and **no** fallback timeout when the key is absent (absent ≡ `0`).

**CLI spinner while waiting (agent-ready).** While rtok waits for the yes/no answer in the terminal, show a spinner on the **left** of the input line:

1. **Placement.** Glyph in the leftmost column of the prompt row, immediately before the input cursor. Line shape: `[spinner] > ` or `[spinner] Your choice: ` (spinner, space, prompt label / `>`, space, then the user's typing). Not above/below the prompt, not after the cursor.
2. **Glyph.** Prefer braille-dot frames `⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏`, cycle ~every 80 ms. If the terminal lacks Unicode / braille, fall back to ASCII `- \ | /`.
3. **Lifecycle.** Start the moment the restart prompt is printed and waiting begins. Stop and **clear** the glyph (erase it; leave a clean prompt line) as soon as the user types anything, presses Enter, or a positive timeout fires. Do not leave a stale spinner character.
4. **Interaction with timeout.** `restart_prompt_timeout_seconds = 0` → spinner runs until input (no auto-no). Positive timeout → spinner runs until input **or** timeout; on timeout clear spinner, treat as No, do not restart.
5. **No flicker / no new lines.** Redraw in place with carriage return (`\r`) and/or ANSI clear-to-end-of-line (`\x1b[K`); never print a fresh line per frame.

**Repro / flow.** Run install or uninstall for a running host. After the config write completes, print the restart prompt with the left-side spinner and wait per the timeout rules above.

**Expected.**
- Prompt + left spinner while waiting.
- Yes: stop then start the host/agent (only after install/uninstall writes finished).
- No: no restart; mention manual restart if the host caches config.
- **No response + positive timeout = No:** clear spinner; do **not** restart; exit successfully.
- **Timeout 0 / key absent:** wait forever (spinner until input); never auto-no.
- Config changes the wait without a rebuild.

**Actual (today).** Install/uninstall edit files and return; no restart offer, no timed prompt, no wait spinner.

**Plan.** Add `restart_prompt_timeout_seconds` defaulting to `0`. End of `setup_host` (install + remove): blocking or timed read with the in-place left spinner. Per-host restart via existing helper or a documented stop/start matrix. Skip prompt under `--dry-run` and non-interactive CI (`!stdin.isatty()` or `--no-restart` / `--yes` — pick one). Tests: yes / no / positive-timeout-silence→no / `0` waits (no auto-no) / spinner cleared on input and on timeout / ASCII fallback path if feasible.

**Check (acceptance).**
- [ ] Yes path: stop then start after config write.
- [ ] No path: process untouched.
- [ ] `restart_prompt_timeout_seconds` defaults to `0`; absent key ≡ `0`; **no** 30s/60s fallback.
- [ ] Positive timeout + silence → No, no restart.
- [ ] `0` → wait indefinitely; spinner until input; no auto-no.
- [ ] Spinner leftmost on the prompt row (`[spinner] > ` / `[spinner] Your choice: `).
- [ ] Braille sequence `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` ~80 ms; ASCII `-\|/` fallback without Unicode.
- [ ] Spinner starts with prompt; cleared on first key / Enter / timeout (no stale glyph).
- [ ] Redraw in place (`\r` / ANSI `\x1b[K`); no per-frame newlines.
- [ ] Dry-run / non-interactive: no prompt, no spinner, no restart.
- [ ] `just check`.

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
