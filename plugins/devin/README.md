# rtok Devin plugin

One plugin directory for Devin (Cognition's `devin` CLI and Devin Desktop, the renamed
Windsurf): rtok's hooks and its MCP server as one unit (D21: plugin and MCP together, one
`rtok mcp` per store). Devin loads a plugin's hooks in local agents only — the CLI and Devin
Desktop — so this one tree covers both.

Install (on this machine only; a local folder cannot go into your Devin Cloud manifest):

- `devin plugins install --local <path to this folder>`. Local installs are linked, so edits
  here apply on the next session. Check with `devin plugins info rtok` or `/hooks` in a session.
- Remove: `devin plugins remove rtok --local`.
- `rtok agents install devin` will print the same line (plan T89); rtok does not write Devin's
  plugin store.

Each hook shells out with `command -v rtok`, falls back to `~/.ketch/bin/rtok`, and exits 0
silently if neither exists (fail open). Devin also treats any non-zero exit other than 2 as
non-blocking, and plugin hooks are best effort on its side. The MCP server needs `rtok` on
`PATH` and does not start without it; install it with ketch: `ketch install listepo/rtok`.

Use the plugin **or** rtok's Claude install, not both. Devin imports hooks from
`~/.claude/settings.json` and `~/.claude.json`, and MCP servers from `~/.claude.json`, by default.
After `rtok agents install claude` those hooks already fire inside Devin, but without
`--host devin`, and every lifecycle event would fire twice beside this plugin. To keep the
plugin, turn the Claude import off in `~/.config/devin/config.json` (it also stops Claude rules
and skills from loading in Devin):

```json
{
  "read_config_from": { "claude": false }
}
```

Files:

- `.devin-plugin/plugin.json` — manifest (`name: "rtok"`, the `/rtok:…` namespace, plus metadata).
- `hooks.json` — at the plugin root, event names at the top level (no `"hooks"` wrapper, the
  layout of Cognition's plugin templates). `rtok hook <event> --host devin` (PATH, then
  `~/.ketch/bin/rtok`, else exit 0) on `PreToolUse` (`^exec$`, `^read$`), `PostToolUse`,
  `UserPromptSubmit`, `SessionStart`, `PostCompaction`, `SessionEnd`, each with `timeout: 5`
  seconds. `--host devin` maps Devin's tool names, `{success, output, error}` results,
  `PostCompaction` and `DEVIN_PROJECT_DIR` onto what the rtok plugins read. Checked by
  `tests/devin_plugin.rs`.
- `.mcp.json` — `mcpServers.rtok` → `rtok mcp`.

Known limits:

- No `PreCompact` hook: Devin has only `PostCompaction`.
- Not verified on a live Devin session: that the plugin loads, the key Devin's `read` tool
  uses for its path (`file_path` or `path`), and whether stdin carries `cwd` (plan T87).
- macOS/Linux only: the hook commands are POSIX shell. How Devin runs hook commands on Windows
  is not documented.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (layout, `.devin-plugin/plugin.json`, root `hooks.json` and `.mcp.json`, `devin plugins install --local`): https://docs.devin.ai/cli/extensibility/plugins/overview
- Hooks (file locations, `matcher` regex, `timeout` seconds, `DEVIN_PROJECT_DIR`, Claude imports): https://docs.devin.ai/cli/extensibility/hooks/overview
- Lifecycle hooks (events, payloads, `tool_response`, exit codes, `PostCompaction`): https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks
- MCP servers (`mcpServers`, stdio `command`/`args`): https://docs.devin.ai/cli/extensibility/mcp/configuration
- Configuration import (`read_config_from`): https://docs.devin.ai/cli/reference/configuration/read-config-from
- Skills (`skills/<name>/SKILL.md`, none shipped here): https://docs.devin.ai/cli/extensibility/skills/overview
- Plugin template with a root `hooks.json` (Cognition's own repo): https://github.com/CognitionAI/plugin-template

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)
