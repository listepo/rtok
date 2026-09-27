-- T287: messages between agents and the user (`rtok agents send` / `inbox`). Local only.
-- `from_agent` NULL = the user at a terminal. `body` is ≤ 4 KiB of UTF-8 with control
-- characters (except \n and \t) stripped — enforced by `Store::send_message`, not here.
-- `delivered_at` is stamped the first time the recipient itself reads it (T288 pushes).
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    from_agent TEXT REFERENCES agents(id),
    to_agent TEXT NOT NULL REFERENCES agents(id),
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    delivered_at INTEGER,
    read_at INTEGER
);
-- `inbox`'s scan: one recipient's queue in send order.
CREATE INDEX messages_to_agent ON messages (to_agent, id);
