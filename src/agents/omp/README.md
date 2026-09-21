# oh my pi

`rtok agents install omp` — oh my pi (`omp`), a fork of pi with native MCP. There is no
desktop app: Zed ACP runs the same binary and config, so one CLI variant covers both.

Two units, one call path each (D21): `<extensions_path>/rtok` → `plugins/pi/` from the rtok
install (the same tree as the `pi` host — omp's loader falls back to the legacy
`pi.extensions` key and follows a symlinked directory), and `mcpServers.rtok` in
`[setup.omp] mcp_path`. Under omp the extension leaves `registerTool` off, so the tools come
from `rtok mcp` only. `remove` unlinks the extension and drops our server, keeping foreign ones.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| plugin | `--yes` | the offer links `plugins/pi`; without a terminal only `--yes` accepts |
| hooks | no | omp hooks are in-process TS modules; the linked pi extension owns that path |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp.json`, `{command, args}` as the omp docs show it, no `type` (off with `[setup] mcp = false`) |
| proxy | no | omp provider base URLs live in models.yml, which setup does not edit |

Verified on omp 18.1.14 (T92): the symlinked `plugins/pi` is discovered through `pi.extensions`;
omp exposes its SDK as `pi.pi`, which the extension reads to skip `registerTool`; a native
`rtok` server and one imported from a Claude Code or Cursor config collapse into one server.
omp applies a revised bash input only when the handler returns `{ input }`, which the extension
does (T92.1).

## rtok plugins this host reaches

The extension carries `cli` (bash → `rtok run`, `tool_result` → `rtok filter`, `context` →
`rtok archive rewrite`); `mcp.json` carries `mcp`. Hook and proxy surfaces have no path in.

Reachable: measure, cmd, read, archive, guard, memory, graph, toon
Not reachable: proxy, inject, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Extensions (`tool_call`, `tool_result`, `context`, `session_start`, returning `{ input }`): https://github.com/can1357/oh-my-pi/blob/main/docs/extensions.md
- Extension loading (`~/.omp/agent/extensions`, `omp.extensions` / legacy `pi.extensions`, symlinks): https://github.com/can1357/oh-my-pi/blob/main/docs/extension-loading.md
- Hooks (in-process TS hook modules): https://github.com/can1357/oh-my-pi/blob/main/docs/hooks.md
- MCP (`~/.omp/agent/mcp.json` `mcpServers.<name>.command` / `args` / `env`): https://github.com/can1357/oh-my-pi/blob/main/docs/mcp-config.md
- Skills: https://github.com/can1357/oh-my-pi/blob/main/docs/skills.md
- Marketplace: https://github.com/can1357/oh-my-pi/blob/main/docs/marketplace.md
- The linked bundle: `plugins/pi/README.md`
