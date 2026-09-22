# AGENTS.md — `plugins/pi`

Agent rules for the **pi** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `package.json`, `extensions/rtok.ts`, `skills/rtok/SKILL.md`, `skills/worktrees/SKILL.md` (copy of the hub skill, T155) |
| Hooks | Extension events — not hooks.json |
| MCP | Optional via `pi.registerTool` when `[setup.pi] tools = true` |
| Installer | [`../src/agents/pi/README.md`](../src/agents/pi/README.md) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- D21: one bash call path; no default MCP.
- Keep tests outside `extensions/`.
