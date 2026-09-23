-- T69.2: recall ranking uses / last_used.
ALTER TABLE notes ADD COLUMN uses INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN last_used INTEGER NULL;
