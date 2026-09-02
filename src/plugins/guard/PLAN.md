# guard — design note (D15)

## Problem

Agents re-fetch the same file or re-run the same command. token-optimizer's refetch guard denies; a deny that was wrong costs a turn. Expand on a denied id must still work.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| token-optimizer refetch guard | 2026-09-01 | 2026-09-01 | windowed duplicate detect | deny-only; Python; false-deny unmeasured |
| lean-ctx deny Grep/Glob | 2026-09-01 | 2026-09-01 | forces MCP reads | 78-tool tax; not a loop detector |
| rtk PreToolUse rewrite | 2026-09-01 | 2026-09-01 | can rewrite input | not duplicate-aware |
| LangGraph recursion limit | 0.2x | 2026-09-02 | hard cap on graph loops | framework-level; not tool-call dedup |

## Mechanism

Do not deny. Rewrite the duplicate call into `expand <id>` of the last matching archive (same tool + normalized args in a 20-turn window). Deny only if there is no archive id (true loop with no payload). False-deny budget: visible in `stats --plugin guard`; keep the plugin only if false-deny stays under 1 % of guarded calls on a working day.

The property that beats the table: the model still gets the bytes; we still save the re-fetch.

## Rejected

- Hard deny as the default — the model retries or invents.
- Repetition-penalty on sampled tokens — wrong layer; we see tools, not logits.

Target: max false-deny rate < 1 % of guarded calls; deny rate visible in `stats --plugin guard`; expand on a denied id still works (guard gate).

Falsified by: a working day where expand on a guarded id fails, or false-deny ≥ 1 %.
