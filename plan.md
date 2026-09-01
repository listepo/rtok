# rtok — implementation plan for a unified, plugin-based token-reduction CLI

Status: plan v1, 2026-09-01. **Progress: P0 done 2026-09-02 (T0.1–T0.8, see `done.md`); next unblocked task: P12 (T12.1), then P13, then T1.1.** Companion evidence: `research.md` (comparison, measurements, fact-check). Shape of the code: `architecture.md`. Per-plugin plan: `roadmap.md`. Propositions (not yet tasks): `ideas.md`. Finished tasks move from here to `done.md` verbatim, with their Check output.
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
| D9 | **Agents are provider-agnostic.** A small/cheap model (any provider) implements every task below (each task ≤ ~200 LOC, ≤ 3 files, one machine check). A mid-tier model reviews each phase gate. Only a frontier model changes this plan. Claude / Haiku / Sonnet / Opus are not the implementer; Claude Code, Codex, Cursor are *host* surfaces. Rewritten 2026-09-02 by user request. | User constraint: low-cost models; any provider. |
| D10 | **Retire, don't stack.** Phase 9 replaces the 81 legacy hooks with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities: 3 tools compress bash, 3 compress reads, 2 memories, 3–4 code graphs. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages (`/v1/messages`) and OpenAI (`/v1/chat/completions`, `/v1/responses`) are `Wire` adapters behind one proxy; plugins that touch requests (`archive`, `toon`) and `usage` capture work on a normalised view of tool results, never on a specific JSON shape. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. Added 2026-09-01 by user request. | Codex, OpenCode, Cursor-with-own-key and aider talk OpenAI; without it `measure` has no ground truth for them and `archive` cannot shrink their context. One proxy, two parsers is cheaper than two proxies. |
| D12 | **One config file holds every setting; every CLI flag is a config key.** `~/.rtok/config.toml` (schema and reference file: `docs/config.md`, embedded as `config/default.toml`). Precedence: defaults < user file < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env < flags. Positional per-call arguments (`hook <event>`, `expand <id>`, `run -- <cmd>`) have no key; everything else does, enforced by a test that walks the clap tree. `rtok config show --sources` tells where each value came from. Added 2026-09-01 by user request. | Hooks are spawned with a fixed command line, the proxy and MCP server run for hours, and a bench needs two reproducible configurations — none of that works with flags alone. One precedence rule beats per-flag special cases. |
| D13 | **Core persists through a sync ORM on bundled SQLite.** Diesel (`sqlite` + bundled `libsqlite3-sys` with FTS5) replaces rusqlite. Plugins never write SQL; `Store` is the only DB owner. Every surface action is a `calls` row carrying host agent, provider, model, and plugin. MCP calls and API request/response bodies are stored in `call_io` (inline under `core.call_io_inline_bytes`, else `archive`). Token counts are stored before and after each plugin run, plus MCP tokens for that plugin. Core, plugin, and module logs go to `logs` (and still to `core.log_file`). Hook path: metadata always, body only if under the inline cap — never archive, never fail the hook (D1). Added 2026-09-01 by user request. | Raw SQL in plugins cannot join MCP vs API vs hook or attribute tokens per plugin. Diesel is sync, so the ≤ 10 ms hook path stays blocking and fail-open. Async ORMs (SeaORM/SQLx) would need a runtime per hook. One schema is what `stats` can join. |
| D14 | **CLI is clap 4 (derive); config layers are figment; TOML writes are toml_edit.** Do not hand-roll flag parsing, file/env merge, or comment-preserving edits. Clap owns the subcommand tree and the T12.4 coverage walk (`features = ["derive", "wrap_help"]`). Figment owns defaults < user file < project file < env < flags and per-key provenance for `config show --sources` (named providers `default`, `user`, `project`, `env`, `flag`). `toml_edit` owns `config set`. Not used: twelf (flattens every config key into root clap args — wrong for subcommands); config-rs (no per-key provenance); confique (no clap overlay). Added 2026-09-02 by user request. | Clap is the Rust CLI standard. Figment’s docs recommend this pairing and track which provider set each key — that is T12.2. |

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

- One task = one commit on branch `rtok/<task-id>`; merge when Check passes. Never skip the Check.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message. Allowed baseline: clap (derive, wrap_help), figment (toml, env), toml_edit, serde, serde_json, diesel (sqlite, bundled libsqlite3-sys with FTS5), regex, anyhow, tokio, hyper/axum, reqwest, rmcp, tree-sitter + tree-sitter-tags, sha2, time. Reason diesel replaces rusqlite: typed models for D13 `calls`/`tokens`/`logs`; sync, so hooks stay ≤ 10 ms. Reason figment + toml_edit replace a direct `toml` dep and a hand-rolled merger: D14.
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` green before every Check.
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.
- Every new CLI flag gets a key in `config/default.toml` and a row in `docs/config.md` in the same commit (D12); flag names in the tasks below imply the key `<subcommand>.<flag>` or `plugins.<id>.<flag>`.
- No plugin shells out to, links, imports from, or reads the data of a third-party tool (D6). Port the *behaviour* described in `research.md`, never the code. Retired tools' names may appear only in `rtok doctor` (inspection), `rtok setup --replace` (retirement) and `rtok bench` (comparison). External plugins are written against the public API (`rtok::plugin`, `Registry::from_plugins`), outside this repo.

## 3. Phases and tasks

Format: **Tn.m title** · model · depends · files · do · **Check** (command → expected).

### P0 — Scaffold (goal: `rtok --version`, DB, plugin registry, hook I/O types) — **done 2026-09-01, moved to `done.md`**

T0.1–T0.8 are complete; their text, Checks and deviations are in `done.md`. Gate P0 (review: trait shape final; no plugin logic yet) is still open.

Gate P0 (review): trait shape final; no plugin logic yet. **Status: open.**

### P1 — Measure (goal: a baseline you can trust before changing anything)

**T1.1 session JSONL parser** · T0.3 · `src/measure/jsonl.rs`
Do: parse Claude Code transcripts (`~/.claude/projects/**/*.jsonl`): `tool_use` (id, name, input), `tool_result` (tool_use_id, content), assistant text, `usage` (input_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens), message index (turn). Spec reference: `scratchpad/token-research/measure_sessions.py` (port the logic, not the code). Skip malformed lines, count them.
Check: `cargo test measure::jsonl` on a 200-line fixture → expected counts; running on your real logs reports 0 parse failures.

**T1.2 `rtok stats`** · T1.1 · `src/measure/stats.rs`
Do: per-tool result sizes (count, total, mean, p95, max), Bash by command family (strip leading `cd … &&`, env assignments), MCP server groups, usage totals, cache hit rate, median final context, and **context-token-turns** per tool: for each tool_result of T tokens at turn t in a session of N turns, ctt = T × (N − t). Output table (default) or `--json`. `--since 30d`.
Check: `rtok stats --since 60d` reproduces H-measured.md within ±5 % on the same 17 sessions (Bash ≈ 1.0 M, Read ≈ 414 K est. tokens).

**T1.3 baseline snapshot** · T1.2 · `src/measure/baseline.rs`
Do: `rtok stats --save-baseline <name>` stores the report JSON in `measurements`; `rtok stats --compare <name>` prints deltas.
Check: save, then compare → all deltas 0.

**T1.4 `rtok doctor`** · T0.2 · `src/doctor.rs`
Do: report: hooks in `~/.claude/settings.json` by event and by tool (count 81 today); MCP servers and their tool counts with estimated description tokens (read `~/.claude.json` / `.mcp.json`; count via T0.5); `ANTHROPIC_BASE_URL` chain (probe each hop's `/health` or TCP); whether MCP tool search is enabled (docs: setting a base URL disables it by default — flag it); `BASH_MAX_OUTPUT_LENGTH`; `autoCompactWindow`.
Check: `rtok doctor` on this machine prints 81 hooks, lists lean-ctx (78 tools), and the 8788→8787 chain.

**T1.5 estimator calibration (optional, needs API key)** · T0.5 · `src/tokens.rs`
Do: `rtok stats --calibrate`: sample 30 archived tool results per class, call `POST /v1/messages/count_tokens`, fit chars-per-token per class, write to config. Skip silently without a key.
Check: with a key, printed fit is within 2.5–4.5 chars/token per class; without a key, exit 0 and message "skipped".

Gate P1: baseline saved (`rtok stats --save-baseline before-rtok`). Record the numbers in research.md §2.

### P2 — Hook surface (goal: one hook command per event, < 10 ms, budgeted injection)

**T2.1 `rtok hook <event>` dispatcher** · T0.6, T13.3 · `src/hooks/mod.rs`
Do: read stdin JSON, dispatch to enabled plugins in registry order, merge outputs (first `deny` wins; `updatedInput` last-writer; `additionalContext` concatenated under budget), write JSON, exit 0. Log a `calls` row (`surface=hook`, `kind=hook`, `host` from `core.host`) with elapsed ms; each plugin that runs is a child `calls` row `kind=plugin_run` with `tokens` phase `before`/`after` (estimator). `call_io` holds stdin/stdout JSON only when under `core.call_io_inline_bytes` (never archive on this path). Any panic → catch_unwind → empty output, exit 0. `events` is not written (superseded by `calls`).
Check: `cat tests/fixtures/hooks/pre_tool_bash.json | rtok hook PreToolUse` → valid JSON, exit 0; malformed stdin → `{}` and exit 0; in-memory store has a `calls` row with `kind=hook`.

**T2.2 latency harness** · T2.1 · `tests/latency.rs`
Do: spawn `rtok hook PreToolUse` 200× with the fixture; assert p95 < 10 ms on this machine (release build).
Check: `cargo test --release latency` passes.

**T2.3 `rtok setup claude`** · T2.1 · `src/setup/claude.rs`
Do: add hook entries to `~/.claude/settings.json` (backup to `settings.json.bak-<ts>` first): PreToolUse(Bash|Read), PostToolUse(*), UserPromptSubmit, SessionStart, PreCompact, PostCompact — each a single `rtok hook <event>` command, `timeout: 5`. Idempotent (skip if present). `--dry-run` prints the diff. `--remove` deletes rtok entries only.
Check: `rtok setup claude --dry-run` shows exactly 7 additions; run twice → second run "no changes".

**T2.4 `inject` plugin + budget** · T2.1 · `src/plugins/inject.rs`
Do: SessionStart/UserPromptSubmit collect `Injection { plugin, text, priority }` from other plugins; sort by priority; emit until `inject_budget_tokens`; record a `Measurement(kind=inject)` with what was emitted and what was dropped. SessionStart text must be byte-identical across two runs with unchanged state (cache friendliness) — no timestamps.
Check: unit test: three injections of 500 tokens, budget 800 → two emitted, one dropped and measured; two consecutive runs produce identical bytes.

**T2.5 PreCompact checkpoint + restore** · T2.4, T1.1 · `src/plugins/checkpoint.rs`
Do: PreCompact: read `transcript_path`, extract last 20 turns' user prompts (≤ 300 chars each), touched file paths, last error lines; store as a `notes` row kind=checkpoint. SessionStart with `source == "compact"` (and PostCompact): inject the latest checkpoint (≤ 400 tokens) through `inject`.
Check: fixture transcript → checkpoint note with 3 paths; SessionStart(compact) output contains them and stays under budget.


**T2.6 `guard` deny duplicate Read/Bash** · T2.1, T3.1 · `src/plugins/guard/mod.rs`
Do: PreToolUse(Read) and PreToolUse(Bash): if the same path or command already ran in this session within `plugins.guard.window_turns` (default 8) and an archive id exists, `Deny` with a reason that names `rtok expand <id>`. Record `Measurement`. Never deny when there is no prior archive. Config keys in the same commit (D12).
Check: two identical Read fixtures in one session → second is Deny naming the archive id; a different path → allow; `stats --plugin guard` has a row.

Gate P2: `rtok setup claude` installed alongside the legacy hooks (additive, nothing removed yet); `rtok doctor` shows 88 hooks; sessions still work.

### P3 — `cmd` plugin (goal: every Bash output archived, filtered, measured)

**T3.1 `rtok run -- <cmd>`** · T0.3 · `src/plugins/cmd/run.rs`
Do: run via `$SHELL -lc`, capture stdout+stderr (merged, ordered), preserve exit code, write raw output to `~/.rtok/archive/<id>` and an `archive` row; print output (unfiltered in this task) plus trailer `[rtok <id> · N lines · expand: rtok expand <id>]` only when > 40 lines.
Check: `rtok run -- printf 'a\nb\n'` prints `a b`, exit 0, no trailer; `rtok run -- sh -c 'exit 3'` → exit 3.

**T3.2 rule engine** · T3.1 · `src/plugins/cmd/rules.rs`
Do: pure function over `&str`: apply `Rule { match, max_lines, head, tail, drop = [regex], keep = [regex], dedupe }` to captured output. Keep-regexes always survive (`error|warning|panic|FAIL|Traceback` built in); drop-regexes remove lines; `dedupe` collapses consecutive repeats to `<line> (×N)`; then head/tail with `… N lines omitted (expand <id>)`. Non-zero exit → last 80 lines verbatim, no rule applied. No I/O, no subprocess.
Check: unit tests: 300 `ok` lines + one `error:` line with `max_lines = 20` → ≤ 20 lines that include the error line; exit-3 input returns its last 80 lines untouched.

**T3.3 family formatters + default rules** · T3.2 · `src/plugins/cmd/formatters.rs`, `rules/default.toml`, `tests/cmd_golden/`
Do: written from scratch here (D6); the command families in `research.md` (rtk's list) are the spec, not the code. A formatter is `fn(argv: &[String], output: &str) -> Option<String>`; `None` falls back to the rules. Formatters: `cargo build|test|clippy` (per-target status, errors as file:line + message, test counts + failing names), `git status|diff|log` (compact paths, stat lines, one line per commit), `pytest|jest|vitest|go test` (pass/fail counts, failing names, first assertion line each), `ls|find|tree` (columns, depth cap). `rules/default.toml` covers grep/rg, sed, cat, make, curl, npm/pnpm/node. Never redact.
Check: golden tests `tests/cmd_golden/*.{in,out}` for 10 families; a fixture with a fake AWS key must appear unchanged in output (no redaction surprises).

**T3.4 PreToolUse(Bash) rewrite** · T2.1, T3.1 · `src/plugins/cmd/hook.rs`
Do: return `updatedInput.command = "rtok run -- " + original` unless: first word is in `plugins.cmd.never_wrap` (default `rtok`, `sudo`), contains heredoc `<<`, `&` background, `-i`/`--interactive`, or config `plugins.cmd.rewrite = false`. Emit `permissionDecisionReason` "wrapped by rtok".
Check: fixture with `git status` → wrapped; fixture with `cat <<EOF` → untouched; fixture with `sudo ls` → untouched.

**T3.5 `rtok expand <id>`** · T3.1 · `src/expand.rs`
Do: print archived payload; `--lines a-b`; `--grep re`. Also exposed later as MCP tool (T4.1).
Check: `rtok expand <id from T3.1>` prints the raw output; unknown id → exit 1 with message.

**T3.6 measurement wiring** · T3.1–T3.4 · `src/plugins/cmd/mod.rs`
Do: every run writes `Measurement { kind: formatter|rule|raw, before, after }`; `rtok stats --plugin cmd` shows per-family savings and archive hit count (how often `expand` was called — the honesty metric).
Check: after 3 runs, `rtok stats --plugin cmd --json` has 3 rows with before ≥ after.

Gate P3: disable the legacy Bash-compression hooks in settings, run one working day, `rtok stats --compare before-rtok`. Keep only if Bash context-token-turns fall and `expand` rate < 5 %.

### P4 — `read` plugin + MCP server (goal: replace lean-ctx's 78 tools with 5 and the 3.1 K/turn banner with 0)

**T4.1 `rtok mcp`** · T0.4, T13.3 · `src/mcp.rs`
Do: rmcp stdio server exposing tools from all plugins' `mcp_tools()`; register `expand` from T3.5. Tool descriptions ≤ 60 tokens each (measured by T0.5 in a test). Every `tools/call` writes `calls` (`kind=mcp_call`, `plugin` from the `ToolDef`, `host` from `core.host`) with full arguments and result in `call_io` (archive if over cap — MCP is not the hook path). `tokens`: phase `before` = estimate of arguments, phase `after` = estimate of result, phase `mcp` on the owning plugin (same after count, so `stats --plugin` includes MCP).
Check: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | rtok mcp` lists `expand`; test asserts every description ≤ 60 tokens; a fixture `tools/call` inserts `calls` + `call_io` + three `tokens` rows.

**T4.2 `read` tool: full/lines** · T4.1 · `src/plugins/read/mod.rs`
Do: `read(path, mode=full|lines, range?)` with line numbers, size cap (default 20 K chars, then head/tail + archive id). Root guard: path must be under cwd or config `allow_paths`.
Check: read a 3-line fixture → 3 numbered lines; a 100 KB file → capped output + archive id; `../etc/passwd` → error.

**T4.3 `read` map/signatures via tree-sitter-tags** · T4.2 · `src/plugins/read/outline.rs`
Do: features `lang-rust, lang-ts, lang-js, lang-python, lang-dart, lang-c, lang-go` (default all); `mode=map` → definitions (kind, name, line) from each grammar's tags query; `mode=signatures` → definition lines verbatim. Unknown language → fall back to `lines 1-60` + note.
Check: `read(src/main.rs, mode=map)` on this crate lists `fn main`; golden tests per language on 20-line fixtures.

**T4.4 re-read dedup** · T4.2 · `src/plugins/read/cache.rs`
Do: on `read`, hash content; if same session already returned this sha256 for this path and the same mode/range → return `unchanged since <archive_id> (N lines)`; record measurement. Invalidate on PostToolUse(Edit|Write) for that path.
Check: two identical reads → second response < 80 chars and a measurement row; edit fixture between reads → full content again.

**T4.5 `search` + `tree`** · T4.1 · `src/plugins/read/search.rs`
Do: `search(pattern, path, max=50)` regex over files respecting `.gitignore` (use `ignore` crate), output `path:line: snippet` (≤ 120 chars); `tree(path, depth=2)` compact listing with sizes.
Check: `search("fn main", ".")` finds `src/main.rs`; results never exceed `max`.

**T4.6 PreToolUse(Read) advice** · T2.1, T4.2 · `src/plugins/read/hook.rs`
Do: for native `Read` of a file > `read.native_max_bytes` (default 32 K) that was not edited in the last 5 turns: `permissionDecision = "deny"` with reason "use rtok read(mode=map) first; native Read allowed for files you are about to edit". Config off switch. Never deny for files under 32 K (edit gate stays cheap).
Check: fixture Read of a 100 KB path → deny with reason; 2 KB path → no output.

**T4.7 register MCP** · T2.3, T4.1 · `src/setup/claude.rs`
Do: `rtok setup claude --mcp` adds `rtok` to `~/.claude.json` mcpServers (stdio, command `rtok mcp`), idempotent, backup.
Check: dry-run shows one server entry; second run "no changes".

Gate P4: disable lean-ctx hooks and MCP server for one day; compare `rtok stats` Read/MCP rows and injection tokens per turn vs baseline.

### P5 — `proxy` + `archive` (goal: ground-truth usage and cache-safe shrinking of old tool results)

**T5.1 passthrough proxy** · T0.3, T13.3 · `src/proxy/mod.rs`
Do: axum on `127.0.0.1:8790`; forward `POST /v1/messages` (and everything else) to `RTOK_UPSTREAM` (default `https://api.anthropic.com`, may be `http://127.0.0.1:8788` to chain behind headroom during A/B). Stream SSE responses unchanged. Parse `usage` from the final `message_delta`/non-streaming body; insert `usage` row (session from `metadata.user_id` or header if present, else request hash). Also write `calls` (`kind=api_request`, `surface=proxy`, `provider`+`model` upserted from the request, `host` from `core.host`) with `call_io` (archive bodies over cap) and `tokens` `source=provider` from the same four counters. `usage.call_id` points at that row.
Check: `RTOK_UPSTREAM=http://127.0.0.1:9999` with a mock upstream (test) → response bytes identical; `usage` row inserted with 4 counters; matching `calls`/`call_io`/`tokens` rows; `models.slug` equals the request `model`.

**T5.2 `rtok proxy` lifecycle** · T5.1 · `src/proxy/cli.rs`
Do: `rtok proxy [--port] [--upstream] [--mode passthrough|compress]`; `rtok setup claude --proxy` sets `env.ANTHROPIC_BASE_URL` in settings.json (backup) and prints how to revert. `/health` endpoint.
Check: `curl :8790/health` → `{"ok":true,"mode":"passthrough"}`.

**T5.3 live-zone archive rewrite** · T5.1 · `src/plugins/archive.rs`
Do: in `compress` mode: for `tool_result` blocks that are (a) older than `archive.keep_turns` (default 4 turns from the end), (b) larger than `archive.min_tokens` (default 1,500 est.), replace content with `[archived <id>: first 8 lines … last 4 lines · N tokens · expand(<id>)]`. **Decisions are keyed by `tool_use_id` and persisted**, so the same block is rewritten identically on every later request (frozen prefix stays byte-stable). Never touch `system`, `tools`, the last `keep_turns` turns, or any `tool_result` whose id was `expand`ed. Record measurement per rewritten block. Child `calls` row `kind=plugin_run` plugin=`archive` with `tokens` phase `before` (est. of the block) and `after` (est. of the pointer).
Check: fixture request with 6 turns → only turns 1–2 large results rewritten; sending the same request twice yields byte-identical rewritten bodies; unit test proves the prefix up to the first rewritten block is unchanged.

**T5.4 `expand` through the proxy** · T5.3, T4.1 · `src/plugins/archive.rs`
Do: MCP `expand(id, lines?)` returns the archived original (from T5.3 store); mark id as expanded → T5.3 stops rewriting it from the next request on. Track expand rate.
Check: expand → next fixture request contains the original block again.

**T5.5 cache-health report** · T5.1 · `src/measure/cache.rs`
Do: `rtok stats --cache`: per session, cache_read vs cache_creation per turn, detect "cache busts" (turn where cache_creation > 20 K and cache_read drops), attribute to tools-array or system-prompt changes when the proxy saw them.
Check: fixture with an injected tools-array change → one bust flagged with cause `tools`.

Gate P5: run proxy in passthrough for two days (usage ground truth), then `compress` for two days; compare cache_read per turn, output tokens, expand rate. Keep `compress` only if context-token-turns fall ≥ 15 % with expand rate < 5 %.

### P6 — `memory` plugin (goal: one memory instead of two, zero LLM cost)

**T6.1 notes API** · T0.3 · `src/plugins/memory/mod.rs`
Do: MCP tools `mem_save(kind, title, body, project?)`, `mem_search(query, limit=5)` → ids + titles + 120-char snippets (FTS5 `bm25`), `mem_get(id)` → full body. Project = git root name of cwd.
Check: save 3, search returns the right one first, get returns the full body.

**T6.2 SessionStart recall** · T6.1, T2.4 · `src/plugins/memory/inject.rs`
Do: inject the last 5 note titles + ids for the current project (≤ 200 tokens) through `inject` with priority 10; never bodies.
Check: fixture with 20 notes → 5 titles, ≤ 200 tokens, byte-stable across runs.

**T6.3 import** · T6.1 · `src/plugins/memory/import.rs`
Do: `rtok memory import <file.jsonl>`: one note per line `{kind, title, body, ts?, project?}`. Users export their previous memory tool to that shape themselves; rtok knows no third-party schema (D6). Dedupe by sha256 of body; print inserted/skipped/malformed counts; exit 0.
Check: a 50-line fixture → 50 rows; re-import → 0 inserted, 50 skipped; one malformed line is counted, skipped, exit 0.

Gate P6: disable engram + claude-mem plugins for a week; compare per-turn injection tokens and MCP tool-description tokens (`rtok doctor`). Revert if recall quality is noticeably worse (subjective, note it).

### P7 — modes + instruction hygiene

**T7.1 modes as data** · T2.4 · `modes/terse.md`, `modes/yagni.md`, `src/plugins/inject.rs`
Do: copy the intent of caveman (terse output) and ponytail (YAGNI ladder) into ≤ 250-token markdown files under `~/.rtok/modes/`; `rtok setup --mode terse,yagni` enables; injected once per session via `inject` (priority 5), not per prompt.
Check: `rtok hook SessionStart` output contains the mode text once; UserPromptSubmit output does not.

**T7.2 instruction audit** · T1.4 · `src/doctor.rs`
Do: `rtok doctor --instructions`: token count of `~/.claude/CLAUDE.md` + project CLAUDE.md + every enabled plugin's SessionStart text (lean-ctx, engram, ponytail, claude-mem, token-optimizer today); flag duplicates (same sentence in two files) and anything > 1,000 tokens.
Check: on this machine, report lists ≥ 4 injectors and their token totals.

Gate P7: A/B (P9 harness) `terse` on/off on 6 tasks; keep only if output tokens fall without task failures.

### P8 — `graph` plugin (goal: `symbol`/`callers`/`outline` from an index rtok builds itself, replacing four graph servers)

**T8.1 symbol index** · T4.3, T4.5 · `src/plugins/graph/index.rs`, next `migrations/NNNN.sql`
Do: table `symbols(path, name, kind, line, is_def, file_sha)`. Walk the repo respecting `.gitignore` (`ignore` crate from T4.5); run the T4.3 tags queries for definitions **and** reference sites per supported language; insert. Incremental: skip files whose sha256 is unchanged, delete rows of removed files. `rtok graph index [path]`, plus lazy indexing on the first tool call; PostToolUse(Edit|Write) marks that file stale (no indexing on the hook path).
Check: index this crate → `symbols` contains `main` (def) and ≥ 1 reference to `Registry`; a second run inserts 0 rows; editing one fixture file re-indexes only that file.

**T8.2 MCP tools** · T8.1, T4.1 · `src/plugins/graph/mod.rs`
Do: `symbol(name)` → definitions (`path:line`, kind); `callers(name)` → reference sites grouped by file with the line text; `outline(path)` → definitions in one file (reuses `read` mode=map). Cap each response at `plugins.graph.max_tokens` (2 K): head + `N more, expand <id>`. Measurement per call (capped vs uncapped estimate).
Check: `symbol("main")` → `src/main.rs`; `callers("estimate")` lists `src/plugin.rs`; a 500-hit fixture is capped and carries an archive id.

Gate P8: measure MCP description tokens saved by disabling the other graph servers (code-review-graph 30 tools, serena ~25, lean-ctx 78); index time on this repo < 2 s.

### P9 — A/B bench + migration (goal: replace 81 hooks with ≤ 8, keep only what measures)

**T9.1 `rtok bench`** · T1.1 · `src/bench.rs`, `bench/tasks.toml`
Do: run `claude -p "<task>" --output-format json --settings <A|B.json>` for each task × n runs (default 3), collect `usage`/`total_cost_usd` from the result JSON and the transcript, print per-config mean input/cache/output tokens and cost, and the task pass rate (each task has a shell `check`). Tasks: 6 small edits on a fixture repo (add a function, fix a bug, write a test, rename, explain a module, run tests).
Check: `rtok bench --dry-run` lists 6 tasks × 2 configs × 3 runs; a real run produces a table.

**T9.2 baseline vs rtok** · T9.1 · `bench/results/*.json`
Do: config A = current settings (81 hooks, both proxies); config B = rtok only (7 hooks, `rtok mcp`, `rtok proxy compress`, legacy hooks/MCP off). Run, save, summarize in research.md §2.
Check: results committed; summary table with cost delta and pass rate.

**T9.3 `rtok setup claude --replace`** · T2.3 · `src/setup/migrate.rs`
Do: with backup: remove hook entries whose command matches a legacy list (`rtk hook`, `lean-ctx hook`, `caveman-proxy`, `caveman shrink-hook`, token-optimizer `python-launcher.sh`), remove `ANTHROPIC_BASE_URL` pointing at 8788/8787 (set 8790), disable MCP servers `lean-ctx`, `code-review-graph` (keep serena optional), keep everything unrelated (orca, holdmylid, tokenbar, cbm). Print the diff; require `--yes`.
Check: dry-run on a copy of today's settings shows 8 remaining rtok hooks + non-token hooks; JSON stays valid.

**T9.4 legacy stack folder** · — · `legacy/`
Do: in `~/GitHub/reduce-token` (separate directory, not this repo): move `docker-compose.yml`, `bifrost-config/`, `caveman/`, `headroom/`, `.env.example` into `legacy/` with a README line "kept for A/B; bifrost semantic cache retired (see research.md)".
Check: `docker compose -f legacy/docker-compose.yml config` still validates.

**T9.5 README** · all · `README.md`
Do: replace the current README with: what rtok is, install, `rtok setup claude`, `rtok stats`, plugin table, measured results table from T9.2, honest caveats (estimates ±15 %, what is lossless, what is not).
Check: every command in the README runs (`make readme-check` executes fenced `bash` blocks that are marked `# check`).

Gate P9 (review + your decision): adopt config B if cost per passed task is lower and pass rate is equal; otherwise keep the measured winners only.

### P10 — other hosts + release

**T10.1 Cursor** · T2.1 · `src/setup/cursor.rs`
Do: write `~/.cursor/hooks.json` entries (beforeShellExecution → `rtok hook PreToolUse --host cursor` mapping fields) and MCP registration. Field mapping documented in code.
Check: fixture Cursor payload → wrapped command JSON.

**T10.2 OpenCode** · T3.1 · `hosts/opencode/rtok.ts`
Do: plugin using `tool.execute.after` to replace bash output with `rtok filter --stdin` (new subcommand: filter text from stdin without executing). This is the one host where post-execution replacement is possible.
Check: `printf '...' | rtok filter --cmd 'git status'` returns filtered text; plugin unit test with the OpenCode plugin API mock.

**T10.3 Codex** · T4.7 · `src/setup/codex.rs`
Do: MCP registration in `~/.codex/config.toml`. Proxy wiring for Codex is T11.5.
Check: dry-run diff shows one `[mcp_servers.rtok]` block.

**T10.4 release** · T0.7 · `dist-workspace.toml`, `.github/workflows/release.yml`
Do: cargo-dist for macOS arm64/x64 + Linux x64, Homebrew tap formula; `rtok --version` prints git sha.
Check: `cargo dist plan` succeeds; tag `v0.1.0` builds artifacts in CI.

### P11 — OpenAI API surface (goal: same proxy, same plugins, same numbers for OpenAI-API hosts) — added 2026-09-01 (D11)

**T11.1 `Wire` adapter + Anthropic behind it** · T5.1, T5.3 · `src/proxy/wire.rs`, `src/proxy/anthropic.rs`
Do: `trait Wire { fn matches(path) -> bool; fn tool_results(req: &mut Value) -> Vec<ToolResultRef>; fn usage_from_body(body) -> Option<Usage>; fn usage_from_sse(event) -> Option<Usage> }` where `ToolResultRef { id, content: &mut String/Value, turn }` and `Usage { input, cache_create, cache_read, output }`. Move every `/v1/messages`-specific line from T5.1/T5.3 into `anthropic.rs`; `archive` and `proxy` call only the trait.
Check: all P5 tests pass unchanged; `grep -r '"tool_result"' src/plugins/archive.rs` finds nothing (format knowledge lives in the wire).

**T11.2 OpenAI Chat Completions wire** · T11.1 · `src/proxy/openai_chat.rs`, `tests/fixtures/proxy/openai_chat_*.json`
Do: route `POST /v1/chat/completions` to `RTOK_OPENAI_UPSTREAM` (default `https://api.openai.com`). Tool results = messages with `role: "tool"` keyed by `tool_call_id`. Usage from `usage.prompt_tokens`, `usage.completion_tokens`, `usage.prompt_tokens_details.cached_tokens` (→ `cache_read`; `cache_create = 0`). Streaming: SSE `data:` lines ending with `data: [DONE]`; when the request streams and lacks `stream_options.include_usage`, add it so the final chunk carries `usage` (this is the one byte-level change passthrough mode makes; documented). Non-streaming: usage from the body.
Check: mock upstream fixture (streaming and non-streaming) → response bytes identical; `usage` row inserted with `api = openai_chat` and `cache_read` populated from `cached_tokens`.

**T11.3 OpenAI Responses wire** · T11.1 · `src/proxy/openai_responses.rs`, `tests/fixtures/proxy/openai_responses_*.json`
Do: route `POST /v1/responses`. Tool results = `input[]` items of type `function_call_output` keyed by `call_id`. Usage from `usage.input_tokens`, `usage.output_tokens`, `usage.input_tokens_details.cached_tokens`; streaming: final `response.completed` event. Respect `previous_response_id` (nothing to rewrite in the request when history is server-side — record usage only).
Check: fixtures → identical bytes; `usage` row with `api = openai_responses`; a request with `previous_response_id` produces zero rewrites in compress mode.

**T11.4 `archive` across wires** · T11.1–T11.3, T5.3 · `src/plugins/archive/mod.rs`, `tests/fixtures/proxy/*_6turns.json`
Do: T5.3's rules (older than `keep_turns`, larger than `min_tokens`, keyed by the wire's tool-result id, persisted, byte-stable) applied through `Wire::tool_results` for all three formats. `expand` marks ids across formats.
Check: a 6-turn fixture per format → only turns 1–2 large results rewritten; same request twice → byte-identical bodies; prefix before the first rewrite unchanged (same test as T5.3, parameterised over wires).

**T11.5 setup for OpenAI hosts** · T5.2, T11.2 · `src/setup/codex.rs`, `src/setup/opencode.rs`, `src/doctor.rs`
Do: `rtok setup codex --proxy` writes a `model_provider` with `base_url = http://127.0.0.1:8790/v1` in `~/.codex/config.toml` (backup, dry-run, idempotent, `--remove`); `rtok setup opencode --proxy` sets `OPENAI_BASE_URL` in its config; `rtok doctor` shows the `OPENAI_BASE_URL` chain next to the Anthropic one. Print how to revert.
Check: each dry-run shows exactly one change; second run "no changes"; `rtok doctor` lists both chains.

**T11.6 `usage.api` + per-API stats** · T11.2, T13.2 · `migrations/0003.sql`, `src/measure/stats.rs`
Do: migration adds `usage.api TEXT NOT NULL DEFAULT 'anthropic'` (`anthropic | openai_chat | openai_responses`); `rtok stats` prints usage totals and cache hit rate per API; `rtok stats --cache` (T5.5) handles OpenAI cached_tokens (no cache_create signal → busts detected from cache_read drops only).
Check: `cargo test store::` still applies migrations idempotently (0001–0003); fixture usage rows for two APIs → two rows in the stats table.


**T11.7 `toon` on Wire tool results** · T11.1, T5.3 · `src/plugins/toon/mod.rs`
Do: `proxy_filter` on the normalised `Wire` view (D11): tabular JSON arrays/objects → TOON when `plugins.toon.enabled` (default **false**). Encoder written here (D6), deterministic. Archive the original JSON first; the encoded block references the archive id. Record `Measurement` per rewritten block. Off → request bytes identical to passthrough.
Check: default off, fixture request bytes identical; enabled on a 3×4 JSON table → `after_bytes` < `before_bytes` and a measurement row; decode of the TOON recovers the same keys.

Gate P11: run one OpenAI-API host (Codex) through the proxy in passthrough for two days; every request has a `usage` row. Then compress for two days; keep only under the P5 gate rule (context-token-turns − 15 %, expand rate < 5 %). Record in research.md §2.

### P12 — Config file (goal: every setting in one file, one precedence rule, no flag without a key) — added 2026-09-01 (D12)

Runs right after P0's gate: P1–P11 tasks that add flags then wire them through this instead of ad-hoc `clap` defaults. Implementation is clap + figment + toml_edit (D14), not a custom merge.

**T12.1 typed schema + reference file** · T0.2 · `src/config.rs`, `config/default.toml`, `docs/config.md`
Do: replace the free-form `[plugins.<id>]` extras with typed sections for every table in `docs/config.md` (`hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `expand`, `filter`, `plugins.<id>` each with its keys). `#[serde(deny_unknown_fields)]` on every section; `#[serde(default)]` everywhere so partial files work. `config/default.toml` is the annotated reference embedded with `include_str!`; a fresh install writes it verbatim (not a serialised struct, so comments survive). Move `core.inject_budget_tokens` to `plugins.inject.budget_tokens`, accepting the old key with a one-line warning. `rtok config init [--force]`, `rtok config path`.
Check: `cargo test config::` → `config/default.toml` parses with zero unknown keys and equals `Config::default()`; `RTOK_HOME=$(mktemp -d) rtok config init && diff $RTOK_HOME/config.toml config/default.toml` is empty.

**T12.2 layering + precedence + `config show`** · T12.1 · `src/config/layers.rs`, `src/main.rs`, `Cargo.toml`
Do: a `Figment` with named providers, merge order: `Serialized::defaults(Config::default())` (`default`) → `Toml::file` user (`user`; path from `RTOK_CONFIG` / `--config`) → `Toml::file` `<git root>/.rtok.toml` (`project`) → `Env::prefixed("RTOK_").split("_")` (`env`; lists comma-separated; map legacy `RTOK_UPSTREAM` / `RTOK_OPENAI_UPSTREAM`) → `Serialized` of clap `Option<T>` fields that are `Some` (`flag`). Extract `Config`. Provenance from figment metadata, not a side table. `rtok config show [--sources] [--json]` and `rtok config get <key>`. Drop the direct `toml` dependency (figment’s `toml` feature parses). Add `toml_edit` here (used by T12.3). Enable clap `wrap_help`. No hand-rolled deep-merge.
Check: `RTOK_PROXY_PORT=1 rtok config show --sources | grep proxy.port` → `1 (env)`; `RTOK_PROXY_PORT=1 rtok proxy --port 2 --dry-run` reports port 2; a project `.rtok.toml` with `[plugins.read] allow_paths` shows as `(project)`; `grep -rn 'toml::' src` → nothing; `grep -rn figment src/config` finds the providers.

**T12.3 `config validate` + `config set`** · T12.1 · `src/config/validate.rs`
Do: `rtok config validate [path]` → unknown key, wrong type, out-of-range (`port` 1–65535, `keep_turns` ≥ 1, `budget_tokens` ≥ 0, `mode` enum) with file:line, exit 1. Elsewhere the same problems are one stderr warning and defaults are used — hooks never fail on config. `rtok config set <key> <value>` edits the user file in place preserving comments (`toml_edit`, D14 — the crate that round-trips TOML comments; figment does not write files).
Check: a file with `[proxy] port = 70000` → exit 1 naming the line; `echo '{}' | RTOK_CONFIG=bad.toml rtok hook PreToolUse` → `{}` and exit 0; `set proxy.port 8791` then `get proxy.port` → 8791 and the comment above `[proxy]` is intact.

**T12.4 flag ↔ key coverage test** · T12.2 · `tests/config_coverage.rs`
Do: walk `Cli::command()` recursively; for every non-positional arg that is not in a tiny allow-list (`--config`, `--home`, `--help`, `--version`, `--json` where `stats.format` covers it, action flags `--remove`, `--replace`, `--calibrate`, `--cache`, `--force`) assert `<path>.<arg>` (dashes → underscores; `run`/`filter` args map under `plugins.cmd`) exists in `config/default.toml`. Also the reverse: every key in `default.toml` is read somewhere (grep the source for the key's last segment) so dead keys fail too.
Check: `cargo test config_coverage` passes; adding `--foo` to any subcommand without a key fails the test.

Gate P12 (review): `docs/config.md`, `config/default.toml` and `Config` agree; no subcommand keeps its own defaults; merge is figment, CLI is clap, `config set` is toml_edit (D14).

### P13 — ORM + action store (goal: every MCP call, API request, plugin run, and log is a typed row) — added 2026-09-01 (D13)

Runs right after P12, before P1 writes any rows. `Store` becomes Diesel over bundled SQLite; plugins keep using `Ctx` and never SQL. `events` is superseded by `calls` (0001 table stays, nothing new writes it). `measurements` (D3) and `usage` stay the savings/ground-truth ledgers; they gain `call_id`. FTS5 for `notes_fts` stays as `diesel::sql_query` (Diesel cannot model `VIRTUAL TABLE`).

Schema (`migrations/0002.sql`). Integers are i64. Booleans are 0/1. `ts` is unixepoch. Foreign keys ON.

```sql
-- Dimension: host agents, API providers, models (upserted from traffic).
CREATE TABLE hosts (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,          -- claude | cursor | codex | opencode | aider | other
    kind TEXT NOT NULL,                 -- cli | ide | other
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE providers (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,          -- anthropic | openai | other
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE models (
    id INTEGER PRIMARY KEY,
    provider_id INTEGER NOT NULL REFERENCES providers(id),
    slug TEXT NOT NULL,                 -- request `model` field
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (provider_id, slug)
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,                -- host session id
    host_id INTEGER REFERENCES hosts(id),
    project TEXT,
    cwd TEXT,
    source TEXT,                        -- startup | resume | compact | …
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER
);

-- Unified action log. parent_id nests plugin_run under hook | mcp_call | api_request.
CREATE TABLE calls (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    host_id INTEGER REFERENCES hosts(id),
    provider_id INTEGER REFERENCES providers(id),
    model_id INTEGER REFERENCES models(id),
    plugin TEXT,                        -- null = core / surface
    surface TEXT NOT NULL,              -- hook | mcp | proxy | cli
    kind TEXT NOT NULL,                 -- hook | mcp_call | api_request | plugin_run | expand | cli
    parent_id INTEGER REFERENCES calls(id),
    name TEXT,                          -- hook event, MCP tool, HTTP path
    ms REAL,
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT
);
CREATE INDEX calls_session ON calls (session_id, ts);
CREATE INDEX calls_kind ON calls (kind, ts);
CREATE INDEX calls_plugin ON calls (plugin, ts);
CREATE INDEX calls_parent ON calls (parent_id);

-- Full MCP args/result and API request/response. Over core.call_io_inline_bytes → archive.
CREATE TABLE call_io (
    call_id INTEGER PRIMARY KEY REFERENCES calls(id),
    request_bytes INTEGER NOT NULL DEFAULT 0,
    response_bytes INTEGER NOT NULL DEFAULT 0,
    request_sha256 TEXT,
    response_sha256 TEXT,
    request_json TEXT,
    response_json TEXT,
    request_archive TEXT REFERENCES archive(id),
    response_archive TEXT REFERENCES archive(id)
);

-- Token counts. One row per (call, plugin, phase).
-- phase=before|after: estimator or provider, around a plugin_run or whole api_request.
-- phase=mcp: tokens of MCP traffic owned by this plugin (args+result of its tools).
CREATE TABLE tokens (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    call_id INTEGER NOT NULL REFERENCES calls(id),
    plugin TEXT,
    phase TEXT NOT NULL,                -- before | after | mcp
    source TEXT NOT NULL,               -- estimate | provider | mcp
    tokens INTEGER NOT NULL,
    bytes INTEGER,
    input INTEGER,
    output INTEGER,
    cache_create INTEGER,
    cache_read INTEGER
);
CREATE INDEX tokens_call ON tokens (call_id, phase);
CREATE INDEX tokens_plugin ON tokens (plugin, ts);

-- Core, plugin, and module logs (also still written to core.log_file).
CREATE TABLE logs (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    level TEXT NOT NULL,                -- error | warn | info | debug
    source TEXT NOT NULL,               -- core | plugin | module
    name TEXT NOT NULL,                 -- plugin id or rust module path
    session TEXT,
    call_id INTEGER REFERENCES calls(id),
    plugin TEXT,
    message TEXT NOT NULL,
    fields TEXT                         -- JSON extras
);
CREATE INDEX logs_ts ON logs (ts);
CREATE INDEX logs_source ON logs (source, name, ts);
CREATE INDEX logs_session ON logs (session, ts);

ALTER TABLE measurements ADD COLUMN call_id INTEGER REFERENCES calls(id);
ALTER TABLE usage ADD COLUMN call_id INTEGER REFERENCES calls(id);

INSERT INTO hosts (id, slug, kind) VALUES
  (1,'claude','cli'),(2,'cursor','ide'),(3,'codex','cli'),
  (4,'opencode','cli'),(5,'aider','cli'),(6,'other','other');
INSERT INTO providers (id, slug, name) VALUES
  (1,'anthropic','Anthropic'),(2,'openai','OpenAI'),(3,'other','Other');
```

**T13.1 Diesel replaces rusqlite** · T0.3 · `Cargo.toml`, `src/store/mod.rs`, `src/store/schema.rs`
Do: convert `src/store.rs` to `src/store/mod.rs`. Depend on `diesel` 2.2 (`sqlite`, `returning_clauses_for_sqlite_3_35`) and `libsqlite3-sys` bundled with FTS5 (confirm `notes_fts` still builds; enable `SQLITE_ENABLE_FTS5` if the bundle omits it). Drop rusqlite. Keep the filename-keyed migration runner and WAL/`synchronous=NORMAL`. `table!` macros for the six 0001 tables. `Store` holds `diesel::sqlite::SqliteConnection`. No `Store::conn()` leaking the driver. Existing `insert_measurement` and the three store tests pass unchanged in behaviour.
Check: `grep -rn rusqlite Cargo.toml src tests` → nothing; `cargo test store::` green; `open_on_disk_uses_wal` still asserts WAL; `notes_fts` MATCH still finds an inserted note.

**T13.2 schema 0002 + models** · T13.1 · `migrations/0002.sql`, `src/store/schema.rs`, `src/store/models.rs` (also `architecture.md` §7, mechanical)
Do: apply the DDL above. Diesel `table!` + structs with associations (`Call` belongs_to host/provider/model/session, `has_many` tokens/logs, `has_one` call_io). `PRAGMA foreign_keys=ON` on open. Update `architecture.md` §7 table list to match. Do not drop `events`.
Check: `cargo test store::` → migrate twice is 0; `sqlite_master` contains `hosts,providers,models,sessions,calls,call_io,tokens,logs`; seed `hosts` has 6 rows; inserting a `calls` row with a bad `host_id` fails.

**T13.3 `Store`/`Ctx` write API** · T13.2 · `src/store/mod.rs`, `src/plugin.rs`
Do: `Store` methods: `upsert_session`, `upsert_model(provider_slug, model_slug)`, `insert_call`, `insert_call_io` (inline or archive by cap; hook surface never archives), `insert_tokens`, `insert_log`, `purge_calls_older_than(days)` (0 = skip). `Ctx`: `record_call`, `record_tokens`, `log(level, source, name, message)` — `log` never returns `Err` to a plugin (fail open; on DB error write `log_file` only). `record` (measurements) sets `call_id` when the plugin supplies one. Plugins and surfaces still have no SQL.
Check: one test inserts `kind=mcp_call` + `call_io` with args/result JSON + `tokens` before/after/mcp + a `logs` row `source=plugin`; round-trip equals; `insert_call_io` with a 70 KiB body and cap 64 KiB writes `archive` and nulls `request_json`; `Ctx::log` after a closed-DB failure still returns.

**T13.4 config keys** · T12.1, T13.3 · `config/default.toml`, `docs/config.md`, `src/config.rs`
Do: `[core] call_io_inline_bytes = 65536`, `retain_calls_days = 30` (0 = keep forever), `log_to_db = true`. Document: hook path never archives `call_io`; `log_file` is always written; `logs` table is written when `log_to_db`. `rtok stats` later joins `calls`/`tokens`; no new CLI in this task.
Check: `cargo test config::` parses the three keys; `docs/config.md` has a row for each; T12.4 coverage still green once those tasks exist, otherwise the keys are present in `default.toml`.

Gate P13 (review): no rusqlite; no SQL outside `src/store/`; hook-path tests never write `archive/` for `call_io`; a plugin_run has before and after token rows.

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


## 4. Definition of done for v0.1

1. `rtok doctor` shows ≤ 8 token-related hooks, one MCP server for reads/memory/graph, one proxy hop (serving both Anthropic and OpenAI wire formats, D11).
2. `rtok stats --compare before-rtok` over ≥ 5 working days shows lower context-token-turns per session and lower output tokens per passed bench task, with expand rate < 5 %.
3. Every plugin has a `Measurement` path and appears in `rtok stats --plugin <id>`.
4. Hook p95 < 10 ms; proxy adds < 20 ms per request (measured in T5.1 test).
5. README documents what is lossless, what is estimated, and how to revert (`rtok setup claude --remove`, backups).
6. `rtok config show --sources` lists every setting with its origin; the coverage test (T12.4) is green.
7. Every hook, MCP `tools/call`, and proxy request has a `calls` row with host agent + provider + model (when known); each plugin run has `tokens` before and after, and MCP tokens for that plugin when it served a tool.

## 5. Order of value (if time is short)

P1 (measure) → P2 (hooks) → P5 (proxy passthrough for ground truth) → P3 (cmd) → P4 (read) → P5 compress → P9 (bench + retire). P6–P8, P10 and P11 only after P9 shows the core pays for itself; P11 first among those if an OpenAI-API host is in daily use. P12 (config) is not optional and comes right after P0's gate, before any task adds a flag. P13 (ORM + action store) comes right after P12, before P1 writes any rows. v0.2+ Later versions (LLM compression, embeddings, LSP graph, daemon, WASM) start only after §4 v0.1 done.

## 6. Plan amendments (recorded while implementing; each is small and evidence-free by nature)

| Date | Change | Why |
|------|--------|-----|
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
| 2026-09-02 | D9 rewritten: drop Haiku/Sonnet/Opus. Tasks no longer name a model; implementer is a small/cheap model from any provider; gates are a mid-tier review; only a frontier model edits this plan. `AGENTS.md` matches. | User request: do not lock agents to Claude models. |
| 2026-09-02 | CLI + config crates: decision D14. clap 4 (derive, wrap_help) stays the CLI; figment replaces hand-rolled layering and the direct `toml` dep; toml_edit is the `config set` writer. T12.2/T12.3/Gate P12 name the crates. | User request: use the best Rust tools for CLIs and configs. |
| 2026-09-02 | `CHANGELOG.md` is generated by git-cliff (`make changelog`, `cliff.toml`, tool pinned in `mise.toml`); commit subjects `<task-id>:`/`plan:`/`docs:`/`ci:` are the grouping. T10.4 (release) runs it before tagging. | User request; the `<task-id>: <title>` commit rule already carries the information, so no hand-written changelog. |
