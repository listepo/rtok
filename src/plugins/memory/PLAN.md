# memory — design note (D15)

## Problem

claude-mem spends tokens extracting observations; engram spends description tokens on 18 tools. Neither is measured against a recall fixture in this repo.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| claude-mem | 2026-09-01 | 2026-09-01 | progressive disclosure; SQLite+Chroma | LLM extraction; not local-LLM-free |
| engram | 2026-09-01 | 2026-09-01 | agent-written notes; FTS5 | 18 MCP tools' descriptions |
| OpenViking L0/L1/L2 | 2026-09-01 | 2026-09-01 | tiered load | AGPL; LLM compression; v0.2+ |
| mem0 / OpenMemory | 2026-09-01 | 2026-09-01 | retrieval eval culture | Docker/Qdrant; not local-first |

## Mechanism

Zero-LLM: the agent writes notes (MCP `memory_write`); FTS5 retrieves titles then bodies on demand. Store: user-approved facts, decisions, bugs. Never store secrets, raw tool dumps, or session transcripts. Recall stays inside the `inject` 800-token budget (titles first). Metric: hit rate on a fixture note set (title+body planted, query must return the id).

The property that beats the table: no extraction tax, and recall is a Check, not a banner percentage.

## Rejected

- LLM extractors in v0.1 (claude-mem, mem0 default) — they spend the tokens we are trying to save.
- Injecting bodies at SessionStart — technique #7 says titles → ids → bodies.

Target: fixture-note recall (id returned for a planted query) and injection tokens below engram+claude-mem (P6 gate: compare injection and MCP description tokens; revert if recall is worse).

Falsified by: fixture recall below the LLM-extractor baseline on the same notes, or memory injection exceeds the 800-token budget.
