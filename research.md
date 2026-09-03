# Token-reduction tools for AI coding agents — research, comparison, evidence

Date: 2026-09-01. Method: 8 Haiku research agents (compression tools, code graphs, memory, host surfaces, architecture, techniques, token-optimizer plugin, session-log measurement) + 2 Haiku adversarial fact-check agents (GitHub API metadata for 19 repos; 27 documentation/blog claims). Local ground truth from `~/.claude/settings.json`, `rtk gain`, `headroom savings`, `lean-ctx gain`, `caveman status`, claude-mem banner, and 17 session transcripts (Jul 29 – Sep 1). Raw reports live in the session scratchpad `token-research/`. Features those tools have that rtok has not scheduled live in `ideas.md`.

## 1. Verdict

1. **Your stack stacks ten tools, 81 hooks and two chained proxies, and nobody measures end to end.** Vendor claims are 60–99 %; your own meters show 3–40 % on the slice each tool touches; the only independent measurement (JetBrains) found rtk cost +7.6 % to 0 % and caveman 8.5 % on agentic work.
2. **The cost lever is context × turns, not single outputs.** Cache hit rate is 98.1 %; every token that enters context is re-read (at 0.1× price, 0.025× on Fable/Mythos 5.1) on every later turn. A 17 K-token Read on turn 10 of 60 costs ~1.25 × 17 K to write and 0.1 × 17 K × 50 to keep. Shrinking early and clearing old results beats shrinking a little everywhere.
3. **Build one Rust binary with plugins, measurement first, and retire what does not pay.** Hooks cannot modify tool results after the fact, so the binary needs three surfaces: hook (PreToolUse rewrite), MCP (tool replacement), proxy (cache-safe live-zone rewriting + ground-truth usage). Plan: `plan.md` (project `~/GitHub/rtok`).

## 2. Where your tokens go (17 sessions, 43,609 transcript lines)

Estimator: 4 chars/token (heuristic). Usage counters are real API numbers.

| Item | Value |
|------|-------|
| Tool results, est. tokens | 2.83 M total; Bash 1.00 M (35 %), Read 0.41 M (15 %), Agent 23 K, MCP engram 8.8 K, MCP lean-ctx 6.2 K |
| Largest single results | Reads of 38–68 K chars (9.5–17 K tokens each, top 8 all `Read`) |
| Bash by family | `cd …&&` chains 435 K (hides the real command), sed 217 K, grep 131 K, cat 17 K, ls 13 K, pnpm 35 K, python3 34 K |
| Assistant output | 8.6 M output tokens; text is 4 % of assistant content, 96 % is tool input (the code it writes) |
| Cache | read 1,367 M, creation 26.7 M, uncached input 42 K → 98.1 % hit rate |
| Median final context | 167 K tokens per session |
| rtk-wrapped commands visible in transcripts | 3 of 3,658 (the PreToolUse rewrite happens after the transcript records the call, so this under-counts) |

### `graph` index accuracy (T8.8, 2026-09-04)

30 symbols of this repo labelled by hand from a plain-text scan, independent of the index that is
being scored: definition files complete per symbol, reference files a must-appear subset, no line
numbers. `tests/graph_truth.rs` re-measures on every run.

| Metric | Value |
|--------|-------|
| Definitions found | 30 / 30, recall 1.000, precision 1.000 |
| References found | 40 / 114, recall 0.351 |
| All sites | 70 / 144, recall 0.486 |
| Cause of every miss | type positions 64, macro bodies 9, path-qualified calls 1 |

The reference number is a property of the tree-sitter Rust tags query, not of rtok's storage: it
captures plain calls, field-expression method calls, macro invocations and `impl` items, nothing
else. `src/plugins/graph/PLAN.md` lists the constructs under "Known misses".

### `graph` v0.2 surface and latency (Gate P8b, 2026-09-04)

Release build. The 3 000-file repo is generated, each file one function calling two others.

| Measurement | Value | P8b bar |
|-------------|-------|---------|
| Tools, description tokens | 4, 62 | ≤ 150 |
| Cold index, 3 000 files / 9 000 rows | 22.1 s | not gated |
| Warm `symbol` / `callers` / `impact` | 23 / 24 / 26 ms | < 100 ms |
| Definition recall, precision | 1.000, 1.000 | ≥ 0.9 |
| Reference recall | 0.351 | published, not gated |

The fourth clause — fewer tool calls per multi-file task on the P9 set — is not measured, so
the gate is open. `callers("estimate")` on this repo fell from 1 959 bytes at v0.1 to 793.

Cost split with p_out = 5 × p_in (input-token equivalents):

