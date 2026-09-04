# Agent notes — `graph`

**Owns** `src/plugins/graph/**` (`mod.rs`, `index.rs`), the `symbols` migrations, and the
`symbol_*` methods in `src/store/symbols*.rs` (P8c).

**Invariants**
- Native only (D6): never spawn, link or import an external graph tool. The index is built
  here from the tree-sitter-tags queries shared with `read`. A storage crate (`lbug`, D18) is
  a library like Diesel, not a tool.
- Incremental: a file whose stat, then sha256, is unchanged is never re-parsed; removed files
  lose their rows; rows are scoped to the canonical root, so one store holds many repos.
- Every response is capped at `plugins.graph.max_tokens` and carries an archive id when truncated.
- Indexing never runs on the hook path; PostToolUse(Edit|Write) only marks a file stale.
- Schema changes are a new `migrations/NNNN.sql`, never an edit to an applied one.
- The plugin never writes SQL or Cypher (D13). Storage is `src/store/symbols.rs`, or
  `src/store/symbols_lbug.rs` under `graph-lbug`; both expose the same `symbol_*` methods.
- `tests/graph_contract.rs` pins the four tools through `rtok mcp`. Output changes are a
  task whose commit updates the expected strings; a backend must pass the file untouched.
- A tool listed by `mcp_tools()` is routed in `src/mcp.rs` `invoke` — `tools/list` and
  `tools/call` must agree (T8.9 found `impact` listed and unreachable).

**Checks**: `plan.md` T8.9–T8.14; earlier ones in `done.md`.
