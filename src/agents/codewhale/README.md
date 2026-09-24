# CodeWhale

`rtok agents install codewhale` — CodeWhale, the open-source multi-provider TUI coding
agent (`codewhale`/`codew`, DeepSeek-lineage `DEEPSEEK_*` env vars kept for compatibility).
One directory, `[setup.codewhale] dir` (default `~/.codewhale`, `$CODEWHALE_HOME`), holds
`config.toml` (hooks) and the sibling `mcp.json` (MCP). No separate desktop app.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `[[hooks.hooks]]` → `{name, event: "message_submit", command: "rtok hook UserPromptSubmit --host codewhale", timeout_secs}` in `config.toml` — the only one of CodeWhale's 15 events that is both steering (its stdout can rewrite the payload) and carries real stdin JSON; the other 2 steering events (`tool_call_before`, `shell_env`) are env-var-only with no stdin, and the 12 observer events discard their hook's result outright |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp.json` as `{type: "stdio", command, args}` (off with `[setup] mcp = false`) |
| proxy | no | CodeWhale providers are [providers.<name>] tables with their own base_url and keys (docs/CONFIGURATION.md); setup does not edit them |
| plugin | no | CodeWhale's plugin-bundle system needs a reviewed remote source and exact environment-source references (docs/PLUGIN_BUNDLES.md); there is no local directory to link |

`--host codewhale` maps `message_submit`'s stdin (`text`) to Claude's `prompt` and sets
`hook_event_name` before the shared plugins run, then folds the reply's
`hookSpecificOutput.additionalContext` into a full `{"text": "..."}` replacement, since
`message_submit` has no separate "add context" field (`src/hooks/types.rs::adapt_codewhale`,
`src/hooks/mod.rs::codewhale_output`). `tool_call_before` — the natural PreToolUse analogue —
and the 7 stdin-JSON-bearing observer events (`turn_end`, `subagent_spawn`,
`subagent_complete`, `session_busy`, `session_idle`, `session_error`, `waiting_for_user`) are
left uninstalled: the former has no stdin JSON to adapt without changing the shared
stdin-parsing entry point every host relies on, and the latter can run a plugin but never
act on its result. Not verified on a live install: the `codewhale`/`codew` binary names and
`--version` output, and whether `message_submit` fires from a fresh install with no prior
`codewhale` run.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`.

Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: proxy, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Repository and general install: https://github.com/Hmbown/Codewhale
- Hooks (event table, steering vs. observer, stdin/stdout contracts): https://github.com/Hmbown/Codewhale/blob/main/docs/HOOKS.md
- MCP (`mcpServers.<name>`, `command`/`args`/`env`, `codewhale mcp add`): https://github.com/Hmbown/Codewhale/blob/main/docs/MCP.md
- Configuration (`~/.codewhale/config.toml`, `$CODEWHALE_HOME`, legacy `~/.deepseek` fallback): https://github.com/Hmbown/Codewhale/blob/main/docs/CONFIGURATION.md
- Plugin bundles (why no bundle ships in v1): https://github.com/Hmbown/Codewhale/blob/main/docs/PLUGIN_BUNDLES.md
- Project site: https://codewhale.net
