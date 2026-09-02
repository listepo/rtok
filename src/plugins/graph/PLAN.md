# graph — design note (D15)

## Problem

Four graph servers (~85 MCP tools) fight for description tokens. tree-sitter-tags miss dynamic dispatch, macros, and generated code; that is acceptable at v0.1 if hit rate on a fixture symbol set is measured.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| codebase-memory-mcp | 2026-09-01 | 2026-09-01 | 162 langs; SQLite | 99.2 % on 5 queries unrepeated; tool sprawl |
| codegraph | 2026-09-01 | 2026-09-01 | SQLite+FTS5; explore | marked legacy here |
| code-review-graph | 2026-09-01 | 2026-09-01 | impact radius | 30 tools; optional embeddings |
| serena | 2026-09-01 | 2026-09-01 | LSP precision | Python; heavy; most precise |
| Universal Ctags | 6.x | 2026-09-02 | cheap tags; many langs | no call graph; regex-ish |

## Mechanism

tree-sitter tags index in-process. Three MCP tools only: `symbol`, `callers`, `outline`. Refresh on mtime of indexed files, not every request. Output capped (N lines) with `expand` for the rest. Dynamic dispatch / macros / generated code are known misses; LSP is v0.2 (`ideas.md` Later).

The property that beats the table: three tools, < 2 s index of this repo, measured hit rate — not 85 descriptions.

## Rejected

- Shipping serena as a subprocess (D6).
- Cypher-like query language in v0.1 — three named tools are enough.

Target: MCP description tokens below the four retired servers; index this repo in < 2 s; hit rate on a fixture symbol set (P8 gate).

Falsified by: index of this repo ≥ 2 s, or fixture symbols that tags should see (plain fn/impl) are misses.
