-- T283.3 (D34): the pids above the hook process that last registered this agent (nearest
-- first, space-separated, at most `link::ANCESTORS`). `link::resolve` matches them against an
-- `rtok mcp` process's own ancestors, so an MCP process finds the session whose host started
-- it. NULL for a row no hook with a known pid has touched.
ALTER TABLE agents ADD COLUMN ancestors TEXT;
