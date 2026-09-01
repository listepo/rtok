# Agent notes — `guard`

**Owns** `src/plugins/guard/**`.

**Invariants**
- Deny only when the prior result is retrievable (an `archive` row exists); otherwise stay silent.
- Normalise before comparing (trim, collapse whitespace, strip `cd … &&` prefixes) so trivially
  different commands still match, but never match different file paths.
- Runs on the hook path: one indexed DB lookup, no filesystem reads.
- Each denial writes `Measurement { kind: "guard" }` with the avoided result size.

**Not scheduled yet.** Add a task to `plan.md` (with a Check) before writing code here.
