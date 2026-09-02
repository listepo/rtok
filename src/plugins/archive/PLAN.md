# archive — design note (D15)

## Problem

Old tool results stay in context and compound (technique #1 in `research.md` §6). Three retired archivers disagree on thresholds; none report expand rate.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| headroom live zone | 2026-09-01 | 2026-09-01 | cache-aligned; retrieve | Python; 11.3 % / 30 d |
| token-optimizer archive >4 KB | 2026-09-01 | 2026-09-01 | lossless; expand | Python; 4 KB is one knob |
| caveman shrink-hook | 2026-09-01 | 2026-09-01 | request rewrite | issue #112 corrupts inline code; proxy inert here |
| Anthropic context editing / tool-result clearing | docs 2026-09-01 | 2026-09-01 | provider-native; cache-safe when used as specified | host-only; not OpenAI wire; we cannot measure it locally |

## Mechanism

Pointer `expand <sha-or-id>` per `tool_use_id`, deterministic: same id always maps to the same archive row. Head/tail keep errors; age/size: archive when result is older than 2 turns **or** > 4 KiB, whichever hits first, subject to expand rate < 5 %. Prefer provider-native clearing when the host exposes it; rewrite ourselves only on the proxy path where we own the bytes.

The property that beats the table: determinism per `tool_use_id` plus an expand-rate ceiling, not a silent crush.

## Rejected

- Rewriting the cached prefix (tools array / system prompt) — breaks prompt cache (D, T14.5).
- LLMLingua-style pruning in v0.1 — quality risk on code (`research.md` §6.9).

Target: context-token-turns fall ≥ 15 % vs passthrough with expand rate < 5 % (P5 compress gate).

Falsified by: expand rate ≥ 5 % after two days, or a cache-hit-rate drop that costs more than the CTT save.
