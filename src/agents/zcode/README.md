# ZCode

`rtok agents install zcode` — Z.ai's ZCode desktop app (the GLM coding harness). One config
file, `[setup.zcode] config_path`, default `~/.zcode/cli/config.json`, carries hooks and MCP.
The app starts without a shell PATH, so every command written is the absolute `rtok` binary.
The linked plugin carries both instead, by default (below).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `hooks.enabled = true` and `hooks.events.<Event>[]` → `<abs rtok> hook <event>` with `timeoutMs` on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart; ZCode has no PreCompact, PostCompact or SessionEnd |
| mcp | yes | `mcp.servers.rtok` → `<abs rtok> mcp` (off with `[setup] mcp = false`) |
| proxy | no | ZCode providers are per-id tables with their own keys and base URLs; setup does not edit them |
| plugin | yes | links `plugins/zcode` to `~/.zcode/cli/plugins/local/rtok` by default, once ZCode itself is detected, and lists it in `plugins.dirs` — ZCode loads inline plugin roots from there, enabled by default; while the plugin is linked it is the only call path, so setup strips its own config-file entries; a stale or foreign destination is never overwritten |

ZCode's hook protocol is Claude's: stdin `session_id`, `cwd`, `hook_event_name`, `tool_name`,
`tool_input`; stdout `hookSpecificOutput`; exit 2 blocks. `rtok hook` runs unchanged with the
default `[hook] host = "claude"`. Not verified on a live install: whether ZCode names its
shell tool `Bash` — the matchers follow the docs. The app bundle path and the inline-plugin
discovery (`plugins.dirs`, enabled by default, marketplace id `inline`) were read from the
installed app (v0.2.0) and its bundled configuration guide.

The plugin (`plugins/zcode/`) wraps the same five hook entries and one `rtok mcp` in launchers
that resolve the binary themselves (`${ZCODE_PLUGIN_ROOT}/scripts/*.sh`, fail open with the
ketch hint). Windows: the launchers are POSIX, so prefer the plain install there, which writes
the absolute exe.

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
