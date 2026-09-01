# Agent notes — `toon`

**Owns** `src/plugins/toon/**`.

**Invariants**
- Lossless: encoding must round-trip; keep a decode function and a property test for it.
- Only arrays of ≥ `min_rows` objects with identical scalar-valued keys are encoded.
- Original JSON is archived first; the encoded block references the archive id.
- Stays `default_on: false` until a `rtok bench` result in `research.md` justifies flipping it.

**Checks**: `plan.md` T11.7. Order: `roadmap.md` § `toon`. Stay off until T9.1 A/B in `research.md`.
