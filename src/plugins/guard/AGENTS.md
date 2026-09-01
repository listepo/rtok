# Agent notes — `guard`

**Owns** `src/plugins/guard/**`.

**Invariants**
- Deny only when the prior result is retrievable (an `archive` row exists); otherwise stay silent.
- Normalise before comparing (trim, collapse whitespace, strip `cd … &&` prefixes) so trivially
  different commands still match, but never match different file paths.
- Runs on the hook path: one indexed DB lookup, no filesystem reads.
- Each denial writes `Measurement { kind: "guard" }` with the avoided result size.

**Checks**: `plan.md` T2.6. Order: `roadmap.md` § `guard`.
