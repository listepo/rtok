-- T69.1: note lifecycle — retire / supersede / pin. Lossless by default (D4): retiring is
-- a tombstone (the row stays, recall and search skip it, mem_get still returns the body),
-- never a DELETE. Existing rows stay live: retired NULL, superseded_by NULL, pinned 0.

ALTER TABLE notes ADD COLUMN retired       INTEGER NULL;
ALTER TABLE notes ADD COLUMN superseded_by INTEGER NULL;
ALTER TABLE notes ADD COLUMN pinned        INTEGER NOT NULL DEFAULT 0;
