# rtok Codex plugin

One plugin directory for Codex (the `codex` CLI and the Codex desktop app): rtok's hooks and its
MCP server as one unit (D21: plugin and MCP together, one `rtok mcp` per store). The layout is
Codex's `.codex-plugin/plugin.json` manifest pointing at `hooks/hooks.json` and `.mcp.json`.

Install:

- `rtok agents install codex` — the automated path (T140): once `codex` is on PATH it runs
  `codex plugin marketplace add listepo/rtok` against this repo's root marketplace
  (`.agents/plugins/marketplace.json`, one entry `rtok` at `./plugins/codex`), then
  `codex plugin add rtok@rtok`, by default — no flag needed. `rtok agents remove codex` reverses
  it (`codex plugin remove rtok@rtok` then `codex plugin marketplace remove rtok`). A missing or
  failing `codex` fails open onto the file-based hooks/MCP surfaces below instead of failing the
  install.
- By hand, from a local checkout: `codex plugin marketplace add <path to this folder>` — the
  folder is also its own local marketplace (same file, one entry `rtok` at `./`) — then
  `codex plugin add rtok@rtok`. The plugin is enabled on install; Codex asks to trust its hooks
  before they run. Remove: `codex plugin remove rtok@rtok`, then
  `codex plugin marketplace remove rtok`.

The hooks resolve `rtok` from `PATH`, then `~/.ketch/bin/rtok`, else exit 0 silently (Codex only
blocks on an explicit decision) — so a hook shell whose `PATH` lacks ketch's install dir still
finds `rtok`. `commandWindows` keeps the bare `rtok hook <event>` for `cmd.exe`, which cannot run
the POSIX fallback. The MCP server still needs `rtok` on `PATH` and does not start without it.
Install with ketch: `ketch install listepo/rtok`.

Use the plugin **or** `rtok agents install codex`, not both. That installer writes the same two
hooks to `~/.codex/hooks.json` and `[mcp_servers.rtok]` to `~/.codex/config.toml`; with both in
place every event fires twice and two `rtok mcp` processes share one store. Run
`rtok agents remove codex` before enabling the plugin (keep `--proxy` separately if you use it).

Files:

- `.codex-plugin/plugin.json` — manifest (name, version, metadata, `hooks` and `mcpServers` paths).
- `hooks/hooks.json` — `PreCompact` and `PostCompact`, each `timeout: 5`, resolving `rtok` from
  `PATH` then `~/.ketch/bin/rtok` (`commandWindows` bare for `cmd.exe`): the same events
  `rtok agents install codex` registers. Checked by `tests/codex_plugin.rs`.
- `.mcp.json` — `mcpServers.rtok` → `rtok mcp`.
- `.agents/plugins/marketplace.json` — the local marketplace that lists this folder.

Known limits:

- Only the compaction hooks. Codex's `PreToolUse` can rewrite a Bash command (`updatedInput`) and
  `PostToolUse` can add context, but rtok's handling of Codex's tool payloads has not been checked
  on a live session, so those events stay off here and in the installer until it is.
- Verified on codex-cli 0.155.1 in a scratch `CODEX_HOME` (2026-09-21): `marketplace add` accepts
  the `./` entry, `plugin add` copies the tree to `plugins/cache/rtok/rtok/0.0.1/` and enables it,
  and `codex mcp list` shows `rtok` → `rtok mcp`. Not verified: the hooks firing in a live session
  (trust prompt) and the working directory Codex gives a plugin's stdio MCP server.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (layout, `.codex-plugin/plugin.json`, `.mcp.json`, `hooks/hooks.json`, marketplaces, `codex plugin marketplace add`): https://developers.openai.com/plugins/build/plugins
- Hooks (events, `matcher`, `timeout` in seconds, plugin-bundled hooks and trust review, `PLUGIN_ROOT`): https://learn.chatgpt.com/docs/hooks
- MCP (`[mcp_servers.<name>]`, stdio servers): https://learn.chatgpt.com/docs/extend/mcp
- Config reference (`~/.codex/config.toml`): https://learn.chatgpt.com/docs/config-file/config-reference

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- Installer and surface table: [`../../src/agents/codex/README.md`](../../src/agents/codex/README.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
