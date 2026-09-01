# Agent notes — `cmd`

**Owns** `src/plugins/cmd/**` (`run.rs`, `rules.rs`, `formatters.rs`, `hook.rs`), `rules/default.toml`,
`src/expand.rs`, `tests/cmd_golden/`.

**Invariants**
- Lossless: the raw output is always archived before anything is shortened. `rtok expand <id>` must return it.
- Exit code of the wrapped command is preserved exactly.
- Never wrap: first word in `never_wrap` (default `rtok`, `sudo`), heredocs (`<<`), trailing `&`,
  `-i`/`--interactive`, or when `rewrite = false`.
- Never redact. A fixture with a fake AWS key must pass through unchanged (T3.3 Check).
- Every run writes one `Measurement { kind: formatter | rule | raw }`.
- The PreToolUse hook path must stay under 10 ms: no filesystem walks, no subprocesses.
- No third-party tool is executed, linked or imported (D6). Formatters are written here from
  the family list in `research.md`.

**Do not** parse shell syntax beyond the first argv word; do not add a shell parser dependency.

**Checks**: `plan.md` T3.1–T3.6. Golden tests live in `tests/cmd_golden/*.{in,out}`.
