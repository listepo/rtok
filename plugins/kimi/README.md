# rtok Kimi Code plugin

One manifest, `kimi.plugin.json`, carries rtok's hooks and its MCP server together (D21: plugin
and MCP as one unit, one `rtok mcp` per store). Kimi Code CLI and Kimi Code Desktop manage the
same plugins, so one install covers both.

Install — Kimi copies the folder into `$KIMI_CODE_HOME/plugins/managed/rtok/` and runs the copy,
so there is nothing to symlink and a changed source needs a reinstall:

- CLI: `/plugins install <path to this folder>`, then `/reload` or a new session.
- Desktop: Settings → Plugins → add this folder as a custom source.
- Remove: `/plugins remove rtok`.

`rtok` must be on `PATH`. If it is missing, every hook exits non-zero without blocking the call
(only exit 2 blocks) and the MCP server does not start; install it with ketch:
`ketch install listepo/rtok`.

Use the plugin **or** `rtok agents install kimi`, not both: the installer writes the same nine
hooks into `config.toml` and the same server into `mcp.json`, and with both in place every event
fires twice and two `rtok mcp` processes share one store.

Files:

- `kimi.plugin.json` — `hooks`: `rtok hook <event>` on PreToolUse (Bash, Read, Skill),
  PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd — the same set
  `rtok agents install kimi` writes, kept equal by
  `agents::kimi::tests::plugin_manifest_matches_the_installer`. `mcpServers.rtok` → `rtok mcp`.

Known limits:

- Kimi runs a plugin hook with its working directory set to the plugin root. `rtok hook` takes
  the project from the `cwd` field on stdin, but the project config layer (`.rtok.toml` at the
  git root, and the nearest `.env`) is found from the process directory, so it does not apply to
  plugin hooks.
- Not verified on a live Kimi install: the working directory Kimi gives a plugin's stdio MCP
  server (the docs only constrain an explicit `cwd` to the plugin root). `rtok mcp` resolves
  relative paths from its process directory.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (`kimi.plugin.json` fields, `hooks`, `mcpServers`, `/plugins install`, `plugins/managed/`, `KIMI_PLUGIN_ROOT`): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/plugins.html
- Hooks (`event` / `matcher` / `command` / `timeout`, event names, stdin, exit codes): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/hooks.html
- MCP (`mcpServers.<name>.command` / `args`): https://www.kimi.com/code/docs/en/kimi-code-cli/customization/mcp.html
- Desktop (Settings → Plugins; settings shared with the CLI): https://www.kimi.com/code/docs/en/kimi-code-desktop/using-desktop.html
