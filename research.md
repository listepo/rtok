# Token-reduction tools for AI coding agents — research, comparison, evidence

Date: 2026-09-01. Method: 8 Haiku research agents (compression tools, code graphs, memory, host surfaces, architecture, techniques, token-optimizer plugin, session-log measurement) + 2 Haiku adversarial fact-check agents (GitHub API metadata for 19 repos; 27 documentation/blog claims). Local ground truth from `~/.claude/settings.json`, `rtk gain`, `headroom savings`, `lean-ctx gain`, `caveman status`, claude-mem banner, and 17 session transcripts (Jul 29 – Sep 1). Raw reports live in the session scratchpad `token-research/`.

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

Cost split with p_out = 5 × p_in (input-token equivalents):

| Component | Standard cache read 0.1× | Fable/Mythos 5.1 read 0.025× |
|-----------|--------------------------|------------------------------|
| Cache reads | 137 M (64 %) | 34 M (31 %) |
| Cache writes (1.25×) | 33 M (16 %) | 33 M (30 %) |
| Output (×5) | 43 M (20 %) | 43 M (39 %) |

Reading: on standard models, context volume dominates → compress tool results and clear old ones. On Fable/Mythos, output tokens dominate → fewer lines written (ponytail-style), fewer turns, terse prose.

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
| codebase-memory-mcp (DeusData) | code graph | tree-sitter 162 langs → SQLite (LZ4), Cypher-like queries | MCP | C · MIT · 41.7 k | ✓ · ✓ · n/a | 99.2 % on 5 queries; Linux kernel 3 min | none independent |
| codegraph (colbymchenry) | code graph | `.codegraph/codegraph.db` (SQLite+FTS5), `explore` | CLI + MCP | C/TS · MIT · 69 k | ✓ · ✓ · n/a | −88 % tool calls, −62 % tokens | none; you marked it legacy |
| code-review-graph (tirth8205) | code graph | tree-sitter → SQLite, impact radius, minimal context | MCP (30 tools) | Python · MIT · 31.1 k | ✓ · optional embeddings · n/a | — | none |
| serena (oraios) | symbols | LSP-backed symbol tools | MCP | Python · MIT · 28.7 k | ✓ · ✓ · n/a | — | none; most precise |
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
| Code graph | code-review-graph + codebase-memory-mcp + serena (+codegraph stale) ≈ 85 MCP tools | `graph` adapter over one installed tool |
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
9. LLM-based compression — negative until proven; skip.

## 7. Fact-check ledger

Checked 27 claims + 19 repos. Refuted: JetBrains rtk post (rtk did not save; +7.6 % on low effort), TOON numbers (42.6 %, 72.2 vs 71.4), "headroom is Rust" (82 % Python), "OpenViking is Rust" (78 % Python), "claude-mem is TypeScript" (JS per API), "37 hook events" (32). Partial/unverifiable: `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (undocumented; settings key exists), cache invalidation order and 20-block lookback (not in docs), `ENABLE_TOOL_SEARCH` value range and 10 % trigger (undocumented), offline Claude tokenizer (docs silent; `count_tokens` is the only official path). Confirmed: PostToolUse cannot modify results; PreToolUse `updatedInput`; caching multipliers incl. 0.025× Fable/Mythos; context-editing names; memory tool name; issue #81967; caveman #112; ponytail bench figures; codebase-memory-mcp and codegraph README figures; lean-ctx README figures. GitHub reports NOASSERTION for caveman and token-optimizer licenses; token-optimizer's local LICENSE file is PolyForm Noncommercial 1.0.0.

## 8. Open questions

- Does `ANTHROPIC_BASE_URL` really disable MCP tool search on your setup (deferred tools are visible in this session, so something enables it)? `rtok doctor` T1.4 answers it.
- Does headroom's live-zone compression keep the prefix byte-stable across turns in your traffic? T5.5 cache-health report answers it before rtok compress replaces it.
- Pricing for Fable 5.1 output tokens: the cost split above assumes p_out = 5 × p_in; adjust in `rtok stats --price` once known.
