-- T8.3: one store, many repos. Without a root, indexing a second repo deleted the first
-- repo's rows (delete_symbols_missing saw them as removed files) and `src/main.rs` in one
-- repo answered for the other. Existing rows are a derived cache with no recoverable root,
-- so they go; the next tool call rebuilds them.
DELETE FROM symbols;
ALTER TABLE symbols ADD COLUMN root TEXT NOT NULL DEFAULT '';
DROP INDEX IF EXISTS symbols_name;
DROP INDEX IF EXISTS symbols_path;
CREATE INDEX IF NOT EXISTS symbols_root_name ON symbols(root, name);
CREATE INDEX IF NOT EXISTS symbols_root_path ON symbols(root, path);
