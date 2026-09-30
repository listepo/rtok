# rtok ZCode plugin

Linked by `rtok agents install zcode` (by default, once ZCode itself is detected) to
`~/.zcode/cli/plugins/local/rtok`, which setup
also lists in `plugins.dirs` in `~/.zcode/cli/config.json` — ZCode loads every `plugins.dirs`
entry as an inline plugin root and enables it by default (D21: hooks). `rtok agents remove
zcode` unlinks it and drops the `plugins.dirs` entry. By hand: symlink this folder to
`~/.zcode/cli/plugins/local/rtok` and add that path to `plugins.dirs`. While the plugin is
linked it is the only call path for hooks: setup strips its own `hooks.events` entries from
`config.json`. The plugin no longer ships an MCP server (T275/D33): `rtok agents install
zcode` always writes `mcp.servers.rtok` into `~/.zcode/cli/config.json` itself, plugin linked
or not, so there is exactly one path to that entry instead of two copies to keep in sync.

Files:

- `.zcode-plugin/plugin.json` — ZCode manifest (`name` `rtok`); `hooks/hooks.json` is
  discovered by convention, so the manifest does not point at it.
- `hooks/hooks.json` — PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart →
  `rtok hook <event>` through `scripts/hook.sh` (`type: "process"`, `timeoutMs` 5000; the
  stdin/stdout protocol is Claude's, so no `--host` is needed).
- `scripts/hook.sh` — a launcher that resolves `rtok` from PATH or the ketch store; a
  missing `rtok` fails the hook open (exit 0), printing `ketch install pyrlyn/rtok`.

Windows: the hook launcher is POSIX, so the plugin cannot run there, but `rtok agents
install zcode` still links it by default (T164) — it has no way to know a launcher will
not run. `rtok agents remove zcode` unlinks it for the plain `config.json` hooks, which
write the absolute exe instead; a later `install` relinks the plugin again, since there
is no flag yet to keep it off.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (manifest `.zcode-plugin/plugin.json`, `hooks/hooks.json` by convention, `${ZCODE_PLUGIN_ROOT}`): https://zcode.z.ai/en/docs/plugin
- Hooks (`type: "process"` `command`/`args`/`timeoutMs`, matcher against the tool name, the seven events): https://zcode.z.ai/en/docs/hooks
- MCP (`mcp.servers` in the config file, written by `rtok agents install zcode` itself): https://zcode.z.ai/en/docs/mcp-services
- Configuration (`~/.zcode/cli/config.json`): https://zcode.z.ai/en/docs/configuration

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

