-- T25.0 (D27): `rtok agent setup` installs `pi` (src/setup/pi.rs) but 0002.sql's seed list
-- never grew a matching row, so `[hook] host = "pi"` silently resolved to `other` (6). Seed
-- the missing slug as data, not as a new match arm. `INSERT OR IGNORE` is additive: an
-- existing DB gains the row and no row already there is rewritten.
INSERT OR IGNORE INTO hosts (slug, kind) VALUES ('pi', 'cli');
