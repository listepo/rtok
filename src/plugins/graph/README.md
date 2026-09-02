# `graph`

Three capped tools over a symbol index rtok builds itself with tree-sitter-tags, instead of
four servers with 130+ tool descriptions in every request.

| | |
|---|---|
| Surfaces | MCP `symbol(name)`, `callers(name)`, `outline(path)` |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Mechanism

Walk the repo (respecting `.gitignore`), run the tags queries of the `read` plugin's grammars
for definitions and reference sites, and store them in the `symbols` table keyed by file
sha256 (incremental: unchanged files are skipped; an edited file is re-indexed on the next
call). `symbol`, `callers` and `outline` query that table and cap output at 2 K tokens
(head + "N more, expand <id>"). It is a tags index, not a type-resolved call graph
(`plan.md` §0 non-goals).

## Config

```toml
[plugins.graph]
enabled = true
max_tokens = 2000
```

## Tasks

See `roadmap.md` § `graph`. Checks in `plan.md`.

T8.1 symbol index · T8.2 MCP tools.

## Status

T8.1 and T8.2 done 2026-09-02. Every tool call runs the incremental index first (unchanged
files are skipped by sha256), so a file marked stale by PostToolUse(Edit|Write) is re-parsed on
the next call. Gate P8 (description tokens vs the retired servers, index time) is open.
