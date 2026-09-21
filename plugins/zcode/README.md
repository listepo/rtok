# rtok ZCode plugin

Linked by `rtok agents install zcode --yes` to `~/.zcode/cli/plugins/local/rtok`, which setup
also lists in `plugins.dirs` in `~/.zcode/cli/config.json` — ZCode loads every `plugins.dirs`
entry as an inline plugin root and enables it by default (D21: hooks and MCP as one unit, one
`rtok mcp` per store). `rtok agents remove zcode` unlinks it and drops the `plugins.dirs`
entry. By hand: symlink this folder to `~/.zcode/cli/plugins/local/rtok` and add that path to
`plugins.dirs`. While the plugin is linked it is the only call path: setup strips its own
`hooks.events` entries and `mcp.servers.rtok` from `config.json`.

Files:

- `.zcode-plugin/plugin.json` — ZCode manifest (`name` `rtok`); `hooks/hooks.json` and
  `.mcp.json` are discovered by convention, so the manifest does not point at them.
- `hooks/hooks.json` — PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart →
  `rtok hook <event>` through `scripts/hook.sh` (`type: "process"`, `timeoutMs` 5000; the
  stdin/stdout protocol is Claude's, so no `--host` is needed).
- `.mcp.json` — `mcpServers.rtok` → `scripts/mcp.sh` (`${ZCODE_PLUGIN_ROOT}` is expanded for
  plugin-provided servers; config-file servers get no expansion, which is why the config-file
  install writes the absolute binary instead).
- `scripts/hook.sh`, `scripts/mcp.sh`, `scripts/mcp.cmd` — launchers that resolve `rtok` from
  PATH or the ketch store; a missing `rtok` fails the hook open (exit 0) and the MCP loudly
  (exit 1), both printing `ketch install listepo/rtok`.

Windows: the hook launchers are POSIX, so on Windows prefer the plain
`rtok agents install zcode` (no `--yes`), which writes the absolute exe into `config.json`.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `.zcode-plugin/plugin.json`, `hooks/hooks.json` and `.mcp.json` by convention, `${ZCODE_PLUGIN_ROOT}`): https://zcode.z.ai/en/docs/plugin
- Hooks (`type: "process"` `command`/`args`/`timeoutMs`, matcher against the tool name, the seven events): https://zcode.z.ai/en/docs/hooks
- MCP (plugin `.mcp.json`, strict server schema, `mcp.servers` in the config file): https://zcode.z.ai/en/docs/mcp-services
- Configuration (`~/.zcode/cli/config.json`): https://zcode.z.ai/en/docs/configuration
