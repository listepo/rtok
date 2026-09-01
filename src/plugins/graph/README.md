# `graph`

Three capped tools over whichever code-graph server is already installed, instead of four
servers with 130+ tool descriptions in every request.

| | |
|---|---|
| Kind | adapter |
| Surfaces | MCP `symbol(name)`, `callers(name)`, `outline(path)` |
| Replaces | codebase-memory-mcp, code-review-graph, serena, codegraph (as MCP surfaces) |
| Default | on (self-disables when no backend is installed) |

## Mechanism

Detect an installed backend (`codebase-memory-mcp` first, then serena), spawn its MCP stdio
process, call the matching tool, and cap the output at 2 K tokens (head + "N more, use expand").
No graph is built by rtok itself (decision D6: adapter until measured otherwise).

## Config

```toml
[plugins.graph]
enabled = true
backend = "auto"     # auto | codebase-memory-mcp | serena
max_tokens = 2000
```

## Tasks

T8.1 detect + wrap.

## Status

Manifest only. First task: T8.1 (after T4.1 `rtok mcp`).
