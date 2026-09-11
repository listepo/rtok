# inject — design note (D15)

## Problem

lean-ctx 3.1 K/turn + engram + claude-mem + ponytail + caveman + token-optimizer nudges compete for SessionStart/UserPromptSubmit. Instruction dilution is real; byte churn breaks the cache.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| lean-ctx banner | 2026-09-01 | 2026-09-01 | always-on repo context | 3.1 K/turn; not byte-stable |
| engram notes | 2026-09-01 | 2026-09-01 | agent-written; FTS5 | 18 tool descriptions/session |
| claude-mem | 2026-09-01 | 2026-09-01 | progressive disclosure | LLM extraction costs tokens |
| ponytail YAGNI | 2026-09-01 | 2026-09-01 | output-token cut on a small bench | prompt file; no budget |
| Lost in the Middle (Liu et al.) | arXiv 2307.03172 | 2023-07 | U-shaped attention; middle drops | not an injector; paper, not a product |

## Mechanism

Every SessionStart/UserPromptSubmit line is an `Injection` through `inject`, never a raw `additionalContext`. Budget 800 tokens; priority order: fail-open status, then memory titles, then graph one-liners, then coaching. Byte-stable: same inputs → identical bytes. A dropped `Injection` becomes one line `dropped:<id>:<tokens>` so the drop is visible, not silent.

The property that beats the table: a hard budget plus byte-stability, so cache and dilution are both bounded.

## Rejected

- Silent drops — we cannot debug a missing note.
- Unbounded concatenation of plugin banners — the current stack.

Target: Setup is additive; sessions still work.

Falsified by: two consecutive turns with identical plugin inputs produce different injection bytes, or 800-token injections raise output tokens / fail tasks vs 0-token.
## P33 pointer — tiered load stays out of inject (2026-09-11)

Full survey: `src/plugins/archive/PLAN.md` § P33 survey (2026-09-11).

**Inject's role if P33 ships:** SessionStart/UserPromptSubmit remain **L0-only** — fail-open status, memory **titles**, graph **one-liners**, coaching — inside the existing 800-token byte-stable cap (D5). **L1/L2 session context** (archived tool bodies, memory bodies, graph symbol bodies) load only via `expand`, `read`, `memory`, or `graph` MCP, not through injected `additionalContext`.

**Why:** Injections are re-read every turn and sit in the cached prefix; stuffing L1 overviews would duplicate `archive` live-zone work and break both budget and byte-stability. OpenViking loads L1 "then L2 when needed" at retrieval time; rtok maps that to **archive proxy depth + MCP**, not inject.

**Gate P33:** inject bytes on the tier fixture must match v0.1 when `archive.tiers = false`; when tiers are on, inject prefix still ≤ 800 tokens and byte-stable across replays (see archive PLAN Gate table).
