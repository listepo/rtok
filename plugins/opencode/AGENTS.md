# AGENTS.md — `plugins/opencode`

Agent rules for the **OpenCode (and Kilo)** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `rtok.ts` (+ `rtok.test.ts`) |
| Hooks | TS events → `guard check` / `hook` / `filter` / compact (`--host opencode`) |
| MCP | Usually host MCP from installer; plugin focuses on tool execute hooks |
| Installer | [`../src/agents/opencode/README.md`](../src/agents/opencode/README.md) |
| Root human guide | [`../README.md`](../README.md) |

## Do

- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.
- Update package tests when changing TS or manifests.

## Do not

- Break host-required paths without updating install code and tests.
- Dual-wire plugin and agents install without documenting the conflict.

## Package-specific notes

- Kilo reuses this file; rows labelled `opencode` until a host tag exists.
- Keep fail-open on missing/unparsable rtok.
