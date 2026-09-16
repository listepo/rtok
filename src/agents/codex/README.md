# Codex

`rtok agents setup codex` — the Codex CLI (`codex`).

File: `~/.codex/config.toml`, edited with `toml_edit` so comments and other servers survive.

## Modules

| Module | Support | Why |
| --- | --- | --- |
| mcp | yes | `[mcp_servers.rtok]` with `command`/`args` → `rtok mcp` |
| proxy | `--proxy` | `[model_providers.rtok]` with `base_url = http://<bind>:<port>/v1` and `model_provider = "rtok"` |
| hooks | no | Codex has no shell hook events |
| plugin | no | Codex loads MCP from config.toml; there is no plugin directory to link |

## rtok plugins this host reaches

- Through MCP: read, archive, memory, graph, toon.
- Through the proxy: measure, archive, proxy, toon, compress.
- Not reachable: cmd, inject, guard — hook-only surfaces.
