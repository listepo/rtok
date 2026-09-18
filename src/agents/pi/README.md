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
| mcp | no | pi philosophy is no MCP; the extension calls rtok run and rtok filter directly |
| proxy | no | pi provider base URLs live in its models config, which setup does not edit |

## rtok plugins this host reaches

The extension owns the bash call path (`tool_call` → `rtok guard check` then bash → `rtok run -- …`, `tool_result` →
`rtok filter`) and the archive live zone without a proxy (`context` →
`rtok archive rewrite --stdin`, T70.2) — both `cli` surface. Hook, MCP and proxy surfaces
have no path in.

Reachable: measure, cmd, archive
Not reachable: read, proxy, inject, guard, memory, graph, toon, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Extensions (`~/.pi/agent/extensions/*.ts` or `*/index.ts`; `tool_call` `{block, reason}`, `tool_result`, `context`): https://pi.dev/docs/latest/extensions
- Packages (`package.json` `pi` key, `pi install <path>`): https://pi.dev/docs/latest/packages
- The linked bundle: `plugins/pi/README.md`