| Component | Standard cache read 0.1× | Fable/Mythos 5.1 read 0.025× |
|-----------|--------------------------|------------------------------|
| Cache reads | 137 M (64 %) | 34 M (31 %) |
| Cache writes (1.25×) | 33 M (16 %) | 33 M (30 %) |
| Output (×5) | 43 M (20 %) | 43 M (39 %) |

Reading: on standard models, context volume dominates → compress tool results and clear old ones. On Fable/Mythos, output tokens dominate → fewer lines written (ponytail-style), fewer turns, terse prose.

### `rtok stats --save-baseline before-rtok` (2026-09-03)

Gate P1. Default `[stats] since = 30d` (not the 17-session slice above). File: `~/.rtok/measurements/before-rtok.json`. `--compare before-rtok` → all Δ0. Estimator: 4 chars/token. No rtok hooks in `settings.json` at save time. Proxy `usage` empty (`api` {}).

| Item | Value |
|------|-------|
| Sessions / lines | 580 / 181 303 (0 malformed) |
| Tool results, est. tokens | 13.03 M total; Bash 7.71 M (59 %), Read 3.07 M (24 %), WebSearch 0.70 M, MCP lean-ctx `ctx_read` 0.48 M |
| Bash by family | sed 1.71 M, cd 1.20 M, cat 0.93 M, grep 0.93 M, git 0.35 M |
| MCP groups | lean-ctx 0.86 M, engram 36 K |
| Cache | read 8 124 M, creation 235 M, uncached input 1.52 M → 97.2 % hit rate |
| Median final context | 65 882 tokens per session |
| Archive replay (estimate) | CTT 11.81 G → 8.42 G (−28.7 %); 1 803 candidates |

### A/B bench (T9.2, 2026-09-02)

Config A = `bench/configs/legacy.json` (81-hook + dual-proxy baseline described above). Config B = `bench/configs/rtok.json` (7 rtok hooks, `rtok mcp`, `ANTHROPIC_BASE_URL` :8790). Six tasks × 3 runs. Live `claude -p` is gated on `RTOK_BENCH_LIVE=1`; this commit ran without it, so usage/cost are zeros and pass rate is the tasks' `check = true`.

| config | mean input | mean cache | mean output | mean cost USD | pass |
|--------|------------|------------|-------------|---------------|------|
| a (legacy) | 0 | 0 | 0 | 0.0000 | 6/6 |
| b (rtok) | 0 | 0 | 0 | 0.0000 | 6/6 |
| **delta (b−a)** | 0 | 0 | 0 | 0.0000 | 0 |

Source: `bench/results/a.json`, `bench/results/b.json`. Re-run with `RTOK_BENCH_LIVE=1` to fill cost.

Your local meters (each measures a different slice, none the bill):

| Tool | Own meter | Note |
|------|-----------|------|
| rtk | 40.4 % of bash output over 5,725 cmds (1.1 M of 2.8 M) | `rtk read` only 6.4 %; grep 18.5 %; diff 90 % |
| headroom | 3.2 % today, 3.5 % 7 d, 11.3 % 30 d (14.35 M of 126.7 M) | proxy on :8788 chained to caveman :8787 |
| lean-ctx | "75 % ratio, 6.1 M difference" | +3.1 K tokens/turn fixed injection; 0 output tokens saved; "not a provider bill" (its words) |
| caveman | proxy in record mode; this session uncompressed | Pro/Max streaming sessions pass through |
| claude-mem | "87 % savings" | ratio of its own retrieval reads (21 K) vs. work it indexed (159 K) |
| token-optimizer | author's sessions: 14.44 M tokens / 30 d, "$313/mo measured" | 3.3 chars/token estimator; 27 Python hooks here |

## 3. Host surfaces (verified against code.claude.com/docs/en/hooks and env-vars, 2026-09-01)

- 32 hook events. PreToolUse may return `permissionDecision` (allow/deny/ask) and `updatedInput`; exit 2 blocks. **PostToolUse cannot modify or replace the tool result**; it only adds context. SessionStart/UserPromptSubmit inject via stdout or `additionalContext`. Command hooks support `async: true`. Timeouts are per event (600 s default, 30 s UserPromptSubmit, 10 s MessageDisplay).
- `ANTHROPIC_BASE_URL` routes traffic through a proxy; docs say it disables MCP tool search by default (check `rtok doctor`). `BASH_MAX_OUTPUT_LENGTH` default 30,000 chars, max 150,000. `autoCompactWindow` is set to 300000 in your settings; the env var name for it is undocumented.
- API side: prompt caching 1.25× (5-min write), 2× (1-h write), 0.1× read (0.025× Fable/Mythos 5.1). Context editing strategies `clear_tool_uses_20250919`, `clear_thinking_20251015`, `compact_20260112` (trigger 100 K, keep 3). `count_tokens` is free and rate-limited. Memory tool `memory_20250818`. Claude Code issue #81967: tools-array mutation invalidates the cache (up to −274 K tokens observed).
- Other hosts: Cursor `hooks.json` (before/after shell), OpenCode plugin API `tool.execute.after` (the one host that can replace results), Codex (MCP; proxy needs Responses API), Gemini CLI (MCP).

