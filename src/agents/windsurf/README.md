# Windsurf

`rtok agents install windsurf` — Windsurf's Cascade agent (the desktop IDE). One config
file, `[setup.windsurf] config_path` (default `~/.codeium/windsurf/mcp_config.json`), gets
`mcpServers.rtok` → `rtok mcp` as `{command, args}`, the stdio shape the Windsurf MCP docs
show, with no `type` field. Foreign servers survive installs and removes.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | no | Cascade hooks speak agent_action_name/tool_info, not hook_event_name/tool_name; rtok hook has no --host windsurf mapping |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp_config.json` as `{command, args}` (off with `[setup] mcp = false`) |
| proxy | no | Windsurf serves its own models; there is no documented base-URL setting to point at the proxy |
| plugin | no | Cascade has no local plugin directory to link; rules and memories live in .windsurf/ |

Cascade hooks (`~/.codeium/windsurf/hooks.json`, twelve events such as `pre_run_command`
and `pre_read_code`) take `agent_action_name` / `trajectory_id` / `tool_info` on stdin and
block with exit code 2 — a different contract from `hook_event_name` / `tool_name` /
`tool_input`, so `rtok hook` would need a `--host windsurf` payload mapping first (the
T46.3 Copilot mapping is the pattern). Not verified on a live install: the app bundle
paths and whether a `windsurf` binary exists on PATH (setup matches the app or the config
directory, like ZCode).

## rtok plugins this host reaches

MCP carries `mcp`. Nothing carries `hook`, `cli` or `proxy`.

Reachable: read, archive, memory, graph, toon
Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- MCP (`~/.codeium/windsurf/mcp_config.json`, `mcpServers.<name>` `command` / `args`, no `type` on stdio): https://docs.windsurf.com/windsurf/cascade/mcp
- Hooks (`~/.codeium/windsurf/hooks.json`, twelve `agent_action_name` events, stdin shape, exit 2 blocks pre-hooks): https://docs.windsurf.com/windsurf/cascade/hooks
