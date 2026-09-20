# VS Code (GitHub Copilot Chat)

GitHub Copilot agent mode in VS Code reads MCP from the user profile `mcp.json`
(`servers.<name>`, `type: "stdio"`). Stable and Insiders each have their own
profile dir (`[setup.vscode] code_user_dir` / `insiders_user_dir`; empty means
the OS default under Code / Code - Insiders).

## Modules

| module | support | why |
| --- | --- | --- |
| hooks | no | VS Code agent hooks use Claude-format I/O; T46.3 copilot mapping serves only the Copilot CLI camelCase hooks in ~/.copilot/hooks/rtok.json |
| mcp | yes | `servers.rtok = {type: "stdio", command, args: ["mcp"]}` in each profile `mcp.json` |
| proxy | no | Copilot in VS Code has no documented base-URL setting to point at the proxy |
| plugin | no | VS Code loads MCP from the user mcp.json; there is no local plugin directory to link |

MCP is the only path in, so only the plugins that declare an MCP surface are carried.

Reachable (desktop): read, archive, memory, graph, toon

Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

- MCP servers (`mcp.json`, `servers`, stdio): https://code.visualstudio.com/docs/agent-customization/mcp-servers
- MCP configuration reference: https://code.visualstudio.com/docs/agents/reference/mcp-configuration
- Agent hooks (Claude-format I/O; not the Copilot CLI mapping): https://code.visualstudio.com/docs/agent-customization/hooks
