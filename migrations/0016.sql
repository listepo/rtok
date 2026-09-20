-- T68.3: hook-staled files stay listed until the next index replaces their rows.
CREATE TABLE IF NOT EXISTS symbol_stale (
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    PRIMARY KEY (root, path)
);

-- T68.3: last successful index time per root (CLI `graph status`).
ALTER TABLE extractor ADD COLUMN indexed_at INTEGER;
