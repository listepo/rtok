# rtok Antigravity plugin

One plugin directory for all three Antigravity surfaces — Antigravity CLI (`agy`), Antigravity 2.0
and Antigravity IDE read global plugins from `~/.gemini/config/plugins/` (D21: plugin and MCP as
one unit, one `rtok mcp` per store). By hand: `agy plugin install <path to this folder>`, or place
this folder at `~/.gemini/config/plugins/rtok` and restart the desktop app.
`rtok agents install antigravity --yes` links it for the desktop apps and prints the
`agy plugin install` line for the CLI (T91.1).

`rtok` must be on `PATH`. If it is missing, install it with ketch: `ketch install listepo/rtok`.

Files:

- `plugin.json` — Antigravity manifest (`name` is the only required field).
- `mcp_config.json` — `mcpServers.rtok` → `rtok mcp`.

There is no `hooks.json`. Antigravity's `PreToolUse` answers only a `decision` (`allow`, `deny`,
`ask`, …) with a `reason` and cannot rewrite tool input, and `PostToolUse` answers `{}` and cannot
add context, so rtok's `rtok run` rewrite is not expressible as a hook here. The shell path is the
rtok skill plus the MCP tools.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `plugin.json`, directory layout, `~/.gemini/config/plugins/`, `agy plugin install`): https://antigravity.google/docs/plugins/
- MCP (`mcpServers.<name>.command` / `args` / `env`, `~/.gemini/config/mcp_config.json`, one format for CLI, 2.0 and IDE): https://antigravity.google/docs/mcp/
- Hooks (events, stdin payload, `decision` contract — why this plugin ships none): https://antigravity.google/docs/hooks/
- Skills (`SKILL.md` format and roots): https://antigravity.google/docs/skills
- CLI install (`agy` binary): https://antigravity.google/docs/cli/install/

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

