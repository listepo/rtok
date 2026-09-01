# Agent notes — `graph`

**Owns** `src/plugins/graph/**`.

**Invariants**
- Adapter only: never parse source or build a call graph here. If a native graph is ever
  justified, it is a plan change (D6), not a quiet addition.
- When no backend is installed, `mcp_tools()` returns an empty list — the tools must not appear.
- Every response is capped and carries an archive id when truncated.
- Backend processes are spawned per call and killed on return; no daemon (D1).

**Checks**: `plan.md` T8.1.
