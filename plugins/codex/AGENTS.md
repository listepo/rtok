# AGENTS.md — `plugins/codex`

Agent rules for the **Codex** host package. Humans: [`README.md`](README.md). Shared rules: [`../AGENTS.md`](../AGENTS.md).

## Contracts

| Item | Location / rule |
| --- | --- |
| Manifest / entry | `.codex-plugin/plugin.json` → `hooks/hooks.json`, `.mcp.json` |
| Marketplace | `.agents/plugins/marketplace.json`, one local entry `rtok` at `./` |
| Hooks | Claude-shaped hooks → `rtok hook <event>`; same event set as `src/agents/codex/` (`COMPACT`) |
| MCP | `.mcp.json` → `rtok mcp` directly (I-37: no launcher scripts) |
| Installer | [`../../src/agents/codex/`](../../src/agents/codex/) writes the user files; it does not offer this plugin yet |
| Tests | `tests/codex_plugin.rs` |

## Do

- Keep the hook events equal to the installer's; change both and the test together.
- Keep README.md and AGENTS.md updated with behaviour changes.
- Call rtok CLIs instead of reimplementing policy.

## Do not

- Add `PreToolUse` / `PostToolUse` before a live Codex payload confirms rtok's rewrite and context shapes.
- Dual-wire plugin and `rtok agents install codex` without documenting the conflict.
