# Cline

`rtok agents install cline` — the CLI (`cline`) and the VS Code extension
(`saoudrizwan.claude-dev`). Both scan `~/Documents/Cline/Hooks`, so one directory
serves both surfaces (D21 singleton): per event the installer links
`plugins/cline/hooks/rtok-hook` as `<hooks_path>/<Event>` (a foreign file at an
event slot is left alone and named in the output — Cline has one slot per event
per directory). MCP is per surface: `mcpServers.rtok` → `rtok mcp` in
`[setup.cline] mcp_path` (default `~/.cline/data/settings/cline_mcp_settings.json`,
the CLI file) and in the extension file
`<VS Code User>/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`
(off with `[setup] mcp = false`).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | one `plugins/cline/hooks/rtok-hook` link per event (`PreToolUse`, `PostToolUse`, `TaskStart`, `UserPromptSubmit`, `SessionEnd`), event from the link name |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in both `cline_mcp_settings.json` files, `{command, args}` with no `type` (off with `[setup] mcp = false`) |
| plugin | `--yes` | the hook links ARE the plugin unit: no second link step, `installed()` reports `plugin` exactly when `hooks` is installed |
| proxy | no | the Anthropic base URL is extension state, not a file rtok may edit |

## rtok plugins this host reaches

Hooks carry the `hook` and `cli` surfaces, MCP carries `mcp`; the hook links are the
plugin unit, so nothing extra is linked. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Hooks (file hooks: one executable per event, `overrideInput` / `context` / `cancel`): https://docs.cline.bot/customization/hooks
- Plugins (`~/.cline/plugins`, `cline plugin install`; SDK/CLI/Kanban only, not the VS Code extension): https://docs.cline.bot/customization/plugins
- MCP (`cline_mcp_settings.json`, adding/configuring MCP servers): https://docs.cline.bot/mcp
- CLI reference (`cline` commands, flags, configuration options): https://docs.cline.bot/cli
- Config (global vs project config, where Cline stores settings): https://docs.cline.bot/customization/settings
- The linked script: `plugins/cline/README.md`
