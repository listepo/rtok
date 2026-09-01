# Agent notes — `memory`

**Owns** `src/plugins/memory/**` (`mod.rs`, `inject.rs`, `import.rs`), `src/plugins/checkpoint.rs`
if split out per T2.5.

**Invariants**
- No LLM calls. Notes are written by the agent through `mem_save` or extracted mechanically.
- Recall injects titles and ids only; bodies are fetched on demand with `mem_get`.
- Recall output is byte-stable across runs with unchanged notes and ≤ 200 tokens.
- Import reads only the generic JSONL shape (no third-party DB schemas, D6) and is idempotent
  (dedupe by sha256 of body).
- Search returns the right note first for the T6.1 fixture (three notes, one obvious match).

**Schema** lives in `migrations/0001.sql` (`notes`, `notes_fts` + triggers). Changing it means a
new migration file, never an edit to `0001.sql`.

**Checks**: `plan.md` T2.5, T6.1–T6.3.
