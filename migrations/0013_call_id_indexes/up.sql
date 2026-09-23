-- `usage.call_id` and `measurements.call_id` (added in 0002) had no index: `call_detail`
-- (one otel span per call), `recent_calls` and `purge_calls_older_than` filtered on them
-- with a full scan per call.

CREATE INDEX IF NOT EXISTS usage_call ON usage (call_id);
CREATE INDEX IF NOT EXISTS measurements_call ON measurements (call_id);