## 4. Comparison matrix

Stars/language/license from the GitHub API on 2026-09-01. "Claimed" is the vendor's number; "Measured" is yours or an independent source.

| Tool | Layer | Mechanism | Surface | Lang · License · Stars | Local · LLM-free · Lossless | Claimed | Measured |
|------|-------|-----------|---------|------------------------|-----------------------------|---------|----------|
| rtk (rtk-ai) | command output | ~80 filters, TOML custom filters, `gain` (bytes/4) | PreToolUse rewrite | Rust · Apache-2.0 · 78.2 k | ✓ · ✓ · ✗ (drops lines) | 60–90 % | 40 % of bash bytes (yours); JetBrains bill +7.6 %/0 % |
| lean-ctx (yvgude) | file reads, search, shell | 78 MCP tools, 10 read modes, 95+ shell patterns, dedup re-reads | MCP + hooks (deny Grep/Glob) | Rust · Apache-2.0 · 3.7 k | ✓ · ✓ · ~ (expand) | 75 % (own) | +3.1 K/turn injection; 0 output saved |
| headroom (headroomlabs-ai) | API request | JSON crusher, code compressor, cache-aligned live zone, CCR retrieve | proxy + MCP + wrap | Python 82 %/Rust 13 % · Apache-2.0 · 68.3 k | ✓ · ✓ · ✓ (retrieve) | up to 95 % | 11.3 % 30 d, 3.2 % today (yours) |
| caveman (JuliusBrussee) | prose + request | terse mode, shrink-hook, proxy record/compress, TOON, MCP compress/retrieve | prompt + proxy + MCP | Go · custom · 102 k | ✓ · ✓ · ~ | 65 % output | 8.5 % agentic (JetBrains); 0 here (proxy inert); issue #112 corrupts inline code |
| ponytail (DietrichGebert) | model output | YAGNI ladder prompt | prompt file | JS · MIT · 120 k | ✓ · ✓ · n/a | −54 % LOC, −22 % tokens (own bench, Haiku 4.5, n=4) | none independent |
| token-optimizer (alexgreensh) | reads, bash, archive, compaction, coaching | delta reads, structure maps, bash compress (111 cmds), archive >4 KB, checkpoints, quality nudges | hooks (Python subprocess) | Python · PolyForm-NC · 2.1 k | ✓ · ✓ · ✓ (archive) | "$313/mo" | author's own meter only; 27 hooks on your machine |
| codebase-memory-mcp (DeusData) | code graph | tree-sitter 158 grammars + hybrid LSP (11 langs) → SQLite (zstd), 15 tools, Cypher-like queries | MCP | C · MIT · 42.1 k (v0.7.0, 2026-09-04) | ✓ · ✓ · n/a | 99.2 % on 5 queries; Linux kernel 3 min | exits on start here (0 tools, `doctor` 2026-09-02); 281 MB binary, 141 MB cache |
| codegraph (colbymchenry) | code graph | `.codegraph/codegraph.db` (SQLite+FTS5), one `codegraph_explore` tool | CLI + MCP | C/TS · MIT · 69.4 k (2026-09-04) | ✓ · ✓ · n/a | −88 % tool calls, −62 % tokens (7 repos) | 97 MB `.codegraph/` for one repo (cross-code); no binary here |
| code-review-graph (tirth8205) | code graph | tree-sitter → SQLite, impact radius, minimal context, communities | MCP (30 tools) | Python · MIT · 31.2 k (2.3.7, 2026-09-04) | ✓ · optional embeddings · n/a | 65× median (36–376×, 6 repos) | 30 tools ~2 295 desc tokens (`doctor`); F1 0.69 against its own edges (own README) |
| graphify (safishamsi) | code graph | 37 tree-sitter grammars + LLM extraction for docs → JSON, Leiden communities, HTML map | CLI + MCP + plugin | Python · Apache-2.0/MIT · 114 k (v8, 2026-09-04) | ✓ · ✗ (docs layer) · n/a | “code maps for free” | not installed; none |
| serena (oraios) | symbols | LSP-backed symbol tools | MCP | Python · MIT · 28.7 k (2026-09-04) | ✓ · ✓ · n/a | — | 22 tools ~1 494 desc tokens; times out at 30 s here; most precise |
| claude-mem (thedotmack) | memory | LLM-extracted observations, SQLite+Chroma, progressive disclosure | plugin + MCP + daemon | JS · Apache-2.0 · 92.9 k | ✓ · ✗ (uses Claude) · n/a | 87 % (own banner) | costs tokens to build memory |
| engram (Gentleman-Programming) | memory | agent-written notes, SQLite FTS5, HTTP+MCP | MCP | Go · MIT · 6.3 k | ✓ · ✓ · n/a | — | 18 tools' descriptions per session |
| mem0 / OpenMemory | memory | LLM extraction + vectors | MCP (Docker, Qdrant) | Python · Apache-2.0 · 64.5 k | ~ · ✗ · n/a | — | not local-first by default |
| OpenViking (volcengine) | context DB | L0/L1/L2 tiered loading, session compression | SDK + MCP | Python 78 %/Rust 14 % · AGPL-3.0 · 34.9 k | ✓ · ✗ · n/a | 34–91 % | none |
| TOON (toon-format) | data format | tabular JSON → TOON | library | TS · MIT · 25.3 k | ✓ · ✓ · ✓ | −42.6 % tokens; accuracy 72.2 vs 71.4 | vendor bench |
| LLMLingua-2 (microsoft) | prompt compression | small-model token pruning | library | Python · MIT · 6.6 k | ✓ · needs model · ✗ | 2–20× | quality risk on code |
| bifrost (maximhq) | gateway | semantic cache (redis, 0.9 threshold) | proxy | Go · Apache-2.0 · 7.7 k | ✓ · embeddings · ✗ | — | agent contexts never repeat; a hit is a wrong answer |
| Anthropic native | platform | prompt caching, context editing, memory tool, deferred tools, auto-compact | API/Claude Code | — | ✓ · — · ✓ | — | free; align with it, do not fight it |
| **Your stack today** | all layers | 10 tools, 81 hooks, 2 proxies (+bifrost in Docker) | hooks + MCP + proxy | mixed | ✓ · mostly · mixed | — | 3–40 % per slice; no end-to-end number |
| **Proposed `rtok`** | all layers | 10 plugins, 1 binary, 3 surfaces, measurement + bench | hooks + MCP + proxy | Rust | ✓ · ✓ · ✓ by default | none until measured | `rtok stats`, `rtok bench` |

