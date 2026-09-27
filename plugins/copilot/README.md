# rtok GitHub Copilot CLI plugin

One plugin directory for Copilot CLI (`copilot`): rtok's hooks. Copilot reads the legacy
plugin format — `plugin.json` at the plugin root pointing at `hooks/hooks.json` — with
Copilot's own camelCase hook events, so this tree exists beside `plugins/claude` rather than
reusing it (the Claude hooks there speak `hook_event_name`, not `preToolUse`). The plugin no
longer ships an MCP server (T275/D33): `rtok agents install copilot` always writes
`mcpServers.rtok` into `mcp-config.json` itself, plugin installed or not, so there is exactly
one path to that entry instead of two copies to keep in sync.

Install:

- `copilot plugin install <path to this folder>`, then start a new session. Copilot caches
  plugin components — re-run the install after changing this folder.
- Remove: `copilot plugin uninstall rtok` (the manifest's `name`, not the path).

`rtok` must be on `PATH` (`ketch install listepo/rtok`) for `mcp-config.json`'s `mcpServers.rtok`
entry to start (written by the installer, not this plugin). Hooks are more forgiving
(T250.2): each line resolves `rtok` from PATH, then `~/.ketch/bin/rtok`, else fails open
silently — except `sessionStart`, whose fallback prints one flat `additionalContext` note
naming `ketch install listepo/rtok`.

D21 singleton (hooks only): use the plugin **or** `rtok agents install copilot`'s hooks, not
both. The installer writes `~/.copilot/hooks/rtok.json` only while this plugin is absent; with
the plugin installed it takes that back instead of firing every event twice. `mcp-config.json`
is unaffected by which one is active.

Files:

- `plugin.json` — legacy manifest: `name` plus the `hooks` component path.
- `hooks/hooks.json` — the resolver line above running `hook <event> --host copilot` on
  `preToolUse`, `postToolUse`, `userPromptSubmitted`, `sessionStart`, `sessionEnd`,
  `preCompact`, `subagentStart` (the T130 spawn brief), each `timeoutSec: 5` (the same seven events `~/.copilot/hooks/rtok.json` carries;
  checked by `tests/copilot_plugin.rs`).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Creating a plugin (structure, `plugin.json`, `copilot plugin install ./my-plugin`, the `com.github.copilot` namespace): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating
- Plugin reference (legacy manifest fields, `hooks`/`mcpServers` paths, file locations, `marketplace.json`, `COPILOT_HOME`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference
- Configuration directory (`~/.copilot`, `mcp-config.json`, `hooks/*.json`, `installed-plugins/`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference
