# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T48.8 | todo | P2 | 3 | 0% | |
| T50.1 | todo | P2 | 3 | 0% | |
| T50.3 | todo | P3 | 3 | 0% | |
| T52.2 | todo | P3 | 3 | 0% | |
| T52.3 | todo | P3 | 4 | 10% | |
| T53.1 | todo | P3 | 3 | 10% | |
| T53.3 | todo | P3 | 3 | 0% | |
| T57.1 | todo | P3 | 3 | 0% | |
| T58.1 | todo | P2 | 3 | 0% | |
| T58.2 | todo | P2 | 3 | 0% | |
| T58.5 | todo | P3 | 3 | 0% | |
| T59.5 | todo | P3 | 3 | 0% | |
| T59.6 | todo | P3 | 3 | 0% | |
| T60.1 | todo | P2 | 3 | 0% | |
| T60.2 | todo | P2 | 3 | 0% | |
| T60.3 | todo | P3 | 3 | 0% | |
| T60.4 | todo | P3 | 4 | 0% | |
| T60.5 | todo | P2 | 3 | 0% | |
| T61.2 | todo | P3 | 3 | 0% | |
| T62.3 | todo | P3 | 3 | 0% | |
| T63.1 | todo | P3 | 3 | 0% | |
| T64.1 | todo | P3 | 3 | 0% | |
| T65.1 | todo | P2 | 3 | 0% | |
| T65.2 | todo | P3 | 3 | 0% | |
| T68.5 | todo | P2 | 3 | 0% | |
| T68.6 | todo | P3 | 3 | 0% | |
| T68.9 | todo | P2 | 3 | 0% | |
| T69.2 | todo | P3 | 3 | 0% | |
| T69.3 | todo | P3 | 3 | 0% | |
| T70.1 | todo | P2 | 3 | 0% | |
| T70.3 | todo | P3 | 4 | 0% | |
| T70.4 | todo | P2 | 3 | 0% | |
| T70.5 | todo | P3 | 3 | 0% | |
| T70.6 | todo | P3 | 3 | 0% | |
| T71.1 | todo | P3 | 3 | 0% | |
| T71.2 | todo | P3 | 3 | 0% | |

### T48.8. VS Code Copilot Chat host

From I-17. GitHub Copilot Chat in VS Code reads MCP servers from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in the VS Code profile dir, and agent mode may run hooks; T46.4 covered only the Copilot CLI and the desktop app.
Done when the VS Code user dir per OS (Code, Code - Insiders) is resolved, `rtok agents install vscode` registers `servers.rtok`, hooks are added only if VS Code documents a hook file the T46.3 Copilot mapping can serve, remove keeps foreign servers, and the host joins the e2e matrix, config and docs.

### T50.1. More `cmd` filter families

