# Cursor

`rtok agents install cursor` — the desktop app (`cursor`) and the CLI (`cursor-agent`). Both
read the same `~/.cursor` tree, so one install covers both; `--cli` / `--desktop` only pick
which app the report shows.

Files: `~/.cursor/hooks.json` (hooks) and `~/.cursor/mcp.json` (MCP).
Plugin link: `~/.cursor/plugins/local/rtok` → `plugins/cursor/` from the rtok install (D21).

Headless ping (`rtok mcp ping cursor --cli`): `cursor-agent -p "<prompt>"` (`agent -p` is the
same flag). `-p` / `--print` is non-interactive mode
(https://cursor.com/docs/cli/using, checked 2026-09-27; `agent -p` at
https://cursor.com/docs/cli/overview). `--desktop` checks `mcp.json` and prints the prompt.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `beforeShellExecution` → PreToolUse, `afterShellExecution` → PostToolUse, `preCompact` → PreCompact; all `--host cursor`; in `hooks.json` only without the plugin — the linked plugin carries the same events, so setup strips ours there (T244) |
| mcp | yes | `mcpServers.rtok` in `mcp.json` (off with `[setup] mcp = false`); independent of the plugin (T275/D33): written on every install/update regardless of plugin state, only remove takes it out |
| plugin | yes | links `plugins/cursor` (hooks only: T275/D33) by default, once Cursor itself is detected; a stale or foreign destination is never overwritten |
| proxy | no | Cursor has no base-URL setting to point at the proxy |

## rtok plugins this host reaches

Hooks carry the `hook` and `cli` surfaces; the linked plugin serves them. MCP carries `mcp`,
served by `mcp.json` on its own. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Plugins (manifest `.cursor-plugin/plugin.json`, local install `~/.cursor/plugins/local/<name>`): https://cursor.com/docs/plugins
- Manifest reference (`hooks` field): https://cursor.com/docs/reference/plugins
- Hooks (`~/.cursor/hooks.json`, `"version": 1`, `beforeShellExecution`, `afterShellExecution`, `preCompact`): https://cursor.com/docs/agent/hooks
- MCP (`~/.cursor/mcp.json`, `mcpServers.<name>.command` / `args`): https://cursor.com/docs/context/mcp
- Skills (`~/.cursor/skills/<name>/`): https://cursor.com/docs/skills
- Non-interactive prompt (`cursor-agent -p` / `--print`; the docs' `agent -p` is the same flag; `rtok mcp ping cursor --cli`): https://cursor.com/docs/cli/using
- The linked bundle: `plugins/cursor/README.md`
