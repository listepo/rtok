-- T16.2 (D19): OpenTelemetry export watermarks. One row per stream — `calls` and `logs` by
-- id, `sessions` by ended_at — holding the last value posted with a 2xx. A flush reads past
-- the mark and advances it only on success, so a row is re-sent rather than lost.
CREATE TABLE IF NOT EXISTS otel_export (
    stream TEXT PRIMARY KEY,
    mark   BIGINT NOT NULL DEFAULT 0
);
