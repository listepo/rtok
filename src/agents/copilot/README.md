# GitHub Copilot

`rtok agents install copilot` — GitHub Copilot CLI (`copilot`) and the GitHub Copilot desktop
app, which read one directory: `[setup.copilot] dir` (default `~/.copilot`, what
`$COPILOT_HOME` points at). Setup writes two files there: `mcp-config.json` is merged and every
other server survives; `hooks/rtok.json` is rtok's own file, since Copilot loads every
`hooks/*.json`, so no user file is touched and `remove` simply deletes it. `list` shows the CLI
and the app as one shared host, as it does for Cursor.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | yes | `hooks/rtok.json` runs `hook <Event> --host copilot` (`bash` and `powershell`, `timeoutSec`) on preToolUse, postToolUse, userPromptSubmitted, sessionStart, sessionEnd, preCompact, subagentStart (spawn brief as `additionalContext`); each line resolves `rtok` from PATH then `~/.ketch/bin/rtok`, else fails open |
| mcp | yes | `mcpServers.rtok` → `rtok mcp` in `mcp-config.json` as `{type: "local", command, args, tools: ["*"]}` (off with `[setup] mcp = false`) |
| proxy | no | Copilot BYOK is env-only (COPILOT_PROVIDER_BASE_URL); there is no config file to point at the proxy |
| plugin (cli) | `--yes` | install runs `copilot plugin install <resolved plugins/copilot>` behind the flag — rtok never writes `installed-plugins/`, that store is Copilot's. While the plugin is installed (a cached copy under `installed-plugins/` names `rtok`), setup takes back `hooks/rtok.json` and `mcpServers.rtok` instead of adding them (D21: the plugin is the hooks and the MCP as one unit) |
| plugin (desktop) | no | the GitHub Copilot app does not document plugin installs; `copilot plugin` serves the CLI |
| hooks (desktop) | no | the GitHub Copilot app does not document hooks; the shared hooks/rtok.json is written for the CLI |

Copilot's hook protocol is its own: camelCase stdin (`sessionId`, `cwd`, `toolName`,
`toolArgs`) and a flat stdout (`permissionDecision`, `permissionDecisionReason`,
`modifiedArgs`, `additionalContext`). `--host copilot` maps both ways (T46.3), so the plugins
see Claude's `Bash` and `Read` and answer in Copilot's shape. Copilot reads a non-zero exit on
preToolUse as deny; `rtok hook` keeps exiting 0 with `{}` on any error, so fail open holds.
Not verified on a live install: Copilot's tool ids (the rename to `Bash`/`Read` goes by
substring), the app bundle paths, and whether the app runs hooks at all.

The table row's resolver: `agents::hook_resolver` (bash), `agents::copilot::hook_resolver_ps`
(powershell, T250.2) — Copilot's only host with a `powershell` field, hence its own twin.

## rtok plugins this host reaches

Hooks carry `hook` and `cli`, MCP carries `mcp`. Nothing carries `proxy`. The app is
MCP-only until its hook support is documented.

Reachable (cli): measure, cmd, read, archive, inject, guard, memory, graph, toon
Not reachable (cli): proxy, compress

Reachable (desktop): read, archive, memory, graph, toon
Not reachable (desktop): measure, cmd, proxy, inject, guard, compress

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Config directory (`~/.copilot`, `$COPILOT_HOME`, `mcp-config.json`, `hooks/`, `installed-plugins/`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference
- MCP (`mcp-config.json` `mcpServers.<name>` with `type`, `command`, `args`, `tools`): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers
- Hooks (`hooks/*.json` shape, event names, stdin and stdout keys, exit codes): https://docs.github.com/en/copilot/reference/hooks-reference
- Plugins (`installed-plugins/`, `copilot plugin`): https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference
- BYOK (`COPILOT_PROVIDER_BASE_URL`, `providers.json`): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models
- Agent skills (`~/.copilot/skills/<name>/`): https://docs.github.com/en/copilot/concepts/agents/about-agent-skills
- GitHub Copilot app (reuses the CLI's MCP, skills and plugins): https://docs.github.com/en/copilot/how-tos/github-copilot-app/customize-github-copilot-app
