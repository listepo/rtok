-- P29: optional note embeddings beside FTS5 (T29.2).
-- Deviation: BLOB + brute-force cosine KNN — bundled SQLite has no sqlite-vec load path without cmake.

CREATE TABLE IF NOT EXISTS note_embeddings (
    note_id     INTEGER PRIMARY KEY NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    model       TEXT    NOT NULL,
    dims        INTEGER NOT NULL,
    text_hash   TEXT    NOT NULL,
    embedded_at INTEGER NOT NULL DEFAULT (unixepoch()),
    vector      BLOB    NOT NULL
);
CREATE INDEX note_embeddings_model ON note_embeddings (model);
