# Agent notes — `guard`

**Owns** `src/plugins/guard/**`.

**Contract**: the `Plugin` trait, `Ctx` and the host capabilities come from the published
`rtok-plugin-sdk` crate (`crates/rtok-plugin-sdk`), not from `crate::plugin` — import them as
`rtok_plugin_sdk::…`.

**Invariants**
- Deny only when the prior result is retrievable (an `archive` row exists); otherwise stay silent.
- Normalise before comparing (trim, collapse whitespace, fold `cd … &&` hops to the last
  target so the key keeps the directory the command runs from, T55.9) so trivially
  different commands still match, but never match different file paths.
- Runs on the hook path: one indexed DB lookup plus a file-existence stat, never a
  body read (`archive_size`, T55.16).
- Each denial writes `Measurement { kind: "guard" }` with the avoided result size.
- `skill.rs` (T62.1, opt-in `skills = true`) is the one path that reads a file on the
  hook: `SKILL.md` of the named skill, resolved from the name only (no separators, no
  `..`); over `skill_max_bytes` it archives the body and denies with the heading map
  (`kind: "skill"`), never when the frontmatter carries `allowed-tools`, `model`,
  `context` or `agent`. The whole reason stays under the cap.

**Checks**: `plan.md` T2.6. Order: `roadmap.md` § `guard`.
