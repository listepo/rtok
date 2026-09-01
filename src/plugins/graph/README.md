# `graph`

Three capped tools over a symbol index rtok builds itself with tree-sitter-tags, instead of
four servers with 130+ tool descriptions in every request.

| | |
|---|---|
| Kind | native (own index; no external graph server) |
| Surfaces | MCP `symbol(name)`, `callers(name)`, `outline(path)` |
| Replaces | codebase-memory-mcp, code-review-graph, serena, codegraph (as MCP surfaces) |
| Default | on |

## Mechanism

Walk the repo (respecting `.gitignore`), run the tags queries of the `read` plugin's grammars
for definitions and reference sites, and store them in the `symbols` table keyed by file
sha256 (incremental: unchanged files are skipped; an edited file is re-indexed on the next
call). `symbol`, `callers` and `outline` query that table and cap output at 2 K tokens
(head + "N more, expand <id>"). It is a tags index, not a type-resolved call graph
(`plan.md` §0 non-goals).

## Config

```toml
[plugins.graph]
enabled = true
max_tokens = 2000
```

## Tasks

T8.1 symbol index · T8.2 MCP tools.

## Status

Manifest only. First task: T8.1 (after T4.3 outline grammars and T4.5 `search`).