## 5. Your stack vs. the proposed app

| Aspect | Today | Proposed |
|--------|-------|----------|
| Hooks | 81 across 16 events (token-optimizer 27 Python, orca 12, caveman-proxy 11, holdmylid 10, lean-ctx 9, cbm 7, tokenbar 2, rtk 1, caveman shrink 1, codegraph 1) | ≤ 8 token-related (`rtok hook <event>`), non-token hooks untouched |
| Per-tool-call overhead | up to ~30 subprocesses per event chain, several Python | one Rust process < 10 ms |
| Bash filtering | rtk + lean-ctx ctx_shell + token-optimizer bash_compress | `cmd` (delegates to rtk filters, archives raw, measures) |
| Reads | lean-ctx (78 tools, 3.1 K/turn) + token-optimizer read_cache + headroom audit | `read` (5 tools, no banner, dedup) |
| Proxies | headroom :8788 → caveman :8787 (inert on Max) → Anthropic; Docker: headroom → bifrost | `rtok proxy :8790` (passthrough → compress), chainable for A/B |
| Memory | claude-mem (LLM) + engram | one, agent-written, FTS5 |
| Code graph | code-review-graph + codebase-memory-mcp + serena (+codegraph stale) ≈ 85 MCP tools | `graph`: native tags index, 3 tools (D6; was “adapter” before D6 was rewritten) |
| Injection per turn | lean-ctx 3.1 K + engram + claude-mem + ponytail + caveman + token-optimizer nudges | one budget (800 tokens), byte-stable |
| Measurement | 5 incompatible meters, none the bill | usage from proxy + transcripts; context-token-turns; A/B bench |
| Reversibility | partial (headroom retrieve, token-optimizer expand) | every rewrite has `expand <id>` |

## 6. Technique ranking (evidence-weighted, for this workload)

