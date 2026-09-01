# rtok — implementation plan for a unified, plugin-based token-reduction CLI

Status: plan v1, 2026-09-01. **Progress: P0 done 2026-09-01 (see `done.md`); next unblocked task: T1.1.** Companion evidence: `research.md` (comparison, measurements, fact-check). Shape of the code: `architecture.md`. Finished tasks move from here to `done.md` verbatim, with their Check output.
Crate and binary: `rtok`, this repo (`~/GitHub/rtok`). Rust 1.97.1 is pinned in `mise.toml`; run cargo as `mise exec -- cargo …` (or `mise activate`). The legacy Docker chain stays in `~/GitHub/reduce-token`. Agent instructions: `AGENTS.md` (`CLAUDE.md` is a symlink to it).

## 0. Decisions (read before any task)

| # | Decision | Why (evidence in research.md) |
|---|----------|-------------------------------|
| D1 | **Rust, one static binary, in-tree plugins behind one trait + Cargo features.** No WASM, no subprocess plugins, no daemon in v1. | Hooks run on every tool call; Rust cold start <5 ms vs ~100 ms per Python hook. Your stack runs 27 Python hooks per event chain today. |
| D2 | **One binary, three surfaces:** `rtok hook` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`). | PostToolUse hooks cannot modify tool results (verified in docs). Only PreToolUse rewrite, MCP tool replacement, or a proxy can shrink what the model sees. |
| D3 | **Measurement first.** Nothing ships until `rtok stats` reads real usage from session logs and the proxy. Every plugin logs before/after. Metric = *context-token-turns* (tokens × turns they stay in context), plus output tokens. | Vendor claims 60–95 %; your measured savings 3–40 %; JetBrains measured rtk at −7.6 % to 0 %. Nobody in the stack measures end to end. |
| D4 | **Lossless by default.** Every compression keeps the original retrievable via `rtok expand <id>` / MCP `expand`. Lossy only where the source is regenerable (re-run the command). | Caveman issue #112 (silent code corruption); trust is the product. |
| D5 | **Injection budget.** All SessionStart/UserPromptSubmit injections go through one plugin with a per-turn token cap (default 800) and a byte-stable prefix. | lean-ctx alone injects ~3.1 K tokens per turn; injections are re-read (cached) every turn. |
| D6 | **Plugins are of two kinds:** *native* (Rust module) or *adapter* (drives an installed tool). Build native only when an adapter is measured to cost more than it saves. | YAGNI. Code graph = adapter over codebase-memory-mcp; filters = delegate to rtk when installed. |
| D7 | **Prompt "modes" (terse, YAGNI) are data files, not code.** | Ponytail/caveman are markdown; measured effect must be A/B tested, not assumed. |
| D8 | **One SQLite file** (`~/.rtok/rtok.db`, WAL): events, measurements, archive index, read cache, memory (FTS5). Raw archived payloads on disk under `~/.rtok/archive/`. | engram, claude-mem, codebase-memory-mcp all converge on SQLite (+FTS5). |
| D9 | **Agents:** Haiku implements every task below (each task ≤ ~200 LOC, ≤ 3 files, one machine check). Sonnet reviews at each phase gate. Opus only to change this plan. | User constraint: low-cost models. |
| D10 | **Retire, don't stack.** Phase 9 replaces the 81 legacy hooks with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities: 3 tools compress bash, 3 compress reads, 2 memories, 3–4 code graphs. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages (`/v1/messages`) and OpenAI (`/v1/chat/completions`, `/v1/responses`) are `Wire` adapters behind one proxy; plugins that touch requests (`archive`, `toon`) and `usage` capture work on a normalised view of tool results, never on a specific JSON shape. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. Added 2026-09-01 by user request. | Codex, OpenCode, Cursor-with-own-key and aider talk OpenAI; without it `measure` has no ground truth for them and `archive` cannot shrink their context. One proxy, two parsers is cheaper than two proxies. |

Non-goals for v0.1 (each rejected on evidence): LLM-based compression (LLMLingua, claude-mem style extraction) — costs tokens to save tokens; embeddings/semantic search — no measured need; own tree-sitter call graph — adapter first; semantic response cache (bifrost) — agent contexts never repeat at 0.9 similarity and a hit returns a wrong answer. (Formerly listed here: Codex Responses-API proxy. Moved into scope as P11 on 2026-09-01, decision D11.)

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
    fn manifest(&self) -> Manifest;                    // id, kind (Native|Adapter), surfaces, default_on
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

Plugin catalogue (v0.1 scope):

| id | kind | replaces in your stack | surface | mechanism |
|----|------|------------------------|---------|-----------|
| `measure` | native | rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard | `stats`, `bench`, proxy | session JSONL ingest + proxy `usage`; context-token-turns |
| `cmd` | native + adapter | rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress | PreToolUse(Bash) → `rtok run` | archive raw output, delegate filtering to rtk if installed, else TOML rules; pointer trailer |
| `read` | native | lean-ctx ctx_read/search/tree (78 tools), token-optimizer read_cache/structure_map | MCP `read`,`search`,`tree` + PreToolUse(Read) advice | modes full/lines/map/signatures via tree-sitter-tags; re-read dedup (hash → "unchanged") |
| `archive` | native | token-optimizer archive_result, headroom CCR, caveman retrieve | proxy live zone + `expand` | replace old, large `tool_result` blocks with pointer + head/tail; deterministic per tool_use_id |
| `proxy` | native | headroom proxy, caveman-proxy | `ANTHROPIC_BASE_URL` | passthrough + SSE streaming; usage capture; never touches system, tools, or last 2 turns |
| `inject` | native | caveman shrink-hook, ponytail/caveman modes, lean-ctx banner, engram/claude-mem SessionStart context | SessionStart, UserPromptSubmit | budgeted, byte-stable prefix; modes as markdown |
| `guard` | native | token-optimizer refetch_guard/loop detection | PreToolUse | identical read/command within N turns → deny with pointer to prior result |
| `memory` | native (engram adapter until T6 measured) | engram, claude-mem | MCP `mem_save/search/get`, PreCompact checkpoint | agent-written notes, SQLite FTS5, progressive disclosure |
| `graph` | adapter | codebase-memory-mcp, code-review-graph, serena, codegraph | MCP `symbol`,`callers`,`outline` | shells to installed graph tool; caps output |
| `toon` | native, off by default | caveman toon, TOON | proxy/MCP | tabular JSON → TOON (vendor bench: 42.6 % fewer tokens) |

## 2. Working agreement for agents

- One task = one commit on branch `rtok/<task-id>`; merge when Check passes. Never skip the Check.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message. Allowed baseline: clap, serde, serde_json, rusqlite (bundled, fts5), toml, regex, anyhow, tokio, hyper/axum, reqwest, rmcp, tree-sitter + tree-sitter-tags, sha2, time.
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` green before every Check.
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.

