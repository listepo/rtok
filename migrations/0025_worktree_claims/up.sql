-- T285: which rtok agent (T282, D34) a worktree is bound to. The git lock reason
-- (`<owner> | <task> | <date> | agent <uuid>`) stays the source of truth and survives a lost
-- store; this table is the fast join. One row per worktree path: a later claim replaces it.
CREATE TABLE worktree_claims (
    path TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    task TEXT NOT NULL,
    claimed_at INTEGER NOT NULL DEFAULT (unixepoch()),
    released_at INTEGER
);
