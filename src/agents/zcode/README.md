# ZCode

`rtok agents setup zcode` — Z.ai's ZCode desktop app (the GLM coding harness). One config
file, `[setup.zcode] config_path`, default `~/.zcode/cli/config.json`, carries hooks and MCP.
The app starts without a shell PATH, so every command written is the absolute `rtok` binary.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `hooks.enabled = true` and `hooks.events.<Event>[]` → `<abs rtok> hook <event>` with `timeoutMs` on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart; ZCode has no PreCompact, PostCompact or SessionEnd |
| mcp | yes | `mcp.servers.rtok` → `<abs rtok> mcp` (off with `[setup] mcp = false`) |
| proxy | no | ZCode providers are per-id tables with their own keys and base URLs; setup does not edit them |
| plugin | no | ZCode plugins come from the Z.ai marketplace; there is no local plugin directory to link |

ZCode's hook protocol is Claude's: stdin `session_id`, `cwd`, `hook_event_name`, `tool_name`,
`tool_input`; stdout `hookSpecificOutput`; exit 2 blocks. `rtok hook` runs unchanged with the
default `[hook] host = "claude"`. Not verified on a live install: the app bundle paths and
whether ZCode names its shell tool `Bash` — the matchers follow the docs.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Configuration (`~/.zcode/cli/config.json`, `provider.<id>`): https://zcode.z.ai/en/docs/configuration
- Hooks (`hooks.enabled`, `hooks.events.<Event>[]`, `timeoutMs`, stdin/stdout shape, exit 2): https://zcode.z.ai/en/docs/hooks
- MCP (`mcp.servers.<name>.command` / `args` / `env`): https://zcode.z.ai/en/docs/mcp-services
- Plugins (marketplace, `.zcode-plugin/plugin.json`): https://zcode.z.ai/en/docs/plugin
