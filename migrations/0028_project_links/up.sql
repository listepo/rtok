-- T329.3: directed links between graph projects ("A sees into B"). `manual` links are the
-- user's; `auto` links come from a project's references (T329.8). Unlinking an auto link keeps
-- the row with `unlinked = 1` so the next index does not re-create what the user removed;
-- unlinking a manual link deletes it. Links die with either project (the cascade makes
-- removing a project leave nothing dangling). No self-links.
CREATE TABLE project_links (
    from_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    to_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('manual', 'auto')),
    reason TEXT,
    unlinked INTEGER NOT NULL DEFAULT 0 CHECK (unlinked IN (0, 1)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (from_id, to_id),
    CHECK (from_id != to_id)
);
CREATE INDEX project_links_to ON project_links(to_id);
