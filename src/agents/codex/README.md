# Codex

`rtok agents install codex` — the Codex CLI (`codex`).

Files: `~/.codex/config.toml` (MCP / proxy) and `~/.codex/hooks.json` (`PreCompact` / `PostCompact`).

## Modules

| Module | Support | Why |
| --- | --- | --- |
| mcp | yes | `[mcp_servers.rtok]` with `command`/`args` → `rtok mcp` |
| proxy | `--proxy` | `[model_providers.rtok]` with `base_url = http://<bind>:<port>/v1` and `model_provider = "rtok"` |
| hooks | yes | `hooks.json` `PreCompact` → `pre_compact`, `PostCompact` → `session_start` `source=compact` |
| plugin | yes | runs `codex plugin marketplace add listepo/rtok` (skipped once Codex already knows the `rtok` marketplace) and `codex plugin add rtok@rtok` (remove: `plugin remove` + `marketplace remove`); installed by default once `codex` is on PATH — no flag needed; while it is installed it is the only call path, so setup strips its own `[mcp_servers.rtok]` and `hooks.json`; a missing or failing `codex` leaves the offer open instead of failing the install |

## rtok plugins this host reaches

MCP carries the `mcp` surface, the proxy carries `proxy`, hooks carry `hook` / `cli`.

Reachable: measure, cmd, read, archive, proxy, inject, guard, memory, graph, toon, compress
Not reachable: -

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- Config reference (`~/.codex/config.toml`: `mcp_servers.<id>.command` / `args`, `model_providers.<id>.base_url`, top-level `model_provider`): https://learn.chatgpt.com/docs/config-file/config-reference
- MCP (`[mcp_servers.<name>]` example): https://learn.chatgpt.com/docs/extend/mcp
- Hooks (`hooks.json` / `[hooks]`: `PreCompact`, `PostCompact`, `transcript_path`): https://developers.openai.com/codex/hooks
- Skills (`~/.codex/skills/<name>/`): https://agentskills.io
