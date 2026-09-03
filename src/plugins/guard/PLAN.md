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

Deny the duplicate Read/Bash call (`PreToolDecision::Deny`) when the same path or command already ran in this session within `plugins.guard.window_turns` and an archive id exists; the reason names `rtok expand <id>` (T2.6). Never deny when there is no prior archive. Deny rate is visible in `stats --plugin guard`; expand on that id must still work. False-deny budget: keep the plugin only if false-deny stays under 1 % of guarded calls on a working day.

The property that beats the table: the bytes stay behind `expand <id>` instead of a silent re-fetch; a wrong deny is measurable.

## Rejected

- Rewrite-to-expand as the default — T2.6 is Deny with an expand pointer; a rewrite would change the tool the host runs.
- Repetition-penalty on sampled tokens — wrong layer; we see tools, not logits.

Target: Same honesty rule as `cmd`: deny rate is visible in `stats --plugin guard`; expand on a denied id must still work.

Falsified by: a working day where expand on a guarded id fails, or false-deny ≥ 1 %.
