---
title: Commands
weight: 2
---

Every command below is a surface onto the same plugin registry and the same SQLite store.
The **Status** column is the task id in `plan.md` that delivers it — `done` means it works
today, anything else prints `not implemented` and exits 0.

| Command | Does | Status |
|---------|------|--------|
| `rtok plugins` | list plugins: id, enabled, surfaces | done |
| `rtok config show\|init\|validate\|get\|set\|path` | one config file; `show --sources` says where each value came from | P12 |
| `rtok hook <event>` | Claude Code hook entry point (stdin JSON → stdout JSON) | T2.1 |
| `rtok mcp` | MCP server over stdio: `read`, `search`, `tree`, `expand`, `mem_*`, graph tools | T4.1 |
| `rtok proxy` | `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` hop: usage capture, optional compress mode | T5.1, P11 |
| `rtok run -- <cmd>` | run a command, archive the raw output, print the filtered version | T3.1 |
| `rtok expand <id>` | print an archived payload | T3.5 |
| `rtok stats` | measurements from session logs and the proxy | T1.2 |
| `rtok doctor` | inspect hooks, MCP servers, proxy chain | T1.4 |
| `rtok setup claude` | install hooks / MCP / proxy into Claude Code, with backups | T2.3 |
| `rtok bench` | A/B two host configurations on fixed tasks | T9.1 |

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
