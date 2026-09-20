-- T69.6: last-written `rtok memory sync` block sha256 (hand-edit guard).
CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
