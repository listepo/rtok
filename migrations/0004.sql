-- T5.3: one persisted decision per tool_use_id so the archive plugin rewrites the same
-- block to byte-identical text on every request. expanded_ts (T5.4) freezes the id: once
-- expanded, the original is sent again from the next request on.
CREATE TABLE IF NOT EXISTS archive_decisions (
    tool_use_id TEXT    PRIMARY KEY,
    archive_id  TEXT    NOT NULL REFERENCES archive(id),
    session     TEXT    NOT NULL,
    pointer     TEXT    NOT NULL,
    expanded_ts INTEGER,
    ts          INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX IF NOT EXISTS archive_decisions_archive ON archive_decisions(archive_id);
