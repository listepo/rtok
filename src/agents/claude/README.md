# Claude

`rtok agents install claude` — the Claude Code CLI (`claude`) and the Claude Desktop app. They
keep separate files, so each selected app installs on its own (`--cli` / `--desktop`).

- CLI: `~/.claude/settings.json` (hooks, proxy) and `~/.claude.json` (MCP). By default, once
  `claude` is on PATH, the plugin (`plugins/claude`, from the GitHub marketplace `listepo/rtok`)
  carries hooks instead; its installed state is read from
  `~/.claude/plugins/installed_plugins.json`. MCP is independent of the plugin (T275): install
  and update always write `mcpServers.rtok` to `~/.claude.json`, plugin or no plugin.
- Desktop: `claude_desktop_config.json` under `~/Library/Application Support/Claude` on
  macOS, `%APPDATA%\Claude` on Windows, `~/.config/Claude` elsewhere. The app starts without a
  shell PATH, so the MCP entry carries the absolute `rtok` binary.

Every file is copied to `_backup/<name>.bak-<ts>` before the first write; an unchanged file is not
copied twice.

Headless ping (`rtok mcp ping claude --cli`): `claude -p "<prompt>"`. `--print` / `-p` prints
the response and exits (https://code.claude.com/docs/en/cli-reference, checked 2026-09-27).
`--desktop` has no headless prompt; ping reads `claude_desktop_config.json` and prints the
prompt to paste.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `rtok hook <event>` on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd, SubagentStart (the spawn brief, T130; inert while `[plugins.memory] spawn_brief` is off); the plugin alone also routes WorktreeCreate and WorktreeRemove through `rtok worktree` (T159) |
| mcp | yes | `mcpServers.rtok` → `rtok mcp --host claude` (off with `[setup] mcp = false`); always written on install/update, plugin or not — only `remove` takes it out (T275) |
| proxy | `--proxy` | `env.ANTHROPIC_BASE_URL` → `http://<bind>:<port>`; opt-in because it routes every request through `rtok proxy` |
| plugin | yes | runs `claude plugin marketplace add listepo/rtok` (skipped once Claude already knows the `rtok` marketplace) and `claude plugin install rtok@rtok` (remove: `uninstall` + `marketplace remove`); installed by default once `claude` is on PATH — no `--yes` needed; Claude Code loads it in the CLI and the desktop Code tab; while it is installed it is the only call path for hooks, so setup strips its own settings-file hooks — `mcpServers.rtok` is unaffected, the plugin carries no MCP server of its own (T275); a missing or failing `claude` leaves the offer open instead of failing the install; `rtok agents update` runs `claude plugin marketplace update rtok` + `claude plugin update rtok@rtok` and reinstalls (`uninstall` + `install`) only when that fails |
| hooks (desktop) | no | Claude Desktop has no hook events |
| mcp (desktop) | yes | `mcpServers.rtok` → `<abs rtok> mcp` in `claude_desktop_config.json`; always written on install/update, whether or not Claude Code's plugin or `~/.claude.json` also serves rtok — the two surfaces are independent (T275 amends T243/T244) |
| proxy (desktop) | no | Claude Desktop has no base-URL setting; its requests do not pass through the proxy |
| plugin (desktop) | no | Claude Desktop loads MCP from claude_desktop_config.json; there is no plugin directory to link |

`--replace` (with `--yes`) drops legacy token hooks (rtk, lean-ctx, caveman) and retargets the
proxy; see `migrate.rs`. On the desktop it is a plain install.

## rtok plugins this host reaches

Each plugin declares its surfaces (hook, mcp, proxy, cli); hooks carry `hook` and `cli`, MCP
carries `mcp`, the proxy carries `proxy`. `rtok agents install claude` prints the split as
installed / not installed / not supported.

Reachable (cli): measure, cmd, read, archive, proxy, inject, guard, memory, graph, toon, compress
Not reachable (cli): -

Reachable (desktop): read, archive, memory, graph, toon
Not reachable (desktop): measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Hooks (`~/.claude/settings.json`, event names incl. `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`): https://code.claude.com/docs/en/hooks
- Hooks, `WorktreeCreate` / `WorktreeRemove` (plugin only: input, path output, exit codes): https://code.claude.com/docs/en/hooks#worktreecreate
- Settings (`env.ANTHROPIC_BASE_URL`): https://code.claude.com/docs/en/settings
- MCP (user scope in `~/.claude.json` `mcpServers`): https://code.claude.com/docs/en/mcp
- Skills (`~/.claude/skills/<name>/SKILL.md`): https://code.claude.com/docs/en/skills
- Claude Desktop (`claude_desktop_config.json` `mcpServers.<name>.command` / `args`): https://modelcontextprotocol.io/docs/develop/connect-local-servers
- Non-interactive prompt (`claude -p "<prompt>"` queries, then exits; `rtok mcp ping claude --cli`): https://code.claude.com/docs/en/cli-reference
