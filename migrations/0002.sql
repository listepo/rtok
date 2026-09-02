-- rtok schema v2 (plan T13.2, decision D13). Applied once; keyed by filename.

-- Dimension: host agents, API providers, models (upserted from traffic).
CREATE TABLE hosts (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE providers (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE models (
    id INTEGER PRIMARY KEY,
    provider_id INTEGER NOT NULL REFERENCES providers(id),
    slug TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (provider_id, slug)
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    host_id INTEGER REFERENCES hosts(id),
    project TEXT,
    cwd TEXT,
    source TEXT,
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER
);

-- Unified action log. parent_id nests plugin_run under hook | mcp_call | api_request.
CREATE TABLE calls (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    host_id INTEGER REFERENCES hosts(id),
    provider_id INTEGER REFERENCES providers(id),
    model_id INTEGER REFERENCES models(id),
    plugin TEXT,
    surface TEXT NOT NULL,
    kind TEXT NOT NULL,
    parent_id INTEGER REFERENCES calls(id),
    name TEXT,
    ms REAL,
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT
);
CREATE INDEX calls_session ON calls (session_id, ts);
CREATE INDEX calls_kind ON calls (kind, ts);
CREATE INDEX calls_plugin ON calls (plugin, ts);
CREATE INDEX calls_parent ON calls (parent_id);

CREATE TABLE call_io (
    call_id INTEGER PRIMARY KEY REFERENCES calls(id),
    request_bytes INTEGER NOT NULL DEFAULT 0,
    response_bytes INTEGER NOT NULL DEFAULT 0,
    request_sha256 TEXT,
    response_sha256 TEXT,
    request_json TEXT,
    response_json TEXT,
    request_archive TEXT REFERENCES archive(id),
    response_archive TEXT REFERENCES archive(id)
);

CREATE TABLE tokens (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    call_id INTEGER NOT NULL REFERENCES calls(id),
    plugin TEXT,
    phase TEXT NOT NULL,
    source TEXT NOT NULL,
    tokens INTEGER NOT NULL,
    bytes INTEGER,
    input INTEGER,
    output INTEGER,
    cache_create INTEGER,
    cache_read INTEGER
);
CREATE INDEX tokens_call ON tokens (call_id, phase);
CREATE INDEX tokens_plugin ON tokens (plugin, ts);

CREATE TABLE logs (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    level TEXT NOT NULL,
    source TEXT NOT NULL,
    name TEXT NOT NULL,
    session TEXT,
    call_id INTEGER REFERENCES calls(id),
    plugin TEXT,
    message TEXT NOT NULL,
    fields TEXT
);
CREATE INDEX logs_ts ON logs (ts);
CREATE INDEX logs_source ON logs (source, name, ts);
CREATE INDEX logs_session ON logs (session, ts);

ALTER TABLE measurements ADD COLUMN call_id INTEGER REFERENCES calls(id);
ALTER TABLE usage ADD COLUMN call_id INTEGER REFERENCES calls(id);

INSERT INTO hosts (id, slug, kind) VALUES
  (1,'claude','cli'),(2,'cursor','ide'),(3,'codex','cli'),
  (4,'opencode','cli'),(5,'aider','cli'),(6,'other','other');
INSERT INTO providers (id, slug, name) VALUES
  (1,'anthropic','Anthropic'),(2,'openai','OpenAI'),(3,'other','Other');
