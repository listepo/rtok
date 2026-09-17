# aider

`rtok agents install aider --proxy` — the aider CLI (`aider`), proxy-only.

File: `~/.aider.conf.yml`, edited line-wise so comments and every other key survive
(no YAML crate). Setup writes `openai-api-base: http://<bind>:<port>/v1`; remove drops
that line only when its value points at this proxy, so a foreign base URL stays.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| hooks | no | aider has no hook events; it reads .aider.conf.yml and .env |
| mcp | no | aider has no MCP support |
| proxy | `--proxy` | `openai-api-base` → `http://<bind>:<port>/v1`; Anthropic models use the same URL with an `openai/` model prefix |
| plugin | no | aider has no plugin directory to link |

## rtok plugins this host reaches

The proxy carries the `proxy` surface. Hook- and MCP-only plugins have no path in.
There is no `anthropic-api-base` in aider's options reference — only `openai-api-base`.

Reachable: measure, archive, proxy, toon, compress
Not reachable: cmd, read, inject, guard, memory, graph

## Docs

Host documentation setup writes against; re-check the links when this host changes.

- YAML config file (locations, `openai-api-base` sample): https://aider.chat/docs/config/aider_conf.html
- Options reference (`--openai-api-base`, no Anthropic base URL): https://aider.chat/docs/config/options.html
- API keys (YAML, `.env`, environment): https://aider.chat/docs/config/api-keys.html
