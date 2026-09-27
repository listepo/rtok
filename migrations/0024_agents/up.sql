-- T282 (D34): the rtok agent id. A host's own session id collides across hosts and is
-- missing on several (research.md §26), so rtok issues its own UUIDv7 per host session,
-- resolved from any unique prefix of 4+ hex chars (`resolve_agent`). `parent_key` tells
-- the main window (empty string) apart from each sub-agent inside the same host session
-- (the host's own `agent_id`, HookInput §17.2); `parent_id` is the resolved rtok id of a
-- sub-agent's parent row. `status_text` is written by the agent itself, starting T284.
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    -- NOT NULL: SQLite treats NULLs as distinct in a UNIQUE index, so a NULL host_id would
    -- make `agents_host_session` below insert a new row on every event instead of upserting.
    host_id INTEGER NOT NULL REFERENCES hosts(id),
    host_session_id TEXT NOT NULL,
    parent_key TEXT NOT NULL DEFAULT '',
    parent_id TEXT REFERENCES agents(id),
    cwd TEXT,
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER,
    activity TEXT,
    status_text TEXT
);
-- One row per (host, host session, sub-agent): `register_agent`'s upsert target.
CREATE UNIQUE INDEX agents_host_session ON agents (host_id, host_session_id, parent_key);
-- `live_agents`' idle-cutoff scan.
CREATE INDEX agents_last_seen ON agents (last_seen);
