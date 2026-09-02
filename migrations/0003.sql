-- T8.1 symbol index (definitions + reference sites).
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    line INTEGER NOT NULL,
    is_def INTEGER NOT NULL,
    file_sha TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS symbols_path ON symbols(path);
