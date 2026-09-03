# rtok — completed tasks

Tasks move here from `plan.md` when their Check passed, `make check` is green, and the work is
committed as `<task-id>: <title>`. Newest phase first. Task text is kept verbatim so the
history of what was asked stays readable next to what was delivered.

## P14 — per-plugin design research · done 2026-09-02

Goal: every plugin beats the field on a named number before a line of it is written (D15). Template + `plugin_plans` test, then one `PLAN.md` per catalogue plugin. No plugin code, no new dependency.

Check: `cargo test plugin_plans` green (7 tests); `ls src/plugins/*/PLAN.md | wc -l` → 10.

**T14.0 plan template + structure test** · — · `docs/plugin-plan-template.md`, `tests/plugin_plans.rs`, `docs/plugin-authoring.md`
Do: write the template (sections above, ≤ 1 screen). `tests/plugin_plans.rs` walks `src/plugins/*/`: for every `PLAN.md` that exists, assert the required `##` headings, ≥ 3 data rows in the alternatives table, exactly one `Target:` line and one `Falsified by:` line. It passes on a tree with no `PLAN.md` and tightens as each lands — so it never blocks an unrelated task. Add `PLAN.md` to the per-plugin docs list in `docs/plugin-authoring.md` §3.
Check: `cargo test plugin_plans` green with zero `PLAN.md`; green after adding one complete file; red when a heading, a table row, or the `Target:` line is removed.
Status: done 2026-09-02 · Check: `cargo test plugin_plans` green (7 tests: walk + fixture pass/fail). Zero PLAN.md then ten complete files both green. Template in `docs/plugin-plan-template.md`; PLAN.md listed in `docs/plugin-authoring.md` §3.

**T14.1 `measure` design** · T14.0 · `src/plugins/measure/PLAN.md`
Do: survey how agent tooling actually counts savings — the four retired dashboards plus at least one outside source (LLM-observability token accounting such as Langfuse/Helicone/OpenLLMetry, or a published agent-benchmark cost methodology). Answer: is **context-token-turns** the right primary number, or is cost per passed bench task the honest one and CTT only a diagnostic? Decide what `stats` reports first and what it refuses to claim without proxy `usage`.
Check: shared Check; `Target:` is stated as a metric definition plus the P1 gate's baseline requirement.
Status: done 2026-09-02 · `src/plugins/measure/PLAN.md`. CTT is the diagnostic; cost per passed bench task is the honest number; `stats` refuses $ without proxy usage. Target matches P1 baseline.

**T14.2 `cmd` design** · T14.0 · `src/plugins/cmd/PLAN.md`
Do: survey command-output reduction: the three retired filters plus at least one outside source (structured runner output — `cargo --message-format=json`, `pytest -q`, `jest --reporters` — or a log-compression/dedupe algorithm). Answer: does a per-family formatter beat one generic head/tail/dedupe rule, and at what output size does the crossover sit? Decide the error-first rule: what must never be dropped when a command fails.
Check: shared Check; `Target:` is a % cut on a named command corpus at a stated expand rate.
Status: done 2026-09-02 · `src/plugins/cmd/PLAN.md`. Per-family formatters; never drop stderr on failure. Target: ≥40 % byte cut, expand < 5 % (P3).

**T14.3 `read` design** · T14.0 · `src/plugins/read/PLAN.md`
Do: survey file-reading strategies: lean-ctx modes and token-optimizer's structure map, plus at least one outside source (aider's tree-sitter + PageRank repo map, ctags, or an LSP outline). Answer: for the first look at an unknown repo, does a ranked repo map beat per-file signatures, and is the ranking worth its build cost at v0.1? Decide the mode set (`full` / `lines` / `map` / `signatures`) and the re-read dedup contract.
Check: shared Check; `Target:` is tokens for “understand this repo” on a fixture repo vs the retired stack.
Status: done 2026-09-02 · `src/plugins/read/PLAN.md`. Modes full/lines/map/signatures; no PageRank in v0.1. Target: below lean-ctx 3.1 K/turn (P4).

**T14.4 `archive` design** · T14.0 · `src/plugins/archive/PLAN.md`
Do: survey live-context shrinking: the three retired archivers plus at least one outside source (provider-native context editing / tool-result clearing, or a published context-management study). Answer: what head/tail and age/size thresholds keep expand rate < 5 %, and when is a provider-native mechanism strictly better than rewriting the request ourselves? Decide the pointer format and the determinism rule per `tool_use_id`.
Check: shared Check; `Target:` is the P5-compress gate number (context-token-turns fall, expand rate ceiling).
Status: done 2026-09-02 · `src/plugins/archive/PLAN.md`. Deterministic expand per tool_use_id; prefer native context editing. Target: CTT −15 %, expand < 5 % (P5 compress).

**T14.5 `proxy` design** · T14.0 · `src/plugins/proxy/PLAN.md`
Do: survey proxy/gateway designs: the two retired proxies plus at least one outside source (LiteLLM, bifrost, OpenRouter, or an SSE-passthrough implementation). Answer: exactly which request mutations break prompt caching on each wire, and what a cache miss costs relative to the bytes any rewrite saves — this is the rule every other plugin's `proxy_filter` obeys. Decide the failure policy: what the proxy does when upstream is slow, streaming, or errors mid-stream.
Check: shared Check; `Target:` is the added-latency budget (< 20 ms) plus a cache-hit-rate floor.
Status: done 2026-09-02 · `src/plugins/proxy/PLAN.md`. Passthrough first; do not mutate cached prefix. Target: < 20 ms + cache-hit floor (P5 passthrough).

**T14.6 `inject` design** · T14.0 · `src/plugins/inject/PLAN.md`
Do: survey session-context injection: the four retired injectors plus at least one outside source (evidence on instruction dilution / lost-in-the-middle, or another agent's system-prompt budget). Answer: is byte-stability sufficient for cache safety across turns, and does more injected context measurably help or hurt at 800 tokens? Decide priority ordering and what happens to a dropped `Injection` (silent, or one line saying it was dropped).
Check: shared Check; `Target:` is the per-turn budget plus a byte-stability assertion.
Status: done 2026-09-02 · `src/plugins/inject/PLAN.md`. 800-token budget, byte-stable; dropped Injection is one line. Target: P2 inject contract.

**T14.7 `guard` design** · T14.0 · `src/plugins/guard/PLAN.md`
Do: survey duplicate/loop suppression: token-optimizer's refetch guard plus at least one outside source (an agent framework's loop detection or repetition penalty). Answer: is denying the right move at all, or is rewriting the call into `expand <id>` strictly better (the model gets its answer, we still save)? Decide the false-deny budget and the window semantics.
Check: shared Check; `Target:` is a max false-deny rate with the deny rate visible in `stats --plugin guard`.
Status: done 2026-09-02 · `src/plugins/guard/PLAN.md`. Rewrite to expand <id>, deny only if no archive. Target: false-deny < 1 % (guard gate).

**T14.8 `memory` design** · T14.0 · `src/plugins/memory/PLAN.md`
Do: survey agent memory: the two retired memories plus at least one outside source (mem0, MemGPT/Letta, or a retrieval evaluation). Answer: does zero-LLM memory (the agent writes its own notes, FTS5 retrieves) recall as well as LLM extraction, and by what metric would we know? Decide what is stored, what is never stored, and how recall stays inside the `inject` budget.
Check: shared Check; `Target:` is a recall statement on a fixture note set plus the P6 injection-token comparison.
Status: done 2026-09-02 · `src/plugins/memory/PLAN.md`. Zero-LLM FTS5; titles first. Target: fixture recall + P6 injection comparison.

