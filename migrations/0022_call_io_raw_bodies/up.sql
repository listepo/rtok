-- T211: `inline_body` hashed the lossy `from_utf8_lossy` text instead of the wire bytes, so
-- `request_sha256`/`response_sha256` did not verify the true payload and `call_io_request`
-- returned U+FFFD-corrupted bytes for inline bodies that were not valid UTF-8. These BLOB
-- columns hold the exact bytes for that case only (valid UTF-8 already round-trips through
-- `request_json`/`response_json`); NULL on every existing row, which keeps their old,
-- lossy behavior until re-recorded.
ALTER TABLE call_io ADD COLUMN request_raw BLOB NULL;
ALTER TABLE call_io ADD COLUMN response_raw BLOB NULL;
