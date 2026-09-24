-- T127: the same-session dedup pointer must not name a body a different context (a
-- sub-agent, or the main window) never saw. NULL is the main window; a sub-agent's
-- `agent_id` scopes its own rows, same first-writer-wins tradeoff `archive` already has
-- across sessions.
ALTER TABLE archive ADD COLUMN agent_id TEXT NULL;
