---
name: rtok
description: Recover output rtok shortened (rtok expand <id>), read/search/graph via rtok tools, install rtok with ketch.
---

# rtok

Invoke when the task needs rtok's token-saving surfaces — not for general coding advice.

## expand

Anything rtok shortened ends with an `expand <id>` trailer. Recover it: `rtok expand <id>`.
Flags: `--lines`, `--grep`, `--context`. See `docs/config.md` (`[expand]`).

## bash output

Where a hook or extension owns the bash path (pi, hook hosts), commands run as
`rtok run -- <command>` and results pass through `rtok filter`: raw output is archived, a
filtered version is what you see. Never add a second bash rewrite (D21).

## tools

Where rtok MCP is wired (on pi: registered only when `[setup.pi] tools = true`):

- `read` with `mode`: `full`, `lines`, `map`, `signatures`, and more (`[plugins.read]`).
- `search` greps the repo; `tree` maps a directory.
- memory: `mem_search`, `mem_save`, … (`[plugins.memory]`).
- graph: `symbol`, `callers`, `impact`, `outline`, `explore` (`docs/lsp.md`, `[plugins.graph]`).

Config keys live in `docs/config.md`. Without these tools, do not add `read`/`search` tools
of your own: one call path per host (D21).

## missing rtok

Fail open and install with ketch: `ketch install listepo/rtok`. No ketch yet: run
`curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash` first.
