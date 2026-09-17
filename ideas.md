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
| I-49 | `research.md` §10.2: the `Skill` tool_result is 22 B and the body lands as the next user message (median 8.9 KB, max 248 KB) | `measure` (`stats`) | **promoted T61.1** — A `skill` family in `stats`: the user message after a `Skill` tool_use counted per skill name — calls, bytes, mean, p95 — so skill bodies stop being invisible. | Measured 2026-09-17; not in the plan until the creator promotes it. |
| I-50 | `research.md` §10.4; T59.7 (host-feature overlap) | `doctor` | **promoted T61.3** — Skill audit: every listed skill with description chars, body bytes, invocations in the window; flags descriptions > 200 chars, bodies > 8 KB, never-invoked skills, and suggests `disable-model-invocation` / project scope. Advice only. | Needs I-49 for the invocation column. |
| I-01 | token-optimizer dashboard; rtk `gain`; headroom `savings` | TUI (`measure`) | **promoted P15** — ratatui `rtok tui` (not HTML); per-plugin, per-day CTT dashboard. | T1.2 done; promoted 2026-09-02 → P15 T15.1–T15.9, D17. |

### `cmd`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-39 | code read 2026-09-17 (second pass) | `cmd` | **promoted T59.1** — `skip_wrap` treats any `-i`/`--interactive` token as interactive, so `ffmpeg -i in.mp4 …`, `curl -i`, `ssh -i key` are never wrapped and their output is never archived/filtered — for `ffmpeg` and `curl` that is the bulk of the family's bytes. A per-stem flag table (interactive only for shells/REPLs: `python`, `node`, `psql`, `sqlite3`, `irb`, …) would recover them; pick stems from `rtok discover`-style counts over real transcripts. | Safe direction today (fail open, just uncompressed). No measured unwrapped-byte share yet; a wrong "non-interactive" verdict wraps a prompt-waiting command and hangs the tool call, so the table needs a per-stem hang Check. |

### `read`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-40 | code read 2026-09-17 (second pass) | `read` | **promoted T59.2** — `display_rel` canonicalizes `cwd` once per hit/row (`dunce::canonicalize` = syscalls), so a `tree` of N rows pays N canonicalizations of the same directory; `search`/`tree` should canonicalize once per call and pass the base down. | MCP path, off the ≤ 10 ms hook budget; no measured latency complaint yet. Fold into the next touch of `search.rs`/`tree.rs` rather than its own task. |
| I-41 | lean-ctx `diff` read mode; token-optimizer-mcp delta reads (`research.md` §9.3) | `read` | **promoted T58.1** — a re-read of a file that changed since the last read returns a unified diff against the archived previous read (the sha256 dedup already stores that id) instead of the whole file; full fallback when the diff is not smaller. | Read is 15 % of tool-result tokens (§2) but the changed-file re-read share is unmeasured; T58.1 step 1 counts it before building. |
| I-43 | lean-ctx `ctx_patch` (line + hash anchors); serena `replace_symbol_body` (`research.md` §9.3) | `read` | **measured 2026-09-17 (T58.3), not built** — an MCP `patch` tool anchored on `(path, line range, file sha)` so the model sends only the new text; every `Edit` today re-emits `old_string` verbatim, which is pure output-token waste on the 96 %-tool-input output slice. | Measured 2026-09-17 (T58.3, `rtok stats --since 90d`, 925 sessions): `old_string` 3.8 % of tool-input bytes, ≈ 1.3 % of output tokens — under the 10 % gate. Re-open only with a workload where `Edit` is not already replaced by an anchored tool. |

### `guard`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-38 | token-optimizer refetch_guard; code read 2026-09-17 | `guard` | **promoted T57.1** — `read_only` is a fixed stem list (`ls cat head tail grep rg find tree wc` + five `git` verbs). Flag-aware classes would key more repeats (`sed -n`, `jq`, `awk`, `git rev-parse`, `cargo metadata`) and stop keying writers that share a stem (`find -delete`, `tail -f`, `cat > f`). Pick the stems from `rtok discover`-style counts over real transcripts. | No measured deny-rate gap yet; T55.8/T55.9 correctness first. A wrong "read-only" verdict denies a call whose output changed (fail-open violation), so this needs a false-deny Check. |

