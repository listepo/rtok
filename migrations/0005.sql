-- T11.6: per-API usage discriminator (plan listed 0003.sql; that file is T8.1 symbols).
ALTER TABLE usage ADD COLUMN api TEXT NOT NULL DEFAULT 'anthropic';
