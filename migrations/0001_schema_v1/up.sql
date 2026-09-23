-- rtok schema v1 (plan T0.3, decision D8). Applied once; keyed by filename.

CREATE TABLE events (
    id      INTEGER PRIMARY KEY,
    ts      INTEGER NOT NULL DEFAULT (unixepoch()),
    session TEXT    NOT NULL,
    event   TEXT    NOT NULL,
    tool    TEXT,
    plugin  TEXT,
    ms      REAL
);
CREATE INDEX events_session ON events (session, ts);

-- The only way a saving enters the DB (decision D3).
CREATE TABLE measurements (
    id           INTEGER PRIMARY KEY,
    ts           INTEGER NOT NULL DEFAULT (unixepoch()),
    session      TEXT    NOT NULL,
    plugin       TEXT    NOT NULL,
    kind         TEXT    NOT NULL,
    before_bytes INTEGER NOT NULL,
    after_bytes  INTEGER NOT NULL,
    est_before   INTEGER NOT NULL,
    est_after    INTEGER NOT NULL,
    ref_id       TEXT
);
CREATE INDEX measurements_plugin ON measurements (plugin, ts);

-- Index of raw payloads on disk under archive_dir (decision D4: lossless).
CREATE TABLE archive (
    id      TEXT    PRIMARY KEY,
    ts      INTEGER NOT NULL DEFAULT (unixepoch()),
    session TEXT    NOT NULL,
    tool    TEXT,
    bytes   INTEGER NOT NULL,
    path    TEXT    NOT NULL,
    sha256  TEXT    NOT NULL
);

CREATE TABLE read_cache (
    session    TEXT    NOT NULL,
    path       TEXT    NOT NULL,
    sha256     TEXT    NOT NULL,
    ts         INTEGER NOT NULL DEFAULT (unixepoch()),
    archive_id TEXT,
    PRIMARY KEY (session, path)
);

CREATE TABLE notes (
    id      INTEGER PRIMARY KEY,
    ts      INTEGER NOT NULL DEFAULT (unixepoch()),
    project TEXT,
    kind    TEXT    NOT NULL,
    title   TEXT    NOT NULL,
    body    TEXT    NOT NULL
);
CREATE VIRTUAL TABLE notes_fts USING fts5 (title, body, content = 'notes', content_rowid = 'id');
CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
END;
CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts (notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
END;
CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts (notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO notes_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
END;

-- Ground-truth token counts from the proxy (plan T5.1).
CREATE TABLE usage (
    id           INTEGER PRIMARY KEY,
    ts           INTEGER NOT NULL DEFAULT (unixepoch()),
    session      TEXT    NOT NULL,
    model        TEXT,
    input        INTEGER NOT NULL DEFAULT 0,
    cache_create INTEGER NOT NULL DEFAULT 0,
    cache_read   INTEGER NOT NULL DEFAULT 0,
    output       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX usage_session ON usage (session, ts);
