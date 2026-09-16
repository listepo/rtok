# Cursor

`rtok agents setup cursor` — the desktop app (`cursor`) and the CLI (`cursor-agent`). Both
read the same `~/.cursor` tree, so one install covers both; `--cli` / `--desktop` only pick
which app the report shows.

Files: `~/.cursor/hooks.json` (hooks) and `~/.cursor/mcp.json` (MCP without the plugin).
Plugin link: `~/.cursor/plugins/local/rtok` → `plugins/cursor/` from the rtok install (D21).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `beforeShellExecution` → PreToolUse, `afterShellExecution` → PostToolUse, both `--host cursor` |
| mcp | yes | `mcpServers.rtok` in `mcp.json`, or served by the linked plugin (then `mcp.json` is left alone: one MCP per store) |
| plugin | `--yes` | the offer links `plugins/cursor` (hooks + MCP as one unit); without a terminal only `--yes` accepts |
| proxy | no | Cursor has no base-URL setting to point at the proxy |

## rtok plugins this host reaches

- Through hooks or the plugin: cmd, read, inject, guard, memory (hook half).
- Through MCP or the plugin: read, archive, memory, graph, toon.
- Not reachable: measure, proxy, compress — proxy-only surfaces.
