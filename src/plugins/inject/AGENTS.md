# Agent notes — `inject`

**Owns** `src/plugins/inject/**`, `modes/*.md`.

**Invariants**
- Budget is a hard cap: three 500-token injections at budget 800 → two emitted, one dropped
  and measured (T2.4 Check).
- Byte-stable: two consecutive SessionStart runs with unchanged state produce identical bytes.
  Anything time- or count-dependent is a bug.
- Modes are data. Do not encode mode text in Rust.
- Mode text appears in SessionStart output once and never in UserPromptSubmit output.

**Checks**: `plan.md` T2.4, T7.1.
