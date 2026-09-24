# AGENTS.md — `plugins/grok`

Agent rules for the **Grok Build** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `.grok-plugin/plugin.json`, `hooks/hooks.json`, `.mcp.json` |
| Hooks | Claude-style hooks → `rtok hook` (envelope via `GROK_HOOK_EVENT`) |
| MCP | `.mcp.json` → `rtok mcp` |
| Installer | Package README (no `src/agents` module yet) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- Pick plugin XOR `rtok agents install claude` (double-fire risk).
- Needs `--trust` / enable in config.
- `src/agents/grok/` only offers `grok plugin install`; Grok owns the plugin store.
- Hook commands are POSIX shell (PATH, then `~/.ketch/bin/rtok`, else exit 0 silently); Grok
  runs them through PowerShell on Windows, so the plugin is macOS/Linux only (T250.4).