## 3. Phases and tasks

Format: **Tn.m title** · model · depends · files · do · **Check** (command → expected).

### P0 — Scaffold (goal: `rtok --version`, DB, plugin registry, hook I/O types) — **done 2026-09-01, moved to `done.md`**

T0.1–T0.7 are complete; their text, Checks and deviations are in `done.md`. Gate P0 (sonnet review: trait shape final; no plugin logic yet) is still open.

Gate P0 (sonnet review): trait shape final; no plugin logic yet. **Status: open.**

### P1 — Measure (goal: a baseline you can trust before changing anything)

**T1.1 session JSONL parser** · haiku · T0.3 · `src/measure/jsonl.rs`
Do: parse Claude Code transcripts (`~/.claude/projects/**/*.jsonl`): `tool_use` (id, name, input), `tool_result` (tool_use_id, content), assistant text, `usage` (input_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens), message index (turn). Spec reference: `scratchpad/token-research/measure_sessions.py` (port the logic, not the code). Skip malformed lines, count them.
Check: `cargo test measure::jsonl` on a 200-line fixture → expected counts; running on your real logs reports 0 parse failures.

**T1.2 `rtok stats`** · haiku · T1.1 · `src/measure/stats.rs`
Do: per-tool result sizes (count, total, mean, p95, max), Bash by command family (strip leading `cd … &&`, env assignments), MCP server groups, usage totals, cache hit rate, median final context, and **context-token-turns** per tool: for each tool_result of T tokens at turn t in a session of N turns, ctt = T × (N − t). Output table (default) or `--json`. `--since 30d`.
Check: `rtok stats --since 60d` reproduces H-measured.md within ±5 % on the same 17 sessions (Bash ≈ 1.0 M, Read ≈ 414 K est. tokens).

**T1.3 baseline snapshot** · haiku · T1.2 · `src/measure/baseline.rs`
Do: `rtok stats --save-baseline <name>` stores the report JSON in `measurements`; `rtok stats --compare <name>` prints deltas.
Check: save, then compare → all deltas 0.

