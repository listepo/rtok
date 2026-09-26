# rtok OpenCode plugin

`rtok.ts` — OpenCode plugin: `tool.execute.before` calls `rtok guard check` (T70.5) and
throws the deny reason so the model sees it; missing `rtok`, non-zero, unparsable or a deny
without a reason fails open. The same before-hook rewrites bash to `rtok run -- '…'`
(skipped when already wrapped). `tool.execute.after` records via `rtok hook PostToolUse`;
skill-tool output goes through `rtok filter --cmd "skill <name>" --archive` (head 30 /
tail 5, headings kept; fail open: any spawn error returns the original output). Bash that
already ran under `rtok run` is left alone. `experimental.session.compacting` calls
`rtok hook PreCompact --host opencode` and
appends the budgeted checkpoint to `output.context` (never `output.prompt`). The next
`experimental.chat.system.transform` injects `PostCompact` restore bytes (T70.6). Hook hosts
are T58.2. `rtok agents install opencode` links it into `<config dir>/plugins/rtok.ts` by
default, once OpenCode itself is detected (T164), for the CLI and the desktop app (T44.5).
`rtok.test.ts` is its unit test (vitest). Missing `rtok`: install with ketch
(`ketch install listepo/rtok`).

Kilo Code 7 runs on the OpenCode server and loads the same file: the default export is
`{ id: "rtok", server }` (Kilo's module descriptor,
https://kilo.ai/docs/automate/extending/plugins). `rtok agents install kilo` links it into
`~/.config/kilo/plugins/rtok.ts` by default, once Kilo Code itself is detected (T164), for
the `kilo` CLI and the VS Code extension (T97). The plugin still passes `--host opencode`, so
Kilo rows are labelled `opencode`.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (`~/.config/opencode/plugins/`, `.opencode/plugins/`, `tool.execute.before`, `tool.execute.after`, `experimental.session.compacting` `context` append vs `prompt` replace): https://opencode.ai/docs/plugins/
- Skills (native `skill` tool, `skill({ name })`, body returned as the tool result): https://opencode.ai/docs/skills/
- Tools (`skill` loads a `SKILL.md` into the conversation): https://opencode.ai/docs/tools/
- MCP (`mcp.<name>` with `type: "local"`, `command` array, `enabled`): https://opencode.ai/docs/mcp-servers/
- Config (`~/.config/opencode/opencode.json`, `plugin` array): https://opencode.ai/docs/config/
- Kilo Code plugins (`~/.config/kilo/plugin/`, same `tool.execute.*` events): https://kilo.ai/docs/automate/extending/plugins

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

