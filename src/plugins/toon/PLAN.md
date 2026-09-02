# toon — design note (D15)

## Problem

Tabular tool results (test lists, grep hits, MCP JSON arrays) are verbose as pretty JSON. TOON claims −42.6 % tokens (`research.md` §4) but accuracy is a vendor bench (72.2 vs 71.4). Off by default until P9.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| TOON (toon-format) | 2026-09-01 | 2026-09-01 | tabular JSON → compact columns | vendor bench; not rtok corpus |
| caveman TOON path | 2026-09-01 | 2026-09-01 | already in the retired stack | #112; unmeasured here |
| minified JSON | RFC 8259 | 2026-09-02 | lossless; models already know it | little save vs TOON on uniform tables |
| CSV / JSONL | RFC 4180 | 2026-09-02 | models read CSV well; one row/line | nested objects flatten badly |

## Mechanism

Detect “tabular enough”: JSON array of objects with ≥ 4 rows, ≥ 3 shared keys, no nested objects deeper than 1. Encode to TOON (or JSONL if nested). Keep off until `rtok bench` on the P9 set shows cost per passed task down and pass rate holds. Accuracy condition: no extra task failures vs JSON on that set.

The property that beats the table: a detection rule plus a bench gate, not a global encoder.

## Rejected

- Encoding every JSON tool result — nested/code blobs lose.
- Default-on in v0.1 — P9 has not run.

Target: bytes saved on tabular results with no pass-rate regression on the P9 task set; stay off until that holds (P9 gate: keep enabled only if cost per passed task falls and pass rate holds).

Falsified by: P9 pass rate drops, or TOON loses to minified JSON on rtok's captured corpus at the detection threshold.
