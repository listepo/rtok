# VS Code

`rtok agents install vscode` — GitHub Copilot Chat in Visual Studio Code (stable and
Insiders). Each edition's user `mcp.json` gets `servers.rtok` → `rtok mcp` as
`{type: "stdio", command, args}`, the stdio shape the VS Code MCP docs show. The user dir
is `Code/User` and `Code - Insiders/User` under Application Support on macOS, `%APPDATA%`
on Windows, and `~/.config` elsewhere. Foreign servers survive installs and removes.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | no | VS Code agent hooks use hook_event_name; T46.3 Copilot mapping does not serve them |
| mcp | yes | `servers.rtok` → `rtok mcp` in user `mcp.json` as `{type: "stdio", command, args}` (off with `[setup] mcp = false`) |
| proxy | no | VS Code Copilot Chat has no documented base-URL setting to point at the proxy |
| plugin | no | Copilot Chat has no local plugin directory to link; Agent Plugins are marketplace packages |

VS Code documents agent hooks (Preview) as Claude-format files (`.github/hooks/*.json`,
user `~/.copilot/hooks`, `hook_event_name` / `tool_name` on stdin). That is not the Copilot
CLI camelCase stdin the T46.3 `--host copilot` mapping serves, and `~/.copilot/hooks` is
already owned by `rtok agents install copilot`. Setup does not write hooks.

## rtok plugins this host reaches

MCP carries `mcp`. Nothing carries `hook`, `cli` or `proxy`.

Reachable: read, archive, memory, graph, toon
Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- MCP (user `mcp.json` in the VS Code profile, `servers.<name>`, `type: "stdio"`): https://code.visualstudio.com/docs/copilot/customization/mcp-servers
- MCP configuration reference (`type`, `command`, `args`): https://code.visualstudio.com/docs/copilot/reference/mcp-configuration
- Copilot Chat MCP in VS Code: https://docs.github.com/en/copilot/how-tos/provide-context/use-mcp-in-your-ide/extend-copilot-chat-with-mcp
- Hooks (Preview, Claude-format `hook_event_name`, user `~/.copilot/hooks`): https://code.visualstudio.com/docs/copilot/customization/hooks
