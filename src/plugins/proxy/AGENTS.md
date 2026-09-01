# Agent notes — `proxy`

**Owns** `src/plugins/proxy/**`, `src/proxy/**` (`mod.rs`, `cli.rs`).

**Invariants**
- Passthrough is byte-exact: the T5.1 test compares response bytes against a mock upstream.
- Added latency < 20 ms per request (definition of done #4). Do not buffer SSE streams.
- Never modify `system`, `tools`, or the last 2 turns, in any mode.
- Every request inserts one `usage` row with all four counters; session id comes from
  `metadata.user_id` or a header, else a request hash.
- Bind `127.0.0.1` only.

**Dependencies allowed**: `tokio`, `axum`/`hyper`, `reqwest` (streaming). One-line reason each.

**Checks**: `plan.md` T5.1, T5.2.
