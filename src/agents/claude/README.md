# Claude Code

`rtok agents setup claude` — the Claude Code CLI (`claude`).

Files: `~/.claude/settings.json` (hooks, proxy) and `~/.claude.json` (MCP). Both are copied to
`<name>.bak-<ts>` before the first write; an unchanged file is not copied twice.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `rtok hook <event>` on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` (off with `[setup] mcp = false`) |
| proxy | `--proxy` | `env.ANTHROPIC_BASE_URL` → `http://<bind>:<port>`; opt-in because it routes every request through `rtok proxy` |
| plugin | no | Claude Code loads hooks and MCP from its own settings; there is no plugin directory to link |

`--replace` (with `--yes`) drops legacy token hooks (rtk, lean-ctx, caveman) and retargets the
proxy; see `migrate.rs`.

## rtok plugins this host reaches

- Through hooks: cmd, read, inject, guard, memory (hook half).
- Through MCP: read, archive, memory, graph, toon.
- Through the proxy: measure, archive, proxy, toon, compress.

`rtok agents setup claude` prints the split as installed / not installed / not supported from
each plugin's surfaces.
