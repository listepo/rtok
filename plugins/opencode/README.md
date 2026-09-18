# rtok OpenCode plugin

`rtok.ts` — OpenCode plugin: `tool.execute.after` replaces bash output with `rtok filter`
(fail open: any spawn error returns the original output). `experimental.session.compacting`
calls `rtok hook PreCompact --host opencode` and appends the budgeted checkpoint to
`output.context` (never `output.prompt`, which would replace the host summary). The next
`experimental.chat.system.transform` injects `PostCompact` restore bytes (T70.6). Hook hosts
are T58.2. `rtok agents install opencode --yes` links it into `<config dir>/plugins/rtok.ts`
for the CLI and the desktop app (T44.5). `rtok.test.ts` is its unit test. Missing `rtok`:
install with ketch (`ketch install listepo/rtok`).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (`~/.config/opencode/plugins/`, `.opencode/plugins/`, `tool.execute.after`, `experimental.session.compacting` `context` append vs `prompt` replace): https://opencode.ai/docs/plugins/
- MCP (`mcp.<name>` with `type: "local"`, `command` array, `enabled`): https://opencode.ai/docs/mcp-servers/
- Config (`~/.config/opencode/opencode.json`, `plugin` array): https://opencode.ai/docs/config/
