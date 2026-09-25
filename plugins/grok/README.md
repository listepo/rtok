# rtok Grok Build plugin

One plugin directory for Grok Build (xAI's `grok` CLI): rtok's hooks and its MCP server as one
unit (D21: plugin and MCP together, one `rtok mcp` per store). Grok reads Claude-style plugin
directories, so the layout is the Claude one with Grok's manifest folder.

Install:

- `grok plugin install <path to this folder> --trust`, then press `r` in the Plugins tab
  (`/plugins`) or start a new session. Hooks and MCP servers stay inactive without `--trust`.
- Or copy it to `~/.grok/plugins/rtok/` (auto-trusted) and list `rtok` in `[plugins].enabled`
  in `~/.grok/config.toml`; plugins are off until enabled.
- Remove: `grok plugin uninstall rtok`.

macOS/Linux only: each hook shells out with `command -v rtok`, falling back to
`~/.ketch/bin/rtok`, and exits 0 silently if neither exists (fail open; no note anywhere, not
even `SessionStart`). Grok runs the same command through PowerShell on Windows, where that
one-liner does not work — Windows users should run `rtok agents install claude` instead (Grok
imports Claude's hooks, see below); `rtok agents install grok` skips the plugin offer there. The
MCP server still needs `rtok` on `PATH` and does not start without it; install it with ketch:
`ketch install listepo/rtok`.

Use the plugin **or** rtok's Claude install, not both. Grok imports hooks from
`~/.claude/settings.json` and MCP servers from `~/.claude.json` by default, so after
`rtok agents install claude` rtok already runs inside Grok (`rtok hook` recognises Grok's
envelope from the runner's `GROK_HOOK_EVENT`, whichever file declared the hook). With both in
place every event fires twice and two `rtok mcp` processes share one store. To keep the plugin,
turn the import off in `~/.grok/config.toml`:

```toml
[compat.claude]
hooks = false
mcps = false
```

Files:

- `.grok-plugin/plugin.json` — manifest (name, version, metadata).
- `hooks/hooks.json` — `rtok hook <event> --host grok` (PATH, then `~/.ketch/bin/rtok`, else
  exit 0) on PreToolUse (Bash), PostToolUse, UserPromptSubmit, SessionStart, PreCompact,
  PostCompact, SessionEnd, each with `timeout: 5` (Grok's PostToolUse default is 600 s).
  Checked by `tests/grok_plugin.rs`.
- `.mcp.json` — `mcpServers.rtok` → `rtok mcp`.

Known limits:

- No `Read` or `Skill` hook. Grok's file read is `read_file`, and Grok blocks a call whose
  `updatedInput` fails the tool's schema; rtok's Read rewrite has not been checked against
  `read_file`, so it stays off until a live payload confirms the shape (plan T100).
- `UserPromptSubmit` context is dropped: Grok discards an allowing hook's stdout for that event.
  `SessionStart` stdout is ignored too, so rtok's injected context does not reach Grok's model.
- Not verified on a live Grok session: that the plugin loads, and the working directory Grok
  gives a plugin's stdio MCP server. `rtok mcp` resolves relative paths from its process directory.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.
Grok also ships the same guides in `~/.grok/docs/user-guide/` (`09-plugins.md`, `10-hooks.md`).

- Plugins (layout, `.grok-plugin/`, `hooks/hooks.json`, `.mcp.json`, `grok plugin install --trust`, `~/.grok/plugins/`, `GROK_PLUGIN_ROOT`): https://docs.x.ai/build/features/skills-plugins-marketplaces
- Hooks (events, `matcher` regex, stdin envelope, `hookSpecificOutput`, exit codes, `GROK_HOOK_EVENT`): https://docs.x.ai/build/features/hooks
- MCP servers (`.mcp.json`, `[mcp_servers.<name>]`, Claude/Cursor imports): https://docs.x.ai/build/features/mcp-servers
- Settings (`[compat.claude]`, `[plugins]`, `GROK_HOME`): https://docs.x.ai/build/settings/reference

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

