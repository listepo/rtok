# Agent notes — `archive`

**Owns** `src/plugins/archive/**`.

**Invariants**
- Determinism: the same `tool_use_id` is rewritten to byte-identical text on every request.
  Persist the decision (id → archived, or id → expanded) in the DB before returning.
- Prefix safety: a unit test must prove the request bytes up to the first rewritten block are
  unchanged (T5.3 Check).
- Never rewrite `system`, `tools`, the last `keep_turns` turns, or an expanded id.
- One `Measurement` per rewritten block; track the expand rate — it is the honesty metric.

**Do not** summarise content with a model, and do not touch anything outside `messages[*].content[*]`
with `type == "tool_result"`.

**Checks**: `plan.md` T5.3, T5.4.
