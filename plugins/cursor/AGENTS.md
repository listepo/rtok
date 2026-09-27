# AGENTS.md — `plugins/cursor`

Agent rules for the **Cursor** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `.cursor-plugin/plugin.json`, root `plugin.json`, `hooks/hooks.json` |
| Hooks | `hooks/hooks.json` → `rtok hook … --host cursor` |
| MCP | not shipped here (T275/D33): `rtok agents install cursor` writes `~/.cursor/mcp.json`'s `mcpServers.rtok` independently, plugin linked or not |
| Installer | [`../src/agents/cursor/README.md`](../src/agents/cursor/README.md) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- Preserve Cursor hook event names and `updated_mcp_tool_output`.
- Install via `rtok agents install cursor` → `~/.cursor/plugins/local/rtok`.
