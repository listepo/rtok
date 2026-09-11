# compress — design note (D15)

Survey date: **2026-09-11**. Task **T28.0** only — no implementation. Implements **I-21 / P28** beside v0.1 lossless `archive` + `cmd` + `read`; coordinates with optional `memory` LLM extraction in T28.2.

## Problem

v0.1 already shrinks context losslessly (`archive` pointers, `cmd` formatters, `toon`) and archives every rewrite under D4. That path is measured at **−28.7 % context-token-turns** on archive replay (`research.md` §2: 11.81 G → 8.42 G, 1 803 candidates). LLM-based methods claim 2–20× prompt ratios or ~87 % “memory savings”, but they spend model tokens to build the shrink, drop bytes with no `expand`, or corrupt code-shaped text (`research.md` §6.9; caveman #112). **P28** asks whether an optional LLM lane can beat the **v0.1 lossless cost per passed bench task** without breaking D4 expand on non-regenerable sources.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| **LLMLingua-2** (Microsoft) | ACL 2024 Findings; `microsoft/LLMLingua` repo; XLM-RoBERTa-large classifier | 2024-03-19 (arxiv 2403.12968); ACL Aug 2024 | Task-agnostic **extractive** token keep/drop; 3–6× faster than LLMLingua-1; strong on QA/summarisation benchmarks | Needs a bundled small model (D6: no wrap); **not lossless** — dropped tokens are gone; quality risk on code, diffs, and identifiers (`research.md` §6.9); savings not attributable without a native `Measurement` row |
| **claude-mem extraction** (thedotmack) | Plugin **v13.12.4** hooks + worker; **v13.23.1** `claude-mem/sdk` (`cmem-sdk`) | 2026 (hooks/worker docs); **2026-09-01** (cmem-sdk changelog) | **Semantic** compression: PostToolUse → async LLM → structured observation (type, title, facts, files); progressive disclosure (search → timeline → get); SQLite+FTS5 (+ optional Chroma) | **Not lossless** for tool payloads — original tool output is not recoverable via `expand`; extraction **costs** Haiku/Sonnet tokens; banner “87 %” is own retrieval ratio, not bill (`research.md` §2); D6 forbids wrapping the daemon/worker |
| **Selective Context** (Li et al., Surrey — *outside* retired stack) | EMNLP 2023 main; `liyucheng09/Selective_Context` | 2023-10 (arxiv 2310.06201; EMNLP Oct 2023) | Phrase/sentence-level **self-information** pruning; ~50 % context cut with small BERTscore drop on summarisation/QA papers | Same lossy extractive core as LLMLingua-1; ignores token interdependence (motivation for LLMLingua); **no reversibility**; weak on chain-of-thought and code-like spans |
| **headroom live-zone code/JSON compressor** (retired stack) | headroomlabs-ai proxy, measured locally | 2026-09-01 (`research.md`) | Cache-aligned live zone; CCR retrieve for some payloads | Python proxy dependency (D6); partial reversibility, not `expand <id>`-uniform; 11.3 % / 30 d on your meter — far below paper claims; different surface (nested JSON in live zone, not old `tool_result` pointers) |

## Mechanism

**Archive-first semantic shrink (native, default off).**

1. **Lossless lane (unchanged, always on):** Every filtered or archived payload is written to `~/.rtok/archive/` and indexed in SQLite; context shows `expand <id>` (D4). `cmd` formatters, `archive` age/size rules, and `toon` stay deterministic. No LLM runs on this path.
2. **LLM lane (opt-in, `compress.enabled = false` until Gate P28):** Only after the lossless archive row exists, optionally replace **in-context** text for qualifying blobs (same eligibility as archive: e.g. older than 2 turns or > 4 KiB, never the cached prefix — T14.5) with a **structured summary** produced by a configured host LLM call (claude-mem-style fields: type, title, narrative, facts, files) or, in a later slice, a **native extractive ranker** trained/evaluated in-tree (LLMLingua-2 *idea*, not the Microsoft package). The summary is a **view**; `expand <id>` always returns the archived **original bytes**. For regenerable sources (re-runnable shell commands per D4), lossy display without expand is allowed only when explicitly tagged regenerable — same rule as v0.1 `cmd`.
3. **Measurement:** Each LLM shrink writes a `Measurement` row: `bytes_in`, `bytes_out`, `tokens_spent` on the compressor call, and `plugin = compress`. No banner ratios.
4. **Memory hook (T28.2):** Optional observation extractor shares the same archive back-pointer so `memory` search returns titles while `expand` still serves full tool output — progressive disclosure without silent loss.

This beats the table because it combines claude-mem’s **semantic** density with rtok’s **mandatory archive + expand** (headroom/LLMLingua/claude-mem do not), stays **native** (D6), and only ships if **`rtok bench`** proves the LLM lane lowers **cost per passed task** net of compressor spend — otherwise the flag stays off and lane A alone is the product.

## Rejected

- **Wrapping `microsoft/LLMLingua`, claude-mem worker, or headroom** — D6; circular measurement against tools P9 retires.
- **Default-on LLM compression** — violates P28 “default off until Gate holds”.
- **Pure LLMLingua-2 token pruning as the only mode** — non-regenerable originals would be unrecoverable (falsifier); code/diff quality risk (`research.md` §6.9).
- **claude-mem-style extraction without archive back-pointer** — fails D4 for Read/Grep/tool results.
- **OpenViking L0/L1/L2 session compression** — AGPL stack + LLM tiers; deferred (`memory/PLAN.md`).
- **In-context LLM rewrite of the cached prefix** (system/tools/early messages) — breaks prompt cache (D, T14.5).

Target: `rtok bench` cost per passed task against the v0.1 lossless path must not rise, and `expand` still recovers a non-regenerable original.

The number to beat is that same configuration on the **P9 task set** (6 tasks × 3 runs, proxy `usage` rows required — same honest metric as `measure/PLAN.md`). The LLM lane may ship only if cost per passed task **≤ that baseline** and pass rate does not fall. Diagnostic reference (not the gate): v0.1 lossless archive replay **−28.7 % CTT** (`research.md` §2); LLM mode must beat **net bill**, not CTT alone.

Falsified by: (1) **cost per passed task rises** vs the v0.1 lossless bench baseline after compressor tokens are included, or pass rate drops; (2) **`expand` cannot recover a non-regenerable original** (archived tool/read output, diff, or file slice) when the LLM lane rewrote its in-context form; (3) any claimed saving lacks a **`Measurement` row** (D1 — that saving does not exist).
