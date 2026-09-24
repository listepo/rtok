# Claude

`rtok agents install claude` — the Claude Code CLI (`claude`) and the Claude Desktop app. They
keep separate files, so each selected app installs on its own (`--cli` / `--desktop`).

- CLI: `~/.claude/settings.json` (hooks, proxy) and `~/.claude.json` (MCP). By default, once
  `claude` is on PATH, the plugin (`plugins/claude`, from the GitHub marketplace `listepo/rtok`)
  carries hooks and MCP instead; its installed state is read from
  `~/.claude/plugins/installed_plugins.json`.
- Desktop: `claude_desktop_config.json` under `~/Library/Application Support/Claude` on
  macOS, `%APPDATA%\Claude` on Windows, `~/.config/Claude` elsewhere. The app starts without a
  shell PATH, so the MCP entry carries the absolute `rtok` binary.

Every file is copied to `_backup/<name>.bak-<ts>` before the first write; an unchanged file is not
copied twice.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `rtok hook <event>` on PreToolUse (Bash, Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` (off with `[setup] mcp = false`) |
| proxy | `--proxy` | `env.ANTHROPIC_BASE_URL` → `http://<bind>:<port>`; opt-in because it routes every request through `rtok proxy` |
| plugin | yes | runs `claude plugin marketplace add listepo/rtok` (skipped once Claude already knows the `rtok` marketplace) and `claude plugin install rtok@rtok` (remove: `uninstall` + `marketplace remove`); installed by default once `claude` is on PATH — no `--yes` needed; Claude Code loads it in the CLI and the desktop Code tab; while it is installed it is the only call path, so setup strips its own settings-file hooks and `mcpServers.rtok`; a missing or failing `claude` leaves the offer open instead of failing the install; `rtok agents update` runs `claude plugin marketplace update rtok` + `claude plugin update rtok@rtok` and reinstalls (`uninstall` + `install`) only when that fails |
| hooks (desktop) | no | Claude Desktop has no hook events |
| mcp (desktop) | yes | `mcpServers.rtok` → `<abs rtok> mcp` in `claude_desktop_config.json`; skipped (and a leftover entry removed) while Claude Code serves rtok MCP — the plugin or `mcpServers.rtok` in `~/.claude.json` — because the desktop Code tab loads this file and those both (T243, T244) |
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

- Hooks (`~/.claude/settings.json`, event names incl. `PreCompact`, `PostCompact`, `SessionEnd`): https://code.claude.com/docs/en/hooks
- Settings (`env.ANTHROPIC_BASE_URL`): https://code.claude.com/docs/en/settings
- MCP (user scope in `~/.claude.json` `mcpServers`): https://code.claude.com/docs/en/mcp
- Skills (`~/.claude/skills/<name>/SKILL.md`): https://code.claude.com/docs/en/skills
- Claude Desktop (`claude_desktop_config.json` `mcpServers.<name>.command` / `args`): https://modelcontextprotocol.io/docs/develop/connect-local-servers
