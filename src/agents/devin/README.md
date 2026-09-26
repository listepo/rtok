# Devin

`rtok agents install devin` — Devin CLI (`devin`) and Devin Desktop (`Devin.app`).
One install writes the same two files. `[setup.devin] config_path` (default
`~/.config/devin/config.json`; `%APPDATA%\devin\config.json` on Windows) holds hooks
under `"hooks"`. MCP goes in the sibling `mcp_config.json`. The `windsurf` host is a
different app and is left alone.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `"hooks"` in `config.json`: `rtok hook <event> --host devin` on PreToolUse (`^exec$`, `^read$`), PostToolUse, UserPromptSubmit, SessionStart, PostCompaction, SessionEnd, timeout in seconds — the same groups as `plugins/devin/hooks.json` |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp_config.json` as `{command, args}` (off with `[setup] mcp = false`) |
| plugin | `--yes` | install prints `devin plugins install --local <resolved plugins/devin path>` behind the flag. Devin does not document the plugin store, so rtok never writes it and never reports `plugin` as installed |
| proxy | no | Devin's proxy key is an HTTP proxy for CLI traffic, not a model API base URL |

`--host devin` maps Devin's tool names and `PostCompaction` before the shared plugins
run. Use the plugin **or** the user-file hooks, not both: with no store marker, a later
install cannot see the plugin and will not strip the files. After
`devin plugins install --local`, `rtok agents remove devin` takes the user-file hooks
and MCP entry back and leaves the plugin where Devin put it.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Plugins (`.devin-plugin/plugin.json`, root `hooks.json`, `.mcp.json`, `devin plugins install --local`): https://docs.devin.ai/cli/extensibility/plugins/overview
- Hooks (event names, `matcher`, `command`, `timeout`, `"hooks"` wrapper in `config.json`, exit codes): https://docs.devin.ai/cli/extensibility/hooks/overview
- MCP (`mcp_config.json`, `mcpServers.<name>.command` / `args`): https://docs.devin.ai/cli/extensibility/mcp/configuration
- Config file (`~/.config/devin/config.json`, `%APPDATA%\devin\` on Windows, `proxy` is HTTP not a model base URL): https://docs.devin.ai/cli/reference/configuration/config-file
