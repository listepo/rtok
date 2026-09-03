# read — design note (D15)

## Problem

lean-ctx injects ~3.1 K tokens/turn for 78 MCP tools (`research.md` §5) and token-optimizer adds structure maps plus a read cache. First look at an unknown repo is a grep-and-read chain.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| lean-ctx read modes | 2026-09-01 | 2026-09-01 | 10 modes; deny Grep/Glob | 78 tools' descriptions; banner every turn |
| token-optimizer structure map | 2026-09-01 | 2026-09-01 | delta reads; signatures | Python hooks; no ranked repo map |
| headroom audit | 2026-09-01 | 2026-09-01 | request-level crush | not a file-read strategy |
| aider repo map (tree-sitter + PageRank) | 0.82 | 2026-09-02 | ranked first look; one artifact | build cost; Python; not MCP-budgeted |

## Mechanism

MCP tools: `full` / `lines` / `map` / `signatures`. v0.1 ranking is cheap (path + size + recency), not PageRank. Re-read of the same path+mtime returns a stub + `expand <id>`. Dedup is the contract, not a cache of stale bytes.

The property that beats the table: five tools, no banner, re-read stub — description tokens stay inside the inject budget.

## Rejected

- Porting aider PageRank for v0.1 — build cost fails the hook ≤ 10 ms / MCP-on-demand rule; v0.2 if P4 still loses.
- 78-tool surface — description tokens dominate the save.

Target: Disable lean-ctx for one day; compare Read/MCP rows and injection tokens vs baseline.

Falsified by: `map`+`signatures` first-look uses more tokens than native Read on the fixture repo, or a re-read serves stale bytes.
