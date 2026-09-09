# Agent notes — `inject`

**Owns** `src/plugins/inject/**`, `modes/*.md`.

**Contract**: the `Plugin` trait, `Ctx` and the host capabilities come from the published
`rtok-plugin-sdk` crate (`crates/rtok-plugin-sdk`), not from `crate::plugin` — import them as
`rtok_plugin_sdk::…`.

**Invariants**
- Budget is a hard cap: three 500-token injections at budget 800 → two emitted, one dropped
  and measured (T2.4 Check).
- Byte-stable: two consecutive SessionStart runs with unchanged state produce identical bytes.
  Anything time- or count-dependent is a bug.
- Modes are data. Do not encode mode text in Rust.
- Mode text appears in SessionStart output once and never in UserPromptSubmit output.

**Checks**: `plan.md` T2.4, T7.1.
