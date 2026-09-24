# AGENTS.md — `plugins/gemini`

Agent rules for the **Gemini CLI** host package. Humans: [`README.md`](README.md). Shared
rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `gemini-extension.json` → `name`, `version`, `description`, `mcpServers.rtok` (no `trust`) |
| Hooks | `hooks/hooks.json`, Gemini's own event names → `rtok hook <event> --host gemini`; same set as `src/agents/gemini::EVENTS`, built by `gemini::hooks_doc` — do not hand-edit the file out of step with that table |
| MCP | `mcpServers.rtok` embedded in `gemini-extension.json` directly (no separate `.mcp.json`) |
| Installer | [`../../src/agents/gemini/`](../../src/agents/gemini/) offers `gemini extensions link <this dir>` behind `--yes`; D21 strips `settings.json`'s own hook entries and `mcpServers.rtok` while this extension is linked |
| Tests | `tests/gemini_plugin.rs`, `tests/host_docs.rs` |

## Do

- Keep `hooks/hooks.json` derived from `src/agents/gemini::EVENTS` — change the table, then
  regenerate the file's expectations in the same commit (`tests/gemini_plugin.rs` pins it via
  `gemini::hooks_doc`).
- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.

## Do not

- Add a `matcher` to `hooks/hooks.json` entries — Gemini's own tool-name spelling
  (`run_shell_command`, …) never matches a Claude-shaped matcher; every event must keep
  firing on every call.
- Add a `skills/` directory here — skills live only in the repo's `skills/`, installers copy
  them, plugins never bundle one (T234).
- Dual-wire the extension and `rtok agents install gemini` without documenting the conflict.
