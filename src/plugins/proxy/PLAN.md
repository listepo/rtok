# proxy — design note (D15)

## Problem

headroom :8788 → caveman :8787 is the current chain; caveman is inert on Max; two hops add latency and neither records `usage` into rtok's store. Any `proxy_filter` that churns the cached prefix burns 0.025×–1× multipliers.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| headroom proxy | 2026-09-01 | 2026-09-01 | JSON crush; CCR retrieve | 82 % Python; extra hop |
| caveman proxy | 2026-09-01 | 2026-09-01 | record/compress path | inert on Max; #112 |
| bifrost | 2026-09-01 | 2026-09-01 | SSE gateway | semantic cache false-hits on agents (`research.md` §4) |
| LiteLLM | 1.7x | 2026-09-02 | many wires; usage logging | heavy; mutates requests; not cache-byte-stable by default |

## Mechanism

Passthrough first (P5): copy bytes, capture `usage`, add < 20 ms. Then opt-in compress that only mutates **after** the cacheable prefix (messages already cached, or the live tail). SSE: if upstream is slow or errors mid-stream, forward what arrived and fail open (no retry that duplicates a tool_use). Every other plugin's `proxy_filter` must not touch `tools`, `system`, or earlier `messages` bytes.

The property that beats the table: `usage` lands in `calls`/`tokens` on both Anthropic and OpenAI wires without a cache miss from rtok itself.

## Rejected

- Semantic response cache (bifrost 0.9) in v0.1 — a hit is a wrong answer for agents.
- Buffering a full stream before forwarding — blows the 20 ms budget.

Target: added latency < 20 ms per request (T5.1) plus a cache-hit-rate floor from proxy `usage` (P5 passthrough gate: two days of usage rows).

Falsified by: p95 added latency ≥ 20 ms, or a documented cache miss caused by rtok rewriting the prefix.
