# AGENTS.md — `plugins/kimi`

Agent rules for the **Kimi Code** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `kimi.plugin.json` only |
| Hooks | Events in `kimi.plugin.json` → `rtok hook <event>` |
| MCP | none in the plugin (T275/D33): `mcpServers.rtok` is written by `rtok agents install kimi` into `mcp.json` directly, plugin installed or not |
| Installer | [`../src/agents/kimi/README.md`](../src/agents/kimi/README.md) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- Keep event list equal to `rtok agents install kimi` (parity test).
- Do not dual-install plugin + installer hooks.
