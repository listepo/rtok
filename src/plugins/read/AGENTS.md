# Agent notes — `read`

**Owns** `src/plugins/read/**` (`mod.rs`, `outline.rs`, `cache.rs`, `search.rs`, `hook.rs`),
tree-sitter grammar features `lang-*` in `Cargo.toml`.

**Invariants**
- Root guard first: reject anything outside cwd/`allow_paths` before touching the filesystem.
- Capped output always carries an archive id; the full content is retrievable via `expand`.
- Dedup responses are < 80 chars and write a `Measurement { kind: "dedup" }`.
- Tool descriptions ≤ 60 estimated tokens each (T4.1 test enforces it).
- The Read-advice hook never denies files under `native_max_bytes` — the edit gate stays cheap.

**Dependencies allowed**: `tree-sitter`, `tree-sitter-tags`, per-language grammars behind
features, `ignore` for gitignore-aware search. Justify each in the commit message.

**Checks**: `plan.md` T4.2–T4.6; golden per-language fixtures are 20-line files.
