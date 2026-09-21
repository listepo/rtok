# AGENTS.md — `plugins/zcode`

Agent rules for the **ZCode** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `.zcode-plugin/plugin.json`, `hooks/hooks.json`, `.mcp.json` |
| Hooks | `scripts/hook.sh` → `rtok hook` (Claude I/O; no `--host`) |
| MCP | `scripts/mcp.sh` / `mcp.cmd` with `${ZCODE_PLUGIN_ROOT}` |
| Installer | [`../src/agents/zcode/README.md`](../src/agents/zcode/README.md) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- POSIX launchers; on Windows prefer config-file install without `--yes`.
- Do not drop `plugins.dirs` assumptions without updating installer.
