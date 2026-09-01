# Agent notes — `graph`

**Owns** `src/plugins/graph/**` (`mod.rs`, `index.rs`) and the `symbols` migration.

**Invariants**
- Native only (D6): never spawn, link or import an external graph tool. The index is built
  here from the tree-sitter-tags queries shared with `read`.
- Incremental: a file whose sha256 is unchanged is never re-parsed; removed files lose their rows.
- Every response is capped at `plugins.graph.max_tokens` and carries an archive id when truncated.
- Indexing never runs on the hook path; PostToolUse(Edit|Write) only marks a file stale.
- Schema changes are a new `migrations/NNNN.sql`, never an edit to an applied one.

**Checks**: `plan.md` T8.1–T8.2.
