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

Target: ≤ 800 tokens per turn and byte-identical injections across turns (P2 inject contract).

Falsified by: two consecutive turns with identical plugin inputs produce different injection bytes, or 800-token injections raise output tokens / fail tasks vs 0-token.
