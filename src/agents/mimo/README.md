# MiMo Code

`rtok agents install mimo` — MiMo Code, Xiaomi's terminal coding agent (`mimo`), an OpenCode
fork. One file, `[setup.mimo] config_path` (default `~/.config/mimocode/mimocode.json`,
`MIMOCODE_HOME`/`MIMOCODE_CONFIG` move it), holds providers, models, tools, permissions and
MCP servers. No Desktop variant: MiMo Desktop exists (early access) but no config path for it
is documented.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| mcp | yes | `mcp.rtok` → `{type: "local", command: [rtok, mcp], enabled: true}` — the same local-server shape OpenCode kept from upstream (off with `[setup] mcp = false`) |
| hooks | no | MiMo Code has no shell hook events; like OpenCode, tool.execute.before/after run in-process through a linked plugin, which this task does not ship |
| proxy | no | MiMo Code has no documented base-URL override; MIMOCODE_HOME/MIMOCODE_CONFIG relocate config, not the model endpoint |
| plugin | no | MiMo's in-process plugin package (the @mimo-ai/plugin analogue of @opencode-ai/plugin) is undocumented; linking one here would be a guess, not a verified path |

`register_mcp`/`unregister_mcp` call `rtok-agent-sdk` through
[`super::register_local_mcp`](../mod.rs), the helper `opencode` was refactored onto for the
same JSON shape (T186) — no duplicated logic between the two forks.

## rtok plugins this host reaches

MCP carries `mcp`. Nothing carries `hook`, `cli`, or `proxy`.

Reachable: read, archive, memory, graph, toon
Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Repository and install (binary `mimo`, npm `@mimo-ai/cli`): https://github.com/XiaomiMiMo/MiMo-Code
- Docs home: https://mimo.xiaomi.com/mimocode/start
- Config files (`~/.config/mimocode/mimocode.json`, `MIMOCODE_HOME`/`MIMOCODE_CONFIG`, project `.mimocode/mimocode.json`): https://mimo.xiaomi.com/mimocode/config-files
- MCP servers (`mcp.<name>`, `type: "local"`/`"remote"`, `command`/`args`/`enabled`): https://mimo.xiaomi.com/mimocode/mcp-servers
- Config overrides (merge order, project vs. global): https://mimo.xiaomi.com/mimocode/config-overrides
- Environment variables (no base-URL override documented): https://mimo.xiaomi.com/mimocode/env-vars
