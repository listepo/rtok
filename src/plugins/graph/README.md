# `graph`

Four capped tools over a symbol index rtok builds itself with tree-sitter-tags, instead of
four servers with 130+ tool descriptions in every request.

| | |
|---|---|
| Surfaces | MCP `symbol(name)`, `callers(name)`, `impact(name, depth)`, `outline(path)` |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Mechanism

Walk the repo (respecting `.gitignore`), run the tags queries of the `read` plugin's grammars
for definitions and reference sites, and store them in the `symbols` table, scoped to the
canonical repo root so one store holds many repos (T8.3). A file whose mtime and size are
unchanged is never opened; a file whose stat moved but whose sha256 did not is not re-parsed
(T8.4). Every reference row also stores `scope`, the innermost definition enclosing it in the
same file, which is the call edge `callers` groups by and `impact` walks (T8.5).

`symbol` returns each definition and its source, at most `body_lines` lines each (T8.6).
`callers` returns one line per calling definition. `impact` walks those edges breadth-first
to `depth`. `outline` is the `read` plugin's `map` mode. Every response is capped at
`max_tokens` (head + "N more, expand <id>", full text archived).

It is a tags index, not a type-resolved call graph (`plan.md` §0 non-goals). Measured recall
against hand labels: definitions 30/30, references 40/114 — the misses are type positions,
macro bodies and path-qualified calls, listed in `PLAN.md` under "Known misses".

## Config

```toml
[plugins.graph]
enabled    = true
max_tokens = 2000
body_lines = 40
```

## Tasks

See `roadmap.md` § `graph`. Checks in `plan.md`.

T8.1 symbol index · T8.2 MCP tools · T8.3 per-root scoping · T8.4 stat-gated freshness ·
T8.8 labelled hit rate · T8.5 call edges · T8.6 definition bodies · T8.7 `impact`.

## Status

T8.1–T8.8 done (T8.1/T8.2 2026-09-02, T8.3–T8.7 2026-09-04). Gate P8 passed 2026-09-03 on
description tokens and index time. Gate P8b is open: it needs the P9 task-set comparison, and
its recall clause was amended after T8.8 measured the index (`plan.md` §6).
