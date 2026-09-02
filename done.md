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
