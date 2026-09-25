-- T210: `measurements` is never pruned (`purge_calls_older_than` deliberately keeps it), and
-- `archive_in_session`'s correlated subquery filters it by `(session, ts)` on every dedup hit
-- (`plugin::identical_result` calls it once per PostToolUse tool result) — only
-- `measurements_plugin (plugin, ts)` existed, so the lookup scanned the whole table. This also
-- serves `last_measurement_ref`'s `(session, plugin, kind)` filter plus `ORDER BY id DESC`,
-- which now narrows through `session` before the id-ordered scan.
CREATE INDEX IF NOT EXISTS measurements_session_ts ON measurements (session, ts);