From I-05; re-scoped by the competitive gap review (`research.md` §9.3, "Command output"). Today: formatters for cargo/git/pytest/jest/vitest/go test/ls/find/tree, nine TOML rules (`rules/default.toml`: grep, rg, sed, cat, make, curl, npm, pnpm, node), and `Rule::default()` (40 lines, head 10 / tail 10, dedupe) for every other stem — so docker, kubectl, gh, aws, pip, python, mvn, gradle, dotnet, tsc, eslint are capped, not passed through, but their error lines and summaries are cut by position, not by meaning. rtk ships 100+ per-command filters; the parity target is a per-family rule for every family that carries real bytes, each one measured. Rules are data, so this task adds TOML and fixtures, no Rust.
Done when:
1. Evidence first: `rtok stats` over real transcripts ranks Bash families by after-bytes where `Measurement.kind = rule` fell back to the default rule (`bash_families` split by kind; a `stats` column, not a new command); the top-20 land in `research.md` with date and command.
2. One `[stem]` rule per family from that list (expected from rtk's list and §2: docker / docker compose, kubectl, gh, aws, pip / uv, python tracebacks, go build / vet, cmake / ctest, mvn / gradle, dotnet, tsc, eslint, brew / apt), each with `keep` patterns for its error and summary lines and a `tests/cmd_golden` fixture with before/after bytes that keeps failures and the `expand <id>` trailer.
3. `Measurement` rows per family show the saving; the family table in the cmd docs page cites them. A family whose rule does not beat the default rule on its fixture is not added (the default already wins there).
4. Families where a rule cannot keep the signal (structured tables, grouped diagnostics) are listed in the card for T58.5, with the fixture that shows why.

### T50.3. Extra `read` modes

From I-07. `read` has full, lines, map and signatures. The measured Read tail (38–68 K char files) may still be served whole when only imports or code without comments are needed.
Done when a measurement on those files shows which extra mode (imports-only, comments-stripped, or none) saves tokens without losing the answer; each added mode goes through tree-sitter where a grammar exists, falls back to `full`, keeps the read cap and dedup, and has a test per language. If no mode wins, the card closes with the numbers.

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

### T58.1. `read` delta since last read

From the competitive gap review (`research.md` §9.3, §9.4 item 2; idea I-41; precedent: lean-ctx `diff` read mode, token-optimizer-mcp delta reads). Read is 15 % of tool-result tokens on the measured workload and the top single results are Reads. The sha256 dedup already answers an unchanged re-read with one line; a re-read of a file that changed since (typically after an Edit) still returns the whole file. The previous read's archive id is already stored, so a unified diff against it is the lossless short form.
Done when:
1. Evidence first: over real transcripts (`measure::stats::collect`) count Read calls of a path already read in the same session with an Edit/Write to that path in between, and their bytes; record in `research.md` §2 with date and command. Below 3 % of Read bytes → close the card with the number and no code.
2. MCP `read` (and the PreToolUse advice for native Read) answers such a re-read with a unified diff against the archived previous content plus that archive id; full content when the diff is not below `read.delta_max_ratio` (default 0.6) of the file or the previous archive is gone. Lossless: `expand <id>` of the new result returns the full file.
3. `Measurement` rows `plugin = read`, `kind = delta`, before = full bytes, after = diff bytes. Vfs unit tests: unchanged → existing "unchanged since" line; small change → hunks; large change → full; missing archive → full; CRLF preserved.
4. Byte-stable for the same file state; `read.delta = true` by default (safe because of the full fallback), documented in the read plugin's docs page with the measured row from step 1.
5. Parity with lean-ctx: its `diff` mode is opt-in per call and its unchanged re-read costs ~13 tokens (own README). rtok's delta is automatic (no mode to remember) and also reachable as `mode = "diff"` for the edit → verify flow; the unchanged-re-read line is measured on the same fixture and stays ≤ 13 tokens or the card says why.

### T58.2. Compaction checkpoint on every host, with archive ids

From the competitive gap review (`research.md` §9.2, §9.4 item 3; idea I-42). What exists (T2.5): on Claude Code `agents install` registers `PreCompact` and `PostCompact`; `checkpoint::save` stores the last 20 prompts, touched paths and 8 error lines as a memory note, and `inject::session_start` re-emits it (priority 9) plus the modes when `source == "compact"`. Two gaps remain. (a) No other host registers its compaction event — Codex (`PreCompact`/`PostCompact`), Cursor (`preCompact`), Gemini CLI (compression hook), Copilot CLI (auto-compact at 80 %) are listed in `research.md` §9.2 as of 2026-09-17, ZCode has none — so on those hosts the modes and the checkpoint vanish after the summary. (b) The checkpoint carries no archive ids, so `expand <id>` of a tool result that the summary dropped needs the id from a transcript the model no longer sees. Neither rtk, headroom nor caveman handle compaction at all (§9.3), so closing (a) and (b) is "better", not parity.
Done when:
1. Evidence: compactions per session counted from transcripts by `rtok stats` (a `compact` count next to the session rows) and recorded in `research.md`; the current checkpoint's injected bytes on the T2.5 fixture recorded as the baseline.
2. `Checkpoint` gains `ids: Vec<String>`: the archive ids of this session's tool results that are still in the live window (from the store, not the transcript), newest first, capped so the rendered note stays under the existing checkpoint budget (`offer_fits_checkpoint_tokens` extended); rendered as `id <archive-id> <tool> <bytes>` lines. Unit test: a fixture with three archived results yields three `id` lines and the restore injection contains them.
3. Per host, the compaction events verified against the current hooks doc (links in `src/agents/<host>/README.md` `## Docs`) and registered by `agents install` where they exist (Codex, Cursor, Gemini if a host, Copilot): the pre-event maps to `pre_compact`, the post-event to `session_start` with `source = "compact"`; hosts without the event are untouched. `tests/agents_doc.rs` regenerated with `RTOK_BLESS=1`. One commit per host if the 3-file limit needs it.
4. Hook e2e per new host: pre-event → note exists; post-event → injection bytes equal Claude Code's for the same store; fail open, ≤ 10 ms.

### T58.5. `cmd` formatters for structured families

Follow-up of T50.1 step 4 (`research.md` §9.3, "Command output"). A TOML rule keeps lines by pattern and position; families whose signal is a table or a grouped diagnostic (expected: `docker ps` / `kubectl get` tables → one row per object; `tsc` / `eslint` → errors grouped by file with counts; `mvn` / `gradle` → the failing module and the last `BUILD` line; `git log`-like paged tools) need a formatter, like the existing cargo/git/pytest ones in `formatters.rs`.
Done when:
1. Only families named by T50.1 step 4, each with the fixture that showed the rule losing the signal.
2. One formatter per family in `formatters.rs`, returning `None` on unrecognized output so the rule path stays the fallback; failures and the `expand <id>` trailer kept; golden fixtures before/after.
3. `Measurement` rows `kind = formatter` per family beat the rule's row on the same fixture; the cmd docs page table cites them.
4. ≤ 200 LOC per commit: split by family group (containers, TypeScript tooling, JVM) if needed.

### T59.5. Byte-stable `tools[]` description rewrite in the proxy

From I-45 (Portkey / LiteLLM "tool description compression + allowlist", 18–28 % claimed, unverified). Redundant on Claude Code with Tool Search deferral (`doctor` flags `mcp_tool_search_disabled`); a host without deferral pays every schema on every turn at cache-read price.
Done when:
1. Evidence: `doctor` already prices descriptions per server; a `stats` row shows description tokens × turns per session for a host without deferral, recorded in `research.md`. Below 3 % of session input, the card closes with the number.
2. Proxy option `proxy.tools_rewrite = { max_description_tokens = N, allow = [..], deny = [..] }`, off by default: descriptions truncated at a sentence boundary to N tokens (the tokenizer `measure` uses), tools outside `allow` or inside `deny` dropped from `tools[]`; the rewrite is deterministic so the cached prefix changes once per session, and `input_schema` is never touched.
3. `Measurement { plugin = "proxy", kind = "tools_rewrite" }` per request with before/after description bytes; wire tests for Anthropic and OpenAI Chat request shapes; a tool the model then calls that was dropped by `deny` is forwarded unchanged (the proxy never blocks a call).

### T59.6. `handoff` MCP tool for sub-agents

From I-46 (lean-ctx `ctx_handoff` / `ctx_agent`). Agent tool results were 23 K of 2.83 M tokens on the measured workload (§2), so this ships only with a number.
Done when:
1. Evidence: `stats` splits Agent/Task tool inputs and results per session; the card records the share, and closes with the number if sub-agents are below 5 % of tokens.
2. `handoff(budget_tokens)` returns one budgeted digest: the session's memory notes (titles first), archive ids of live tool results with tool and bytes (T58.2 field), touched paths, and the last N user prompts (`checkpoint::extract` reused, not copied); deterministic order; the digest itself is archived and carries an `expand <id>`.
3. Description ≤ 40 tokens; Vfs unit test on a fixture store; docs next to the memory tools.

### T60.1. `--json` on every reading command

Survey 2026-09-17 (`src/cli.rs`): 22 user-facing commands, `--json` only on `stats`, `info` and `config show`. `doctor`, `plugins`, `agents list`, `agents sessions`, `logs print`, `demon status` and `otel status` print tables only, so a script or another agent has to scrape text, and the web/TUI model already carries the same rows (D27).
Done when every reading command that prints a table accepts `--json` and emits the `web::model` type that page renders (`DoctorPage`, `PluginPage` list, `SessionTotals`, log lines, demon/otel status) through one `serde` path — no second struct, no hand-built JSON; each command has a trycmd golden on the fixture store next to `stats-price`; `docs/config.md` mapping table lists the flag once; `tests/surface_parity.rs` gains the check that a reading command without `--json` fails the gate.

### T60.2. trycmd goldens for every subcommand

Survey 2026-09-17: trycmd (`tests/cli_trycmd.rs`, `tests/trycmd/*.toml`) covers `help`, `version`, `config-show`, `completions-bash`, `bench-dry-run`, `stats-price` — 6 of 22 commands. The other 16 have behaviour tests but no byte-level snapshot of what the binary prints, so a wording, column or ordering change on `doctor`, `info`, `plugins`, `expand`, `report` and the rest lands unnoticed (T59.4 changed the `mcp` help line and only the top-level `help` golden caught it).
Done when every subcommand has at least one trycmd case of its real output, hermetic the way `stats-price.toml` is (`inherit = false`, `RTOK_HOME` under `target/tmp/`, `--config tests/trycmd/input/<case>.toml`, fixture store or empty dirs), plus a `--help` case for every subcommand and nested subcommand (`agents`, `config`, `demon`, `logs`, `otel`, `memory`, `graph`). Command list and how each becomes deterministic:
- `stats` (table and `--json` on the fixture store), `info` (`--json`; paths via `[..]`), `doctor`, `plugins`, `config init|path|get|validate|set`, `expand <fixture id>` with `--lines` and `--grep`, `filter --cmd`, `run -- echo`, `completions` for zsh/fish/powershell, `man`, `agents list`, `agents sessions` (empty store), `demon status` (nothing running), `otel status`, `logs print` (empty log), `report --format md` (fixture store, date via `[..]`), `bench --dry-run` (exists), `hook <event>` with a fixture stdin JSON, `mcp` with a `tools/list` frame on stdin, `proxy --dry-run`.
- `web` and `tui` get `--help` only (a server and a TTY are not snapshot material; `tests/web.rs` and `tests/tui_tty.rs` stay the behaviour tests).
- Timestamps, ids, versions and absolute paths are matched with trycmd `[..]` / `[EXE]`, never frozen; a golden must not depend on the machine.
- One `tests/trycmd/README.md` line per case saying what it pins; `README.md`'s command table is checked against the trycmd case list by the existing README test so a new command cannot land without a golden.
Blessing: `TRYCMD=overwrite cargo nextest run -p rtok --test cli_trycmd`, reviewed by eye once, then committed. Split into ≤ 3-file commits: help cases; reading commands on the fixture store; stdin-driven commands (`hook`, `mcp`, `filter`, `run`).

### T60.3. Per-session drill-down on `tui` and `web`

`SessionTotals` carries `project`, `api`, `started_at`, `last_activity`, `ended_at` (survey 2026-09-17, `src/web/model.rs`) and neither surface shows them; the Sessions page is a list on both, so "what did this session cost and which calls made it" needs the CLI.
Done when Enter on a Sessions row (TUI) and a click (web) open a detail pane with those fields, the API row, and the session's calls filtered from the same snapshot; both surfaces read the same `model` accessor (D23: one model, two renderings), `tests/surface_parity.rs` asserts the detail exists on both, and a TUI `TestBackend` test plus a Slint e2e case cover the selection.

### T60.4. Archive `expand` on `tui` and `web`

Lossless by default means every trailer id is retrievable, but only `rtok expand <id>` retrieves it; the Calls detail on both surfaces prints `ref_id` as text (survey 2026-09-17).
Done when a Calls row with an archive id opens the payload in a scrollable pane — `e` on the TUI, a button on the web — through `expand::fetch` with `--lines`/`--grep` parity (a `/` filter on the TUI, a filter box on the web); the web path is one inbound WebSocket request `{"expand": id}` answered with the payload, capped by `[expand] max_lines` like the CLI; fetching a live-zone pointer freezes it exactly as the CLI does (same function, no second path); tests: TUI `TestBackend` on a fixture store, `tests/web.rs` request/response, and `surface_parity` lists the page on both.

### T60.5. Plugin toggle on the web Plugins page

The TUI Plugins tab toggles `plugins.<id>.enabled` through `config set`; the web page renders the same rows read-only and `src/web/mod.rs` has no inbound WebSocket message at all (survey 2026-09-17) — a D23 defect.
Done when the web Plugins page has the same toggle, sent as one inbound WebSocket message `{"set": {"key": "plugins.<id>.enabled", "value": bool}}` handled by the same `config set` function the TUI and CLI use (keys limited to that allow-list; anything else is refused with a message frame), the next snapshot reflects it, `tests/web.rs` covers accept and refuse, and the Slint e2e test clicks the toggle.

### T61.2. Archive skill bodies outside the live zone

From I-51 (`research.md` §10.7). A skill body is re-sent in every later request of its session; the `archive` plugin already replaces old tool results with byte-stable pointers, keyed by `tool_use_id`, but a skill body is a user text block, not a tool result, so it is never touched.
Gated on T61.1: proceeds only when the `resident` column shows skill bodies ≥ 2 % of input tokens over a 30-day window on this machine; otherwise the card leaves the plan for `ideas.md` with the number.
Done when the wire normaliser yields a `SkillRef { id: <tool_use_id of the preceding "Launching skill" result>, name, content, turn }` for a user text block that starts with `Base directory for this skill:` right after that result; `archive::rewrite` treats it like a result outside `keep_turns` (archive once, pointer `[archived <id>: skill <name> · N lines · expand(<id>)]`, byte-identical on every later request, `Measurement { plugin = "archive", kind = "skill" }`); `expand <id>` returns the body; a proxy test replays a 3-turn fixture and asserts the pointer appears on turn `keep_turns + 1` and the body never re-archives; off switch `[plugins.archive] skills = true` documented next to `live_blobs`.

### T62.3. OpenCode plugin shortens skill bodies in `tool.execute.after`

From `research.md` §10.8. `plugins/opencode/rtok.ts` already replaces bash output through `rtok filter` in `tool.execute.after`; if OpenCode delivers a skill body through a tool call, the same hook sees it.
Step 1 (decides the task): verify against OpenCode's current docs and one real session log (`~/.local/share/opencode/opencode.db`, `part` rows) which tool carries a skill body and whether `tool.execute.after` receives its `output`; record the finding in the card. If skills are injected outside the tool path, close the task with that finding and no code.
Done when (if step 1 passes) the plugin routes that tool's output through `rtok filter --cmd "skill <name>"` with a `[skill]` rule in `rules/default.toml` (head 30 / tail 5, keep headings), the cut is lossless — `filter` archives the raw body and prints the `expand <id>` trailer, adding an `--archive` flag to `filter` if it has none today (check first; one code path with `run`) — `rtok.test.ts` covers a 3,000-line body and a small one, `Measurement { plugin = "cmd", kind = "skill" }`, and `plugins/opencode/README.md` documents it with the verified docs link (`tests/host_docs.rs`).

### T63.1. Skills page on `tui` and `web`

Asked 2026-09-18. Nothing on the operator surfaces shows what the skills cost: which of the 66 listed skills (`research.md` §10.2, this machine) were ever invoked, which never, how many bytes each body is, and how much of the input a session carried as skill bodies. `rtok stats` gains the numbers in T61.1 and `doctor` the audit in T61.3; this task renders both on the same page.
Done when `web::model::pages()` gains `("skills", "skills")` and the TUI gets the same page (D23: one `model` accessor, two renderings, `tests/surface_parity.rs` asserts the page exists on both): one row per skill the host lists — name, source (user / project / plugin), description chars, body bytes, invocations in the window, bytes resident (T61.1's column), last invoked — sorted by resident bytes, never-invoked rows marked; a header line with totals (skills listed, description bytes ≈ tokens per request, resident bytes in the window, share of input tokens); TUI `↑/↓` + `n` toggling never-invoked-only, web the same as a checkbox; empty state when the store has no skill rows yet ("run T61.1's `rtok stats` first" is not acceptable — the listing half from T61.3 renders even with zero invocations). Gated on T61.1 and T61.3 landing; tests: a `TestBackend` snapshot with three skills (one never invoked) and a Slint e2e case for the filter.

### T64.1. `cmd` grouping pass: files by directory, diagnostics by type

From `research.md` §11 (rtk's four strategies, 2026-09-18). rtk groups similar items — files by directory, errors by type; rtok's rule engine (`src/plugins/cmd/rules.rs`) only keeps, drops, cuts by position and folds adjacent duplicates, and the `ls`/`find`/`tree` formatters just take the first 40 lines.
Done when a rule may set `group = "dir"` (path-per-line output: `find`, `rg -l`, `git status` untracked, `ls -R`) or `group = "diag"` (diagnostics keyed by code or rule id: `cargo` `error[E…]`, `tsc` `TS…`, `eslint` rule, `pytest` exception class), the pass rewrites the lines as `dir/ (N files): a, b, c …` and `E0308 ×N: first message (file:line, …)` before the head/tail cut, stays lossless (raw output archived as today, `expand <id>` trailer), and a fixture per family in `tests/cmd_golden` records before/after bytes that beat the same rule without `group` — a family that does not win is not switched on. T58.5 keeps its per-family formatters; this is the generic pass a TOML rule turns on.

### T65.1. Content-hash dedup of tool output within a session

From `research.md` §11 (sqz, 2026-09-18). sqz's flagship: content seen before in the session comes back as a 13-token `§ref:HASH§` instead of the text. rtok's `guard` dedups by input key (`guard::cache_key`: same tool, same normalised input), so `cat a` followed by `head -1000 a`, or the same `cargo test` failure printed twice, is paid twice.
Step 1 (gate): `stats` gains a `repeat` column — share of tool_result bytes whose SHA-256 (`sha2` is already a dependency, T13.3) equals an earlier result in the same session — measured over 30 d on this machine into `research.md` §11. Proceeds only above 1 % of result bytes; otherwise the card leaves for `ideas.md` with the number.
Done when `cmd::run` and the `read` plugin hash the raw output before archiving, a hit in the same session returns `[rtok <id> · identical to a result N turns ago · expand: rtok expand <id>]` instead of the body (`Measurement { kind = "dedup" }`, before = body bytes), a miss archives as today, the lookup is one indexed query on the archive table (≤ 10 ms, fail open), and a test replays two different commands with identical output.

### T65.2. `cmd` JSON output compaction

From `research.md` §11. sqz strips nulls and flattens arrays in JSON output; rtok cuts `gh … --json`, `aws`, `kubectl -o json` and `curl` bodies by line position, which keeps the opening of the document and loses the keys the model asked for. `toon` (off) is the wire-side encoder and does not run in the hook path.
Step 1 (gate): `stats` share of Bash result bytes whose body parses as JSON, 30 d, this machine, into `research.md` §11.
Done when output that parses as JSON is rewritten before the line cut: null / empty-string / empty-container fields dropped, arrays beyond `json_items` (default 20) elements shown as `… +K more`, object keys kept, strings longer than `json_string` (default 200) cut with their length, one line per top-level key; lossless via the archived raw body and the trailer; a fixture per source (`gh pr list --json`, `aws ec2 describe-instances`, `kubectl get pods -o json`) records the bytes against the default rule; a body that does not parse is untouched.

### T68.5. `affected`: which tests a change touches

From the codegraph / graphify review. codegraph `affected` traces a diff to the test files it reaches so the agent runs those instead of the suite; rtok has `impact(name)` and `is_test_path`, and no path from "these files changed" to "run these tests", so `cargo test` / `pytest` output — the largest Bash family in `research.md` §2 — is paid for the whole suite.
Done when `rtok graph affected [--since <ref> | --staged]` (CLI, `--json`) takes changed files from `git diff --name-only` (no libgit — `cmd` already shells out to git), their definitions from `symbol_defs`, `impact_bfs` to `depth` (default 3), and prints the reachable definitions whose file passes `is_test_path` as `test file ← via symbol` grouped by file, with the command to run them per language (`cargo test <name>`, `pytest path::name`, `go test -run`, `vitest path`); MCP `impact` accepts `path` alone (no `name`) with the same semantics; an empty result says `no indexed test reaches the change; run the suite`; `Measurement { kind = "affected" }` is written only when a transcript or T68.9 shows the subset actually ran (before = the suite's last measured bytes, after = the subset's), never on the print alone; test on a fixture repo with two tests, one reaching the change.

### T68.6. Import edges in the index

From the codegraph / graphify review. Both tools store `imports` edges (codegraph resolves them to source files; graphify's `module_source`); rtok's rows are definitions and reference sites only, so a file that imports a module without calling a uniquely named symbol has no edge, and T68.5 cannot reach it. T52.5 already appends rtok's own tags queries to the grammar's, so this is query data plus one row kind.
Done when the extra queries capture `use` / `import` / `require` / `from … import` for Rust, TS/JS, Python, Go and Dart as rows of kind `import` whose `name` is the last path segment, `scope` empty, `is_def = false`; `symbol_imports(root, path)` lists a file's imports and `symbol_importers(root, module)` the files importing a module; `outline` prints an `imports:` line first; `impact_bfs` follows an import row to the file's definitions at cost 1 (one extra step in the same query, argument `follow_imports` default true); T8.8 recall on the 30-symbol set unchanged (imports never count as references); index time on this repo before / after in `research.md` with the command; no migration (kind is a string) — the extractor fingerprint bump re-indexes.

### T68.9. With / without bench for the graph tools

From the codegraph / graphify review. codegraph's number is the only measured one in the pair: median of 4 runs, 7 repos, Claude Opus 4.8 answering architecture questions with and without the graph — tool calls, wall time, tokens, cost — and it also reports the cost (80 % more retrieval context resident at session end). rtok's `docs/comparison.md` §5 still says no end-to-end win is demonstrated, and Gate P8b's task-set clause was never closable in code.
Done when `rtok bench --suite graph` runs N fixed questions (≥ 10, three repos including this one, in `bench/graph.toml`) through the existing `claude -p` harness twice — rtok MCP on, rtok MCP off (native Read / Grep only) — and reports per question and in total: tool calls, tokens in / out / cache-read, wall time, cost via `stats --price`, resident context at the last turn, pass / fail against an expected-answer regex; `--dry-run` prints the schedule without spend; the live run needs the creator's go (API spend) and its result goes into `research.md` and `docs/comparison.md` §4 / §5 with the date and command; the vendor's 88 % / 62 % numbers are quoted there only next to rtok's own.

### T69.2. Recall ranking: recency decay and use counts, off by default

From the graymatter gap review (`research.md` §14). graymatter ranks recall by vector + keyword + recency with a deterministic 30-day half-life and per-signal "receipts"; facts fade without access and are never hard-deleted. rtok: SessionStart recall is the newest `recall_titles` (5) ids of the project; `mem_search` is bare BM25 (`search_notes`) or RRF over BM25 + hash-embed when `embed.enabled` — a note used in every session for a month drops out of recall the moment five newer notes exist, and a stale note ranks as high as a fresh one.
Done when:
1. Evidence first: `rtok memory status` (T69.4) on this machine — notes per project and how many projects hold more than `recall_titles` live notes — recorded in `research.md` §14. If no project does, the order never matters and the card closes with the number.
2. A migration adds `notes.uses INTEGER NOT NULL DEFAULT 0` and `notes.last_used INTEGER NULL`; `mem_get` and every `mem_search` hit bump them; inclusion in a SessionStart recall does not (the hook path writes nothing per note, D13).
3. `[plugins.memory] half_life_days = 0` — 0 keeps today's id-desc order with byte-identical output; N > 0 scores `ln(1 + uses) × 0.5^(age_days / N)`, ties by id desc. Recall and search share one scoring function; search re-ranks the top `3 × limit` BM25 / RRF hits so FTS5 still does the retrieval. `NoteHit` gains `score` and `age_days` (graymatter's receipts), so the MCP result shows why a hit ranked. Decay ranks, never prunes (D4).
4. Tests: a fixture of 20 notes where a 60-day-old note used 10× outranks a fresh unused one only when `half_life_days > 0`; `half_life_days = 0` reproduces the T6.2 recall bytes exactly; scoring is deterministic under a frozen clock.
5. The default stays 0 until T69.3 shows a higher hit rate on the 100-session run without more recall bytes; the card records the numbers either way. `docs/config.md` row in the same commit (D12).

### T69.3. Memory recall bench: planted, drifted, superseded facts

From the graymatter gap review (`research.md` §14). graymatter publishes a no-LLM benchmark (`go run ./benchmarks/token_count`, keyword embedder): tokens per session against full-history injection at 1 / 10 / 30 / 100 sessions, a fact planted 96 sessions ago retrieved 83 % of the time, superseded facts returned 0 % (its numbers, not re-measured). rtok's `memory` has no recall-quality number at all — `graph` has one (T8.8, 30 hand-labelled symbols) — and Gate P6 ("revert if recall is worse") has nothing to compare against. D3.
Done when:
1. `tests/memory_bench.rs` (`cargo test --test memory_bench -- --nocapture`, the `mode_bench` shape) builds an in-memory store from a seeded generator: N sessions (1, 10, 30, 100) × K notes of realistic length, 20 target facts planted at known session offsets, 5 of them revised later (T69.1); no network, no LLM.
2. Reported per configuration — FTS5 default; `half_life_days = 30` (T69.2); `embed.enabled` hybrid (P29): hit rate of the target in `mem_search` top-`search_limit` for a query built from the fact's own words; superseded facts returned (the test asserts 0 after T69.1); SessionStart recall bytes per session against the "full injection" baseline (every live body of the project) — rtok's own version of graymatter's table.
3. Numbers land in `research.md` §14 with the command and date and on the memory docs page; `README.md` / `docs/comparison.md` cite that row and never graymatter's. The gate for T69.2's default is written from this run.
4. The generator and the expected hit rates are checked in; a change that lowers the hit rate on any row fails the test.

### T70.1. pi extension shortens every tool result, not only bash

From `research.md` §15.3. D2's constraint is that a PostToolUse hook can only add context, so on Claude Code every tool except `Bash` (rewritten to `rtok run` in PreToolUse) enters context whole; on a host with no proxy there is no second chance. pi's `tool_result` event is documented to return replacement `content` for **any** tool, and `plugins/pi/extensions/rtok.ts` uses it for bash only. Read is 15 % of tool-result tokens and its largest single results are 9.5–17 K tokens each (§2), so the tools worth adding are pi's file and search tools.
Done when:
1. Step 1 (decides the task): verify against pi's current docs (`## Docs` links in `plugins/pi/README.md`, re-checked as `tests/host_docs.rs` requires) and one real pi session that a `tool_result` handler's returned `content` replaces what the model sees for a non-bash built-in tool, and record pi's tool names in the card. If only bash may be replaced, close the task with that finding and no code.
2. The extension routes the result of pi's read / grep / find / list tools through `rtok filter --stdin --cmd "<tool> <path-or-pattern>"`, keeping the existing bash path unchanged and reusing the one `rtok()` helper already in the file — no second spawn path (D21: one call path per capability). Every shortened result carries the `expand <id>` trailer (D4).
3. Fail open exactly as today: a missing `rtok`, a spawn error, or empty stdout returns the original content; the ketch hint is printed once.
4. `plugins/pi/tests/rtok.test.ts` covers a large read result (shortened, trailer present), a small one (byte-identical passthrough) and a spawn failure (original returned); `Measurement { plugin = "cmd", kind = "rule" }` rows appear per tool family.
5. `plugins/pi/README.md` and `src/agents/pi/README.md` list the new call path and the reached plugins; `RTOK_BLESS=1 mise exec -- cargo test --test agents_doc` re-blesses the host table in `docs/agents.md` if the reached set changes.

### T70.3. pi tools without MCP: `read`, `search`, `graph`, `memory` through `pi.registerTool`

From `research.md` §15.3. `src/agents/pi/README.md` records "Not reachable: read, archive, proxy, inject, guard, memory, graph, toon, compress" because pi's philosophy is no MCP. `pi.registerTool` is documented as pi's own tool registration, which is not MCP, so the MCP-surface plugins have a path in on pi after all. The cost is description tokens in every pi request, which is the thing D15 holds `graph` and `memory` to (4 tools / 94 tokens, 3 memory tools).
Done when:
1. Step 1 (decides the task): verify `pi.registerTool`'s signature and result shape against pi's current docs and one real session; confirm a registered tool's description rides the request the way an MCP tool's does, and measure the byte cost of the set. If registration is not available to an extension, close with the finding.
2. One call path per capability (D21): the extension's registered tools are thin callers of the same `rtok mcp` tool implementations through a CLI shim (`rtok mcp --call <tool> --json <args>` or the existing subcommands), never a second implementation of `read` / `search` / `symbol` / `mem_search`.
3. Which tools: the measured-value set only — `read`, `search`, `tree`, `symbol`, `callers`, `expand`, `mem_search`, `mem_get` — with the total description budget at or under what `rtok doctor` prices for the same tools on an MCP host, recorded in the card. A tool that does not fit the budget is not registered.
4. Off by default until step 1 and step 3 numbers are in: `[setup.pi] tools = false` (D12: config key + `docs/config.md` row in the same commit).
5. Tests: `plugins/pi/tests/rtok.test.ts` registers against a fake `rtok` and asserts one call path per tool and fail-open on a missing binary; `src/agents/pi/README.md` module table and the reached set updated, host table re-blessed.

### T70.4. Cursor plugin shortens MCP results the host launched

From `research.md` §15.3. T59.4 landed `rtok mcp -- <server argv>`, which only wraps servers **rtok itself spawns**; a server Cursor launches from its own `mcp.json` is untouched, and foreign MCP results are the measured 27 % of tool-result bytes over 30 days (§2, lean-ctx). The scan of 2026-09-18 reports that Cursor's post-MCP hook may return replacement output, which is the only surface that reaches those results without re-launching the server under rtok.
Done when:
1. Step 1 (decides the task): verify against https://cursor.com/docs/agent/hooks which event carries an MCP result and whether its output may be replaced (the scan says yes for MCP and no for shell; `src/agents/cursor/mod.rs` writes only `beforeShellExecution` / `afterShellExecution` today, so the event names must be re-read, not assumed). Record the verified schema in the card. Not replaceable → close with the finding, and the wrapper stays the only path.
2. `plugins/cursor/hooks/hooks.json` gains that event pointing at `rtok hook PostToolUse --host cursor`, and the existing hook path shortens the result through the same code `rtok mcp --wrap` uses (T59.4) — one implementation, lossless, `expand <id>` trailer.
3. Never blocks and never changes a call: only the result text, only above the existing size threshold, fail open in ≤ 10 ms; results of rtok's own MCP server are skipped (they are already short).
4. Tests: a hook e2e per result size on a fixture payload, `Measurement { plugin = "archive", kind = "mcp" }` rows, and `plugins/cursor/README.md` + `src/agents/cursor/README.md` updated with the verified docs link (`tests/host_docs.rs`); host table re-blessed.

### T70.5. `guard` on pi and OpenCode through the plugin

From `research.md` §15.3. `guard` denies a repeated identical read or command within N turns, and it answers on `PreToolUse` — so it is unreachable on pi, OpenCode and Codex, which have no hook events. pi documents `tool_call` returning a block with a reason, and OpenCode documents `tool.execute.before`, which is the same position.
Done when:
1. Step 1: verify both APIs (block shape and whether the reason reaches the model) against their current docs and one real session each; a host where the block has no reason string is closed with the finding, because a silent deny violates fail-open expectations.
2. Each plugin calls one new CLI path — `rtok guard check --tool <name> --json <input>` printing the same allow/deny verdict the hook path produces from `plugins::guard` — with no second key-building or dedup implementation.
3. Fail open everywhere: missing `rtok`, non-zero exit, unparsable output, or any spawn error allows the call. T57.1's false-deny concern carries over: a wrong "read-only" verdict must not deny a call whose output changed, so the same tests run against this path.
4. Tests: `plugins/pi/tests/rtok.test.ts` and `plugins/opencode/rtok.test.ts` each cover allow, deny-with-reason and fail-open; `Measurement { plugin = "guard", kind = "deny" }` rows; both READMEs and the host table updated.

### T70.6. Compaction on pi and OpenCode through the plugin

From `research.md` §15.3; the plugin-side half of T58.2, which registers host **hook** events and therefore cannot reach pi or OpenCode. Both document a compaction event that owns the summary — pi's may supply it or cancel, OpenCode's may replace the prompt — which is stronger than Claude Code's checkpoint note (T2.5), where rtok writes a note and hopes the summary keeps it.
Done when:
1. Step 1: verify both events against current docs and one real session; record what each accepts back.
2. Each plugin calls `rtok hook PreCompact --host <host>` (or the CLI equivalent) so the existing `checkpoint::save` runs unchanged — the checkpoint content, its budget and its archive ids (T58.2 step 2) are not re-implemented in TypeScript.
3. Where the host accepts a summary, the plugin returns the rendered checkpoint **appended to** the host's own summary, never replacing it: rtok's checkpoint is prompts, paths, errors and ids, not a conversation summary, and replacing the summary would lose what the host knows.
4. Restore: the next call injects the checkpoint the way `inject::session_start` does on `source = "compact"`, inside the same budget (D5).
5. Tests per plugin for a compaction with and without rtok present (fail open), a Rust test that the injected bytes equal Claude Code's for the same store, and both READMEs updated with verified links; cross-reference T58.2 so the two cards do not both claim the host list.

### T71.1. `curl` / `wget` HTML pages as readable text

From I-71 (tinyjuice `TokenJuice`, read 2026-09-18: HTML/RSS → text, 77.0 % smaller self-reported). The `[curl]` rule (`rules/default.toml`) cuts by position (head 10 / tail 10, keep `error|HTTP|curl:`), so a page fetched with `curl` or `wget -O-` keeps 20 lines of `<head>` boilerplate and drops the body text the model asked for. A TOML rule cannot do it (T50.1 is data only); it is one formatter in `formatters.rs`, the T58.5 shape.
Gated on T50.1 step 1: proceeds only when the `stats` family table shows HTML bodies as a visible slice of the `curl` / `wget` family on this machine (≥ 1 % of Bash result bytes); otherwise the card leaves for `ideas.md` with the number.
Done when:
1. A `curl` / `wget` formatter detects `<html` or `<!doctype html` (case-insensitive) in the first KB and returns `None` otherwise, so the rule path stays the fallback for JSON, plain text and errors.
2. Output keeps `<title>`, headings (`h1`–`h6` as `# …` lines) and text nodes in document order; `script`, `style`, `nav`, `footer`, `svg`, comments and tags are dropped; whitespace collapsed; entities decoded for the common set (`&amp; &lt; &gt; &quot; &#39; &nbsp;`); the `curl` progress and `HTTP/` status lines stay. A character scanner, no new dependency (an HTML parser crate needs creator approval and a one-line reason).
3. Lossless: the raw page is archived as today and the `expand <id>` trailer is kept; the head/tail cut of the `[curl]` rule applies after the pass.
4. `tests/cmd_golden` fixtures: one real page (saved, ≤ 100 KB), one JSON response (untouched), one error; `Measurement { plugin = "cmd", kind = "formatter" }` beats the `[curl]` rule on the page fixture or the formatter is not added; the cmd docs page family table cites the row.

### T71.2. Session handoff: `SessionEnd` checkpoint, injected at the next `SessionStart`

From I-56 (engram `mem_context`, `research.md` §13; MemPalace Stop-hook checkpoint). T2.5 writes a checkpoint only at `PreCompact`, so a session that ends without compacting leaves nothing: on this machine ≥ 80 % of Claude sessions over 20 KB since 2026-09-14 ended with no note (96 sessions touched, 18 checkpoints — a rough mtime count, `ideas.md` I-56). `SessionEnd` is already registered and dispatched (`src/agents/claude/mod.rs`, unhandled), so the save is one call site. The injection half costs up to `checkpoint_tokens` on every startup, which is why it stays off until measured.
Done when:
1. Evidence: `stats` counts sessions with and without a checkpoint note (next to the T58.2 compaction count) and the number replaces the rough one in `research.md` §13 with date and command.
2. `rtok hook SessionEnd` runs the existing `checkpoint::save` (same extractor and render as `PreCompact`, no second implementation) under kind `session:<session-id>` with the project from the hook cwd; hook ≤ 10 ms, fail open; nothing is injected by this half.
3. `[plugins.memory] startup_recall = false` (D12 row in the same commit): when `true`, `SessionStart` with `source = "startup"` offers the newest `session:*` note of the project at the checkpoint priority inside `checkpoint_tokens`, rendered by the same function as the compact restore, byte-stable for an unchanged store; `Measurement { plugin = "memory", kind = "handoff" }`.
4. Hook e2e: end → note exists; start with the key off → bytes identical to today; on → the restore lines present and within budget; a second start → the same bytes.
5. Stays off by default until a P7-style A/B (T53.1 shape) shows cost per passed task does not rise; the dry result is recorded on the card. Hosts other than Claude Code that register `SessionEnd` get it through the same dispatcher (`docs/agents.md` re-blessed if the reached set changes).

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
