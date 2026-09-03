-- T8.4: freshness from the file's stat, git's index rule. Without it every tool call read
-- and sha256'd the whole tree; with it an unchanged file is skipped without being opened.
-- Existing rows keep 0/0, which never matches a real stat, so they are re-read exactly once.
ALTER TABLE symbols ADD COLUMN mtime BIGINT NOT NULL DEFAULT 0;
ALTER TABLE symbols ADD COLUMN size BIGINT NOT NULL DEFAULT 0;
