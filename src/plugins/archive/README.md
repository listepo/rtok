# `archive`

Shrinks old, large tool results in the request the proxy forwards — without breaking the
prompt cache.

| | |
|---|---|
| Surfaces | proxy `compress` mode; MCP `expand` |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on (active only when the proxy runs in `compress` mode) |

## Mechanism

For each `tool_result` block that is older than `keep_turns` turns from the end **and** larger
than `min_tokens`, the content becomes:

```
[archived <id>: first 8 lines … last 4 lines · N tokens · expand(<id>)]
```

Decisions are keyed by `tool_use_id` and persisted, so every later request rewrites the same
block identically — the frozen prefix stays byte-stable and cache hits survive. Never touched:
`system`, `tools`, the last `keep_turns` turns, or any block whose id was `expand`ed.

## Config

```toml
[plugins.archive]
enabled = true
keep_turns = 4
min_tokens = 1500
```

## Tasks

See `roadmap.md` § `archive`. Checks in `plan.md`.

T5.3 live-zone rewrite · T5.4 `expand` through the proxy · T11.4 across wires.

## Status

Manifest only. First task: T5.3 (after T5.1 proxy).