### `archive` / `proxy`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-51 | `research.md` §10.3 (2): one 248 KB skill body stays in every later request of its session | `archive` live zone | **promoted T61.2** — Extend the live-zone matcher to skill-body user messages older than N turns: pointer + `expand <id>`, same path as old tool results. | Gate on I-49: how many requests carry a skill body. |
| I-44 | atlassian-labs/mcp-compressor; headroom MCP wrapper (`research.md` §9.1) | `archive` / `proxy` | **promoted T59.4** — Wrap foreign MCP servers (`rtok mcp --wrap <server cmd>`) so their fresh results are shortened losslessly (`expand <id>`) the way `cmd`/`read` results already are; old results already shrink in the proxy live zone. | Foreign MCP results were 15 K of 2.83 M tool-result tokens on the measured workload (§2). Revisit when a `stats` row shows a foreign server above 5 %. |
| I-45 | Portkey / LiteLLM "tool description compression + allowlist" (18–28 % claimed, unverified) | `proxy` | **promoted T59.5** — Rewrite the request `tools[]` descriptions in the proxy (shorter text, allowlist per host) with a byte-stable rewrite per session. | Claude Code Tool Search already defers MCP schemas and `doctor` flags `mcp_tool_search_disabled`; a rewrite changes what the model reads and the cached prefix once per session. Redundant on the main host; no measured host without deferral. |
| I-53 | recursive-llm `re.search` over an externalised context (`research.md` §12) | `expand` (`cmd` / `archive`) | **promoted T67.1** — `--grep` as a regex whose hits print `N:line`, so `--lines a-b` can follow a hit instead of a full expand. | Promoted 2026-09-18 on the creator's request. |
| I-54 | recursive-llm slice around a hit (`research.md` §12) | `expand` | **promoted T67.2** — `--context N` returns hit ± N numbered lines in one call; two calls cost a turn each in the context-token-turns metric. | Promoted 2026-09-18; lands after T67.1. |
| I-55 | recursive-llm `RunBudget` (hard cap on calls, soft on tokens / cost, wall clock) (`research.md` §12) | `guard` / `report` | A per-session budget that warns or denies when est. tokens or `stats --price` cost cross a cap. | Not a saving lever for a tool that is not the agent loop; hosts auto-compact and `report` already prices sessions. Parked until a workload shows runaway sessions. |

### `memory` / `graph`

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-30 | codebase-memory-mcp (Linux kernel in 3 min) | `graph` | **promoted T59.3** — Batch the cold index: one transaction per N files instead of per file. | Measured 2026-09-04 (T8.4, release): 3 000 files cold 27.2 s, warm 0.053 s. Only the warm path is gated (P8b), and the cold path is paid once per repo, so this is not a task yet. |

### Hosts and product

