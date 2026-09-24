# Gemini CLI

`rtok agents install gemini` — Google's Gemini CLI. One config file,
`[setup.gemini] dir` (default `~/.gemini`), carries both hooks and MCP in
`settings.json`. No separate desktop app.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `hooks.<Event>[]` → `{hooks: [{type: "command", command: "rtok hook <Event> --host gemini", timeout}]}` on BeforeTool, AfterTool, BeforeAgent, SessionStart, SessionEnd, PreCompress — no `matcher`, so every call fires (a Claude-shaped tool-name matcher would never match Gemini's own tool names) |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `settings.json` as `{command, args}` (off with `[setup] mcp = false`) |
| proxy | no | Gemini CLI has no documented base-URL setting; its own HTTP_PROXY/HTTPS_PROXY covers MCP server transport only, not the model API |
| plugin | `--yes` | `gemini extensions link <plugins/gemini>` (T118.3); D21 — while linked, `hooks`/`mcp` above go instead of coming |

`--host gemini` (T118.1) maps Gemini's own event names to Claude's before the shared
plugins run (`BeforeTool`→`PreToolUse`, `AfterTool`→`PostToolUse`, `BeforeAgent`→
`UserPromptSubmit`, `PreCompress`→`PreCompact`, `SessionStart`/`SessionEnd` unchanged) and
shapes the reply back (`{decision: "deny", reason}` to block, `{hookSpecificOutput: …}` to
rewrite or add context). `AfterAgent`, `BeforeModel`, `BeforeToolSelection`, `AfterModel` and
`Notification` have no rtok plugin behind them and are left uninstalled. Not verified on a
live install: the `gemini` binary name and `--version` output (docs do not show either
explicitly), and whether hooks fire from a fresh install with no prior `gemini` run.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Settings (`~/.gemini/settings.json`, user vs. workspace, `GEMINI_CLI_HOME`): https://geminicli.com/docs/cli/settings/
- Hooks (`hooks.<Event>[]`, event names, `matcher`/`hooks`/`type`/`command`/`timeout` shape): https://geminicli.com/docs/hooks/reference/
- MCP (`mcpServers.<name>`, `command`/`args`/`env`/`timeout`/`trust`): https://geminicli.com/docs/tools/mcp-server/
- Enterprise/base-URL configuration (no proxy-able model endpoint): https://geminicli.com/docs/cli/enterprise/
- Extensions (`gemini-extension.json` fields, `hooks/hooks.json`, `~/.gemini/extensions/`, `gemini extensions link/install/uninstall`, T118.3, fetched 2026-09-24): https://geminicli.com/docs/extensions/reference/
