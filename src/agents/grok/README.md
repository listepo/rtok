# Grok Build — `rtok agents install grok`

Grok Build (xAI's `grok` CLI) reads Claude-style plugin directories, so rtok's hooks and its
MCP server install as one plugin (`plugins/grok`, plan T99). Grok owns its plugin store —
`rtok` never writes `~/.grok/plugins/`; it prints the exact line:

```
grok plugin install <resolved plugins/grok> --trust
```

`--trust` is required: without it Grok leaves the hooks and MCP servers inactive. Remove with
`grok plugin uninstall rtok`. Alternative install: copy the folder to `~/.grok/plugins/rtok/`
(auto-trusted) and list `rtok` in `[plugins].enabled` in `~/.grok/config.toml`.

Without the plugin, `rtok agents install grok` writes `[mcp_servers.rtok]` into
`~/.grok/config.toml` — only while the plugin is absent (D21). The same singleton rule covers
Grok's Claude import: Grok fires hooks from `~/.claude/settings.json` and MCP servers from
`~/.claude.json` while `[compat.claude]` `hooks`/`mcps` are on (the default), so while rtok's
Claude install already serves a capability, setup says so instead of adding a second set —
two sets would fire every event twice and run two `rtok mcp` processes on one store.

The Read path stays Grok's own for now: Grok's file read is `read_file`, and Grok blocks a call
whose `updatedInput` fails the tool's schema, so the `read_file` → `Read` mapping lands only
after a live payload confirms the shape (plan T100).

The plugin is macOS/Linux only: each hook resolves `rtok` from `PATH`, then
`~/.ketch/bin/rtok`, else exits 0 silently (fail open; `ketch install listepo/rtok` fixes a
missing binary). Grok runs the same command through PowerShell on Windows, where that shell
one-liner does not work — Windows users should run `rtok agents install claude` instead (Grok
imports Claude's hooks, see above).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | no | Grok fires one hook set — the plugin's, or rtok's Claude hooks through [compat.claude] hooks — and setup adds no second (D21) |
| mcp | yes | `[mcp_servers.rtok]` → `rtok mcp` in `config.toml`, Grok's documented shape, only while the plugin is absent (off with `[setup] mcp = false`) |
| plugin | `--yes` | install prints `grok plugin install <resolved plugins/grok> --trust` behind the flag — rtok never writes `~/.grok/plugins/`, that store is Grok's. While the plugin is installed (`~/.grok/plugins/rtok/` exists), setup takes back `[mcp_servers.rtok]` instead of adding it (D21 singleton: the plugin is the hooks and the MCP as one unit) |
| proxy | no | Grok Build providers live in its own settings tables; setup does not edit them |

## rtok plugins this host reaches

The plugin carries `hook` and `mcp`; the MCP table carries `mcp`. Nothing carries `proxy`.

Reachable: cmd, read, archive, inject, guard, memory, graph, toon
Not reachable: measure, proxy, compress

## Docs

Host documentation this installer is written against. Re-check every link when it changes.
Grok also ships the same guides in `~/.grok/docs/user-guide/` (`09-plugins.md`, `10-hooks.md`).

- Plugins (layout, `.grok-plugin/`, `hooks/hooks.json`, `.mcp.json`, `grok plugin install --trust`, `~/.grok/plugins/`, `GROK_PLUGIN_ROOT`): https://docs.x.ai/build/features/skills-plugins-marketplaces
- Hooks (events, `matcher` regex, stdin envelope, `hookSpecificOutput`, exit codes, `GROK_HOOK_EVENT`): https://docs.x.ai/build/features/hooks
- MCP servers (`.mcp.json`, `[mcp_servers.<name>]`, Claude/Cursor imports): https://docs.x.ai/build/features/mcp-servers
- Settings (`[compat.claude]`, `[plugins]`, `GROK_HOME`): https://docs.x.ai/build/settings/reference