| ID | Inspired by | Area | Proposition | Why it is not in the plan |
|----|-------------|------|-------------|---------------------------|
| I-52 | Creator request 2026-09-17; `research.md` §10.5 | hosts (`agents install`) | rtok's own skill per host: description ≤ 120 chars, ≤ 2 KB hub body pointing at `docs/`, installed and removed with the host plugin by `rtok agents install/remove`. | Awaiting the creator's go on scope (one skill or one per surface). |
| I-42 | Claude Code / Codex `PreCompact`+`PostCompact`, Cursor `preCompact`, Gemini compression hook (`research.md` §9.2) | `inject` / hosts | **promoted T58.2** — the T2.5 checkpoint (prompts, paths, errors; modes re-injected on `source = compact`) exists only on Claude Code and carries no archive ids; register the compaction events on the other hosts and add the live archive ids to the checkpoint so `expand` survives the summary everywhere. | Number of compactions per session is unmeasured; T58.2 counts them from transcripts and verifies each host's event names first. |
| I-46 | lean-ctx `ctx_handoff` / `ctx_agent`; "sub-agent context isolation" theme (`research.md` §9.3) | hosts | **promoted T59.6** — A `handoff` MCP tool that packs the archive ids, memory notes and open files of the session into one budgeted digest for a sub-agent. | Agent tool results were 23 K of 2.83 M tokens on the measured workload (§2); nothing to save until a workload shows sub-agents above 5 %. |
| I-47 | Claude Code auto-memory (v2.1.59+), OpenCode two-phase compaction, Cursor "Dynamic Context" (`research.md` §9.2) | `doctor` | **promoted T59.7** — `doctor` names the host-native feature that duplicates a rtok surface on this host (auto-memory vs `memory` recall injection, native tool-output pruning vs `archive`) and suggests the config switch, so a saving is not counted twice. | Advice only; needs a per-host Measurement of the overlap first (archive rows on OpenCode with pruning on vs off). |
| I-48 | caveman `learn`; context-budget plugin; lean-ctx mode predictor (`research.md` §9.3) | `report` | **promoted T59.8** — Rank token sinks per file path and per command over the session history (top-N by bytes with the rtok switch that would have shortened each) as a `report` rule. | `stats` already has per-family and per-tool rows and `report` renders D24 rules; add the ranking only when a real sink is missed by the existing rows. |

---

## Later (v0.2+)

Scheduled for a higher version, **not rejected**. v0.1 §5 is done; I-21..I-26 done/promoted; I-30 closed via P39 (SQLite only; Ladybug/Grafeo removed)
2026-09-10 to `plan.md` P28–P33 (see Later versions table / phase sections).
`roadmap.md` Later points at those phase ids. v0.1 `graph` stays tags-only; v0.1 hooks stay process-per-event.

