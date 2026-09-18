# `memory`

One memory instead of two, with zero LLM cost: notes the agent writes, SQLite FTS5 search,
progressive disclosure.

| | |
|---|---|
| Surfaces | MCP `mem_save`, `mem_search`, `mem_get`, `mem_update`; PreCompact checkpoint; SessionStart recall |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Tools

- `mem_save(kind, title, body, project?)` — project defaults to the git root name of cwd.
  The title is the topic key: a second save with the same project, kind and title updates
  that row (`{"id", "updated": true}`) instead of adding a stale twin to recall (T66.1).
  An explicit re-save of a retired topic revives it (clears the tombstone).
- `mem_search(query, limit=5)` — ids, titles, 120-char snippets ranked by FTS5 `bm25`.
  Retired notes never appear.
- `mem_get(id)` — full body. A retired note still returns its body, prefixed by one line
  `retired <ts>[, superseded by <id>]`.
- `mem_update(id, retire?, superseded_by?, pinned?)` — the lifecycle (T69.1): `retire`
  tombstones the id (never deletes), `superseded_by` names the replacement, `pinned`
  pins/unpins. The same operations run as `rtok memory retire|pin|unpin` and
  `rtok memory revise <id> --title --body` (revise = save the replacement through the
  `mem_save` path, then retire the old id naming it).

## Lifecycle

Retire, supersede, pin — never delete (D4). A retired note is skipped by recall and search
but keeps its body readable through `mem_get`; pinned notes lead the SessionStart recall
ahead of newest-first order; both orders are byte-stable for an unchanged store.

## Hooks

- PreCompact: extracts the last 20 user prompts (≤ 300 chars each), touched file paths and
  last error lines from the transcript into a `checkpoint` note.
- SessionStart with `source == "compact"` (and PostCompact): injects the latest checkpoint
  (≤ 400 tokens) through `inject`.
- SessionStart recall: last 5 note titles + ids for the project (≤ 200 tokens, priority 10),
  never bodies.

## Import and export

`rtok memory import <file.jsonl>` reads one note per line (`{kind, title, body, ts?, project?}`),
deduped by body sha256. Export your previous memory tool to that shape yourself; rtok knows
no third-party schema (D6).

`rtok memory export [--project <name>]` prints the same shape from the store — every note
but the session-local `checkpoint:*` rows — so notes move between machines through a file
you commit or copy (T66.2). An export piped into `import` on a second store inserts each
row once; a second import skips them all.

## Recall bench (T69.3)

`cargo test --test memory_bench -- --nocapture` (2026-09-18). Seeded in-memory store, 20 planted facts, 5 revised. FTS5 and P29 hybrid both 20/20 at N=1/10/30/100; superseded returned 0. SessionStart recall is 95–100 bytes against 6 331–371 866 bytes of full live-body injection. `half_life_days` is N/A (T69.2 shipped no scorer). Numbers and the command live in `research.md` §14; never graymatter's 83 %.

## Tasks

See `roadmap.md` § `memory`. Checks in `plan.md`.

T6.1 notes API · T2.5 checkpoint · T6.2 recall · T6.3 import · T66.1 upsert · T66.2 export · T69.1 lifecycle · T69.3 recall bench.

## Status

Manifest only. Schema (`notes`, `notes_fts`) exists since T0.3.
