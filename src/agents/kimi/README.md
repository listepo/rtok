# Kimi Code

`rtok agents install kimi` — Moonshot's Kimi Code CLI (`kimi`) and Kimi Code Desktop
(`Kimi Code.app`): one install writes the same two files. Two files under one key,
`[setup.kimi] config_path` (default `~/.kimi-code/config.toml`): hooks go into `config.toml`
as `[[hooks]]` tables, MCP into the sibling `mcp.json`. Edits keep the user's comments and
every hook or server that is not ours.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `[[hooks]]` with `event`, `matcher`, `command = "rtok hook <event>"`, `timeout` (seconds) on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp.json`, `{command, args}` as the Kimi docs show it, no `type` (off with `[setup] mcp = false`) |
| plugin | `--yes` | install prints `/plugins install <resolved plugins/kimi path>` behind the flag — rtok never writes `plugins/managed/`, that store is Kimi's and undocumented. While the plugin is installed (`<kimi home>/plugins/managed/rtok/kimi.plugin.json` exists), setup strips rtok's own `[[hooks]]` tables and `mcpServers.rtok` instead of adding them (D21 singleton: the plugin is the hooks and the MCP as one unit) |
| proxy | no | Kimi Code providers are [providers.<name>] tables with their own base_url and keys; setup does not edit them |

Kimi's hook protocol is Claude's: stdin `hook_event_name`, `session_id`, `cwd`, `tool_name`,
`tool_input`; exit 2 blocks with stderr as the reason; `hookSpecificOutput.permissionDecision`
denies. `rtok hook` runs unchanged with the default `[hook] host = "claude"`. Not in the Kimi
docs: `hookSpecificOutput.updatedInput` on PreToolUse — the command rewrite that `cmd` relies
on is unverified there, the deny path is the one the docs promise.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Config files (`~/.kimi-code/config.toml`, `mcp.json`, `[providers.<name>]`): https://moonshotai.github.io/kimi-code/en/configuration/config-files.html
- Hooks (`[[hooks]]` `event` / `matcher` / `command` / `timeout`, event names, stdin, exit codes): https://moonshotai.github.io/kimi-code/en/customization/hooks.html
- MCP (`mcp.json` `mcpServers.<name>.command` / `args`): https://moonshotai.github.io/kimi-code/en/customization/mcp.html
- Plugins (`plugins/managed/`, `kimi plugin install`): https://moonshotai.github.io/kimi-code/en/customization/plugins.html
