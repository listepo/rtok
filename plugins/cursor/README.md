# rtok Cursor plugin

Linked by `rtok agents install cursor` (by default, once Cursor itself is detected) to
`~/.cursor/plugins/local/rtok` (D21: hooks and
MCP as one unit, one `rtok mcp` per store). `rtok agents install cursor --remove` unlinks it.
By hand: symlink this folder to `~/.cursor/plugins/local/rtok` and restart Cursor.

Files:

- `.cursor-plugin/plugin.json` — Cursor manifest: `hooks` → `hooks/hooks.json`, `mcpServers` → `mcp.json`.
- `hooks/hooks.json` — `beforeShellExecution` → `rtok hook PreToolUse --host cursor`,
  `afterShellExecution` → `rtok hook PostToolUse --host cursor`, `afterMCPExecution` →
  `rtok hook AfterMCPExecution --host cursor`, `postToolUse` (matcher `MCP:`) →
  `rtok hook PostToolUse --host cursor` (replaces long MCP results via `updated_mcp_tool_output`).
- `mcp.json` — `mcpServers.rtok` → `rtok mcp` directly (T85/I-37: launcher
  scripts never run; Cursor has a single `command`/`args` pair with no per-OS
  slot to wire them into, so there is no `scripts/` tree).
- `plugin.json` — Agent Plugins manifest (root `plugin.json`) for other hosts of that spec.

`rtok` must be on `PATH`. If it is missing, the MCP server does not start;
install it with ketch: `ketch install listepo/rtok`.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `.cursor-plugin/plugin.json`, local install `~/.cursor/plugins/local/<name>`): https://cursor.com/docs/plugins
- Manifest reference (`hooks` and `mcpServers` accept file paths; defaults `hooks/hooks.json`, `mcp.json`): https://cursor.com/docs/reference/plugins
- Hooks (`"version": 1`, `beforeShellExecution`, `afterShellExecution`, `afterMCPExecution`, `postToolUse` / `updated_mcp_tool_output`): https://cursor.com/docs/agent/hooks
- MCP (`mcpServers.<name>.command` / `args`, `~/.cursor/mcp.json`): https://cursor.com/docs/context/mcp
- Agent Plugins spec (root `plugin.json`, `$schema`, `mcp.json`): https://agent-plugins.org/specification

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

