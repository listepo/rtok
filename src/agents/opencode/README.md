# OpenCode

`rtok agents setup opencode` — the CLI (`opencode`) and the desktop app. They keep separate
config files, so each selected app installs on its own:

- CLI: `[setup.opencode] config_path`, default `~/.config/opencode/opencode.json`
- Desktop: `~/Library/Application Support/ai.opencode.desktop/opencode.json` on macOS,
  `%APPDATA%\ai.opencode.desktop\opencode.json` on Windows,
  `~/.config/ai.opencode.desktop/opencode.json` elsewhere

## Modules

| Module | Support | Why |
| --- | --- | --- |
| proxy | yes | `env.OPENAI_BASE_URL` → `http://<bind>:<port>/v1` |
| mcp | no | OpenCode reads MCP from opencode.json, but setup does not write the `mcp` table yet |
| hooks | no | OpenCode has no shell hook events; hosts/opencode/rtok.ts filters tool output instead |
| plugin | no | the OpenCode plugin (hosts/opencode/rtok.ts) is copied by hand; setup does not link it yet |

Both `no` rows for mcp and plugin are "not yet": the config has an `mcp` table and a plugin
directory (`~/.config/opencode/plugin/`), so an installer can take them in a later task.

## rtok plugins this host reaches

- Through the proxy: measure, archive, proxy, toon, compress.
- Through `hosts/opencode/rtok.ts` (by hand): cmd, via `rtok filter` on `tool.execute.after`.
- Not reachable today: read, inject, guard, memory, graph.
