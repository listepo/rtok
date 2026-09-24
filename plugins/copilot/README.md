# rtok GitHub Copilot CLI plugin

One plugin directory for Copilot CLI (`copilot`): rtok's hooks and its MCP server as one unit
(D21: plugin and MCP together, one `rtok mcp` per store). Copilot reads the legacy plugin
format — `plugin.json` at the plugin root pointing at `hooks/hooks.json` and `.mcp.json` —
with Copilot's own camelCase hook events, so this tree exists beside `plugins/claude` rather
than reusing it (the Claude hooks there speak `hook_event_name`, not `preToolUse`).

Install:

- `copilot plugin install <path to this folder>`, then start a new session. Copilot caches
  plugin components — re-run the install after changing this folder.
- Remove: `copilot plugin uninstall rtok` (the manifest's `name`, not the path).

`rtok` must be on `PATH` (`ketch install listepo/rtok`); the MCP server does not start without
it. Hooks are more forgiving (T250.2): each line resolves `rtok` from PATH, then
`~/.ketch/bin/rtok`, else fails open silently — except `sessionStart`, whose fallback prints
one flat `additionalContext` note naming `ketch install listepo/rtok`.

D21 singleton: use the plugin **or** `rtok agents install copilot`, not both. The installer
writes `~/.copilot/hooks/rtok.json` and `mcp-config.json` only while this plugin is absent;
with the plugin installed it takes those two back instead of firing every event twice and
running two `rtok mcp` processes on one store.

Files:

- `plugin.json` — legacy manifest: `name` plus the `hooks` and `mcpServers` component paths.
- `hooks/hooks.json` — the resolver line above running `hook <event> --host copilot` on
  `preToolUse`, `postToolUse`, `userPromptSubmitted`, `sessionStart`, `sessionEnd`,
  `preCompact`, each `timeoutSec: 5` (the same six events `~/.copilot/hooks/rtok.json` carries;
  checked by `tests/copilot_plugin.rs`).
- `.mcp.json` — `mcpServers.rtok` → `rtok mcp` (`{type: "local", command, args, tools}`).

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Creating a plugin (structure, `plugin.json`, `copilot plugin install ./my-plugin`, the `com.github.copilot` namespace): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating
- Plugin reference (legacy manifest fields, `hooks`/`mcpServers` paths, file locations, `marketplace.json`, `COPILOT_HOME`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference
- Configuration directory (`~/.copilot`, `mcp-config.json`, `hooks/*.json`, `installed-plugins/`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference
