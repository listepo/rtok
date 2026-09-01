# `memory`

One memory instead of two, with zero LLM cost: notes the agent writes, SQLite FTS5 search,
progressive disclosure.

| | |
|---|---|
| Kind | native |
| Surfaces | MCP `mem_save`, `mem_search`, `mem_get`; PreCompact checkpoint; SessionStart recall |
| Replaces | engram, claude-mem |
| Default | on |

## Tools

- `mem_save(kind, title, body, project?)` — project defaults to the git root name of cwd.
- `mem_search(query, limit=5)` — ids, titles, 120-char snippets ranked by FTS5 `bm25`.
- `mem_get(id)` — full body.

## Hooks

- PreCompact: extracts the last 20 user prompts (≤ 300 chars each), touched file paths and
  last error lines from the transcript into a `checkpoint` note.
- SessionStart with `source == "compact"` (and PostCompact): injects the latest checkpoint
  (≤ 400 tokens) through `inject`.
- SessionStart recall: last 5 note titles + ids for the project (≤ 200 tokens, priority 10),
  never bodies.

## Import

`rtok memory import <file.jsonl>` reads one note per line (`{kind, title, body, ts?, project?}`),
deduped by body sha256. Export your previous memory tool to that shape yourself; rtok knows
no third-party schema (D6).

## Tasks

T2.5 checkpoint · T6.1 notes API · T6.2 recall · T6.3 import.

## Status

Manifest only. Schema (`notes`, `notes_fts`) exists since T0.3.
