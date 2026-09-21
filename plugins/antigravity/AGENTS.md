# AGENTS.md — `plugins/antigravity`

Agent rules for the **Antigravity** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `plugin.json`, `mcp_config.json` |
| Hooks | None — host cannot rewrite tool I/O; use skill + MCP. |
| MCP | `mcp_config.json` → `rtok mcp` |
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

- Do not add hooks.json unless Antigravity gains rewrite/context APIs.
- Keep MCP server name `rtok`.
- No `src/agents/antigravity` module yet — install is in the package README.