| ID | Inspired by | Area | Proposition | Why v0.2+, not v0.1 |
|----|-------------|------|-------------|---------------------|
| I-21 | LLMLingua-2, claude-mem extraction | `compress` / `memory` | **promoted P28** — LLM-based compression and/or observation extraction. Default off. | Costs tokens; quality risk on code. Ship only if a bench beats v0.1 lossless. |
| I-22 | code-review-graph embeddings, mem0 | `memory`, `graph` | **promoted P29** — Optional embeddings / semantic search beside FTS5. | No measured need in current sessions; FTS5 is enough for v0.1. |
| I-23 | bifrost | `proxy` | **promoted P31** — Semantic response cache (similarity threshold). Opt-in. | Agent contexts rarely repeat; a hit can be a wrong answer. Needs a false-hit Check. |
| I-24 | serena | `graph` | **promoted P30** — LSP-grade / type-resolved backend behind the same MCP tools. | v0.1 tags index covers `symbol`/`callers`/`outline`; LSP is the precision ceiling. |
| I-25 | OpenViking L0/L1/L2 | `archive` / `inject` | **promoted P33** — Tiered session context loading. | Needs a model path and an AGPL license call-out; unmeasured vs v0.1 archive. |
| I-26 | (architecture) | core | **promoted P32** — WASM plugin host for out-of-tree plugins. | D1 v0.1 is in-tree + `from_plugins`. WASM is how third parties ship without linking. D6 still: this repo does not vendor those plugins. |
| I-30 | LadybugDB / P8c cost | `graph` | **done via P39 (2026-09-12)** — keep SQLite; delete `lbug` / `graph-lbug` / `symbols_lbug.rs`; Grafeo spike abandoned (PR #22). | C++/cmake cost + P8c (2)(3)(5); Grafeo measured worse on warm impact. |

---

## Rejected

Nothing permanently rejected. Scope by version (Open / Later), do not discard.

---

## Promoted

| I-01 | P15 T15.1–T15.9 | ratatui `rtok tui` dashboard (D17) | 2026-09-02 |
| I-27 | P20 T20.1 | `rtok demon` supervises `proxy`/`mcp`/`dashboard` (D22) | 2026-09-09 |
| I-34 | P19 T19.1–T19.3 | Slint WASM + axum WebSocket `rtok dashboard` (D20) | 2026-09-08 |
| I-17 (pi only) | T10.6 | pi host plugin: `plugins/pi/` package + `rtok setup pi` | 2026-09-08 |
| I-02 | T49.1 | `rtok stats --price` with per-model input/cache/output rates (Fable/Mythos 0.025× cache read). | 2026-09-17 |
| I-03 | T49.2 | Ingest host logs beyond Claude Code JSONL (Cursor, Codex, OpenCode). | 2026-09-17 |
| I-04 | covered (no task) | Detect whether `ANTHROPIC_BASE_URL` disabled MCP tool search; report how to keep deferred tools. Already shipped: `rtok doctor` (`mcp_tool_search likely disabled`) | 2026-09-17 |
| I-05 | T50.1 | Broader family coverage (sed, grep, cat, pnpm, python — the bulk of your Bash tokens in research.md §2) beyond the T3.3 | 2026-09-17 |
| I-06 | T50.2 | Documented user filter API (`rules/*.toml` drop-in) matching rtk’s extension model, written here (D6). | 2026-09-17 |
| I-07 | T50.3 | Extra modes beyond full/lines/map/signatures (e.g. imports-only, comments-stripped) if T4.3 does not cover the measured | 2026-09-17 |
| I-08 | T50.4 | PreToolUse deny of native Grep/Glob with a pointer to MCP `search`/`tree`. | 2026-09-17 |
| I-09 | T51.1 | Compress JSON/code *inside* the live zone that is not a `tool_result` (nested dumps, huge `data:` blobs). | 2026-09-17 |
| I-10 | T51.2 | Optionally emit native context-editing instead of (or with) rtok rewrite, so the platform does the shrink. | 2026-09-17 |
| I-11 | T51.3 | Third `Wire` (e.g. Gemini). | 2026-09-17 |
| I-12 | T51.4 | `rtok wrap -- <agent>` that sets `ANTHROPIC_BASE_URL`/`OPENAI_BASE_URL` for one process. | 2026-09-17 |
| I-13 | covered (no task) | Titles → ids → bodies strictly; never inject bodies at SessionStart (research.md §6 #7). Already shipped: SessionStart recall injects `id title` lines only; bodies via `mem_get`. | 2026-09-17 |
| I-14 | T52.1 | A small query language over the tags index (beyond `symbol`/`callers`/`outline`). | 2026-09-17 |
| I-15 | covered (no task) | `impact(path)` / changed-symbol fan-out for reviews. Already shipped: MCP `impact` (T8.7). | 2026-09-17 |
| I-16 | T52.2 | More grammars + compressed index payloads. | 2026-09-17 |
| I-28 | T52.3 | SessionStart map of the most-referenced definitions, ranked by reference count from the `symbols` table, under the `inje | 2026-09-17 |
| I-29 | T52.4 | Definitions with zero reference sites in the index. | 2026-09-17 |
| I-31 | T52.5 | rtok's own tags query on top of the grammar's, for type positions and `scoped_identifier` calls. | 2026-09-17 |
| I-17 | T48.5–T48.8 | Installers beyond Claude / Cursor / OpenCode / Codex (T10.1–T10.3). Pi agent promoted 2026-09-08 → T10.6; the rest stays — promoted T48.5–T48.8 | 2026-09-17 |
| I-18 | T53.1 | Prompt nudges (“don’t re-read”, “use expand”). | 2026-09-17 |
| I-19 | covered (no task) | Dedicated `tracing` logger: levels (`error`–`trace`), `core.log_file`, no stderr on the hook path; `Ctx::log` stays the Already shipped: `[log]` rotating file with levels (T24.0, D26). | 2026-09-17 |
| I-20 | T53.2 | Shell completions (`clap_complete`) and a man page (`clap_mangen`). | 2026-09-17 |
| I-33 | T53.4 | Keep the two Docker recipes in `docs/otel.md` as a repeatable check: a `just otel-check` that starts Jaeger 2.11 and Gra | 2026-09-17 |
| I-32 | T53.3 | Stop linking Security.framework and CoreFoundation into the one binary: they cost 1.3–1.5 ms of dyld time on every hook | 2026-09-17 |
| I-35 | T48.1 | The linked `~/.pi/agent/extensions/rtok/` has no `index.ts`; pi documents loading `extensions/*.ts` and `extensions/*/in | 2026-09-17 |
| I-36 | T48.2 | `extensions/rtok.ts` sends the ketch hint with `pi.appendEntry`, which pi documents as "does NOT participate in LLM cont | 2026-09-17 |
| I-38 | T57.1 | Flag-aware `guard` read-only classes: writer markers (`>`, `-delete`, `sed -i`, `tail -f`, pipe into a writer) take the mutating path; new read-only stems only with transcript counts. | 2026-09-17 |
| I-41 | T58.1 | `read` delta since last read: unified diff against the archived previous read; full fallback. | 2026-09-17 |
| I-42 | T58.2 | Compaction hooks: re-inject the SessionStart budget after `PostCompact`, one memory note with live archive ids at `PreCompact`. | 2026-09-17 |
| I-43 | T58.3 (T58.4 dropped) | `old_string` measured at 3.8 % of tool-input bytes / ≈ 1.3 % of output tokens over 925 sessions; the `patch` tool stays an idea with that number. | 2026-09-17 |
| I-39 | T59.1 | Per-stem interactive table for `skip_wrap`: `-i` is interactive only for REPL stems, `ffmpeg -i` / `curl -i` / `ssh -i` get wrapped. | 2026-09-17 |
| I-40 | T59.2 | Canonicalize `cwd` once per `search` / `tree` call instead of per row. | 2026-09-17 |
| I-30 | T59.3 | Batch the cold `graph` index in one transaction per 200 files; re-run the T8.4 cold bench. | 2026-09-17 |
| I-44 | T59.4 (done) | Lossless MCP wrapper landed as `rtok mcp -- <server argv>`; lean-ctx measured at ≈ 27 % of tool-result bytes over 30 d, above the 5 % gate. | 2026-09-17 |
| I-45 | T59.5 | Byte-stable `tools[]` description rewrite in the proxy, off by default, behind evidence. | 2026-09-17 |
| I-46 | T59.6 | `handoff` MCP tool: budgeted digest for sub-agents, behind evidence. | 2026-09-17 |
| I-47 | T59.7 | `doctor` names host-native features that duplicate a rtok surface. | 2026-09-17 |
| I-48 | T59.8 | Token-sink ranking rule in `report`. | 2026-09-17 |
| I-49 | T61.1 | `stats` counts injected skill bodies (`isMeta` + `sourceToolUseID`). | 2026-09-17 |
| I-50 | T61.3 | `doctor` skill audit: listing cost, oversized bodies, never-invoked skills. | 2026-09-17 |
| I-51 | T61.2 | Archive skill bodies outside the live zone, gated on T61.1. | 2026-09-17 |
| I-37 | T48.3 | `plugins/cursor/mcp.json` spawns `rtok mcp` directly, so `scripts/mcp.sh` / `mcp.cmd` (the ketch hint) never run; the sp | 2026-09-17 |

| ID | Became | Date |
|----|--------|------|
| (OpenAI Responses / Codex proxy) | D11 / P11 | 2026-09-01 |
| (config file for every flag) | D12 / P12 | 2026-09-01 |
| (ORM + action store) | D13 / P13 | 2026-09-01 |
| (`guard` / `toon` numbered tasks) | T2.6 / T11.7 | 2026-09-02 |
| I-53 | T67.1 | `expand --grep` as a regex with `N:line` hits (recursive-llm search-then-slice). | 2026-09-18 |
| I-54 | T67.2 | `expand --context N` around grep hits. | 2026-09-18 |