**T1.4 `rtok doctor`** · haiku · T0.2 · `src/doctor.rs`
Do: report: hooks in `~/.claude/settings.json` by event and by tool (count 81 today); MCP servers and their tool counts with estimated description tokens (read `~/.claude.json` / `.mcp.json`; count via T0.5); `ANTHROPIC_BASE_URL` chain (probe each hop's `/health` or TCP); whether MCP tool search is enabled (docs: setting a base URL disables it by default — flag it); `BASH_MAX_OUTPUT_LENGTH`; `autoCompactWindow`.
Check: `rtok doctor` on this machine prints 81 hooks, lists lean-ctx (78 tools), and the 8788→8787 chain.

**T1.5 estimator calibration (optional, needs API key)** · haiku · T0.5 · `src/tokens.rs`
Do: `rtok stats --calibrate`: sample 30 archived tool results per class, call `POST /v1/messages/count_tokens`, fit chars-per-token per class, write to config. Skip silently without a key.
Check: with a key, printed fit is within 2.5–4.5 chars/token per class; without a key, exit 0 and message "skipped".

Gate P1: baseline saved (`rtok stats --save-baseline before-rtok`). Record the numbers in research.md §2.

### P2 — Hook surface (goal: one hook command per event, < 10 ms, budgeted injection)

**T2.1 `rtok hook <event>` dispatcher** · haiku · T0.6 · `src/hooks/mod.rs`
Do: read stdin JSON, dispatch to enabled plugins in registry order, merge outputs (first `deny` wins; `updatedInput` last-writer; `additionalContext` concatenated under budget), write JSON, exit 0. Log `events` row with elapsed ms. Any panic → catch_unwind → empty output, exit 0.
Check: `cat tests/fixtures/hooks/pre_tool_bash.json | rtok hook PreToolUse` → valid JSON, exit 0; malformed stdin → `{}` and exit 0.

**T2.2 latency harness** · haiku · T2.1 · `tests/latency.rs`
Do: spawn `rtok hook PreToolUse` 200× with the fixture; assert p95 < 10 ms on this machine (release build).
Check: `cargo test --release latency` passes.

**T2.3 `rtok setup claude`** · haiku · T2.1 · `src/setup/claude.rs`
Do: add hook entries to `~/.claude/settings.json` (backup to `settings.json.bak-<ts>` first): PreToolUse(Bash|Read), PostToolUse(*), UserPromptSubmit, SessionStart, PreCompact, PostCompact — each a single `rtok hook <event>` command, `timeout: 5`. Idempotent (skip if present). `--dry-run` prints the diff. `--remove` deletes rtok entries only.
Check: `rtok setup claude --dry-run` shows exactly 7 additions; run twice → second run "no changes".

**T2.4 `inject` plugin + budget** · haiku · T2.1 · `src/plugins/inject.rs`
Do: SessionStart/UserPromptSubmit collect `Injection { plugin, text, priority }` from other plugins; sort by priority; emit until `inject_budget_tokens`; record a `Measurement(kind=inject)` with what was emitted and what was dropped. SessionStart text must be byte-identical across two runs with unchanged state (cache friendliness) — no timestamps.
Check: unit test: three injections of 500 tokens, budget 800 → two emitted, one dropped and measured; two consecutive runs produce identical bytes.

**T2.5 PreCompact checkpoint + restore** · haiku · T2.4, T1.1 · `src/plugins/checkpoint.rs`
Do: PreCompact: read `transcript_path`, extract last 20 turns' user prompts (≤ 300 chars each), touched file paths, last error lines; store as a `notes` row kind=checkpoint. SessionStart with `source == "compact"` (and PostCompact): inject the latest checkpoint (≤ 400 tokens) through `inject`.
Check: fixture transcript → checkpoint note with 3 paths; SessionStart(compact) output contains them and stays under budget.

Gate P2: `rtok setup claude` installed alongside the legacy hooks (additive, nothing removed yet); `rtok doctor` shows 88 hooks; sessions still work.

### P3 — `cmd` plugin (goal: every Bash output archived, filtered, measured)

**T3.1 `rtok run -- <cmd>`** · haiku · T0.3 · `src/plugins/cmd/run.rs`
Do: run via `$SHELL -lc`, capture stdout+stderr (merged, ordered), preserve exit code, write raw output to `~/.rtok/archive/<id>` and an `archive` row; print output (unfiltered in this task) plus trailer `[rtok <id> · N lines · expand: rtok expand <id>]` only when > 40 lines.
Check: `rtok run -- printf 'a\nb\n'` prints `a b`, exit 0, no trailer; `rtok run -- sh -c 'exit 3'` → exit 3.

**T3.2 rtk delegation** · haiku · T3.1 · `src/plugins/cmd/rtk.rs`
Do: if `rtk` is on PATH and the first argv word is in rtk's supported families (probe once with `rtk --help`, cache in DB for 24 h), execute `rtk <argv>` instead of the raw command, still archiving the raw output by running the raw command? No — run once: execute `rtk <argv>` and archive **its** output; mark measurement kind=`rtk`. Raw output is regenerable by re-running.
Check: with rtk installed, `rtok run -- git status` produces rtk-formatted output and a measurement row; with `PATH` lacking rtk, falls back to T3.1 behaviour.

**T3.3 filter rules (fallback when rtk absent or unsupported)** · haiku · T3.1 · `src/plugins/cmd/rules.rs`, `rules/default.toml`
Do: TOML rules: `[[rule]] match = "^(grep|rg)\\b"`, `max_lines`, `head`, `tail`, `drop = [regex]`, `keep = [regex]`, `dedupe = true`. Ship rules for the families that dominate your logs: grep/rg, sed, cat, ls, find, cargo build/test, pnpm/npm/node test, pytest, make, curl. Always keep lines matching `error|warning|panic|FAIL|Traceback`. Non-zero exit → last 80 lines verbatim.
Check: golden tests `tests/cmd_golden/*.{in,out}` for 10 families; a fixture with a fake AWS key must appear unchanged in output (no redaction surprises).

**T3.4 PreToolUse(Bash) rewrite** · haiku · T2.1, T3.1 · `src/plugins/cmd/hook.rs`
Do: return `updatedInput.command = "rtok run -- " + original` unless: already starts with `rtok`/`rtk`, contains heredoc `<<`, `sudo`, `&` background, `-i`/`--interactive`, or config `cmd.rewrite = false`. Emit `permissionDecisionReason` "wrapped by rtok".
Check: fixture with `git status` → wrapped; fixture with `cat <<EOF` → untouched; fixture with `rtk git status` → untouched.

**T3.5 `rtok expand <id>`** · haiku · T3.1 · `src/expand.rs`
Do: print archived payload; `--lines a-b`; `--grep re`. Also exposed later as MCP tool (T4.1).
Check: `rtok expand <id from T3.1>` prints the raw output; unknown id → exit 1 with message.

**T3.6 measurement wiring** · haiku · T3.1–T3.4 · `src/plugins/cmd/mod.rs`
Do: every run writes `Measurement { kind: rtk|rule|raw, before, after }`; `rtok stats --plugin cmd` shows per-family savings and archive hit count (how often `expand` was called — the honesty metric).
Check: after 3 runs, `rtok stats --plugin cmd --json` has 3 rows with before ≥ after.

Gate P3: disable `rtk hook claude` in settings (rtok wraps it), run one working day, `rtok stats --compare before-rtok`. Keep only if Bash context-token-turns fall and `expand` rate < 5 %.

### P4 — `read` plugin + MCP server (goal: replace lean-ctx's 78 tools with 5 and the 3.1 K/turn banner with 0)

**T4.1 `rtok mcp`** · haiku · T0.4 · `src/mcp.rs`
Do: rmcp stdio server exposing tools from all plugins' `mcp_tools()`; register `expand` from T3.5. Tool descriptions ≤ 60 tokens each (measured by T0.5 in a test).
Check: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | rtok mcp` lists `expand`; test asserts every description ≤ 60 tokens.

**T4.2 `read` tool: full/lines** · haiku · T4.1 · `src/plugins/read/mod.rs`
Do: `read(path, mode=full|lines, range?)` with line numbers, size cap (default 20 K chars, then head/tail + archive id). Root guard: path must be under cwd or config `allow_paths`.
Check: read a 3-line fixture → 3 numbered lines; a 100 KB file → capped output + archive id; `../etc/passwd` → error.

**T4.3 `read` map/signatures via tree-sitter-tags** · haiku · T4.2 · `src/plugins/read/outline.rs`
Do: features `lang-rust, lang-ts, lang-js, lang-python, lang-dart, lang-c, lang-go` (default all); `mode=map` → definitions (kind, name, line) from each grammar's tags query; `mode=signatures` → definition lines verbatim. Unknown language → fall back to `lines 1-60` + note.
Check: `read(src/main.rs, mode=map)` on this crate lists `fn main`; golden tests per language on 20-line fixtures.

**T4.4 re-read dedup** · haiku · T4.2 · `src/plugins/read/cache.rs`
Do: on `read`, hash content; if same session already returned this sha256 for this path and the same mode/range → return `unchanged since <archive_id> (N lines)`; record measurement. Invalidate on PostToolUse(Edit|Write) for that path.
Check: two identical reads → second response < 80 chars and a measurement row; edit fixture between reads → full content again.

**T4.5 `search` + `tree`** · haiku · T4.1 · `src/plugins/read/search.rs`
Do: `search(pattern, path, max=50)` regex over files respecting `.gitignore` (use `ignore` crate), output `path:line: snippet` (≤ 120 chars); `tree(path, depth=2)` compact listing with sizes.
Check: `search("fn main", ".")` finds `src/main.rs`; results never exceed `max`.

**T4.6 PreToolUse(Read) advice** · haiku · T2.1, T4.2 · `src/plugins/read/hook.rs`
Do: for native `Read` of a file > `read.native_max_bytes` (default 32 K) that was not edited in the last 5 turns: `permissionDecision = "deny"` with reason "use rtok read(mode=map) first; native Read allowed for files you are about to edit". Config off switch. Never deny for files under 32 K (edit gate stays cheap).
Check: fixture Read of a 100 KB path → deny with reason; 2 KB path → no output.

**T4.7 register MCP** · haiku · T2.3, T4.1 · `src/setup/claude.rs`
Do: `rtok setup claude --mcp` adds `rtok` to `~/.claude.json` mcpServers (stdio, command `rtok mcp`), idempotent, backup.
Check: dry-run shows one server entry; second run "no changes".

Gate P4: disable lean-ctx hooks and MCP server for one day; compare `rtok stats` Read/MCP rows and injection tokens per turn vs baseline.

### P5 — `proxy` + `archive` (goal: ground-truth usage and cache-safe shrinking of old tool results)

**T5.1 passthrough proxy** · haiku · T0.3 · `src/proxy/mod.rs`
Do: axum on `127.0.0.1:8790`; forward `POST /v1/messages` (and everything else) to `RTOK_UPSTREAM` (default `https://api.anthropic.com`, may be `http://127.0.0.1:8788` to chain behind headroom during A/B). Stream SSE responses unchanged. Parse `usage` from the final `message_delta`/non-streaming body; insert `usage` row (session from `metadata.user_id` or header if present, else request hash).
Check: `RTOK_UPSTREAM=http://127.0.0.1:9999` with a mock upstream (test) → response bytes identical; `usage` row inserted with 4 counters.

**T5.2 `rtok proxy` lifecycle** · haiku · T5.1 · `src/proxy/cli.rs`
Do: `rtok proxy [--port] [--upstream] [--mode passthrough|compress]`; `rtok setup claude --proxy` sets `env.ANTHROPIC_BASE_URL` in settings.json (backup) and prints how to revert. `/health` endpoint.
Check: `curl :8790/health` → `{"ok":true,"mode":"passthrough"}`.

**T5.3 live-zone archive rewrite** · haiku · T5.1 · `src/plugins/archive.rs`
Do: in `compress` mode: for `tool_result` blocks that are (a) older than `archive.keep_turns` (default 4 turns from the end), (b) larger than `archive.min_tokens` (default 1,500 est.), replace content with `[archived <id>: first 8 lines … last 4 lines · N tokens · expand(<id>)]`. **Decisions are keyed by `tool_use_id` and persisted**, so the same block is rewritten identically on every later request (frozen prefix stays byte-stable). Never touch `system`, `tools`, the last `keep_turns` turns, or any `tool_result` whose id was `expand`ed. Record measurement per rewritten block.
Check: fixture request with 6 turns → only turns 1–2 large results rewritten; sending the same request twice yields byte-identical rewritten bodies; unit test proves the prefix up to the first rewritten block is unchanged.

**T5.4 `expand` through the proxy** · haiku · T5.3, T4.1 · `src/plugins/archive.rs`
Do: MCP `expand(id, lines?)` returns the archived original (from T5.3 store); mark id as expanded → T5.3 stops rewriting it from the next request on. Track expand rate.
Check: expand → next fixture request contains the original block again.

**T5.5 cache-health report** · haiku · T5.1 · `src/measure/cache.rs`
Do: `rtok stats --cache`: per session, cache_read vs cache_creation per turn, detect "cache busts" (turn where cache_creation > 20 K and cache_read drops), attribute to tools-array or system-prompt changes when the proxy saw them.
Check: fixture with an injected tools-array change → one bust flagged with cause `tools`.

Gate P5: run proxy in passthrough for two days (usage ground truth), then `compress` for two days; compare cache_read per turn, output tokens, expand rate. Keep `compress` only if context-token-turns fall ≥ 15 % with expand rate < 5 %.

### P6 — `memory` plugin (goal: one memory instead of two, zero LLM cost)

**T6.1 notes API** · haiku · T0.3 · `src/plugins/memory/mod.rs`
Do: MCP tools `mem_save(kind, title, body, project?)`, `mem_search(query, limit=5)` → ids + titles + 120-char snippets (FTS5 `bm25`), `mem_get(id)` → full body. Project = git root name of cwd.
Check: save 3, search returns the right one first, get returns the full body.

**T6.2 SessionStart recall** · haiku · T6.1, T2.4 · `src/plugins/memory/inject.rs`
Do: inject the last 5 note titles + ids for the current project (≤ 200 tokens) through `inject` with priority 10; never bodies.
Check: fixture with 20 notes → 5 titles, ≤ 200 tokens, byte-stable across runs.

**T6.3 import** · haiku · T6.1 · `src/plugins/memory/import.rs`
Do: `rtok memory import --engram <path>` and `--claude-mem <path>`: copy observations (title, body, ts, project) into `notes`. Read-only on sources.
Check: import a copied engram DB → row count matches; re-import → no duplicates (dedupe by sha256 of body).

Gate P6: disable engram + claude-mem plugins for a week; compare per-turn injection tokens and MCP tool-description tokens (`rtok doctor`). Revert if recall quality is noticeably worse (subjective, note it).

### P7 — modes + instruction hygiene

**T7.1 modes as data** · haiku · T2.4 · `modes/terse.md`, `modes/yagni.md`, `src/plugins/inject.rs`
Do: copy the intent of caveman (terse output) and ponytail (YAGNI ladder) into ≤ 250-token markdown files under `~/.rtok/modes/`; `rtok setup --mode terse,yagni` enables; injected once per session via `inject` (priority 5), not per prompt.
Check: `rtok hook SessionStart` output contains the mode text once; UserPromptSubmit output does not.

**T7.2 instruction audit** · haiku · T1.4 · `src/doctor.rs`
Do: `rtok doctor --instructions`: token count of `~/.claude/CLAUDE.md` + project CLAUDE.md + every enabled plugin's SessionStart text (lean-ctx, engram, ponytail, claude-mem, token-optimizer today); flag duplicates (same sentence in two files) and anything > 1,000 tokens.
Check: on this machine, report lists ≥ 4 injectors and their token totals.

Gate P7: A/B (P9 harness) `terse` on/off on 6 tasks; keep only if output tokens fall without task failures.

### P8 — `graph` adapter

**T8.1 detect + wrap** · haiku · T4.1 · `src/plugins/graph.rs`
Do: detect installed `codebase-memory-mcp` (or serena) binary; MCP tools `symbol(name)`, `callers(name)`, `outline(path)` that spawn the tool's MCP stdio, call the matching tool, and cap output to 2 K tokens (head + "N more, use expand"). Off when nothing is installed.
Check: with codebase-memory-mcp installed and this repo indexed, `symbol("main")` returns a location; without it, tool list omits graph tools.

Gate P8: measure MCP description tokens saved by disabling the other graph servers (code-review-graph 30 tools, serena ~25, lean-ctx 78).

### P9 — A/B bench + migration (goal: replace 81 hooks with ≤ 8, keep only what measures)

**T9.1 `rtok bench`** · haiku · T1.1 · `src/bench.rs`, `bench/tasks.toml`
Do: run `claude -p "<task>" --output-format json --settings <A|B.json>` for each task × n runs (default 3), collect `usage`/`total_cost_usd` from the result JSON and the transcript, print per-config mean input/cache/output tokens and cost, and the task pass rate (each task has a shell `check`). Tasks: 6 small edits on a fixture repo (add a function, fix a bug, write a test, rename, explain a module, run tests).
Check: `rtok bench --dry-run` lists 6 tasks × 2 configs × 3 runs; a real run produces a table.

**T9.2 baseline vs rtok** · haiku · T9.1 · `bench/results/*.json`
Do: config A = current settings (81 hooks, both proxies); config B = rtok only (7 hooks, `rtok mcp`, `rtok proxy compress`, legacy hooks/MCP off). Run, save, summarize in research.md §2.
Check: results committed; summary table with cost delta and pass rate.

**T9.3 `rtok setup claude --replace`** · haiku · T2.3 · `src/setup/migrate.rs`
Do: with backup: remove hook entries whose command matches a legacy list (`rtk hook`, `lean-ctx hook`, `caveman-proxy`, `caveman shrink-hook`, token-optimizer `python-launcher.sh`), remove `ANTHROPIC_BASE_URL` pointing at 8788/8787 (set 8790), disable MCP servers `lean-ctx`, `code-review-graph` (keep serena optional), keep everything unrelated (orca, holdmylid, tokenbar, cbm). Print the diff; require `--yes`.
Check: dry-run on a copy of today's settings shows 8 remaining rtok hooks + non-token hooks; JSON stays valid.

**T9.4 legacy stack folder** · haiku · — · `legacy/`
Do: in `~/GitHub/reduce-token` (separate directory, not this repo): move `docker-compose.yml`, `bifrost-config/`, `caveman/`, `headroom/`, `.env.example` into `legacy/` with a README line "kept for A/B; bifrost semantic cache retired (see research.md)".
Check: `docker compose -f legacy/docker-compose.yml config` still validates.

**T9.5 README** · haiku · all · `README.md`
Do: replace the current README with: what rtok is, install, `rtok setup claude`, `rtok stats`, plugin table, measured results table from T9.2, honest caveats (estimates ±15 %, what is lossless, what is not).
Check: every command in the README runs (`make readme-check` executes fenced `bash` blocks that are marked `# check`).

Gate P9 (sonnet review + your decision): adopt config B if cost per passed task is lower and pass rate is equal; otherwise keep the measured winners only.

### P10 — other hosts + release

**T10.1 Cursor** · haiku · T2.1 · `src/setup/cursor.rs`
Do: write `~/.cursor/hooks.json` entries (beforeShellExecution → `rtok hook PreToolUse --host cursor` mapping fields) and MCP registration. Field mapping documented in code.
Check: fixture Cursor payload → wrapped command JSON.

**T10.2 OpenCode** · haiku · T3.1 · `hosts/opencode/rtok.ts`
Do: plugin using `tool.execute.after` to replace bash output with `rtok filter --stdin` (new subcommand: filter text from stdin without executing). This is the one host where post-execution replacement is possible.
Check: `printf '...' | rtok filter --cmd 'git status'` returns filtered text; plugin unit test with the OpenCode plugin API mock.

**T10.3 Codex** · haiku · T4.7 · `src/setup/codex.rs`
Do: MCP registration in `~/.codex/config.toml`. Proxy wiring for Codex is T11.5.
Check: dry-run diff shows one `[mcp_servers.rtok]` block.

**T10.4 release** · haiku · T0.7 · `dist-workspace.toml`, `.github/workflows/release.yml`
Do: cargo-dist for macOS arm64/x64 + Linux x64, Homebrew tap formula; `rtok --version` prints git sha.
Check: `cargo dist plan` succeeds; tag `v0.1.0` builds artifacts in CI.

### P11 — OpenAI API surface (goal: same proxy, same plugins, same numbers for OpenAI-API hosts) — added 2026-09-01 (D11)

**T11.1 `Wire` adapter + Anthropic behind it** · haiku · T5.1, T5.3 · `src/proxy/wire.rs`, `src/proxy/anthropic.rs`
Do: `trait Wire { fn matches(path) -> bool; fn tool_results(req: &mut Value) -> Vec<ToolResultRef>; fn usage_from_body(body) -> Option<Usage>; fn usage_from_sse(event) -> Option<Usage> }` where `ToolResultRef { id, content: &mut String/Value, turn }` and `Usage { input, cache_create, cache_read, output }`. Move every `/v1/messages`-specific line from T5.1/T5.3 into `anthropic.rs`; `archive` and `proxy` call only the trait.
Check: all P5 tests pass unchanged; `grep -r '"tool_result"' src/plugins/archive.rs` finds nothing (format knowledge lives in the wire).

**T11.2 OpenAI Chat Completions wire** · haiku · T11.1 · `src/proxy/openai_chat.rs`, `tests/fixtures/proxy/openai_chat_*.json`
Do: route `POST /v1/chat/completions` to `RTOK_OPENAI_UPSTREAM` (default `https://api.openai.com`). Tool results = messages with `role: "tool"` keyed by `tool_call_id`. Usage from `usage.prompt_tokens`, `usage.completion_tokens`, `usage.prompt_tokens_details.cached_tokens` (→ `cache_read`; `cache_create = 0`). Streaming: SSE `data:` lines ending with `data: [DONE]`; when the request streams and lacks `stream_options.include_usage`, add it so the final chunk carries `usage` (this is the one byte-level change passthrough mode makes; documented). Non-streaming: usage from the body.
Check: mock upstream fixture (streaming and non-streaming) → response bytes identical; `usage` row inserted with `api = openai_chat` and `cache_read` populated from `cached_tokens`.

**T11.3 OpenAI Responses wire** · haiku · T11.1 · `src/proxy/openai_responses.rs`, `tests/fixtures/proxy/openai_responses_*.json`
Do: route `POST /v1/responses`. Tool results = `input[]` items of type `function_call_output` keyed by `call_id`. Usage from `usage.input_tokens`, `usage.output_tokens`, `usage.input_tokens_details.cached_tokens`; streaming: final `response.completed` event. Respect `previous_response_id` (nothing to rewrite in the request when history is server-side — record usage only).
Check: fixtures → identical bytes; `usage` row with `api = openai_responses`; a request with `previous_response_id` produces zero rewrites in compress mode.

**T11.4 `archive` across wires** · haiku · T11.1–T11.3, T5.3 · `src/plugins/archive/mod.rs`, `tests/fixtures/proxy/*_6turns.json`
Do: T5.3's rules (older than `keep_turns`, larger than `min_tokens`, keyed by the wire's tool-result id, persisted, byte-stable) applied through `Wire::tool_results` for all three formats. `expand` marks ids across formats.
Check: a 6-turn fixture per format → only turns 1–2 large results rewritten; same request twice → byte-identical bodies; prefix before the first rewrite unchanged (same test as T5.3, parameterised over wires).

