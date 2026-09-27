# Kilo Code

`rtok agents install kilo` — the CLI (`kilo`) and the VS Code extension (`kilocode.kilo-code`).
Kilo Code 7 runs on the OpenCode server, and both surfaces read one global config dir, so both
variants install the same files:

- Config: `[setup.kilo] config_path`, default `~/.config/kilo/kilo.json`
- Plugin: `<config dir>/plugins/rtok.ts`

rtok writes `kilo.json`, never `kilo.jsonc`: Kilo merges both, and rewriting a JSONC file would
drop its comments (T79). The extension's install directory carries its version, so the desktop
variant is detected by VS Code itself.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| mcp | yes | `mcp.rtok` → `{type: local, command: [rtok, mcp], enabled: true}` (off with `[setup] mcp = false`) |
| plugin | yes | links `plugins/opencode/rtok.ts` to `<config dir>/plugins/rtok.ts` by default, once Kilo Code itself is detected; a stale or foreign destination is never overwritten |
| hooks | no | Kilo Code has no shell hook events; the linked plugin filters bash output instead |
| proxy | no | Kilo Code keeps provider base URLs in its provider settings, which setup does not edit |

Plugin link (D21): Kilo loads every `*.ts` / `*.js` in `<config dir>/{plugin,plugins}/`, and
`plugins/opencode/rtok.ts` imports nothing from OpenCode, so the OpenCode plugin is reused as
is. The plugin and the MCP entry are two capabilities, not two paths to one: the plugin
rewrites bash to `rtok run -- '…'` (skills still go through `rtok filter`), the MCP serves
`read`/`search`/`memory`/`graph`. The default export is `{ id: "rtok", server }` for Kilo's
module descriptor. The plugin reports as `--host opencode`. A missing `rtok` fails open
(output unchanged) and prints the ketch install line once.

## rtok plugins this host reaches

MCP carries `mcp`, and the linked plugin carries the bash call path (`cli`). Nothing carries
`hook` or `proxy`.

Reachable: measure, cmd, read, archive, guard, memory, graph, toon
Not reachable: proxy, inject, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- CLI (`kilo`, config in `~/.config/kilo/`): https://kilo.ai/docs/code-with-ai/platforms/cli
- MCP (`mcp.<name>` with `type: "local"`, `command` array, `enabled`): https://kilo.ai/docs/automate/mcp/using-in-kilo-code
- Plugins (`~/.config/kilo/plugin/`, `.kilo/plugin/`, `tool.execute.before`, `tool.execute.after`): https://kilo.ai/docs/automate/extending/plugins
- Skills (`~/.kilo/skills/`, `.kilo/skills/`): https://kilo.ai/docs/customize/skills
- Custom rules: https://kilo.ai/docs/customize/custom-rules
