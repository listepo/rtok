# AGENTS.md — `plugins/devin`

Agent rules for the **Devin** host package (Devin CLI and Devin Desktop). Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `.devin-plugin/plugin.json`, `hooks.json` (plugin root, events top level), `.mcp.json` |
| Hooks | Claude-shaped output → `rtok hook <event> --host devin` (input mapped by `HookInput::adapt_devin`) |
| MCP | `.mcp.json` → `rtok mcp` |
| Installer | Package README; `src/agents/devin/` is plan T89 |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update `tests/devin_plugin.rs` when changing the manifests.
- Use Devin's names: tools `exec`/`read`/`edit`/`write`, event `PostCompaction`.

## Do not

- Wrap `hooks.json` in a `"hooks"` key: a plugin's file is the events object itself.
- Add `PreCompact`: Devin has no such event.
- Dual-wire plugin and `rtok agents install claude` without the README's `read_config_from` note.

## Package-specific notes

- Hook commands are POSIX shell (PATH, then `~/.ketch/bin/rtok`, else exit 0 silently); macOS/Linux only.
- Devin blocks only on exit 2; rtok never exits 2 from a hook.
