# rtok Cursor plugin

Linked by `rtok agents install cursor --yes` to `~/.cursor/plugins/local/rtok` (D21: hooks and
MCP as one unit, one `rtok mcp` per store). `rtok agents install cursor --remove` unlinks it.
By hand: symlink this folder to `~/.cursor/plugins/local/rtok` and restart Cursor.

Files:

- `.cursor-plugin/plugin.json` — Cursor manifest: `hooks` → `hooks/hooks.json`, `mcpServers` → `mcp.json`.
- `hooks/hooks.json` — `beforeShellExecution` → `rtok hook PreToolUse --host cursor`,
  `afterShellExecution` → `rtok hook PostToolUse --host cursor`.
- `mcp.json` — `mcpServers.rtok` → `rtok mcp`.
- `plugin.json` — Agent Plugins manifest (root `plugin.json`) for other hosts of that spec.
- `scripts/mcp.sh`, `scripts/mcp.cmd` — `rtok mcp` launchers that print the ketch install hint
  when `rtok` is missing (`ketch install listepo/rtok`).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `.cursor-plugin/plugin.json`, local install `~/.cursor/plugins/local/<name>`): https://cursor.com/docs/plugins
- Manifest reference (`hooks` and `mcpServers` accept file paths; defaults `hooks/hooks.json`, `mcp.json`): https://cursor.com/docs/reference/plugins
- Hooks (`"version": 1`, `beforeShellExecution`, `afterShellExecution`): https://cursor.com/docs/agent/hooks
- MCP (`mcpServers.<name>.command` / `args`, `~/.cursor/mcp.json`): https://cursor.com/docs/context/mcp
- Agent Plugins spec (root `plugin.json`, `$schema`, `mcp.json`): https://agent-plugins.org/specification
