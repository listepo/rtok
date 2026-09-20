# rtok OpenCode plugin

`rtok.ts` — OpenCode plugin: `tool.execute.before` calls `rtok guard check` (T70.5) and
throws the deny reason so the model sees it; missing `rtok`, non-zero, unparsable or a deny
without a reason fails open. `tool.execute.after` records via `rtok hook PostToolUse` then
replaces bash output with `rtok filter` and skill-tool output with
`rtok filter --cmd "skill <name>" --archive` (head 30 / tail 5, headings kept; fail open:
any spawn error returns the original output). `experimental.session.compacting` calls
`rtok hook PreCompact --host opencode` and
appends the budgeted checkpoint to `output.context` (never `output.prompt`). The next
`experimental.chat.system.transform` injects `PostCompact` restore bytes (T70.6). Hook hosts
are T58.2. `rtok agents install opencode --yes` links it into `<config dir>/plugins/rtok.ts`
for the CLI and the desktop app (T44.5). `rtok.test.ts` is its unit test. Missing `rtok`:
install with ketch (`ketch install listepo/rtok`).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (`~/.config/opencode/plugins/`, `.opencode/plugins/`, `tool.execute.before`, `tool.execute.after`, `experimental.session.compacting` `context` append vs `prompt` replace): https://opencode.ai/docs/plugins/
- Skills (native `skill` tool, `skill({ name })`, body returned as the tool result): https://opencode.ai/docs/skills/
- Tools (`skill` loads a `SKILL.md` into the conversation): https://opencode.ai/docs/tools/
- MCP (`mcp.<name>` with `type: "local"`, `command` array, `enabled`): https://opencode.ai/docs/mcp-servers/
- Config (`~/.config/opencode/opencode.json`, `plugin` array): https://opencode.ai/docs/config/
