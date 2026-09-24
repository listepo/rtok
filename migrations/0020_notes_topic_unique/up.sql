-- T209: `upsert_note` enforced "one row per (project, kind, title)" (T66.1) purely in
-- application code — SELECT the newest match, then UPDATE or INSERT, with the mutex even
-- dropped before the INSERT. Nothing at the database level stopped two writers (hooks,
-- MCP, proxy, `otel flush` are separate processes/connections) from both inserting the
-- same topic key. This migration adds the UNIQUE index `upsert_note` now relies on.
--
-- `project` is nullable and SQLite treats NULL as a distinct value in a plain UNIQUE
-- index, so `(project, kind, title)` alone would not stop two NULL-project notes with the
-- same (kind, title) from duplicating. Indexing `COALESCE(project, '')` instead folds
-- every "no project" note onto one key, matching how the rest of this file already reads
-- a NULL project as "no project" rather than as a wildcard.
--
-- A store that already holds duplicates (any db older than this migration) would fail to
-- create the index, so duplicates are dropped first — keeping the newest row per key,
-- "newest" being the highest id, the same tie-break `upsert_note`'s prior SELECT used
-- (`ORDER BY id DESC LIMIT 1`). A plain DELETE fires the existing `notes_ad` trigger
-- (migration 0001), so `notes_fts` stays in sync with no extra statement here.

DELETE FROM notes
WHERE id NOT IN (
    SELECT MAX(id)
    FROM notes
    GROUP BY COALESCE(project, ''), kind, title
);

CREATE UNIQUE INDEX notes_topic ON notes (COALESCE(project, ''), kind, title);
