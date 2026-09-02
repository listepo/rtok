# measure — design note (D15)

## Problem

Five incompatible meters (rtk `gain`, headroom savings, lean-ctx gain, token-optimizer dashboard, caveman) none of which match the bill. `research.md` §2 has 17 sessions / 43,609 JSONL lines and no end-to-end number. Stats that claim dollars without proxy `usage` are guesses.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| rtk gain | rtk-ai 2026-09-01 | 2026-09-01 | byte/4 on bash filters; simple | 40 % of bash bytes; JetBrains bill +7.6 %; drops lines |
| headroom savings | 2026-09-01 | 2026-09-01 | request-level; retrieve is lossless | 11.3 % / 30 d; Python proxy; not a session CTT |
| token-optimizer dashboard | 2026-09-01 | 2026-09-01 | per-hook counters on this machine | author's meter only; 27 Python hooks; no bill |
| Langfuse generation usage | 3.x | 2026-09-02 | provider `usage` as ground truth; traces | not local-first; no context-token-turns; SaaS default |

## Mechanism

Parse host JSONL + proxy `usage` into `calls`/`tokens`. Primary diagnostic is **context-token-turns** (result tokens × remaining turns). Honest keep/revert number is **cost per passed bench task** (P9). `stats` reports CTT, sizes, cache hit first; it refuses a dollar column without proxy `usage` rows.

The property that beats the table: one number, from the same store the plugins write, that can be compared to a named baseline.

## Rejected

- Treating chars/4 as the product metric — it is an estimator class, not the bill.
- Shipping rewrite plugins before `--save-baseline` exists — Gate P1.

Target: context-token-turns defined; `rtok stats --save-baseline before-rtok` stored before any rewrite plugin ships (P1 gate: baseline saved; numbers in `research.md` §2).

Falsified by: after a week of proxy `usage`, CTT rankings contradict cost-per-passed-task on the P9 set and we still steer by CTT.
