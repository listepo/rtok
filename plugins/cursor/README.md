# rtok Cursor plugin

Linked by `rtok agents install cursor` (by default, once Cursor itself is detected) to
`~/.cursor/plugins/local/rtok` (D21: hooks). `rtok agents install cursor --remove` unlinks
it. By hand: symlink this folder to `~/.cursor/plugins/local/rtok` and restart Cursor. The
plugin no longer ships an MCP server (T275/D33): `rtok agents install cursor` always writes
`mcpServers.rtok` into `~/.cursor/mcp.json` itself, plugin linked or not, so there is exactly
one path to that entry instead of two copies to keep in sync.

Files:

- `.cursor-plugin/plugin.json` — Cursor manifest: `hooks` → `hooks/hooks.json`.
- `hooks/hooks.json` — `beforeShellExecution` → `rtok hook PreToolUse --host cursor`,
  `afterShellExecution` → `rtok hook PostToolUse --host cursor`, `afterMCPExecution` →
  `rtok hook AfterMCPExecution --host cursor`, `postToolUse` (matcher `MCP:`) →
  `rtok hook PostToolUse --host cursor` (replaces long MCP results via `updated_mcp_tool_output`).
- `plugin.json` — Agent Plugins manifest (root `plugin.json`) for other hosts of that spec.

Each hook line finds `rtok` on `PATH`, then at `~/.ketch/bin/rtok` (a Cursor started from
the Dock has no shell `PATH`), and otherwise exits 0 without output; only `sessionStart`
then prints one `additional_context` note: `ketch install listepo/rtok`. The line is a
`{ …; }` group because Cursor appends the payload as a heredoc to the command. On Windows
the plugin is a copy and its hooks keep the bare `rtok hook … --host cursor` line
(PowerShell runs them).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `.cursor-plugin/plugin.json`, local install `~/.cursor/plugins/local/<name>`): https://cursor.com/docs/plugins
- Manifest reference (`hooks` accepts a file path; default `hooks/hooks.json`): https://cursor.com/docs/reference/plugins
- Hooks (`"version": 1`, `beforeShellExecution`, `afterShellExecution`, `afterMCPExecution`, `postToolUse` / `updated_mcp_tool_output`): https://cursor.com/docs/agent/hooks
- MCP (`mcpServers.<name>.command` / `args`, `~/.cursor/mcp.json`, written by `rtok agents install cursor` itself): https://cursor.com/docs/context/mcp
- Agent Plugins spec (root `plugin.json`, `$schema`): https://agent-plugins.org/specification

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

