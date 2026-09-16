-- T45.3: archive_decisions PK was bare `tool_use_id`; reads/writes scope by
-- (session, tool_use_id). A repeated id in a second session hit INSERT OR IGNORE
-- and was never persisted. Rebuild with composite PK (session, tool_use_id).
-- `mark_expanded` was already session-scoped in the SQL but the index did not help.

CREATE TABLE IF NOT EXISTS archive_decisions_new (
    session     TEXT    NOT NULL,
    tool_use_id TEXT    NOT NULL,
    archive_id  TEXT    NOT NULL REFERENCES archive(id),
    pointer     TEXT    NOT NULL,
    expanded_ts INTEGER,
    ts          INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (session, tool_use_id)
);
INSERT OR IGNORE INTO archive_decisions_new (session, tool_use_id, archive_id, pointer, expanded_ts, ts)
    SELECT session, tool_use_id, archive_id, pointer, expanded_ts, ts FROM archive_decisions;
DROP TABLE archive_decisions;
ALTER TABLE archive_decisions_new RENAME TO archive_decisions;
CREATE INDEX IF NOT EXISTS archive_decisions_archive ON archive_decisions(archive_id);