**T11.5 setup for OpenAI hosts** · haiku · T5.2, T11.2 · `src/setup/codex.rs`, `src/setup/opencode.rs`, `src/doctor.rs`
Do: `rtok setup codex --proxy` writes a `model_provider` with `base_url = http://127.0.0.1:8790/v1` in `~/.codex/config.toml` (backup, dry-run, idempotent, `--remove`); `rtok setup opencode --proxy` sets `OPENAI_BASE_URL` in its config; `rtok doctor` shows the `OPENAI_BASE_URL` chain next to the Anthropic one. Print how to revert.
Check: each dry-run shows exactly one change; second run "no changes"; `rtok doctor` lists both chains.

**T11.6 `usage.api` + per-API stats** · haiku · T11.2 · `migrations/0002.sql`, `src/measure/stats.rs`
Do: migration adds `usage.api TEXT NOT NULL DEFAULT 'anthropic'` (`anthropic | openai_chat | openai_responses`); `rtok stats` prints usage totals and cache hit rate per API; `rtok stats --cache` (T5.5) handles OpenAI cached_tokens (no cache_create signal → busts detected from cache_read drops only).
Check: `cargo test store::` still applies both migrations idempotently; fixture usage rows for two APIs → two rows in the stats table.

Gate P11: run one OpenAI-API host (Codex) through the proxy in passthrough for two days; every request has a `usage` row. Then compress for two days; keep only under the P5 gate rule (context-token-turns − 15 %, expand rate < 5 %). Record in research.md §2.

## 4. Definition of done for v0.1

1. `rtok doctor` shows ≤ 8 token-related hooks, one MCP server for reads/memory/graph, one proxy hop (serving both Anthropic and OpenAI wire formats, D11).
2. `rtok stats --compare before-rtok` over ≥ 5 working days shows lower context-token-turns per session and lower output tokens per passed bench task, with expand rate < 5 %.
3. Every plugin has a `Measurement` path and appears in `rtok stats --plugin <id>`.
4. Hook p95 < 10 ms; proxy adds < 20 ms per request (measured in T5.1 test).
5. README documents what is lossless, what is estimated, and how to revert (`rtok setup claude --remove`, backups).

## 5. Order of value (if time is short)

P1 (measure) → P2 (hooks) → P5 (proxy passthrough for ground truth) → P3 (cmd) → P4 (read) → P5 compress → P9 (bench + retire). P6–P8, P10 and P11 only after P9 shows the core pays for itself; P11 first among those if an OpenAI-API host is in daily use.

## 6. Plan amendments (recorded while implementing; each is small and evidence-free by nature)

| Date | Change | Why |
|------|--------|-----|
| 2026-09-01 | Estimator rates are `[estimator] code/prose/json/cjk` in config, not `core.estimator_chars_per_token`. | T0.5 needs four classes; one key per class is what `--calibrate` (T1.5) will rewrite. |
| 2026-09-01 | Crate is a library plus a thin `src/main.rs`. | Tests, `examples/hello_plugin.rs` and every surface share one API; no code duplication in the bin. |
| 2026-09-01 | Every plugin directory carries `README.md` (users) and `AGENTS.md` (invariants, owned files, Checks). | Per-plugin instructions keep the root `AGENTS.md` under 350 tokens while giving Haiku the constraints it needs per task. |
| 2026-09-01 | Finished tasks move to `done.md`; `plan.md` keeps only open work. | The plan stays short enough to load every session. |
| 2026-09-01 | `guard` and `toon` are in the catalogue and registry but have no numbered task yet. | Noted in their READMEs; add a task with a Check before implementing either. |
| 2026-09-01 | `README.md` exists now as a status page; T9.5 still replaces it with measured results. | Newcomers need install + layout before P9. |
| 2026-09-01 | OpenAI API support added: decision D11, phase P11 (T11.1–T11.6), non-goal on the Codex Responses proxy withdrawn. | User request; OpenAI-API hosts (Codex, OpenCode, aider) were otherwise unmeasurable and uncompressible. |
