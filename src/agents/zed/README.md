# Zed

`rtok agents install zed` — Zed (the CLI and the desktop app read one file). One config
file, `[setup.zed] config_path` (default `~/.config/zed/settings.json`), gets
`context_servers.rtok` → `rtok mcp` as `{command, args}`, the local-server shape the Zed
MCP docs show. The settings file is JSON with `//` and `/* */` comments, so setup edits
the text surgically instead of reprinting it: comments and foreign servers survive
installs and removes.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | no | Zed has no shell hook events; its agent runs tools itself and takes external agents over ACP |
| mcp | yes | `context_servers.rtok` → `rtok mcp` in `settings.json` as `{command, args}` (off with `[setup] mcp = false`) |
| proxy | no | Zed serves hosted models or provider API keys; there is no documented base-URL setting to point at the proxy |
| plugin | no | Zed extensions install from the marketplace; there is no local directory to link |

Zed has no shell hook events, so there is no `rtok hook` call path here — unlike Cursor
(`beforeShellExecution`) or Copilot (`hooks/*.json`). The Zed agent reads
`context_servers` directly, and Zed forwards the same servers to external agents over
ACP, so one entry serves both paths. Not verified on a live install: the desktop app
bundle paths, and settings with constructs beyond comments (a file that does not parse
as JSON-with-comments is left untouched with an error).

## rtok plugins this host reaches

MCP carries `mcp`. Nothing carries `hook`, `cli` or `proxy`.

Reachable: read, archive, memory, graph, toon
Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- MCP (`context_servers` in the settings file, local `{command, args, env}` and remote `{url}` shapes, forwarding to external agents over ACP): https://zed.dev/docs/assistant/model-context-protocol
- Models (hosted models or own API keys; no base-URL proxy surface): https://zed.dev/docs/ai/models
- MCP server extensions (marketplace extensions; no linkable plugin directory): https://zed.dev/docs/extensions/mcp-extensions
