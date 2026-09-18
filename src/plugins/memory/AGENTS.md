# Agent notes — `memory`

**Owns** `src/plugins/memory/**` (`mod.rs`, `inject.rs`, `import.rs`), `src/plugins/checkpoint.rs`
if split out per T2.5.

**Contract**: the `Plugin` trait, `Ctx` and the host capabilities come from the published
`rtok-plugin-sdk` crate (`crates/rtok-plugin-sdk`), not from `crate::plugin` — import them as
`rtok_plugin_sdk::…`.

**Invariants**
- No LLM calls. Notes are written by the agent through `mem_save` or extracted mechanically.
- `mem_save` is an upsert on `(project, kind, title)` — the title is the topic key (T66.1);
  checkpoints go through `insert_note` and are never upserted.
- Lifecycle is retire/supersede/pin, never `DELETE` (T69.1): recall and search filter
  `retired IS NULL` in SQL; `mem_get` still returns the body with a `retired` prefix; an
  explicit re-save through `mem_save` clears the tombstone. Pinned rows lead recall
  (`ORDER BY pinned DESC, id DESC`).
- MCP `mem_update` and `rtok memory retire|pin|unpin|revise` call the same functions here —
  one call path per capability (D21); `revise` is `mem_save` + `retire_note`, nothing else.
- Recall injects titles and ids only; bodies are fetched on demand with `mem_get`.
- `memory export` and `memory import` share one JSONL shape; `checkpoint:*` rows never leave.
- Recall output is byte-stable across runs with unchanged notes and ≤ 200 tokens.
- Import reads only the generic JSONL shape (no third-party DB schemas, D6) and is idempotent
  (dedupe by sha256 of body).
- Search returns the right note first for the T6.1 fixture (three notes, one obvious match).

**Schema** lives in `migrations/0001.sql` (`notes`, `notes_fts` + triggers). Changing it means a
new migration file, never an edit to `0001.sql`.

**Checks**: `plan.md` T2.5, T6.1–T6.3.
