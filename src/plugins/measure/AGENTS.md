# Agent notes — `measure`

**Owns** `src/plugins/measure/**`, `src/measure/**` (parser, stats, baseline, cache report),
`src/bench.rs`, `bench/tasks.toml`.

**Invariants**
- Estimates are labelled as estimates (±15 %); proxy `usage` rows are the only real counts.
- Skip malformed transcript lines and *count* them; never abort a report on one bad line.
- Context-token-turns for a tool_result of T tokens at turn t in an N-turn session = T × (N − t).
- `rtok stats` must reproduce the numbers in `research.md` §2 within ±5 % (T1.2 Check).

**Do not** add LLM calls, embeddings, or new dependencies here. `--calibrate` is the only
network call and must exit 0 with "skipped" when no API key is present.

**Checks** are in `plan.md` under T1.1–T1.5, T5.5, T9.1. Run the task's Check verbatim, then
`make check`.
