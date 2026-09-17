# Cursor

`rtok agents install cursor` — the desktop app (`cursor`) and the CLI (`cursor-agent`). Both
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

Hooks carry the `hook` and `cli` surfaces, MCP carries `mcp`; the linked plugin serves both.
Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Plugins (manifest `.cursor-plugin/plugin.json`, local install `~/.cursor/plugins/local/<name>`): https://cursor.com/docs/plugins
- Manifest reference (`hooks`, `mcpServers` fields): https://cursor.com/docs/reference/plugins
- Hooks (`~/.cursor/hooks.json`, `"version": 1`, `beforeShellExecution`, `afterShellExecution`): https://cursor.com/docs/agent/hooks
- MCP (`~/.cursor/mcp.json`, `mcpServers.<name>.command` / `args`): https://cursor.com/docs/context/mcp
- The linked bundle: `plugins/cursor/README.md`
