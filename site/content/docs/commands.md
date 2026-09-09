---
title: Commands
weight: 2
---

Every command below is a surface onto the same plugin registry and the same SQLite store,
and every one of them works today. A subcommand whose plugin was compiled out (for example
`rtok run` without the `cmd` feature) prints `not implemented` and exits 0, so a stripped
build never blocks the host.

| Command | Does |
|---------|------|
| `rtok plugins` | list plugins: id, enabled, surfaces |
| `rtok config show\|init\|validate\|get\|set\|path` | one config file; `show --sources` says where each value came from |
| `rtok hook <event>` | Claude Code hook entry point (stdin JSON → stdout JSON) |
| `rtok mcp` | MCP server over stdio: `read`, `search`, `tree`, `expand`, `mem_*`, graph tools |
| `rtok proxy` | `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` hop: usage capture, optional compress mode |
| `rtok dashboard` | local Slint/WASM UI over a WebSocket API, reading the same store |
| `rtok run -- <cmd>` | run a command, archive the raw output, print the filtered version |
| `rtok filter --stdin` | filter a payload without executing it (OpenCode `tool.execute.after`) |
| `rtok expand <id>` | print an archived payload, whole or by `--lines` / `--grep` |
| `rtok stats` | measurements from session logs and the proxy |
| `rtok doctor` | inspect hooks, MCP servers, proxy chain — including what each costs per turn |
| `rtok agent setup claude\|cursor\|codex\|opencode\|pi` | install into a host, with backups; `--dry-run` |
| `rtok agent remove claude\|cursor\|codex\|opencode\|pi` | take rtok back out of a host, with backups; `--dry-run` |
| `rtok graph index [path]` | build the tree-sitter symbol index for a tree |
| `rtok memory import <file>` | import notes as JSONL, deduped by body hash |
| `rtok otel flush\|status` | export the ledgers over OTLP/HTTP, or report the watermarks |
| `rtok bench` | A/B two host configurations on fixed tasks |

## Every flag is a config key

There is no flag that cannot be made permanent. `rtok proxy --port 8791` and
`[proxy] port = 8791` are the same setting reached two ways, and `rtok config show --sources`
reports which layer won. See [Configuration](../reference/configuration) for the precedence
chain.

## Shortening and getting it back

`rtok run -- <cmd>` executes the command, archives the raw output, and prints a filtered
version with an id attached. `rtok expand <id>` prints the original payload back. That pair
is the whole lossless contract — nothing rtok shortens is unrecoverable.

## Measurement

`rtok stats` reads the `measurements` table. Rows come from plugins calling `Ctx::record`
with a `Measurement`, and from the proxy capturing real `usage` from API responses.
`rtok stats --calibrate` refits the characters-per-token estimator against those real
counts.

`rtok bench` runs two host configurations over the fixed task set in `bench/tasks.toml` and
compares them.
