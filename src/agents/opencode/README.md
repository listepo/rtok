# OpenCode

`rtok agents install opencode` — the CLI (`opencode`) and the desktop app. They keep separate
config files, so each selected app installs on its own:

- CLI: `[setup.opencode] config_path`, default `~/.config/opencode/opencode.json`
- Desktop: `~/Library/Application Support/ai.opencode.desktop/opencode.json` on macOS,
  `%APPDATA%\ai.opencode.desktop\opencode.json` on Windows,
  `~/.config/ai.opencode.desktop/opencode.json` elsewhere

## Modules

| Module | Support | Why |
| --- | --- | --- |
| proxy | yes | `env.OPENAI_BASE_URL` → `http://<bind>:<port>/v1` |
| mcp | yes | `mcp.rtok` → `{type: local, command: [rtok, mcp], enabled: true}` (off with `[setup] mcp = false`) |
| plugin | `--yes` | the offer links `plugins/opencode/rtok.ts` to `<config dir>/plugins/rtok.ts`; without a terminal only `--yes` accepts |
| hooks | no | OpenCode has no shell hook events; the linked plugin filters bash output instead |

Plugin link (D21): OpenCode loads every `*.ts` in `<config dir>/plugins/`, so the link is one
file, no manifest. The plugin and the MCP entry are two capabilities, not two paths to one:
`tool.execute.after` filters bash output through `rtok filter`, the MCP serves
`read`/`search`/`memory`/`graph`. OpenCode plugins run in-process and cannot register an
MCP server, so no singleton clear applies. A missing `rtok` fails open (output unchanged) and
prints the ketch install line once.

## rtok plugins this host reaches

The proxy carries `proxy`, MCP carries `mcp`, and the linked plugin carries the bash call
path (`cli`). Nothing carries `hook`.

Reachable: measure, cmd, read, archive, proxy, memory, graph, toon, compress
Not reachable: inject, guard

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Config (`~/.config/opencode/opencode.json`): https://opencode.ai/docs/config/
- MCP (`mcp.<name>` with `type: "local"`, `command` array, `enabled`): https://opencode.ai/docs/mcp-servers/
- Plugins (`~/.config/opencode/plugins/`, `.opencode/plugins/`, `tool.execute.after`): https://opencode.ai/docs/plugins/
