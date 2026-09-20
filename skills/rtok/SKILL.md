---
name: rtok
description: Use rtok to shrink tool output losslessly (expand <id>), read/search the repo, and query the code graph via MCP.
---

# rtok

Invoke when the task needs rtok's token-saving surfaces — not for general coding advice.

## expand

Recover anything rtok shortened: `rtok expand <id>`. Flags: `--lines`, `--grep`, `--context`. See `docs/config.md` (`[expand]`).

## read

MCP `read` with `mode`: `full`, `lines`, `map`, `signatures`, and more. See `docs/config.md` (`[plugins.read]`).

## search / tree

MCP `search` greps the repo; `tree` maps a directory. See `docs/config.md` (MCP tools table).

## memory

MCP memory tools (`mem_search`, `mem_save`, …) when rtok MCP is wired. See `docs/config.md` (`[plugins.memory]`).

## graph

MCP `symbol`, `callers`, `impact`, `outline`, `explore`. See `docs/lsp.md` and `docs/config.md` (`[plugins.graph]`).

## missing rtok

`ketch install listepo/rtok`
