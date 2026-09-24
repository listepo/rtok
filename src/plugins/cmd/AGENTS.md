# Agent notes — `cmd`

**Owns** `src/plugins/cmd/**` (`run.rs`, `rules.rs`, `formatters.rs`, `hook.rs`), `rules/default.toml`,
`src/expand.rs`, `tests/cmd_golden/`.

**Contract**: the `Plugin` trait, `Ctx` and the host capabilities come from the published
`rtok-plugin-sdk` crate (`crates/rtok-plugin-sdk`), not from `crate::plugin` — import them as
`rtok_plugin_sdk::…`.

**Invariants**
- Lossless: anything a reader could miss is archived before it is dropped; `rtok expand <id>` returns the raw bytes. A whitespace/ANSI-only change drops nothing and stores no archive row (T160).
- Exit code of the wrapped command is preserved exactly.
- Never wrap: first word in `never_wrap` (default `rtok`, `sudo`), heredocs (`<<`), trailing `&`,
  `-i`/`--interactive`, or when `rewrite = false`.
- Never redact. A fixture with a fake AWS key must pass through unchanged (T3.3 Check).
- Every run writes one `Measurement { kind: formatter | rule | raw | unmatched | skill }`.
  `raw` is a tiny body below the trailer gate or a T176 bounded passthrough (both by
  design); `unmatched` is a picked rule/formatter that shrank nothing (T177's actionable
  share — `rtok stats` splits the two).
- The PreToolUse hook path must stay under 10 ms: no filesystem walks, no subprocesses.
- No third-party tool is executed, linked or imported (D6). Formatters are written here from
  the family list in `research.md`.

**Do not** parse shell syntax beyond the first argv word; do not add a shell parser dependency.
One exception (T176): `bounded.rs` lexes quotes, `\`-newline continuation, and
`|`/`&&`/`||`/`;`/newline to spot a command the agent already bounded (`sed -n a,bp`,
`head`/`tail -n`, `grep -A/-B/-C/-m` — a recursive `grep` (`-r`/`-R`) also needs
`-m`/`--max-count`, or the hit list is unbounded; `rg` is recursive by default and has no such
recursive flag (`-r` is `--replace`), so this gate is `grep`-only and `rg` stays on the plain
context/max-count check — `cat -n` of exactly one named file); those pass through unchanged up
to `bounded::MAX_BYTES`. `formatters::compress` reuses the same lexer
(`bounded::mixed_chain`, T177) to route a single Bash string only when it chains 2+ DISTINCT
programs to `[script]` in `rules/default.toml` — `cargo build && cargo test` or
`cd x && cargo test` keep using `cargo`'s own formatter/rule.

**Checks**: `plan.md` T3.1–T3.6. Golden tests live in `tests/cmd_golden/*.{in,out}`.
