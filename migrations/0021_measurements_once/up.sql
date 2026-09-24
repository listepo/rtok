-- T245: one tool call reaches `rtok hook` more than once when a host delivers the same event
-- twice (Claude Code with both the plugin's hooks and leftover settings-file hooks), and
-- each delivery used to add its own `measurements` row, so savings double-counted (D3).
-- The hook surface now stamps each row with `once_key` (event, tool_use_id, plugin, kind,
-- ref_id); the second delivery's insert hits this index and is dropped. Rows without a key
-- (proxy, MCP, `rtok run`) stay NULL, and SQLite keeps NULLs distinct in a UNIQUE index.
ALTER TABLE measurements ADD COLUMN once_key TEXT;
CREATE UNIQUE INDEX measurements_once ON measurements (once_key);
