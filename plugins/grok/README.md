# rtok Grok Build plugin

One plugin directory for Grok Build (xAI's `grok` CLI): rtok's hooks (D21). Grok reads
Claude-style plugin directories, so the layout is the Claude one with Grok's manifest folder.
It ships no MCP server of its own (T275/D33): `rtok agents install grok` always writes
`[mcp_servers.rtok]` into `~/.grok/config.toml` itself, plugin installed or not, so there is
exactly one path to that entry instead of two copies to keep in sync (skipped only while the
`[compat.claude]` import already covers it, see below).

Install:

- `grok plugin install <path to this folder> --trust`, then press `r` in the Plugins tab
  (`/plugins`) or start a new session. Hooks stay inactive without `--trust`.
- Or copy it to `~/.grok/plugins/rtok/` (auto-trusted) and list `rtok` in `[plugins].enabled`
  in `~/.grok/config.toml`; plugins are off until enabled.
- Remove: `grok plugin uninstall rtok`.

macOS/Linux only: each hook shells out with `command -v rtok`, falling back to
`~/.ketch/bin/rtok`, and exits 0 silently if neither exists (fail open; no note anywhere, not
even `SessionStart`). Grok runs the same command through PowerShell on Windows, where that
one-liner does not work — Windows users should run `rtok agents install claude` instead (Grok
imports Claude's hooks, see below); `rtok agents install grok` skips the plugin offer there.

Use the plugin **or** rtok's Claude install for hooks, not both. Grok imports hooks from
`~/.claude/settings.json` by default, so after `rtok agents install claude` rtok already runs
inside Grok (`rtok hook` recognises Grok's envelope from the runner's `GROK_HOOK_EVENT`,
whichever file declared the hook). With both in place every event fires twice. To keep the
plugin, turn the import off in `~/.grok/config.toml`:

```toml
[compat.claude]
hooks = false
```

MCP needs no such workaround: `register_mcp` already skips writing `[mcp_servers.rtok]` while
`[compat.claude] mcps` (on by default) reports rtok's MCP as already served through
`~/.claude.json`, so at most one `rtok mcp` process ever runs per store.

Files:

- `.grok-plugin/plugin.json` — manifest (name, version, metadata).
- `hooks/hooks.json` — `rtok hook <event> --host grok` (PATH, then `~/.ketch/bin/rtok`, else
  exit 0) on PreToolUse (Bash), PostToolUse, UserPromptSubmit, SessionStart, PreCompact,
  PostCompact, SessionEnd, each with `timeout: 5` (Grok's PostToolUse default is 600 s).
  Checked by `tests/grok_plugin.rs`.

Known limits:

- No `Read` or `Skill` hook. Grok's file read is `read_file`, and Grok blocks a call whose
  `updatedInput` fails the tool's schema; rtok's Read rewrite has not been checked against
  `read_file`, so it stays off until a live payload confirms the shape (plan T100).
- `UserPromptSubmit` context is dropped: Grok discards an allowing hook's stdout for that event.
  `SessionStart` stdout is ignored too, so rtok's injected context does not reach Grok's model.
- Not verified on a live Grok session: that the plugin loads.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.
Grok also ships the same guides in `~/.grok/docs/user-guide/` (`09-plugins.md`, `10-hooks.md`).

- Plugins (layout, `.grok-plugin/`, `hooks/hooks.json`, `grok plugin install --trust`, `~/.grok/plugins/`, `GROK_PLUGIN_ROOT`): https://docs.x.ai/build/features/skills-plugins-marketplaces
- Hooks (events, `matcher` regex, stdin envelope, `hookSpecificOutput`, exit codes, `GROK_HOOK_EVENT`): https://docs.x.ai/build/features/hooks
- MCP servers (`[mcp_servers.<name>]`, written by `rtok agents install grok` itself; Claude/Cursor imports): https://docs.x.ai/build/features/mcp-servers
- Settings (`[compat.claude]`, `[plugins]`, `GROK_HOME`): https://docs.x.ai/build/settings/reference

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

