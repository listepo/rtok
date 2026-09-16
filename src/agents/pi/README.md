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

The extension owns the bash call path (`tool_call` bash → `rtok run -- …`, `tool_result` →
`rtok filter`), which is the `cli` surface. Hook, MCP and proxy surfaces have no path in.

Reachable: measure, cmd
Not reachable: read, archive, proxy, inject, guard, memory, graph, toon, compress
