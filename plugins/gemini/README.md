# rtok Gemini CLI extension

One extension directory for Gemini CLI (`gemini`): rtok's hooks and its MCP server as one
unit (D21: plugin and MCP together, one `rtok mcp` per store). Gemini extensions carry a
`gemini-extension.json` manifest plus their own `hooks/hooks.json` — the same
`{hooks: {<Event>: [...]}}` shape `~/.gemini/settings.json` uses
(`src/agents/gemini/mod.rs`, T118.2) — with no `trust` field under `mcpServers` (the
extension manifest does not support it; only `settings.json` does).

Install:

- `gemini extensions link <path to this folder>` — the dev-style local link Gemini
  documents; changes to this folder apply immediately, no reinstall. `rtok agents install
  gemini --yes` runs exactly this line through the `gemini` CLI.
- Remove: `gemini extensions uninstall rtok` (the manifest's `name`, not the path).

`rtok` must be on `PATH` (`ketch install listepo/rtok`); every hook fails open when it is
missing and the MCP server does not start.

D21 singleton: use the extension **or** `rtok agents install gemini`, not both. The
installer writes rtok's own hook entries and `mcpServers.rtok` into `~/.gemini/settings.json`
only while this extension is absent; with the extension linked/installed it takes those back
instead of firing every event twice and running two `rtok mcp` processes on one store.

Files:

- `gemini-extension.json` — `name`, `version`, `description`, `mcpServers.rtok` (`command`,
  `args`); no `trust`.
- `hooks/hooks.json` — `rtok hook <ClaudeEvent> --host gemini` on Gemini's own event names
  (`BeforeTool`, `AfterTool`, `BeforeAgent`, `SessionStart`, `SessionEnd`, `PreCompress`), no
  `matcher` — a Claude-shaped tool-name matcher would never match Gemini's own tool names, so
  every call fires instead, the same reasoning `settings.json` writes under. Built from the
  same `src/agents/gemini::EVENTS` table `settings.json` merges from
  (`gemini::hooks_doc`) — one map, two surfaces — and pinned by `tests/gemini_plugin.rs`.

Not verified on a live install: the exact folder name `gemini extensions link` gives the
linked extension under `~/.gemini/extensions/` (the installer's `plugin_installed` check
scans every entry's manifest instead of assuming a fixed name).

## Docs

Host documentation this extension is written against. Re-check every link when it changes.
Fetched 2026-09-24.

- Extensions reference (`gemini-extension.json` fields, `hooks/hooks.json` location, `~/.gemini/extensions/`, `gemini extensions link/install/uninstall`): https://geminicli.com/docs/extensions/reference/
- Hooks reference (event names, the `hooks.<Event>[]`/`matcher`/`hooks`/`type`/`command` shape `hooks/hooks.json` reuses): https://geminicli.com/docs/hooks/reference/

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- Installer and surface table: [`../../src/agents/gemini/README.md`](../../src/agents/gemini/README.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
