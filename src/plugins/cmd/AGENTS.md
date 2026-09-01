# Agent notes — `cmd`

**Owns** `src/plugins/cmd/**` (`run.rs`, `rtk.rs`, `rules.rs`, `hook.rs`), `rules/default.toml`,
`src/expand.rs`, `tests/cmd_golden/`.

**Invariants**
- Lossless: the raw output (or rtk's output, which is regenerable) is always archived before
  anything is shortened. `rtok expand <id>` must return it.
- Exit code of the wrapped command is preserved exactly.
- Never wrap: commands starting with `rtok`/`rtk`, heredocs (`<<`), `sudo`, trailing `&`,
  `-i`/`--interactive`, or when `rewrite = false`.
- Never redact. A fixture with a fake AWS key must pass through unchanged (T3.3 Check).
- Every run writes one `Measurement { kind: rtk | rule | raw }`.
- The PreToolUse hook path must stay under 10 ms; do not probe `rtk --help` on the hook path
  (probe once from `rtok run`, cache 24 h in the DB).

**Do not** parse shell syntax beyond the first argv word; do not add a shell parser dependency.

**Checks**: `plan.md` T3.1–T3.6. Golden tests live in `tests/cmd_golden/*.{in,out}`.
