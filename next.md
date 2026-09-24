# Next: token saving

Plan / diffs only for token-economy work. Drawn from `research.md`,
`docs/comparison.md`, `docs/prompt-cache.md`, `ideas.md`, and `plan.md` /
`done.md`. Promote cards into `plan.md` before implementing.

## 1. Token saving

### Already in rtok

- **Terse / YAGNI modes** — byte-stable inject under the shared ~800-token budget (`docs/comparison.md`, modes).
- **Lean MCP tool descriptions** — ~143 desc tokens/turn vs multi-k competitor stacks (`README.md`, `docs/comparison.md`).
- **Archive + expand** — raw tool output archived before shorten; reversible via `rtok expand` (archive/cmd/read plugins).
- **Proxy compress / live zone** — cache-preserving rewrites shrink old tool results (`proxy`; `docs/comparison.md`).
- **Prompt-cache preserving** — documented hit rates and bust ledger (`docs/prompt-cache.md`); pricing via T49.1 `stats --price`.
- **Guard early-exit** — block duplicate native work without an LLM call (`guard` plugin).
- **Semantic response cache** — opt-in P31 (`plugins.proxy.semantic_cache`, `docs/config.md`; false-hit risk I-23).
- **Output / prose compress** — extractive + caveman-style benches (`plugins.compress`, `docs/comparison.md`).

### To add

- **Batch / Flex API path** — ~50% provider discount (and cache stacking where supported) for async or Flex-tier traffic; rtok today only rides sync agent calls. **Planned** opt-in CLI (not implemented): `rtok batch submit <jsonl>`, `rtok batch status <id>`, `rtok batch fetch <id> <out>` — pass-through to provider `/v1/batches` via `rtok proxy`, outside the agent loop, does not replace agent `BASE_URL` (`docs/batch-flex.md`).
- **Model routing (small → large)** — cheap model for classify/route/simple edits, large only on hard turns; not in plan today.
- **LLM soft compression (P28)** — Later / default-off LLMLingua-class shrink of archived context; ship only if a bench beats v0.1 lossless (`plan.md` P28, I-21).
