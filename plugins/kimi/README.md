# rtok Kimi Code plugin

One manifest, `kimi.plugin.json`, carries rtok's hooks (D21). Kimi Code CLI and Kimi Code
Desktop manage the same plugins, so one install covers both. The plugin no longer ships an
MCP server of its own (T275/D33): `rtok agents install kimi` always writes `mcpServers.rtok`
into `mcp.json` itself, plugin installed or not, so there is exactly one path to that entry
instead of two copies to keep in sync.

Install — Kimi copies the folder into `$KIMI_CODE_HOME/plugins/managed/rtok/` and runs the copy,
so there is nothing to symlink and a changed source needs a reinstall:

- CLI: `/plugins install <path to this folder>`, then `/reload` or a new session.
- Desktop: Settings → Plugins → add this folder as a custom source.
- Remove: `/plugins remove rtok`.

`rtok` must be on `PATH`. If it is missing, every hook exits non-zero without blocking the call
(only exit 2 blocks); install it with ketch: `ketch install pyrlyn/rtok`.

Use the plugin **or** `rtok agents install kimi`, not both, for hooks — unless you installed
the plugin through Kimi: while `<kimi home>/plugins/managed/rtok/kimi.plugin.json` exists,
the installer detects it and strips its own `[[hooks]]` tables instead of adding them (D21
singleton), so no event fires twice. Without the plugin, the installer writes the same nine
hooks into `config.toml` itself. MCP is independent of either path (T275/D33).

Files:

- `kimi.plugin.json` — `hooks`: `rtok hook <event>` on PreToolUse (Bash, Read, Skill),
  PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd — the same set
  `rtok agents install kimi` writes, kept equal by
  `agents::kimi::tests::plugin_manifest_matches_the_installer`.

Known limits:

- Kimi runs a plugin hook with its working directory set to the plugin root. `rtok hook` takes
  the project from the `cwd` field on stdin, but the project config layer (`.rtok.toml` at the
  git root, and the nearest `.env`) is found from the process directory, so it does not apply to
  plugin hooks.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (`kimi.plugin.json` fields, `hooks`, `/plugins install`, `plugins/managed/`, `KIMI_PLUGIN_ROOT`): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/plugins.html
- Hooks (`event` / `matcher` / `command` / `timeout`, event names, stdin, exit codes): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/hooks.html
- MCP (`mcpServers.<name>.command` / `args`, `mcp.json`, written by `rtok agents install kimi` itself): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/mcp.html
- Desktop (Settings → Plugins; settings shared with the CLI): https://www.kimi.com/code/docs/en/kimi-code-desktop/using-desktop.html

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