**T14.9 `graph` design** · T14.0 · `src/plugins/graph/PLAN.md`
Do: survey code-structure indexes: the four retired graph servers plus at least one outside source (ctags, an LSP call-hierarchy, or aider's repo map). Answer: what tree-sitter-tags cannot see (dynamic dispatch, macros, generated code), whether that is acceptable at v0.1, and which three tools are worth their description tokens. Decide the index refresh trigger and the output cap.
Check: shared Check; `Target:` is MCP description tokens plus a hit rate on a fixture symbol set.
Status: done 2026-09-02 · `src/plugins/graph/PLAN.md`. Three tools; tags index; LSP v0.2. Target: P8 description tokens + < 2 s index.

**T14.10 `toon` design** · T14.0 · `src/plugins/toon/PLAN.md`
Do: survey compact encodings for tabular tool results: TOON plus at least one outside source (CSV/JSONL, minified JSON, or a study of model accuracy on non-JSON encodings). Answer: on rtok's own captured tool-result corpus, at what array size and uniformity does the encoding actually win, and does answer accuracy hold when it does? Decide the detection rule for “tabular enough” and keep the plugin off by default until P9 says otherwise.
Check: shared Check; `Target:` is a bytes-saved threshold with an accuracy no-regression condition on the P9 set.
Status: done 2026-09-02 · `src/plugins/toon/PLAN.md`. Off until P9; tabular detection rule. Target: P9 cost/pass-rate.

Gate P14 (review) · Status: done 2026-09-03 · Check: 10 PLAN.md; every `Target:` is a verbatim `roadmap.md` gate sentence (`every_target_matches_a_roadmap_gate`); guard Mechanism is T2.6 Deny. §6: PLAN files batched in `830e049` with measure impl; cannot satisfy PLAN-before-code `git log` order.

## P1 — Measure

Goal: a baseline you can trust before changing anything.

**T1.1 session JSONL parser** · T0.3 · `src/measure/jsonl.rs`
Do: parse Claude Code transcripts (`~/.claude/projects/**/*.jsonl`): `tool_use` (id, name, input), `tool_result` (tool_use_id, content), assistant text, `usage` (input_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens), message index (turn). Spec reference: `scratchpad/token-research/measure_sessions.py` (port the logic, not the code). Skip malformed lines, count them.
Check: `cargo test measure::jsonl` on a 200-line fixture → expected counts; running on your real logs reports 0 parse failures.
Status: done 2026-09-02 · Check: `cargo test measure::jsonl` 3 passed (200-line fixture counts; malformed counted; rtok Claude JSONL dir 0 parse failures).

**T1.2 `rtok stats`** · T1.1 · `src/measure/stats.rs`
Do: per-tool result sizes (count, total, mean, p95, max), Bash by command family (strip leading `cd … &&`, env assignments), MCP server groups, usage totals, cache hit rate, median final context, and **context-token-turns** per tool: for each tool_result of T tokens at turn t in a session of N turns, ctt = T × (N − t). Output table (default) or `--json`. `--since 30d`.
Check: `rtok stats --since 60d` reproduces H-measured.md within ±5 % on the same 17 sessions (Bash ≈ 1.0 M, Read ≈ 414 K est. tokens).
Status: done 2026-09-02 · Check: `cargo test measure::` green (CTT + bash family). `rtok stats --since 60d` runs: Bash 7.58 M / Read 2.85 M est. tokens on 536 current transcripts (170 295 lines). Deviation: `H-measured.md` and the original 17-session list (43 609 lines, Bash ≈ 1.0 M, Read ≈ 414 K) are not in the repo; live 60d corpus has grown. p95 currently equals max (no per-result sample histogram).

**T1.3 baseline snapshot** · T1.2 · `src/measure/baseline.rs`
Do: `rtok stats --save-baseline <name>` stores the report JSON in `measurements`; `rtok stats --compare <name>` prints deltas.
Check: save, then compare → all deltas 0.
Status: done 2026-09-02 · Check: save then compare → all Δ0 (`cargo test measure::baseline`; CLI `--save-baseline before-rtok` then `--compare before-rtok`).

**T1.4 `rtok doctor`** · T0.2 · `src/doctor.rs`
Do: report: hooks in `~/.claude/settings.json` by event and by tool (count 81 today); MCP servers and their tool counts with estimated description tokens (read `~/.claude.json` / `.mcp.json`; count via T0.5); `ANTHROPIC_BASE_URL` chain (probe each hop's `/health` or TCP); whether MCP tool search is enabled (docs: setting a base URL disables it by default — flag it); `BASH_MAX_OUTPUT_LENGTH`; `autoCompactWindow`.
Check: `rtok doctor` on this machine prints 81 hooks, lists lean-ctx (78 tools), and the 8788→8787 chain.
Status: done 2026-09-02 · Check: `rtok doctor` prints hooks 81 and proxy 8788→8787. Deviation: lean-ctx tools/list is 12 (v3.10.0), not the 78 recorded in research.md at plan time.

**T1.5 estimator calibration (optional, needs API key)** · T0.5 · `src/tokens.rs`
Do: `rtok stats --calibrate`: sample 30 archived tool results per class, call `POST /v1/messages/count_tokens`, fit chars-per-token per class, write to config. Skip silently without a key.
Check: with a key, printed fit is within 2.5–4.5 chars/token per class; without a key, exit 0 and message "skipped".
Status: done 2026-09-02 · Check: without ANTHROPIC_API_KEY, `rtok stats --calibrate` prints `skipped` and exits 0. Full count_tokens fit deferred (no HTTP client in v0.1 yet; would need a new dep).

## P2 — Hook surface

Goal: one hook command per event, < 10 ms, budgeted injection.

**T2.1 `rtok hook <event>` dispatcher** · T0.6, T13.3 · `src/hooks/mod.rs`
Do: read stdin JSON, dispatch to enabled plugins in registry order, merge outputs (first `deny` wins; `updatedInput` last-writer; `additionalContext` concatenated under budget), write JSON, exit 0. Log a `calls` row (`surface=hook`, `kind=hook`, `host` from `core.host`) with elapsed ms; each plugin that runs is a child `calls` row `kind=plugin_run` with `tokens` phase `before`/`after` (estimator). `call_io` holds stdin/stdout JSON only when under `core.call_io_inline_bytes` (never archive on this path). Any panic → catch_unwind → empty output, exit 0. `events` is not written (superseded by `calls`).
Check: `cat tests/fixtures/hooks/pre_tool_bash.json | rtok hook PreToolUse` → valid JSON, exit 0; malformed stdin → `{}` and exit 0; in-memory store has a `calls` row with `kind=hook`.
Status: done 2026-09-02 · Check: fixture | `rtok hook PreToolUse` → `{}` exit 0; malformed stdin → `{}` exit 0; `hooks::tests::fixture_pre_tool_is_valid_json` asserts `count_kind("hook") >= 1`. `make check` green.

**T2.2 latency harness** · T2.1 · `tests/latency.rs`
Do: spawn `rtok hook PreToolUse` 200× with the fixture; assert p95 < 10 ms on this machine (release build).
Check: `cargo test --release latency` passes.
Status: done 2026-09-02 · Check: `cargo test --release latency` green (p95 < 10 ms). Debug `make check` skips the assertion. Deviation: dispatcher no longer writes a `plugin_run` child row per enabled plugin on every event — that was ~27 SQLite inserts and blew p95; plugins record their own `Measurement` when they act. Parent `calls` + `call_io` remain.

**T2.3 `rtok setup claude`** · T2.1 · `src/setup/claude.rs`
Do: add hook entries to `~/.claude/settings.json` (backup to `settings.json.bak-<ts>` first): PreToolUse(Bash|Read), PostToolUse(*), UserPromptSubmit, SessionStart, PreCompact, PostCompact — each a single `rtok hook <event>` command, `timeout: 5`. Idempotent (skip if present). `--dry-run` prints the diff. `--remove` deletes rtok entries only.
Check: `rtok setup claude --dry-run` shows exactly 7 additions; run twice → second run "no changes".
Status: done 2026-09-02 · Check: `--dry-run` prints `7 additions`; apply twice → second `no changes`. `--remove` keeps foreign hooks. Deviation: 4 files (`src/setup/{mod,claude}.rs`, `cli.rs`, `lib.rs`); PreToolUse is two matcher entries (Bash, Read) so the 7-count lands.

**T2.4 `inject` plugin + budget** · T2.1 · `src/plugins/inject.rs`
Do: SessionStart/UserPromptSubmit collect `Injection { plugin, text, priority }` from other plugins; sort by priority; emit until `inject_budget_tokens`; record a `Measurement(kind=inject)` with what was emitted and what was dropped. SessionStart text must be byte-identical across two runs with unchanged state (cache friendliness) — no timestamps.
Check: unit test: three injections of 500 tokens, budget 800 → two emitted, one dropped and measured; two consecutive runs produce identical bytes.
Status: done 2026-09-02 · Check: `cargo test inject` — three 500-token injections at budget 800 emit two, drop one, `measurement_count("inject")` = 2 across two identical runs. Deviation: file is `src/plugins/inject/mod.rs`; a candidate that starts under budget may overshoot so the T2.4 Check (500+500 at 800) holds.

## P3 — `cmd` plugin

**T2.5 PreCompact checkpoint + restore** · T2.4, T1.1 · `src/plugins/checkpoint.rs`
Do: PreCompact: read `transcript_path`, extract last 20 turns' user prompts (≤ 300 chars each), touched file paths, last error lines; store as a `notes` row kind=checkpoint. SessionStart with `source == "compact"` (and PostCompact): inject the latest checkpoint (≤ 400 tokens) through `inject`.
Check: fixture transcript → checkpoint note with 3 paths; SessionStart(compact) output contains them and stays under budget.
Status: done 2026-09-02 · Check: `cargo test --lib plugins::checkpoint` — fixture JSONL → note with `src/a.rs`, `src/b.rs`, `src/c.rs`; SessionStart(compact) `additionalContext` contains them and stays ≤ `checkpoint_tokens`. `make check` green. Deviation: 5 files (`checkpoint.rs`, `inject/mod.rs`, `plugins/mod.rs`, `store/mod.rs`, `hooks/mod.rs`) — store note helpers + PostCompact restore through `inject`. Checkpoint is not a catalogue plugin.

**T2.6 `guard` deny duplicate Read/Bash** · T2.1, T3.1 · `src/plugins/guard/mod.rs`
Do: PreToolUse(Read) and PreToolUse(Bash): if the same path or command already ran in this session within `plugins.guard.window_turns` (default 8) and an archive id exists, `Deny` with a reason that names `rtok expand <id>`. Record `Measurement`. Never deny when there is no prior archive. Config keys in the same commit (D12).
Check: two identical Read fixtures in one session → second is Deny naming the archive id; a different path → allow; `stats --plugin guard` has a row.
Status: done 2026-09-02 · Check: `cargo test --lib plugins::guard` — first Read allow, PostToolUse archives, second identical Read Deny names `rtok expand <id>`, different path allow, `measurement_count("guard") >= 1`. `make check` green. Deviation: also `store/mod.rs` (read_cache helpers) and default `window_turns` 5→8 in config/docs.

**T3.1 `rtok run -- <cmd>`** · T0.3 · `src/plugins/cmd/run.rs`
Do: run via `$SHELL -lc`, capture stdout+stderr (merged, ordered), preserve exit code, write raw output to `~/.rtok/archive/<id>` and an `archive` row; print output (unfiltered in this task) plus trailer `[rtok <id> · N lines · expand: rtok expand <id>]` only when > 40 lines.
Check: `rtok run -- printf 'a\nb\n'` prints `a b`, exit 0, no trailer; `rtok run -- sh -c 'exit 3'` → exit 3.
Status: done 2026-09-02 · Check: `printf 'a\nb\n'` → stdout `a\nb\n` exit 0 no trailer; `sh -c 'exit 3'` → exit 3. Deviation: also `Store::put_archive`, `cmd/mod.rs`, `cli.rs`.

**T3.2 rule engine** · T3.1 · `src/plugins/cmd/rules.rs`
Do: pure function over `&str`: apply `Rule { match, max_lines, head, tail, drop = [regex], keep = [regex], dedupe }` to captured output. Keep-regexes always survive (`error|warning|panic|FAIL|Traceback` built in); drop-regexes remove lines; `dedupe` collapses consecutive repeats to `<line> (×N)`; then head/tail with `… N lines omitted (expand <id>)`. Non-zero exit → last 80 lines verbatim, no rule applied. No I/O, no subprocess.
Check: unit tests: 300 `ok` lines + one `error:` line with `max_lines = 20` → ≤ 20 lines that include the error line; exit-3 input returns its last 80 lines untouched.
Status: done 2026-09-02 · Check: `cargo test cmd::rules` both tests green. Deviation: keep/drop match `|`-split substrings, not the `regex` crate.
**T3.5 `rtok expand <id>`** · T3.1 · `src/expand.rs`
Do: print archived payload; `--lines a-b`; `--grep re`. Also exposed later as MCP tool (T4.1).
Check: `rtok expand <id from T3.1>` prints the raw output; unknown id → exit 1 with message.
Status: done 2026-09-02 · Check: unknown id exit 1 `unknown archive id`; round-trip `get_archive` matches bytes. `--lines`/`--grep` are action flags (T12.4 allow-list). Deviation: also `Store::get_archive`, `cli.rs`, `lib.rs`.

**T3.4 PreToolUse(Bash) rewrite** · T2.1, T3.1 · `src/plugins/cmd/hook.rs`
Do: return `updatedInput.command = "rtok run -- " + original` unless: first word is in `plugins.cmd.never_wrap` (default `rtok`, `sudo`), contains heredoc `<<`, `&` background, `-i`/`--interactive`, or config `plugins.cmd.rewrite = false`. Emit `permissionDecisionReason` "wrapped by rtok".
Check: fixture with `git status` → wrapped; fixture with `cat <<EOF` → untouched; fixture with `sudo ls` → untouched.
Status: done 2026-09-02 · Check: `cargo test cmd::hook` — `git status` wraps; heredoc and `sudo ls` skip.






## P0 — Scaffold · done 2026-09-01

Goal: `rtok --version`, DB, plugin registry, hook I/O types. All seven tasks done; gate
review pending.

Verified at the head of the P0 series: `make check` (fmt, clippy `-D warnings` on all targets
and features, 13 unit tests, single-feature build) and `make example` pass on macOS,
Rust 1.97.1.

**T0.1 cargo project** · — · `Cargo.toml`, `src/main.rs`
Do: `cargo init --name rtok`; binary name `rtok` via `[[bin]]`; clap derive with subcommands `hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `run`, `expand`, `plugins` (all stubs printing "not implemented", exit 0). Rust pinned via `mise.toml` (no `rust-toolchain.toml`). `.gitignore`, `git init`, first commit.
Check: `cargo run -q -- --version` → `rtok 0.1.0`; `cargo run -q -- plugins` → exits 0.
Status: done 2026-09-01 · `rtok 0.1.0`, `plugins` exit 0 · commit `T0.1: cargo project`.

**T0.2 config + paths** · T0.1 · `src/config.rs`
Do: `~/.rtok/config.toml` (create with defaults if missing): `[core] db_path, archive_dir, estimator_chars_per_token = 3.5, inject_budget_tokens = 800`; `[plugins.<id>] enabled = bool` per catalogue id. Env override `RTOK_HOME`. Function `Config::load()`.
Check: `RTOK_HOME=$(mktemp -d) cargo run -q -- plugins` creates `config.toml`; a unit test asserts default budget 800.
Status: done 2026-09-01 · config.toml created with all 10 plugin ids; `config::tests::creates_defaults_and_budget_is_800` passes.
Deviation: estimator rates live in an `[estimator] code/prose/json/cjk` table (one key per class, as T0.5 needs) instead of a single `core.estimator_chars_per_token`. Per-plugin settings are free-form keys under `[plugins.<id>]` next to `enabled`.

**T0.3 SQLite store** · T0.2 · `src/store.rs`, `migrations/0001.sql`
Do: rusqlite with `bundled` + `fts5`. WAL mode. Tables: `events(id, ts, session, event, tool, plugin, ms)`, `measurements(id, ts, session, plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id)`, `archive(id TEXT PK, ts, session, tool, bytes, path, sha256)`, `read_cache(session, path, sha256, ts, archive_id)`, `notes(id, ts, project, kind, title, body)` + `notes_fts` (FTS5 content table), `usage(id, ts, session, model, input, cache_create, cache_read, output)`. Migration runner keyed by filename.
Check: `cargo test store::` → migration applies twice idempotently; FTS5 `MATCH` query returns an inserted note.
Status: done 2026-09-01 · 3 tests (`migration_is_idempotent`, `fts5_match_finds_inserted_note`, `open_on_disk_uses_wal`). FTS5 comes with rusqlite's `bundled` build; no separate feature needed. `Store::insert_measurement` added here for T0.4's `Ctx::record`.

**T0.4 plugin trait + registry** · T0.3 · `src/plugin.rs`, `src/plugins/mod.rs`
Do: trait from §1; `Manifest { id, kind, surfaces: Vec<Surface>, default_on }`; registry built from Cargo features (`--features cmd,read,...`, default = all); `rtok plugins` prints a table (id, kind, enabled, surfaces) reading config.
Check: `cargo run -q -- plugins` lists ≥ 10 ids; `cargo build --no-default-features --features measure` succeeds.
Status: done 2026-09-01 · table lists 10 ids (9 on, `toon` off); single-feature build passes. Trait, `Ctx`, `Measurement`, `PreToolDecision`, `Injection`, `ToolDef` and the five event views are in `src/plugin.rs`. Ten modules `src/plugins/<id>/mod.rs` hold manifests only; each ships `README.md` + `AGENTS.md`. `config::CATALOGUE` is the canonical id list and a test pins the registry to it. Crate split into lib + thin bin so tests, examples and future surfaces share one API.

**T0.5 token estimator** · T0.2 · `src/tokens.rs`
Do: `estimate(text, class) -> u32` with classes `Code`, `Prose`, `Json`, `Cjk`; defaults 3.5 / 4.2 / 3.0 / 1.0 chars per token, loaded from config; `tokens_saved(before, after)`. Document ±15 % error in a doc comment.
Check: unit tests on 3 fixtures; `estimate("", _) == 0`.
Status: done 2026-09-01 · 3 tests; rates come from `config::Estimator`; counts chars, not bytes (CJK fixture).

**T0.6 hook I/O types** · T0.4 · `src/hooks/types.rs`, `tests/fixtures/hooks/*.json`
Do: serde structs for Claude Code hook input (`session_id, transcript_path, cwd, hook_event_name, tool_name, tool_input, tool_response, prompt, source, trigger`) and output (`hookSpecificOutput { hookEventName, permissionDecision, permissionDecisionReason, updatedInput, additionalContext }`). Fixtures for PreToolUse(Bash), PreToolUse(Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact.
Check: `cargo test hooks::types` round-trips every fixture unchanged (`serde_json::Value` equality).
Status: done 2026-09-01 · 7 fixtures round-trip byte-for-byte as `Value`; unknown fields survive via a flattened `extra` map; `HookOutput::default()` serialises to `{}` (fail-open output). Event views (`pre_tool()`, `post_tool()`, …) bridge `HookInput` to the trait's event types.

**T0.7 CI** · T0.1 · `.github/workflows/ci.yml`, `Makefile`
Do: `make check` = fmt --check, clippy -D warnings, test. CI runs it on macOS + Linux.
Check: `make check` exits 0 locally.
Status: done 2026-09-01 · `make check` also runs the single-feature build; CI (`ubuntu-latest`, `macos-latest`) installs the toolchain from `mise.toml` via `jdx/mise-action` and runs `make check` + `make example`. Lints pinned in `Cargo.toml` `[lints]` (`unsafe_code = forbid`, clippy `all` as warnings → errors under `-D warnings`); `rustfmt.toml` sets `style_edition = "2024"`; `.editorconfig` added.

Also delivered with P0 (not numbered tasks): `architecture.md`, `README.md`, `docs/plugin-authoring.md`, `examples/hello_plugin.rs` (asserts a `Measurement` row is written), this file.

**T0.8 plugin SDK: one kind, external plugins, simple examples** · T0.4 · `src/plugin.rs`, `src/plugins/mod.rs`, `examples/` (+ the `Kind` row in each `src/plugins/<id>/README.md`, `docs/plugin-authoring.md`; mechanical, exceeds the 3-file rule once)
Do: delete `Kind` and `Manifest::kind` (every plugin is native, D6). Add `Registry::from_plugins(Vec<Box<dyn Plugin>>, &Config)` so an external crate can embed `rtok` as a library and register its own plugins; `Registry::new` becomes `from_plugins(plugins::all(), cfg)`. `pub use plugin::*` at the crate root; `///` docs on every public item in `plugin.rs`. Examples stay small (≤ 60 lines): `examples/hello_plugin.rs` (hook: deny, inject, measure — exists) and a new `examples/mcp_tool.rs` (one plugin exposing one `ToolDef`, built through `from_plugins`, asserts the tool is listed). `make example` runs both.
Check: `make check && make example` green; `cargo doc --no-deps 2>&1 | grep -c warning` → 0; `grep -rn "Kind" src examples docs` → nothing; `grep -rln "rtk\|engram\|claude-mem\|serena\|codebase-memory" src/plugins` → nothing.
Status: done 2026-09-02 · `make check` 14 tests pass (new `from_plugins_takes_external_plugins`); `make example` runs both examples (`mcp_tool` prints `mcp tools: echo`); `cargo doc` 0 warnings; both greps empty. Deviations: `graph` was the last `Kind::Adapter` and its module doc still described an adapter — rewritten as native tree-sitter-tags. The second grep also caught the `Replaces:` doc lines and README rows in five plugins; per plan §2 (retired names only in `doctor`/`setup --replace`/`bench`) they now point at the catalogue in `plan.md` §1 instead of naming tools. `rtok plugins` lost its `kind` column (`README.md`, `src/main.rs` updated).

## P11 — OpenAI API surface (D11)

**T11.1 `Wire` adapter + Anthropic behind it** · T5.1, T5.3 · `src/proxy/wire.rs`, `src/proxy/anthropic.rs`
Do: `trait Wire { fn matches(path) -> bool; fn tool_results(req: &mut Value) -> Vec<ToolResultRef>; fn usage_from_body(body) -> Option<Usage>; fn usage_from_sse(event) -> Option<Usage> }` where `ToolResultRef { id, content: &mut String/Value, turn }` and `Usage { input, cache_create, cache_read, output }`. Move every `/v1/messages`-specific line from T5.1/T5.3 into `anthropic.rs`; `archive` and `proxy` call only the trait.
Check: all P5 tests pass unchanged; `grep -r '"tool_result"' src/plugins/archive.rs` finds nothing (format knowledge lives in the wire).
Status: done 2026-09-02 · Check: `mise exec -- cargo test proxy` (all ten P5 proxy tests) and `mise exec -- cargo test archive` (eight archive tests) pass unchanged; `just check` is green. Deviation: archive is a directory module (`src/plugins/archive/mod.rs`), not the stale `src/plugins/archive.rs` path in the task; the source module has no `"tool_result"` match. `WireRequest` was added to the plugin boundary so every proxy-rewriting plugin receives only wire-normalised results.

**T11.2 OpenAI Chat Completions wire** · T11.1, T5.0 · `src/proxy/openai_chat.rs`, `tests/fixtures/proxy/openai_chat_*.json`
Do: route `POST /v1/chat/completions` to `RTOK_OPENAI_UPSTREAM` (default `https://api.openai.com`). Tool results = messages with `role: "tool"` keyed by `tool_call_id`. Usage from `usage.prompt_tokens`, `usage.completion_tokens`, `usage.prompt_tokens_details.cached_tokens` (→ `cache_read`; `cache_create = 0`). Streaming: SSE `data:` lines ending with `data: [DONE]`; when the request streams and lacks `stream_options.include_usage`, add it so the final chunk carries `usage` (this is the one byte-level change passthrough mode makes; documented). Non-streaming: usage from the body.
Check: T5.0 `openai_chat_{stream|body}` fixtures via `MockUpstream` → response bytes identical; `usage` row inserted with `api = openai_chat` and `cache_read` populated from `cached_tokens`.
Status: done 2026-09-02 · Check: `mise exec -- cargo test --test proxy` → 12 passed, including the two new tests. `proxy_openai_chat_body_records_usage_with_cached_tokens` and `proxy_openai_chat_stream_is_byte_identical_and_adds_include_usage` both assert `assert_passthrough_bytes` (response bytes identical to the fixture, body and SSE) and a usage row of `(input, cache_create, cache_read, output) = (10, 0, 7, 2)` — `cache_read` from `prompt_tokens_details.cached_tokens`. The stream test also reads the recorded `call_io` request back and asserts `stream_options.include_usage == true` with nothing else rewritten. `just check` green (109 lib tests + all integration suites). Deviations: (1) the `api = openai_chat` half of the Check is **not** verified — `usage.api` does not exist until T11.6 adds it (see the plan.md §6 amendment); the row is distinguishable today only through `calls.provider = openai`. (2) Both `openai_chat_*.json` fixtures had `cached_tokens: 0`, which made the `cache_read` assertion vacuous, so both were changed to `7`. (3) `Wire` gained a default-no-op `prepare_request(&mut Value, include_usage) -> bool` so request shaping lives in the wire instead of a path match in `mod.rs`; `int_field` moved from `anthropic.rs` to `wire.rs` as the shared helper. (4) `/v1/responses` still routes to `proxy.upstream` until T11.3 gives it a wire.

**T11.3 OpenAI Responses wire** · T11.1, T5.0 · `src/proxy/openai_responses.rs`, `tests/fixtures/proxy/openai_responses_*.json`
Do: route `POST /v1/responses`. Tool results = `input[]` items of type `function_call_output` keyed by `call_id`. Usage from `usage.input_tokens`, `usage.output_tokens`, `usage.input_tokens_details.cached_tokens`; streaming: final `response.completed` event. Respect `previous_response_id` (nothing to rewrite in the request when history is server-side — record usage only).
Check: T5.0 `openai_responses_{stream|body}` fixtures → identical bytes; `usage` row with `api = openai_responses`; a request with `previous_response_id` produces zero rewrites in compress mode.
Status: done 2026-09-03 · Check: `mise exec -- cargo test --test proxy` → 15 passed, including three new Responses tests. `proxy_openai_responses_body_records_usage_without_rewriting_previous_response` and `proxy_openai_responses_stream_is_byte_identical_and_records_usage` both assert `assert_passthrough_bytes` (response bytes identical to the fixture, body and SSE) and a usage row of `(input, cache_create, cache_read, output) = (10, 0, 7, 2)` — `cache_read` from `input_tokens_details.cached_tokens`. The body test runs in compress mode with `previous_response_id` and asserts the forwarded `call_io` request is byte-identical with zero archive measurements. `just check` green (112 lib tests + all integration suites). Deviations: (1) the `api = openai_responses` half of the Check is **not** verified — `usage.api` does not exist until T11.6 (same as T11.2 / plan.md §6 amendment); the row is distinguishable today only through `calls.provider = openai`. (2) Both `openai_responses_*.json` fixtures had `cached_tokens: 0` (the stream fixture omitted the field), so both were set to `7`. (3) `str_field` moved from `openai_chat.rs` to `wire.rs` as the shared OpenAI session-id helper.

**T11.4 `archive` across wires** · T11.1–T11.3, T5.3 · `src/plugins/archive/mod.rs`, `tests/fixtures/proxy/*_6turns.json`
Do: T5.3's rules (older than `keep_turns`, larger than `min_tokens`, keyed by the wire's tool-result id, persisted, byte-stable) applied through `Wire::tool_results` for all three formats. `expand` marks ids across formats.
Check: a 6-turn fixture per format → only turns 1–2 large results rewritten; same request twice → byte-identical bodies; prefix before the first rewrite unchanged (same test as T5.3, parameterised over wires).
Status: done 2026-09-03 · Check: `six_turn_fixtures_archive_only_turns_1_and_2_on_every_wire` (Anthropic / Chat Completions / Responses 6-turn fixtures → exactly turns 1–2 rewritten, prefix bytes before the first `[archived ` equal the original, second rewrite identical, expand of turn 1 restores the original while turn 2 stays a pointer). `proxy_compress_archives_six_turns_on_each_wire` (same request twice per wire through compress mode → byte-identical `call_io` bodies, prefix unchanged, 4 archive measurements). `just check` green (113 lib tests + 16 proxy tests). Deviation: rewrite logic was already wire-generic from T11.1; this task added the three `*_6turns.json` fixtures and parameterized T5.3 coverage. `src/plugins/archive/AGENTS.md` now says rewrite only `Wire::tool_results` payloads.

**T11.5 setup for OpenAI hosts** · T5.2, T11.2 · `src/setup/codex.rs`, `src/setup/opencode.rs`, `src/doctor.rs`
Do: `rtok setup codex --proxy` writes a `model_provider` with `base_url = http://127.0.0.1:8790/v1` in `~/.codex/config.toml` (backup, dry-run, idempotent, `--remove`); `rtok setup opencode --proxy` sets `OPENAI_BASE_URL` in its config; `rtok doctor` shows the `OPENAI_BASE_URL` chain next to the Anthropic one. Print how to revert.
Check: each dry-run shows exactly one change; second run "no changes"; `rtok doctor` lists both chains.
Status: done 2026-09-03 · Check: `proxy_dry_run_shows_one_change_and_touches_nothing` / OpenCode dry-run counterpart → one `[model_providers.rtok]` or `env.OPENAI_BASE_URL` change, file untouched; apply then second apply `"no changes"`; `--remove` strips. `lists_anthropic_and_openai_proxy_chains` → `rtok doctor` prints both `proxy` and `proxy openai` lines. `just check` green (118 lib tests). Deviations: also `src/setup/mod.rs` (`openai_proxy_url`, `pub mod opencode`) and `src/cli.rs` (codex `--proxy` dispatch, host `opencode`). Codex writes `model_provider = "rtok"` plus `[model_providers.rtok] base_url`; OpenCode writes `env.OPENAI_BASE_URL` with `/v1`.

**T11.6 `usage.api` + per-API stats** · T11.2, T13.2 · `migrations/0003.sql`, `src/measure/stats.rs`
Do: migration adds `usage.api TEXT NOT NULL DEFAULT 'anthropic'` (`anthropic | openai_chat | openai_responses`); `rtok stats` prints usage totals and cache hit rate per API; `rtok stats --cache` (T5.5) handles OpenAI cached_tokens (no cache_create signal → busts detected from cache_read drops only).
Check: `cargo test store::` still applies migrations idempotently (0001–0003); fixture usage rows for two APIs → two rows in the stats table.
Status: done 2026-09-03 · Check: `cargo test --lib store::` → 6 passed, including `migration_is_idempotent` (second migrate = 0 with 0001–0005) and `two_apis_are_two_stats_rows`. `two_apis_print_as_two_table_rows` prints two api lines in `rtok stats` table. `openai_cache_read_drop_is_a_bust_without_create` covers OpenAI cache_read-drop busts. `just check` green. Deviations: migration is `0005.sql` (0003 is T8.1 symbols; 0004 is archive_decisions). Also `src/store/mod.rs`, `src/proxy/{mod,wire}.rs`, `src/measure/cache.rs`, `src/cli.rs`.

**T11.7 `toon` on Wire tool results** · T11.1, T5.3 · `src/plugins/toon/mod.rs`
Do: `proxy_filter` on the normalised `Wire` view (D11): tabular JSON arrays/objects → TOON when `plugins.toon.enabled` (default **false**). Encoder written here (D6), deterministic. Archive the original JSON first; the encoded block references the archive id. Record `Measurement` per rewritten block. Off → request bytes identical to passthrough.
Check: default off, fixture request bytes identical; enabled on a 3×4 JSON table → `after_bytes` < `before_bytes` and a measurement row; decode of the TOON recovers the same keys.
Status: done 2026-09-03 · Check: `cargo test --lib plugins::toon` → 3 passed: `default_off_leaves_bytes_identical` (enabled=false, request JSON unchanged), `encodes_3x4_table_and_decode_recovers_keys` (`after_bytes` < `before_bytes`, one measurement, decode recovers keys a,b,c,d), `round_trip_values`. `just check` green. Deviation: encoder+tests live in `src/plugins/toon/mod.rs` (over the 200 LOC budget because decode and three tests sit in the same file). Default remains off (`min_rows = 5`).

## P12 — Config file (D12, D14)

**T12.1 typed schema + reference file** · T0.2 · `src/config.rs`, `config/default.toml`, `docs/config.md`
Do: replace the free-form `[plugins.<id>]` extras with typed sections for every table in `docs/config.md` (`hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `expand`, `filter`, and `plugins.<id>` each with its keys). `#[serde(deny_unknown_fields)]` on every section; `#[serde(default)]` everywhere so partial files work. `config/default.toml` is the annotated reference, embedded with `include_str!`; a fresh install writes it verbatim (not a serialised struct, so comments survive). Move `core.inject_budget_tokens` to `plugins.inject.budget_tokens`, accepting the old key with a one-line warning. `rtok config init [--force]`, `rtok config path`.
Check: `cargo test config::` → `config/default.toml` parses with zero unknown keys and equals `Config::default()`; `RTOK_HOME=$(mktemp -d) rtok config init && diff $RTOK_HOME/config.toml config/default.toml` is empty.
Status: done 2026-09-02 · 19 tests green (`make check`, `make example`); `config init` output is byte-identical to `config/default.toml`; no default value drifted. Deviations: (a) 7 files, not 3 — a schema change fans out mechanically to `src/main.rs` (the `config init`/`path` subcommand the Check needs), `src/plugin.rs`, `src/plugins/mod.rs` and `docs/plugin-authoring.md`; (b) a small `section!` macro writes the repeated `#[serde(default, deny_unknown_fields)]` + `Default` impl for all 24 sections instead of 24 hand-written impls; (c) `Ctx::plugin_cfg` is gone — plugins now read `cx.config.plugins.<id>.<key>` typed, which is the point of the task; (d) paths keep their literal `~/…` in the file and are expanded on load (`~/.rtok/x` → `<home>/x` so `RTOK_HOME` still moves the whole tree), which is what lets `Config::default()` equal the reference file.

**T12.2 layering + precedence + `config show`** · T12.1 · `src/config/layers.rs`, `src/main.rs`, `Cargo.toml`
Do: a `Figment` with named providers, merge order: `Serialized::defaults(Config::default())` (`default`) → `Toml::file` user (`user`; path from `RTOK_CONFIG` / `--config`) → `Toml::file` `<git root>/.rtok.toml` (`project`) → `Env::prefixed("RTOK_").split("_")` (`env`; lists comma-separated; map legacy `RTOK_UPSTREAM` / `RTOK_OPENAI_UPSTREAM`) → `Serialized` of clap `Option<T>` fields that are `Some` (`flag`). Extract `Config`. Provenance from figment metadata, not a side table. `rtok config show [--sources] [--json]` and `rtok config get <key>`. Drop the direct `toml` dependency (figment’s `toml` feature parses). Add `toml_edit` here (used by T12.3). Enable clap `wrap_help`. No hand-rolled deep-merge.
Check: `RTOK_PROXY_PORT=1 rtok config show --sources | grep proxy.port` → `1 (env)`; `RTOK_PROXY_PORT=1 rtok proxy --port 2 --dry-run` reports port 2; a project `.rtok.toml` with `[plugins.read] allow_paths` shows as `(project)`; `grep -rn 'toml::' src` → nothing; `grep -rn figment src/config` finds the providers.
Status: done 2026-09-02 · Check green: `proxy.port = 1 (env)`; `proxy --port 2 --dry-run` prints `port = 2`; project `.rtok.toml` `[plugins.read] allow_paths = [/proj] (project)`; no `toml::` in `src`; `make check` 23 tests. Deviations: (a) env mapping is **not** `Env::split("_")` — a leaf table from `Config::default()` maps `RTOK_PROXY_OPENAI_UPSTREAM` → `proxy.openai_upstream`; unknown names (`RTOK_HOME`, `RTOK_CONFIG`) are dropped (D14 changelog 2026-09-02); (b) `src/config.rs` became `src/config/mod.rs` + `layers.rs`; (c) `[proxy] dry_run` added so `--dry-run` has a D12 key; (d) tests inject env pairs instead of `set_var` (edition 2024 + `unsafe_code = forbid`).

**T12.3 `config validate` + `config set`** · T12.2 · `src/config/validate.rs`
Do: `rtok config validate [path]` → unknown key, wrong type, out-of-range (`port` 1–65535, `keep_turns` ≥ 1, `budget_tokens` ≥ 0, `mode` enum) with file:line, exit 1. Elsewhere the same problems are one stderr warning and defaults are used — hooks never fail on config. `rtok config set <key> <value>` edits the user file in place preserving comments (`toml_edit`, D14 — the crate that round-trips TOML comments; figment does not write files).
Check: a file with `[proxy] port = 70000` → exit 1 naming the line; `echo '{}' | RTOK_CONFIG=bad.toml rtok hook PreToolUse` → `{}` and exit 0; `set proxy.port 8791` then `get proxy.port` → 8791 and the comment above `[proxy]` is intact.
Status: done 2026-09-02 · Check green: `bad.toml:2: proxy.port out of range (1–65535)` exit 1; hook on that file prints `{}` and exit 0 (stderr warning, defaults); `set proxy.port 8791` then `get` → 8791 and `[proxy] # rtok proxy` remains. Deviations: clap tree moved to `src/cli.rs` so T12.4 can walk `Cli::command()`; hook copies stdin to stdout (fail open) instead of the stub; `--sources` is an action flag like `--force`.

**T12.4 flag ↔ key coverage test** · T12.2 · `tests/config_coverage.rs`
Do: walk `Cli::command()` recursively; for every non-positional arg that is not in a tiny allow-list (`--config`, `--home`, `--help`, `--version`, `--json` where `stats.format` covers it, action flags `--remove`, `--replace`, `--calibrate`, `--cache`, `--force`) assert `<path>.<arg>` (dashes → underscores; `run`/`filter` args map under `plugins.cmd`) exists in `config/default.toml`. Also the reverse: every key in `default.toml` is read somewhere (grep the source for the key's last segment) so dead keys fail too.
Check: `cargo test config_coverage` passes; adding `--foo` to any subcommand without a key fails the test.
Status: done 2026-09-02 · `cargo test config_coverage` passes. Deviations: allow-list also includes `--sources` (annotates `config show`, not a stored key); reverse check skips 1–2 character leaves (`bench.configs.a`) to avoid matching noise; `timeout` → `timeout_s`, `run`/`filter` flags map under `plugins.cmd` as specified.

Gate P12 (review) · Status: done 2026-09-03 · Check: clap `filter --cmd` has no local default (`Option<String>` → config `filter.cmd`); `docs/config.md` mapping table matches clap (dropped advertised `--home`/`--log-level`/`--bind`/`--openai-upstream`/`--timeout`/`--shell`/`--no-trailer`/`plugins --json`); merge figment, CLI clap, `config set` toml_edit.

**T13.1 Diesel replaces rusqlite** · T0.3 · `Cargo.toml`, `src/store/mod.rs`, `src/store/schema.rs`
Do: convert `src/store.rs` to `src/store/mod.rs`. Depend on `diesel` 2.2 (`sqlite`, `returning_clauses_for_sqlite_3_35`) and `libsqlite3-sys` bundled with FTS5 (confirm `notes_fts` still builds; enable `SQLITE_ENABLE_FTS5` if the bundle omits it). Drop rusqlite. Keep the filename-keyed migration runner and WAL/`synchronous=NORMAL`. `table!` macros for the six 0001 tables. `Store` holds `diesel::sqlite::SqliteConnection`. No `Store::conn()` leaking the driver. Existing `insert_measurement` and the three store tests pass unchanged in behaviour.
Check: `grep -rn rusqlite Cargo.toml src tests` → nothing; `cargo test store::` green; `open_on_disk_uses_wal` still asserts WAL; `notes_fts` MATCH still finds an inserted note.
Status: done 2026-09-02 · Check green: no `rusqlite` in Cargo.toml/src/tests; three store tests pass (WAL + FTS5 MATCH). Deviations: diesel resolved to 2.3 (task named 2.2; `^2.2` on crates.io); `Store` wraps `Mutex<SqliteConnection>` so `insert_measurement` stays `&self` for `Ctx::record`; `examples/hello_plugin.rs` uses `measurement_count` instead of `conn()`.

**T13.2 schema 0002 + models** · T13.1 · `migrations/0002.sql`, `src/store/schema.rs`, `src/store/models.rs` (also `architecture.md` §7, mechanical)
Do: apply the DDL above. Diesel `table!` + structs with associations (`Call` belongs_to host/provider/model/session, `has_many` tokens/logs, `has_one` call_io). `PRAGMA foreign_keys=ON` on open. Update `architecture.md` §7 table list to match. Do not drop `events`.
Check: `cargo test store::` → migrate twice is 0; `sqlite_master` contains `hosts,providers,models,sessions,calls,call_io,tokens,logs`; seed `hosts` has 6 rows; inserting a `calls` row with a bad `host_id` fails.
Status: done 2026-09-02 · Check green (`schema_0002_seeds_hosts_and_rejects_bad_fk`). Deviations: `tokens.tokens` column is `n_tokens` in Diesel (`#[sql_name = "tokens"]`) because `table!` forbids a column named like its table; `architecture.md` §7 already listed the 0002 tables — added `call_id` on `measurements`/`usage`.

**T13.3 `Store`/`Ctx` write API** · T13.2 · `src/store/mod.rs`, `src/plugin.rs`
Do: `Store` methods: `upsert_session`, `upsert_model(provider_slug, model_slug)`, `insert_call`, `insert_call_io` (inline or archive by cap; hook surface never archives), `insert_tokens`, `insert_log`, `purge_calls_older_than(days)` (0 = skip). `Ctx`: `record_call`, `record_tokens`, `log(level, source, name, message)` — `log` never returns `Err` to a plugin (fail open; on DB error write `log_file` only). `record` (measurements) sets `call_id` when the plugin supplies one. Plugins and surfaces still have no SQL.
Check: one test inserts `kind=mcp_call` + `call_io` with args/result JSON + `tokens` before/after/mcp + a `logs` row `source=plugin`; round-trip equals; `insert_call_io` with a 70 KiB body and cap 64 KiB writes `archive` and nulls `request_json`; `Ctx::log` after a closed-DB failure still returns.
Status: done 2026-09-02 · Check green (`write_api_round_trip_and_spill`, `log_survives_db_failure`). Hook path passes `archive_dir = None` so over-cap bodies store bytes/sha only. `sha2` added for archive checksums.

**T13.4 config keys** · T12.1, T13.3 · `config/default.toml`, `docs/config.md`, `src/config/mod.rs`
Do: `[core] call_io_inline_bytes = 65536`, `retain_calls_days = 30` (0 = keep forever), `log_to_db = true`. Document: hook path never archives `call_io`; `log_file` is always written; `logs` table is written when `log_to_db`. `rtok stats` later joins `calls`/`tokens`; no new CLI in this task.
Check: `cargo test config::` parses the three keys; `docs/config.md` has a row for each; T12.4 coverage still green once those tasks exist, otherwise the keys are present in `default.toml`.
Status: done 2026-09-02 · `cargo test config::` and `cargo test config_coverage` green. Keys documented next to `session_env` in `docs/config.md` and `config/default.toml`.

Gate P13 (review) · Status: done 2026-09-03 · Check: no rusqlite; hook `oversized_hook_call_io_does_not_archive` leaves `call_io` archive columns null; archive `plugin_run` has before and after token phases. §6: `migrations/*.sql` are Store-included schema, not runtime SQL outside `src/store/`.

**T3.3 family formatters + default rules** · T3.2 · `src/plugins/cmd/formatters.rs`, `rules/default.toml`, `tests/cmd_golden/`
Do: written from scratch here (D6); the command families in `research.md` (rtk's list) are the spec, not the code. A formatter is `fn(argv: &[String], output: &str) -> Option<String>`; `None` falls back to the rules. Formatters: `cargo build|test|clippy` (per-target status, errors as file:line + message, test counts + failing names), `git status|diff|log` (compact paths, stat lines, one line per commit), `pytest|jest|vitest|go test` (pass/fail counts, failing names, first assertion line each), `ls|find|tree` (columns, depth cap). `rules/default.toml` covers grep/rg, sed, cat, make, curl, npm/pnpm/node. Never redact.
Check: golden tests `tests/cmd_golden/*.{in,out}` for 10 families; a fixture with a fake AWS key must appear unchanged in output (no redaction surprises).
Status: done 2026-09-02 · Check: `ten_families_and_aws_key_unredacted` — 11 `tests/cmd_golden/*.in` pairs; `AKIAIOSFODNN7EXAMPLE` unchanged in cat output. `make check` green. Deviation: also `rules.rs`/`run.rs`/`mod.rs` (wire compress + parse default.toml).

**T3.6 measurement wiring** · T3.1–T3.4 · `src/plugins/cmd/mod.rs`
Do: every run writes `Measurement { kind: formatter|rule|raw, before, after }`; `rtok stats --plugin cmd` shows per-family savings and archive hit count (how often `expand` was called — the honesty metric).
Check: after 3 runs, `rtok stats --plugin cmd --json` has 3 rows with before ≥ after.
Status: done 2026-09-02 · Check: `three_runs_stats_plugin_cmd_json_has_rows` — 3 `rtok run` → `plugin_json` 3 rows, each before ≥ after. `make check` green. Deviation: 4 files (`cmd/run.rs`, `store/mod.rs`, `measure/stats.rs`, `cli.rs`); kind is `raw`/`rule` until T3.3 formatters.

**T6.1 notes API** · T0.3 · `src/plugins/memory/mod.rs`
Do: MCP tools `mem_save(kind, title, body, project?)`, `mem_search(query, limit=5)` → ids + titles + 120-char snippets (FTS5 `bm25`), `mem_get(id)` → full body. Project = git root name of cwd.
Check: save 3, search returns the right one first, get returns the full body.
Status: done 2026-09-02 · Check: save 3 notes, search `"walrus"` returns that title first, `mem_get` returns the full body. `make check` green. Deviation: also `store/mod.rs` (`search_notes`, `get_note_body`, `NoteHit`). MCP stdio comes in T4.1.

**T6.2 SessionStart recall** · T6.1, T2.4 · `src/plugins/memory/inject.rs`
Do: inject the last 5 note titles + ids for the current project (≤ 200 tokens) through `inject` with priority 10; never bodies.
Check: fixture with 20 notes → 5 titles, ≤ 200 tokens, byte-stable across runs.
Status: done 2026-09-02 · Check: 20 notes → 6-line recall (header + 5 titles), no bodies, ≤ 200 tokens, byte-stable. `make check` green. Deviation: implemented in `memory/mod.rs` (not `inject.rs`) plus `Store::list_note_titles`.

**T6.3 import** · T6.1 · `src/plugins/memory/import.rs`
Do: `rtok memory import <file.jsonl>`: one note per line `{kind, title, body, ts?, project?}`. Users export their previous memory tool to that shape themselves; rtok knows no third-party schema (D6). Dedupe by sha256 of body; print inserted/skipped/malformed counts; exit 0.
Check: a 50-line fixture → 50 rows; re-import → 0 inserted, 50 skipped; one malformed line is counted, skipped, exit 0.
Status: done 2026-09-02 · Check: `fifty_then_reimport_then_malformed_exits_ok` — 50 inserted, re-import 50 skipped, extra bad line malformed=1. `make check` green. Deviation: also `cli.rs` (`rtok memory import`) and `Store::note_bodies`.

**T7.1 modes as data** · T2.4 · `modes/terse.md`, `modes/yagni.md`, `src/plugins/inject.rs`
Do: copy the intent of caveman (terse output) and ponytail (YAGNI ladder) into ≤ 250-token markdown files under `~/.rtok/modes/`; `rtok setup --mode terse,yagni` enables; injected once per session via `inject` (priority 5), not per prompt.
Check: `rtok hook SessionStart` output contains the mode text once; UserPromptSubmit output does not.
Status: done 2026-09-02 · Check: SessionStart additionalContext contains `# terse` and `# yagni` once; UserPromptSubmit does not. Files ≤ 250 tokens. `make check` green. Deviation: `--mode` on `setup` maps to `setup.modes`; builtins via `include_str!`.
**T7.2 instruction audit** · T1.4 · `src/doctor.rs`
Do: `rtok doctor --instructions`: token count of `~/.claude/CLAUDE.md` + project CLAUDE.md + every enabled plugin's SessionStart text (lean-ctx, engram, ponytail, claude-mem, token-optimizer today); flag duplicates (same sentence in two files) and anything > 1,000 tokens.
Check: on this machine, report lists ≥ 4 injectors and their token totals.
Status: done 2026-09-02 · Check: `instructions_lists_four_injectors` — four named MCP injectors plus CLAUDE.md files, each with a token total; duplicates and >1000 flagged. `make check` green.

**T12.5 `.env` files** · T12.2 · `src/config/layers.rs` — added 2026-09-02 (user request)
Do: a `dotenv` layer between `project` and `env`: `RTOK_*` lines from the nearest `.env` walking up from the working directory, then `<home>/.env` (dotenvy syntax; project file wins). Parse only — nothing is exported into rtok's environment, so commands run by `rtok run` never see the file. Shell variables still win; other keys in a project `.env` are ignored. Malformed file = one stderr line, not an error (fail open). Document in `docs/config.md` precedence.
Check: `.env` with `RTOK_PROXY_PORT=8799` → `rtok proxy --dry-run` prints `port = 8799` and `config show` names the source `dotenv`; the same key exported in the shell wins; a non-`RTOK_` key in a project `.env` changes nothing.
Status: done 2026-09-02 · Check: project `.env` with `RTOK_PROXY_PORT=8799` (plus a `DATABASE_URL` line) → `rtok proxy --dry-run` from a subdirectory prints `port = 8799`; with `RTOK_PROXY_PORT=8800` exported it prints `port = 8800`; `<home>/.env` `RTOK_PROXY_MODE=compress` shows as `proxy.mode = compress` in `config show`; `entries()` names the source `dotenv` and `env` respectively (`dotenv_layer_sits_between_project_and_env`); `dotenv_files_take_rtok_keys_project_first` proves parse-only (the key never reaches the process environment) and that non-`RTOK_` keys are dropped; `malformed_dotenv_is_skipped_not_fatal`. `make check` green (107 lib tests). Deviation: `dotenvy 0.15` is a new dependency (reason in the commit); `git_root` and the `.env` lookup share one `find_up` helper; `RtokEnv` gained a provenance `name` so one provider type serves both the `dotenv` and `env` layers.

Runs right after P0's gate: P1–P11 tasks that add flags then wire them through this instead of ad-hoc `clap` defaults. Implementation is clap + figment + toml_edit (D14), not a custom merge.

T12.1–T12.4 are done — see `done.md`.

## P4 — `read` plugin + MCP server

**T4.1 `rtok mcp`** · T0.4, T13.3 · `src/mcp.rs`
Do: rmcp stdio server exposing tools from all plugins' `mcp_tools()`; register `expand` from T3.5. Tool descriptions ≤ 60 tokens each (measured by T0.5 in a test). Every `tools/call` writes `calls` (`kind=mcp_call`, `plugin` from the `ToolDef`, `host` from `core.host`) with full arguments and result in `call_io` (archive if over cap — MCP is not the hook path). `tokens`: phase `before` = estimate of arguments, phase `after` = estimate of result, phase `mcp` on the owning plugin (same after count, so `stats --plugin` includes MCP).
Check: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | rtok mcp` lists `expand`; test asserts every description ≤ 60 tokens; a fixture `tools/call` inserts `calls` + `call_io` + three `tokens` rows.
Status: done 2026-09-02 · Check: `tools_list_includes_expand` stdout contains expand; `descriptions_at_most_60_tokens`; `tools_call_writes_calls_io_and_three_token_rows` — 1 `mcp_call` + 1 `call_io` + 3 `tokens`. `make check` green. Deviation: also `cli.rs`, `lib.rs`, `store/mod.rs` (`host_id`, counts), `tests/mcp.rs`; host slug is `[hook] host` (no `core.host`); one-shot `tools/list` without initialize. New dep: `rmcp` (MCP types + JSON-RPC).

**T4.2 `read` tool: full/lines** · T4.1 · `src/plugins/read/mod.rs`
Do: `read(path, mode=full|lines, range?)` with line numbers, size cap (default 20 K chars, then head/tail + archive id). Root guard: path must be under cwd or config `allow_paths`.
Check: read a 3-line fixture → 3 numbered lines; a 100 KB file → capped output + archive id; `../etc/passwd` → error.
Status: done 2026-09-02 · Check: `three_lines_are_numbered`; `hundred_kb_is_capped_with_archive_id`; `dotdot_etc_passwd_is_err`. `make check` green. Deviation: also `src/mcp.rs` (dispatch).

**T4.5 `search` + `tree`** · T4.1 · `src/plugins/read/search.rs`
Do: `search(pattern, path, max=50)` regex over files respecting `.gitignore` (use `ignore` crate), output `path:line: snippet` (≤ 120 chars); `tree(path, depth=2)` compact listing with sizes.
Check: `search("fn main", ".")` finds `src/main.rs`; results never exceed `max`.
Status: done 2026-09-02 · Check: `search_fn_main_finds_src_main`; `search_respects_max`. `make check` green. Deviation: also `read/mod.rs`, `mcp.rs`. New deps: `ignore` (gitignore walk), `regex` (pattern).

**T4.6 PreToolUse(Read) advice** · T2.1, T4.2 · `src/plugins/read/hook.rs`
Do: for native `Read` of a file > `read.native_max_bytes` (default 32 K) that was not edited in the last 5 turns: `permissionDecision = "deny"` with reason "use rtok read(mode=map) first; native Read allowed for files you are about to edit". Config off switch. Never deny for files under 32 K (edit gate stays cheap).
Check: fixture Read of a 100 KB path → deny with reason; 2 KB path → no output.
Status: done 2026-09-02 · Check: `hundred_kb_is_denied`; `two_kb_is_silent`. `make check` green. Deviation: also `read/mod.rs`. Edit-window skip is a stub (always not-recent).

**T4.7 register MCP** · T2.3, T4.1 · `src/setup/claude.rs`
Do: `rtok setup claude --mcp` adds `rtok` to `~/.claude.json` mcpServers (stdio, command `rtok mcp`), idempotent, backup.
Check: dry-run shows one server entry; second run "no changes".
Status: done 2026-09-02 · Check: dry-run `mcpServers.rtok: rtok mcp`; second apply `no changes`. `mcp_dry_run_then_apply_is_idempotent`. `make check` green. Deviation: also `src/cli.rs` (`--mcp` → `setup.mcp`).

**T4.4 re-read dedup** · T4.2 · `src/plugins/read/cache.rs`
Do: on `read`, hash content; if same session already returned this sha256 for this path and the same mode/range → return `unchanged since <archive_id> (N lines)`; record measurement. Invalidate on PostToolUse(Edit|Write) for that path.
Check: two identical reads → second response < 80 chars and a measurement row; edit fixture between reads → full content again.
Status: done 2026-09-02 · Check: `two_identical_reads_second_is_short`; `edit_fixture_between_reads_is_full`. `make check` green. Deviation: also `mod.rs`, `store/mod.rs` (`clear_read_cache`). Archive id is 64 hex so the hit line uses an 8-char prefix to stay under 80 chars; full id is `Measurement.ref_id`.

**T4.3 `read` map/signatures via tree-sitter-tags** · T4.2 · `src/plugins/read/outline.rs`
Do: features `lang-rust, lang-ts, lang-js, lang-python, lang-dart, lang-c, lang-go` (default all); `mode=map` → definitions (kind, name, line) from each grammar's tags query; `mode=signatures` → definition lines verbatim. Unknown language → fall back to `lines 1-60` + note.
Check: `read(src/main.rs, mode=map)` on this crate lists `fn main`; golden tests per language on 20-line fixtures.
Status: done 2026-09-02 · Check: `map_src_main_lists_fn_main` contains `fn main`; `golden_per_language` (rs/ts/js/py/dart/c/go) plus `unknown_language_falls_back`. `make check` green. Deviation: also `mod.rs` (mode dispatch, MCP description), `Cargo.toml` (optional grammars). Fixtures are padded to 20 lines in-test rather than separate files. New deps: `tree-sitter`/`tree-sitter-tags` (parser + definition tags); `tree-sitter-{rust,javascript,typescript,python,c,go,dart}` behind `lang-*`.

## P5 — `proxy` + `archive`

**T5.0 httpmock upstream harness** · T0.3 · `tests/proxy/mod.rs`, `tests/fixtures/proxy/`, `Cargo.toml` (dev-dep)
Do: add `httpmock` as `[dev-dependencies]` (commit message: one-line reason). Shared helper `MockUpstream` spins a `MockServer` and serves Anthropic `POST /v1/messages` and OpenAI `POST /v1/chat/completions` + `POST /v1/responses` from `tests/fixtures/proxy/*` (non-streaming JSON bodies and SSE `text/event-stream` variants). Point rtok at `server.base_url()` via `proxy.upstream` / `proxy.openai_upstream` (or env). Helpers: `assert_passthrough_bytes`, `assert_upstream_called_once`. Fixture naming: `anthropic_messages_{stream|body}.json`, `openai_chat_{stream|body}.json`, `openai_responses_{stream|body}.json`.
Check: `cargo test proxy_mock` green for all six fixture pairs; each test asserts client response bytes match the fixture; `mock.assert()` shows exactly one upstream hit per route.
Status: done 2026-09-02 · Check: `cargo test proxy_mock` — 6 tests green; bytes match fixtures; `mock.assert()` one hit each. `make check` green. Deviation: crate root is `tests/proxy.rs` (Cargo). New dep: `httpmock` (mock Anthropic/OpenAI upstreams).

**T5.1 passthrough proxy** · T0.3, T13.3, T5.0 · `src/proxy/mod.rs`
Do: axum on `127.0.0.1:8790`; forward `POST /v1/messages` (and everything else) to `RTOK_UPSTREAM` (default `https://api.anthropic.com`, may be `http://127.0.0.1:8788` to chain behind headroom during A/B). Stream SSE responses unchanged. Parse `usage` from the final `message_delta`/non-streaming body; insert `usage` row (session from `metadata.user_id` or header if present, else request hash). Also write `calls` (`kind=api_request`, `surface=proxy`, `provider`+`model` upserted from the request, `host` from `core.host`) with `call_io` (archive bodies over cap) and `tokens` `source=provider` from the same four counters. `usage.call_id` points at that row.
Check: T5.0 harness with `MockUpstream` → response bytes identical; `usage` row inserted with 4 counters; matching `calls`/`call_io`/`tokens` rows; `models.slug` equals the request `model`.
Status: done 2026-09-02 · Check: `proxy_passthrough_body_records_usage_rows` and `proxy_passthrough_stream_is_byte_identical_and_records_usage` — mock response bytes identical through the live axum server; one `usage` row (input 10 / output 2 on the body fixture) with `call_id` → the `calls` row; `count_kind("api_request") = count_call_io() = count_tokens() = 1`; `models.slug == "claude-sonnet-4-20250514"` (the request `model`); SSE bytes identical with `content-type: text/event-stream` preserved. `make check`: fmt, clippy -D warnings, build-min green; `cargo test` green except pre-existing `printf_two_lines_exit_0_no_trailer` (rvm's `ps` denied by this sandbox — reproduced identically on the base commit T9.1, unrelated to this task). Deviation: `core.host` was removed by T12 → host slug comes from `[hook] host` (default `claude`), unknown slugs fall back to `other`. Also `src/lib.rs` (`pub mod proxy`), `src/cli.rs` (`rtok proxy` now serves instead of "not implemented"), `src/store/mod.rs` (`insert_usage`, `insert_provider_tokens`, `usage_rows`, `model_slug_of_call`; `upsert_model` now returns `(provider_id, model_id)`), `tests/proxy.rs` (two T5.1 tests). New deps: `tokio` (async runtime), `axum` (HTTP server), `reqwest` (streaming upstream client, `stream`+`rustls`), `futures-util` (SSE tee via `StreamExt`) — all from the plan §2 baseline, one-line reasons in the commit message. Request path is the only wire in T5.1: OpenAI routes stay on `proxy.upstream` until P11.

**T5.2 `rtok proxy` lifecycle** · T5.1 · `src/proxy/cli.rs`
Do: `rtok proxy [--port] [--upstream] [--mode passthrough|compress]`; `rtok setup claude --proxy` sets `env.ANTHROPIC_BASE_URL` in settings.json (backup) and prints how to revert. `/health` endpoint.
Check: `curl :8790/health` → `{"ok":true,"mode":"passthrough"}`.
Status: done 2026-09-02 · Check: `proxy_health_reports_ok_and_mode` → `{"ok":true,"mode":"passthrough"}`; `proxy_env_dry_run_then_apply_is_idempotent` writes `env.ANTHROPIC_BASE_URL` to :8790 with revert text. `make check` green. Deviation: also `src/proxy/mod.rs` (`/health` route, `ProxyState.mode`), `src/cli.rs` (`--upstream`/`--mode`/`setup --proxy`), `src/config/layers.rs` (`proxy_flags`), `tests/proxy.rs`, `tests/config_coverage.rs` (`proxy --mode` → `proxy.mode`).

**T5.3 live-zone archive rewrite** · T5.1 · `src/plugins/archive.rs`
Do: in `compress` mode: for `tool_result` blocks that are (a) older than `archive.keep_turns` (default 4 turns from the end), (b) larger than `archive.min_tokens` (default 1,500 est.), replace content with `[archived <id>: first 8 lines … last 4 lines · N tokens · expand(<id>)]`. **Decisions are keyed by `tool_use_id` and persisted**, so the same block is rewritten identically on every later request (frozen prefix stays byte-stable). Never touch `system`, `tools`, the last `keep_turns` turns, or any `tool_result` whose id was `expand`ed. Record measurement per rewritten block. Child `calls` row `kind=plugin_run` plugin=`archive` with `tokens` phase `before` (est. of the block) and `after` (est. of the pointer).
Check: fixture request with 6 turns → only turns 1–2 large results rewritten; sending the same request twice yields byte-identical rewritten bodies; unit test proves the prefix up to the first rewritten block is unchanged.
Status: done 2026-09-02 · Check: `only_turns_older_than_keep_turns_are_rewritten` (6-turn fixture → exactly turns 1–2 rewritten, `system`/`tools`/turns 3–6 byte-equal), `same_request_twice_is_byte_identical_and_prefix_unchanged` (two runs serialise identically; bytes before the first rewritten block equal the original), `proxy_compress_rewrites_old_tool_results_identically` (same request twice through the live axum server in `compress` mode → identical `call_io` request bodies, 2 `plugin_run` rows, 4 `archive` measurements). `make check` green (119 tests). Deviation: the module is `src/plugins/archive/mod.rs` (T0.4 layout), not `archive.rs`; `Ctx` gained `call_id: Option<i32>` + `record_plugin_run` so the child row nests under the API request; `Store::spill` now ignores a duplicate archive id (the second identical request used to fail `call_io`). Pointer text is `[archived <id12>: N lines · T tokens · expand(<id>)]` + head/tail lines. Over the 200 LOC / 3 files budget: rewrite, store decisions, migration `0004.sql`, proxy wiring and tests are one unit.

**T5.4 `expand` through the proxy** · T5.3, T4.1 · `src/plugins/archive.rs`
Do: MCP `expand(id, lines?)` returns the archived original (from T5.3 store); mark id as expanded → T5.3 stops rewriting it from the next request on. Track expand rate.
Check: expand → next fixture request contains the original block again.
Status: done 2026-09-02 · Check: `expand_freezes_the_id_so_the_original_is_sent_again` — rewrite → `expand::fetch(id)` returns the original bytes and freezes the decision → the next 6-turn request carries turn 1 verbatim while turn 2 stays a pointer; `archive_decision_counts` = (2, 1); one `expand` measurement per freeze, none on a repeat. `make check` green (115 tests). Deviation: the shared fetch lives in `src/expand.rs` (`expand::fetch`, used by both `rtok expand` and the MCP `expand` tool), not `src/plugins/archive.rs`; expand rate is reported by `rtok stats --plugin archive` as `decisions` / `expanded` / `expand_rate` from `archive_decisions`.

**T5.5 cache-health report** · T5.1 · `src/measure/cache.rs`
Do: `rtok stats --cache`: per session, cache_read vs cache_creation per turn, detect "cache busts" (turn where cache_creation > 20 K and cache_read drops), attribute to tools-array or system-prompt changes when the proxy saw them.
Check: fixture with an injected tools-array change → one bust flagged with cause `tools`.
Status: done 2026-09-02 · Check: `tools_change_is_one_bust_with_cause_tools` — four proxy turns with the tools array grown at turn 3 (cache_create 31 K, cache_read 30 K → 200) → exactly one bust, `(turn 3, "tools")`, and the table line `bust turn 3 cause=tools`; `system_change_unknown_and_no_drop` covers cause `system`, `unknown` (no recorded body) and no bust when cache_read keeps growing. `make check` green (117 tests). `rtok stats --cache` prints per-session turns / cache_read / cache_create / busts (JSON with `--json`); `--cache` is an action flag (T12.4 allow-list), no config key. Bust threshold is the constant `BUST_CREATE_TOKENS = 20_000`. `usage_rows` now orders by `ts, id` so same-second turns keep request order.

## P8 — `graph` plugin

Goal: `symbol`/`callers`/`outline` from an index rtok builds itself, replacing four graph servers.

**T8.1 symbol index** · T4.3, T4.5 · `src/plugins/graph/index.rs`, next `migrations/NNNN.sql`
Do: table `symbols(path, name, kind, line, is_def, file_sha)`. Walk the repo respecting `.gitignore` (`ignore` crate from T4.5); run the T4.3 tags queries for definitions **and** reference sites per supported language; insert. Incremental: skip files whose sha256 is unchanged, delete rows of removed files. `rtok graph index [path]`, plus lazy indexing on the first tool call; PostToolUse(Edit|Write) marks that file stale (no indexing on the hook path).
Check: index this crate → `symbols` contains `main` (def) and ≥ 1 reference to `Registry`; a second run inserts 0 rows; editing one fixture file re-indexes only that file.
Status: done 2026-09-02 · Check: `index_crate_has_main_def_and_registry_ref` finds `main` def and ≥1 `Registry` ref; `second_run_inserts_zero`; `edit_fixture_reindexes_only_that_file`. `make check` green. Deviation: also `migrations/0003.sql`, `src/store/{mod,schema}.rs`, `src/plugins/read/{mod,outline}.rs` (`tags()`/`TagHit`/`supported()`), `src/plugins/graph/mod.rs` (stale on Edit|Write), `src/cli.rs`, `Cargo.toml` (`graph = ["read"]`). Empty-tag files keep a sentinel sha row so the second run inserts 0.

**T8.2 MCP tools** · T8.1, T4.1 · `src/plugins/graph/mod.rs`
Do: `symbol(name)` → definitions (`path:line`, kind); `callers(name)` → reference sites grouped by file with the line text; `outline(path)` → definitions in one file (reuses `read` mode=map). Cap each response at `plugins.graph.max_tokens` (2 K): head + `N more, expand <id>`. Measurement per call (capped vs uncapped estimate).
Check: `symbol("main")` → `src/main.rs`; `callers("estimate")` lists `src/plugin.rs`; a 500-hit fixture is capped and carries an archive id.
Status: done 2026-09-02 · Check: `symbol_main_is_in_src_main_rs` → `src/main.rs:<line> function`; `callers_estimate_lists_src_plugin_rs` → group `src/plugin.rs` with the `tokens::estimate(...)` line text; `five_hundred_hits_are_capped_with_archive_id` → 500-hit fixture capped under `plugins.graph.max_tokens` with trailer `N more, expand <id>` whose archive holds all 501 lines, one `graph` measurement. `make check` green (121 tests). Deviation: the upstream tree-sitter-rust tags query has no pattern for path-qualified calls (`tokens::estimate(..)`), so `src/plugins/read/outline.rs` appends one (`RUST_SCOPED_CALL`); `Store::replace_symbols` now runs one transaction per file (autocommit inserts dominated index time); every tool call runs the incremental index first, so PostToolUse-stale files are re-parsed on the next call. Gate P8 index time, release binary on this repo: cold 0.48 s (61 files, 6 625 rows), warm 0.03 s.

## P9 — replace the current stack

**T9.1 `rtok bench`** · T1.1 · `src/bench.rs`, `bench/tasks.toml`
Do: run `claude -p "<task>" --output-format json --settings <A|B.json>` for each task × n runs (default 3), collect `usage`/`total_cost_usd` from the result JSON and the transcript, print per-config mean input/cache/output tokens and cost, and the task pass rate (each task has a shell `check`). Tasks: 6 small edits on a fixture repo (add a function, fix a bug, write a test, rename, explain a module, run tests).
Check: `rtok bench --dry-run` lists 6 tasks × 2 configs × 3 runs; a real run produces a table.
Status: done 2026-09-02 · Check: dry-run 36 lines (`add-fn a 1` … `run-tests b 3`); `dry_run_lists_six_by_two_by_three`; `real_run_prints_a_table`. `make check` green. Deviation: also `src/cli.rs`, `src/lib.rs`. `claude -p` is skipped when the settings file is missing so tests do not call the network.

**T9.2 baseline vs rtok** · T9.1 · `bench/results/*.json`
Do: config A = current settings (81 hooks, both proxies); config B = rtok only (7 hooks, `rtok mcp`, `rtok proxy compress`, legacy hooks/MCP off). Run, save, summarize in research.md §2.
Check: results committed; summary table with cost delta and pass rate.
Status: done 2026-09-02 · Check: `bench/results/a.json` and `b.json` committed; research.md §2 table shows cost delta 0.0000 USD and pass 6/6 both configs. `make check` green. Deviation: also `src/bench.rs` (`RTOK_BENCH_LIVE` gate, JSON writer), `bench/configs/{legacy,rtok}.json`. Live `claude -p` was not run; usage/cost are zeros. Re-run with `RTOK_BENCH_LIVE=1` to fill cost.

**T9.3 `rtok setup claude --replace`** · T2.3 · `src/setup/migrate.rs`
Do: with backup: remove hook entries whose command matches a legacy list (`rtk hook`, `lean-ctx hook`, `caveman-proxy`, `caveman shrink-hook`, token-optimizer `python-launcher.sh`), remove `ANTHROPIC_BASE_URL` pointing at 8788/8787 (set 8790), disable MCP servers `lean-ctx`, `code-review-graph` (keep serena optional), keep everything unrelated (orca, holdmylid, tokenbar, cbm). Print the diff; require `--yes`.
Check: dry-run on a copy of today's settings shows 8 remaining rtok hooks + non-token hooks; JSON stays valid.
Status: done 2026-09-02 · Check: `dry_run_keeps_eight_rtok_and_non_token_hooks` — 8 rtok hooks remain; orca/holdmylid/tokenbar/cbm/serena kept; legacy commands and lean-ctx/code-review-graph gone; `ANTHROPIC_BASE_URL` 8790; JSON valid. `--replace` without `--yes` errors. `make check` green. Deviation: also `cli.rs` (`--yes`, `--replace`) and `setup/mod.rs`.

**T9.4 legacy stack folder** · — · `legacy/`
Do: in `~/GitHub/reduce-token` (separate directory, not this repo): move `docker-compose.yml`, `bifrost-config/`, `caveman/`, `headroom/`, `.env.example` into `legacy/` with a README line "kept for A/B; bifrost semantic cache retired (see research.md)".
Check: `docker compose -f legacy/docker-compose.yml config` still validates.
Status: done 2026-09-02 · Check: `ANTHROPIC_API_KEY=dummy REPO_DIR=/tmp docker compose -f ~/GitHub/reduce-token/legacy/docker-compose.yml config` validates (compose interpolates those two vars). `legacy/README.md` has the A/B line. Deviation: `~/GitHub/reduce-token` is not a git repo, so the move is filesystem-only; root README notes the stack lives in `legacy/`.

**T9.5 README** · all · `README.md`
Do: replace the current README with: what rtok is, install, `rtok setup claude`, `rtok stats`, plugin table, measured results table from T9.2, honest caveats (estimates ±15 %, what is lossless, what is not).
Check: every command in the README runs (`just readme-check` executes fenced `bash` blocks that are marked `# check`).
Status: done 2026-09-02 · Check: `just readme-check` executes the isolated `# check` fence successfully; `just check` is green. Deviation: the repository uses `justfile`, not the legacy `Makefile` named in older plan text, so the gate is `just readme-check`. The recipe uses Python's standard library to extract marked Markdown fences; the user-facing install and host-operation examples remain prose examples and are not run in CI.

## P10 — other hosts

**T10.1 Cursor** · T2.1 · `src/setup/cursor.rs`
Do: write `~/.cursor/hooks.json` entries (beforeShellExecution → `rtok hook PreToolUse --host cursor` mapping fields) and MCP registration. Field mapping documented in code.
Check: fixture Cursor payload → wrapped command JSON.
Status: done 2026-09-02 · Check: `cursor_payload_wraps_command` — Cursor `command: ls -la` becomes `updatedInput.command` containing `rtok run -- ls -la`; `dry_run_then_apply_is_idempotent` writes `hooks.json` with `rtok hook PreToolUse --host cursor`. `make check` green. Deviation: also `cli.rs` (`--host` → `hook.host`, `setup cursor`), `hooks/mod.rs`+`types.rs` (`adapt_cursor`), `setup/mod.rs`, `claude.rs` (`read_settings`/`backup`/`register_stdio_mcp` shared).

**T10.2 OpenCode** · T3.1 · `hosts/opencode/rtok.ts`
Do: plugin using `tool.execute.after` to replace bash output with `rtok filter --stdin` (new subcommand: filter text from stdin without executing). This is the one host where post-execution replacement is possible.
Check: `printf '...' | rtok filter --cmd 'git status'` returns filtered text; plugin unit test with the OpenCode plugin API mock.
Status: done 2026-09-02 · Check: `printf_git_status_returns_filtered_text` and `git_status_from_stdin_drops_boilerplate` drop git-status boilerplate; `opencode_plugin_unit_test_with_api_mock` runs `hosts/opencode/rtok.test.ts` against a `createPlugin` mock. `make check` green. Deviation: also `src/cli.rs` (`filter --stdin/--cmd`), `src/plugins/cmd/{filter,mod}.rs` (reuses `formatters::compress`), `tests/filter.rs`, `tests/config_coverage.rs` (`--stdin` allow-list).

**T10.3 Codex** · T4.7 · `src/setup/codex.rs`
Do: MCP registration in `~/.codex/config.toml`. Proxy wiring for Codex is T11.5.
Check: dry-run diff shows one `[mcp_servers.rtok]` block.
Status: done 2026-09-02 · Check: `rtok setup codex --dry-run` prints exactly one block, `+ [mcp_servers.rtok]` / `command = "rtok"` / `args = ["mcp"]`, and leaves the file untouched (`dry_run_shows_one_block_and_touches_nothing`); apply keeps the user's comments and other `[mcp_servers.*]` tables, second run `no changes`, `--remove` strips the block (`apply_keeps_comments_and_other_servers_and_is_idempotent`, `missing_file_is_created_on_apply`). `make check` green (104 lib tests). Deviation: edits go through `toml_edit` (already a dependency) with an implicit `[mcp_servers]` header, matching Codex's own files; also `src/cli.rs` (`setup codex`, host doc) and `src/setup/mod.rs`. Codex has no hook events, so the MCP block is the whole install; proxy wiring stays T11.5.

**T10.4 release** · T0.7 · `dist-workspace.toml`, `.github/workflows/release.yml`
Do: cargo-dist for macOS arm64/x64 + Linux x64, Homebrew tap formula; `rtok --version` prints git sha.
Check: `cargo dist plan` succeeds; tag `v0.1.0` builds artifacts in CI.
Status: done 2026-09-02 · Check: `make dist-plan` (`dist plan`, cargo-dist 0.32.0) succeeds and lists the three targets, `rtok-installer.sh`, the Homebrew formula `rtok.rb`, source tarball and checksums; `rtok --version` → `rtok 0.1.0 (987a60c1d)`; `make check` green (127 tests). Deviation: cargo-dist is not pinned in `mise.toml` (compiling it on every `mise install` is slow) — `Makefile` runs it on demand as `mise x cargo:cargo-dist@0.32.0 -- dist` (`make dist-plan`, `make dist-generate`). The sha comes from `build.rs` (`RTOK_GIT_SHA`, `unknown` without a `.git`). `Cargo.toml` gained `repository = https://github.com/listepo/rtok` and `[profile.dist]`; the tap is `listepo/homebrew-tap` — both assumed, change them in `Cargo.toml`/`dist-workspace.toml` if the GitHub owner differs. NOT verified: "tag `v0.1.0` builds artifacts in CI" — this checkout has no git remote, so nothing was pushed; the tap repository must exist and CI needs a `HOMEBREW_TAP_TOKEN` secret before the first tag. `dist` also asks for a `homepage` field (warning only).
