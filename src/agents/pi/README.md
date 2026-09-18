# pi

`rtok agents install pi` — the pi coding agent (`pi`). Desktop and CLI read the same
`~/.pi/agent/extensions` tree.

No config file is edited: the install is one linked extension, `<extensions_path>/rtok` →
`plugins/pi/` from the rtok install (D21). Nothing to back up; `remove` unlinks.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| plugin | `--yes` | the offer links `plugins/pi`; without a terminal only `--yes` accepts |
| hooks | no | pi has no hook events; the extension owns the bash call path |
| mcp | no | pi philosophy is no MCP; `[setup.pi] tools` registers the measured set through `pi.registerTool` |
| proxy | no | pi provider base URLs live in its models config, which setup does not edit |

## rtok plugins this host reaches

The extension owns the bash call path (`tool_call` → `rtok guard check` then bash → `rtok run -- …`,
`tool_result` bash → `rtok filter`), pi's file/search tools (`tool_result` read/grep/find/ls →
`rtok filter --stdin --cmd "<tool> <path-or-pattern>"`, T70.1), and the archive live zone
without a proxy (`context` → `rtok archive rewrite --stdin`, T70.2) — all `cli` surface.
When `[setup.pi] tools = true`, `session_start` registers the measured MCP set through
`pi.registerTool` as `rtok mcp --call` (T70.3; 138 description tokens: read 17, search 12,
tree 12, symbol 30, callers 27, expand 22, mem_search 11, mem_get 7). Hook and proxy surfaces
have no path in. `toon` declares MCP but is not registered.

Reachable: measure, cmd, archive, guard, read, memory, graph
Not reachable: proxy, inject, toon, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Extensions (`~/.pi/agent/extensions/*.ts` or `*/index.ts`; `tool_call` `{block, reason}`, `tool_result`, `context`, `registerTool`): https://pi.dev/docs/latest/extensions
- Packages (`package.json` `pi` key, `pi install <path>`): https://pi.dev/docs/latest/packages
- The linked bundle: `plugins/pi/README.md`
