# rtok Cline plugin

Linked by `rtok agents install cline --yes` into `~/Documents/Cline/Hooks/` (T96): one
link per event (`PreToolUse`, `PostToolUse`, `TaskStart`, …) pointing at
`hooks/rtok-hook` (D21: hooks and MCP as one unit, one `rtok mcp` per store).
By hand: link (or copy, if Cline does not follow symlinks on your setup) this
script once per event into `~/Documents/Cline/Hooks/<Event>`, then enable hooks
in the extension settings:

```sh
ln plugins/cline/hooks/rtok-hook ~/Documents/Cline/Hooks/PreToolUse
ln plugins/cline/hooks/rtok-hook ~/Documents/Cline/Hooks/PostToolUse
ln plugins/cline/hooks/rtok-hook ~/Documents/Cline/Hooks/TaskStart
```

Files:

- `hooks/rtok-hook` — the only script. Cline runs one executable per event, so the
  event is the script's own file name (`${0##*/}`, extension stripped:
  `PreToolUse`, `PreToolUse.sh` and `PreToolUse.exe` all mean `PreToolUse`) and it
  runs `rtok hook <event> --host cline` on the file-hook JSON from stdin.

What the extension honours (T94 verify, `src/hooks/mod.rs` `cline_output`):

- `overrideInput: {commands: [...]}` replaces the tool input — PreToolUse only, and
  only when the rewrite is a single command. rtok only rewrites a one-entry
  `run_commands` `commands: [cmd]` (a multi-entry call passes through untouched —
  no rewrite, no measurement), so its `overrideInput` is always one command.
- `context` is injected into the next turn (PostToolUse archive pointers, session
  notes).
- `cancel: true` + `errorMessage` blocks a denied call.
- `{}` means do nothing — including garbage stdin (fail open, exit 0).
- What it does not honour: MCP rewrites (no `updated_mcp_tool_output` — PostToolUse
  keeps `context` only) and anything Claude-shaped (`hookSpecificOutput` is never
  read back).

With `rtok` missing the hook prints `{}`, exits 0, and says on stderr to install
with ketch (`ketch install listepo/rtok`).

MCP is not a file in this tree: T96 writes `mcpServers.rtok` → `rtok mcp` directly
into `cline_mcp_settings.json` (CLI: `~/.cline/data/settings/`; extension: the
VS Code globalStorage settings file).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Hooks (file hooks: one executable per event, `overrideInput` / `context` / `cancel`): https://docs.cline.bot/customization/hooks.md
- Plugins (`~/.cline/plugins`, `cline plugin install`; SDK/CLI/Kanban only, not the VS Code extension): https://docs.cline.bot/customization/plugins.md
- MCP (`cline_mcp_settings.json`, adding/configuring MCP servers): https://docs.cline.bot/mcp/mcp-overview.md
- CLI reference (`cline` commands, flags, configuration options): https://docs.cline.bot/cli/cli-reference.md
- Config (global vs project config, where Cline stores settings): https://docs.cline.bot/getting-started/config.md

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)
