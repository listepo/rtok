-- T69.6: sha256 of the last written <!-- rtok:memory --> block.
CREATE TABLE IF NOT EXISTS memory_sync (
    path TEXT PRIMARY KEY,
    block_sha256 TEXT NOT NULL
);
