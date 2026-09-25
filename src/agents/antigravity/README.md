# Antigravity

`rtok agents install antigravity` — Antigravity CLI (`agy`) and the desktop apps (Antigravity 2.0,
Antigravity IDE). The plugin is the only unit (D21, T90): `plugins/antigravity` carries rtok's MCP
(`mcp_config.json`) and no hooks. No config file is edited, so there is nothing to back up.

- **Desktop:** `--yes` links `<plugins_path>/rtok` (default `~/.gemini/config/plugins/rtok`) →
  `plugins/antigravity/` from the rtok install; `remove` unlinks exactly that link and leaves a
  foreign tree alone. Not by default: whether the desktop loader follows a symlinked plugin
  directory is undocumented.
- **CLI:** `--yes` prints `agy plugin install <resolved plugins/antigravity path>` and never runs
  it — `agy` stages its own copy under `<cli_plugins_path>/rtok` (default
  `~/.gemini/antigravity-cli/plugins/rtok`), which rtok only reads (`plugin.json` naming `rtok`).
  `remove` keeps that copy and names `agy plugin uninstall rtok`.
- **Skills:** every install copies the hub skills (`skills/rtok`, `skills/worktrees`) into the
  variant's own global root — desktop `~/.gemini/config/skills/`, CLI
  `~/.gemini/antigravity-cli/skills/` (beside `plugins_path` / `cli_plugins_path`); `remove`
  takes back only rtok-marked copies (T91.2).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| plugin | `--yes` | desktop: links `plugins/antigravity`; CLI: prints the `agy plugin install` line (the store is `agy`'s own) |
| hooks | no | Antigravity's PreToolUse cannot rewrite tool input and PostToolUse cannot add context |
| mcp | no | rtok's MCP ships inside the plugin, the only install path |
| proxy | no | Antigravity documents no model base-URL override |

## rtok plugins this host reaches

Only the plugin's MCP server (`rtok mcp`): the `mcp` surface. Hook, CLI and proxy surfaces have
no path in.

Reachable: read, archive, memory, graph, toon
Not reachable: measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Plugins (`plugin.json`, `mcp_config.json`; CLI `agy plugin install <path>` / `agy plugin uninstall <name>` staging into `~/.gemini/antigravity-cli/plugins/<name>/`; desktop global root `~/.gemini/config/plugins/`): https://antigravity.google/docs/plugins
- Hooks (`PreToolUse` cannot rewrite input, `PostToolUse` cannot add context): https://antigravity.google/docs/hooks/
- The plugin bundle: `plugins/antigravity/README.md`