1. Clear or shrink old tool results in context (archive live zone; context editing) — compounds over turns.
2. Keep the cached prefix byte-stable (no tools-array or system-prompt churn; stable injections).
3. Read less: outline/signature modes on first read, dedup re-reads, deny giant native reads.
4. Fewer output tokens: YAGNI/terse modes (measure), fewer turns via better first reads.
5. Command output filtering — real but small in the bill (JetBrains); keep it lossless.
6. Tabular JSON → TOON where results are tables (−40 %).
7. Memory with progressive disclosure (titles → ids → bodies), never bodies at SessionStart.
8. Code graph queries instead of grep-and-read chains — plausible, unmeasured; adapter first.
9. LLM-based compression — negative until proven; **v0.2+**, not v0.1 (`ideas.md` I-21).



### Plugin design surveys (P14, 2026-09-02)

Every alternative in `src/plugins/*/PLAN.md` with the survey date. Stars/versions for retired tools are from §4 (GitHub API 2026-09-01) unless noted.

| Alternative | Version / date | Cited by |
|-------------|----------------|----------|
| rtk gain | rtk-ai · 2026-09-01 | measure, cmd, guard |
| headroom savings / live zone / proxy / audit | 2026-09-01 | measure, read, archive, proxy |
| token-optimizer dashboard / bash_compress / structure map / archive / refetch | 2026-09-01 | measure, cmd, read, archive, guard |
| lean-ctx ctx_shell / read modes / banner / deny Grep | 2026-09-01 | cmd, read, inject, guard |
| caveman shrink / proxy / TOON | 2026-09-01 | archive, proxy, toon |
| engram | 2026-09-01 | inject, memory |
| claude-mem | 2026-09-01 | inject, memory |
| ponytail | 2026-09-01 | inject |
| codebase-memory-mcp | v0.7.0 · 2026-09-04 | graph |
| codegraph | 2026-09-04 | graph |
| code-review-graph | 2.3.7 · 2026-09-04 | graph |
| graphify | v8 · 2026-09-04 | graph |
| serena | 2026-09-04 | graph |
| OpenViking | 2026-09-01 | memory |
| bifrost | 2026-09-01 | proxy |
| TOON (toon-format) | 2026-09-01 | toon |
| Langfuse generation usage | 3.x · 2026-09-02 | measure (outside retired stack) |
| cargo --message-format=json | rustc 1.90 · 2026-09-02 | cmd (outside) |
| aider repo map (tree-sitter + PageRank) | 0.82 · 2026-09-02 | read (outside) |
| Anthropic context editing / tool-result clearing | docs 2026-09-01 | archive (outside) |
| LiteLLM | 1.7x · 2026-09-02 | proxy (outside) |
| Lost in the Middle (Liu et al.) | arXiv 2307.03172 · 2023-07 | inject (outside) |
| LangGraph recursion limit | 0.2x · 2026-09-02 | guard (outside) |
| mem0 / OpenMemory | 2026-09-01 | memory (outside) |
| Universal Ctags | 6.x · 2026-09-02 | graph (outside) |
| minified JSON / CSV / JSONL | RFC 8259 / 4180 · 2026-09-02 | toon (outside) |

## 7. Fact-check ledger

Checked 27 claims + 19 repos. Refuted: JetBrains rtk post (rtk did not save; +7.6 % on low effort), TOON numbers (42.6 %, 72.2 vs 71.4), "headroom is Rust" (82 % Python), "OpenViking is Rust" (78 % Python), "claude-mem is TypeScript" (JS per API), "37 hook events" (32). Partial/unverifiable: `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (undocumented; settings key exists), cache invalidation order and 20-block lookback (not in docs), `ENABLE_TOOL_SEARCH` value range and 10 % trigger (undocumented), offline Claude tokenizer (docs silent; `count_tokens` is the only official path). Confirmed: PostToolUse cannot modify results; PreToolUse `updatedInput`; caching multipliers incl. 0.025× Fable/Mythos; context-editing names; memory tool name; issue #81967; caveman #112; ponytail bench figures; codebase-memory-mcp and codegraph README figures; lean-ctx README figures. GitHub reports NOASSERTION for caveman and token-optimizer licenses; token-optimizer's local LICENSE file is PolyForm Noncommercial 1.0.0.

## 8. Open questions

- Does `ANTHROPIC_BASE_URL` really disable MCP tool search on your setup (deferred tools are visible in this session, so something enables it)? `rtok doctor` T1.4 answers it.
- Does headroom's live-zone compression keep the prefix byte-stable across turns in your traffic? T5.5 cache-health report answers it before rtok compress replaces it.
- Pricing for Fable 5.1 output tokens: the cost split above assumes p_out = 5 × p_in; adjust in `rtok stats --price` once known.
