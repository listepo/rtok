-- T329.1: the graph's project registry. One row per canonical root rtok indexes; the symbol
-- index stays keyed by `root`, so a project's rows are never mixed with another's. `name`
-- NULL means "the directory name". The partial unique index lets at most one row be selected.
-- Whether the root still exists is read from the filesystem, never stored.
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    root TEXT NOT NULL UNIQUE,
    name TEXT,
    origin TEXT NOT NULL CHECK (origin IN ('manual', 'session', 'worktree', 'mcp', 'reference')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER NOT NULL DEFAULT (unixepoch()),
    selected INTEGER NOT NULL DEFAULT 0 CHECK (selected IN (0, 1))
);
CREATE UNIQUE INDEX projects_one_selected ON projects(selected) WHERE selected = 1;

-- An existing store keeps what it already indexed: one project per root, the most recently
-- indexed one selected.
INSERT INTO projects (root, origin) SELECT DISTINCT root, 'manual' FROM symbols WHERE root != '';
UPDATE projects SET selected = 1 WHERE id = (
    SELECT p.id FROM projects p LEFT JOIN extractor e ON e.root = p.root
    ORDER BY COALESCE(e.indexed_at, 0) DESC, p.id LIMIT 1
);
