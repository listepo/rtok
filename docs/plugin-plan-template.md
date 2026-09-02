# Plugin plan template (D15)

Copy to `src/plugins/<id>/PLAN.md` before that plugin's first implementation task.
Keep it to one screen. Fill every heading; the `plugin_plans` test fails if any are missing.

## Problem

One paragraph: the token waste in measurable terms (what the retired stack costs you, on which surface).

## Alternatives

Survey ≥ 3 tools, **≥ 1 from outside the stack rtok retires**. Version and date on every row.

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| (retired) | | YYYY-MM-DD | | |
| (retired) | | YYYY-MM-DD | | |
| (outside) | | YYYY-MM-DD | | |

## Mechanism

rtok's mechanism in one paragraph, then the one property that makes it better than the table (not “written in Rust”).

## Rejected

≥ 2 options we will not take, each with a one-line reason.

Target: <the one number this plugin's `roadmap.md` gate must beat>

Falsified by: <the observation that kills this design>
