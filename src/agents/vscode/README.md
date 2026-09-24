# VS Code (GitHub Copilot Chat)

GitHub Copilot agent mode in VS Code reads MCP from the user profile `mcp.json`
(`servers.<name>`, `type: "stdio"`), or from a linked Agent Plugin's own `.mcp.json` /
`hooks/hooks.json` once its directory is registered in `chat.pluginLocations`
(`settings.json`, an object mapping path to enabled state — JSONC, T79/T117). Stable and
Insiders each have their own profile dir (`[setup.vscode] code_user_dir` / `insiders_user_dir`;
empty means the OS default under Code / Code - Insiders).

Files: `<user-dir>/mcp.json` (MCP without the plugin) and `<user-dir>/settings.json`
(`chat.pluginLocations`).
Plugin link: `<user-dir>/plugins/rtok` → `plugins/claude/` from the rtok install — VS Code
accepts that tree's layout (`.claude-plugin/plugin.json`, `.mcp.json`, `hooks/hooks.json`) as
one of its documented formats, so setup links it as is rather than shipping a separate
`plugins/vscode/` copy (D21).

## Modules

| module | support | why |
| --- | --- | --- |
| hooks | no | no direct hooks file rtok writes; the linked plugin's hooks/hooks.json is what VS Code's Local harness runs (chat.useClaudeHooks) |
| mcp | yes | `servers.rtok = {type: "stdio", command, args: ["mcp"]}` in each profile `mcp.json`, or served by the linked plugin (then `mcp.json` is left alone: one MCP per store) |
| proxy | no | Copilot in VS Code has no documented base-URL setting to point at the proxy |
| plugin | `--yes` | registers `plugins/claude` (hooks + MCP as one unit) in `chat.pluginLocations`, per profile; a stale or foreign destination is never overwritten |

MCP is the only direct path in; the linked plugin also carries hooks, so the plugins that
declare either surface are reachable once it is registered.

## rtok plugins this host reaches

Hooks carry the `hook` surface, MCP carries `mcp`; the linked plugin serves both.

Reachable (desktop): cmd, read, archive, inject, guard, memory, graph, toon
Not reachable (desktop): measure, proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.
Plugin format and `chat.pluginLocations` confirmed by fetching these pages 2026-09-24 (T117):
VS Code accepts a plugin whose manifest lives at `.claude-plugin/plugin.json` (Claude's own
layout, matching `plugins/claude/` unchanged), and its hook event names
(`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PreCompact`,
`SubagentStart`, `SubagentStop`, `Stop`) match rtok's.

- Agent plugins (`chat.pluginLocations`, manifest formats, directory layout): https://code.visualstudio.com/docs/agent-customization/agent-plugins
- MCP servers (`mcp.json`, `servers`, stdio): https://code.visualstudio.com/docs/agent-customization/mcp-servers
- MCP configuration reference: https://code.visualstudio.com/docs/agents/reference/mcp-configuration
- Agent hooks (event names, `chat.useHooks`, `chat.useClaudeHooks`, plugin discovery): https://code.visualstudio.com/docs/agent-customization/hooks
- The linked bundle: `plugins/claude/README.md`
