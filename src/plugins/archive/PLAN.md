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

Target: Two days compress after passthrough; keep only if context-token-turns fall ≥ 15 % and expand rate < 5 %.

Falsified by: expand rate ≥ 5 % after two days, or a cache-hit-rate drop that costs more than the CTT save.
## P33 survey — tiered session context L0/L1/L2 (2026-09-11)

Survey for **T33.0** (`plan.md` P33). No implementation; feature stays **off** until Gate P33 has a recorded `Measurement` row. Sources: OpenViking docs (2026-01), MemGPT paper + Letta docs (2023–2026), Claude Code compaction findings (2026), `research.md` §4 row OpenViking, D4 (lossless), D5 (inject budget), D6 (native from scratch).

### License call-out (read before any code)

**OpenViking** (`volcengine/OpenViking`, docs.openviking.ai): the **main project is AGPL-3.0**; `crates/ov_cli` and `examples/` are Apache-2.0. AGPL obliges source offer when modified software is run as a network service. **rtok must not vendor, link, subprocess, or ship OpenViking code** (D6; same bar as every tool in `research.md`). OpenViking is a *behaviour spec* for tier depth and progressive load only. Any L0/L1 bodies rtok stores must be **native, deterministic, and lossless** unless P28's optional LLM path is separately enabled and measured.

**MemGPT / Letta**: Apache-2.0 (`letta-ai/letta`, `letta-ai/letta-code`). Permissive, but still D6 — no import or subprocess; pattern only.

**Claude Code compaction**: proprietary; behaviour inferred from public docs and third-party write-ups, not code to wrap.

### Alternatives surveyed (≥ 3)

| Scheme | Version / date | Tier model | Gets right | Gets wrong for rtok |
|--------|----------------|------------|------------|---------------------|
| **OpenViking** | 2026-01 · AGPL-3.0 (main) | **L0** abstract (~100–256 chars) for recall/filter; **L1** overview (~2–4 K tokens) for navigation/rerank; **L2** full originals on demand. Directory-level `.abstract.md` / `.overview.md` sidecars; filesystem (`viking://`) not flat chunks. | Progressive depth; observable retrieval trajectory; tiered load cuts prompt stuffing. | AGPL + Python/Rust service; LLM-written sidecars; directory semantics ≠ `tool_use_id` archive units; no rtok `Measurement` rows (research.md: 34–91 % claimed, none measured here). |
| **MemGPT / Letta** | paper 2023-10 · Letta Apache-2.0 | **Main context** (core memory blocks + FIFO queue) always in-window; **recall storage** (conversation search); **archival storage** (semantic insert/search). Agent pages between tiers via tool calls (`archival_memory_search`, `conversation_search`). | OS-style paging; core blocks byte-stable across turns; archival separate from live chat. | Extra tool surface and description tokens; paging is agent-driven, not proxy-deterministic; not aligned with `expand <id>` honesty metric. |
| **Claude Code compaction** | docs / findings 2026 | **Tier 1** microcompact (clear stale tool results, no model); **Tier 2** API/cache strategies; **Tier 3** full LLM summarization + session-memory compact (pre-extracted notes). Threshold ≈ 83 % of model window before proactive auto-compact. | Escalating cost ladder; preserves recent turns; cache-safe `cache_edits` on warm prefix. | Host-only; summarization is lossy without rtok store; not measurable on proxy path alone; seven pre-request layers we cannot replicate without owning the host. |

### How tiers compose with v0.1 `archive` + `inject`

v0.1 already implements a **degenerate two-tier** stack:

| rtok v0.1 piece | Tier analogue | Lossless? |
|-----------------|---------------|-----------|
| `archive` pointer in live zone (`expand(<id>)` + head/tail) | **L0** in prompt | L2 in store; pointer is retrievable |
| `expand` / MCP | **L2** load | yes (D4) |
| `inject` titles / one-liners (≤ 800 tokens, byte-stable) | **L0** only at SessionStart/UserPromptSubmit | bodies via `memory`/`graph` MCP, not injected |

**Proposed composition (native, flag-gated — T33.1/T33.2):**

1. **`archive` owns tier materialization and proxy depth.** On archive write: persist **L2** (full `tool_use_id` payload, unchanged). Optionally persist **L1** as a deterministic lossless extract (structure map, head/tail, line/symbol index — same family as `read` signatures, no model in P33). Live-zone rewrite emits **L0** (existing pointer format) by default; config may promote hot ids to **L1** inline without dropping the pointer or breaking prefix stability. **Never** emit L2 in the live zone except after `expand` (T5.4). One `Measurement` row per rewrite records `tier=l0|l1` and token estimate.

2. **`inject` does not load L1/L2 into the cached prefix.** Progressive disclosure for session context stays **out of inject**: memory/graph MCP and `expand` upgrade depth. If tiered loading adds SessionStart lines, they must remain **L0-shaped** (ids/titles/one-liners) and fit the existing 800-token, byte-stable budget (D5). See pointer in `src/plugins/inject/PLAN.md`.

3. **P28 boundary.** LLM-generated L0/L1 abstracts (OpenViking-style sidecars) are **P28** optional compression, not P33 default. P33 tier *routing* may ship with deterministic L1 only; LLM-filled tiers require P28 flag + separate gate.

4. **Flag off.** Bytes on the proxy and inject paths are **identical** to v0.1 `archive`+`inject` (T33.2 Check).

### Chosen direction (for T33.1+)

Native **three-depth store** on existing archive rows (L2 full, L1 deterministic extract, L0 pointer), with proxy selecting depth per block. Spec influenced by OpenViking's *load depth* idea, implemented like MemGPT's *out-of-context until requested* discipline, with Claude Code's *cheapest tier first* ordering — all without vendoring any of the three.

### Gate P33 measurement shape

**Measured against:** v0.1 `archive`+`inject` on the **same** fixture replay (not against a no-rtok baseline).

| Field | Value |
|-------|--------|
| **Fixture** | `tests/fixtures/tier_context/` — deterministic multi-turn transcript: ≥ 10 tool results that cross archive thresholds (> 4 KiB or > 2 turns old), planted `memory` notes, and ≥ 3 `graph` symbols. Checked in; no live traffic. |
| **Replay** | `rtok proxy` passthrough replay of fixture JSONL (or `rtok bench` subset when wired); two configs: `archive.tiers = false` (baseline) and `archive.tiers = true` (treatment). |
| **Primary metric** | **Context-token-turns** (D3): sum over replay turns of `(tokens in live context after rewrite) × (turns until evicted or compacted)`. |
| **Guardrails** | `expand` rate < 5 %; inject prefix ≤ 800 tokens and byte-stable across identical replays; every archived id recovers L2 via `expand` / MCP; prefix bytes before first rewritten block unchanged (T5.3). |
| **Rows** | `Measurement` ledger: `plugin=archive`, `method=tier` (treatment) vs `method=pointer` (baseline); include `before_tokens`, `after_tokens`, `expand_count`, `tier=l0|l1` counts. |
| **Ship bar (review)** | Treatment CTT **≤ 90 %** of baseline on the fixture **and** no guardrail breach. If CTT does not fall, tiers stay off. Record the row window in `research.md` or gate note; unrecorded savings do not exist (D3). |

### Rejected (P33)

- **Vendor or subprocess OpenViking** — AGPL-3.0 copyleft on the main project + D6; measurement would be circular.
- **OpenViking directory sidecars as the storage unit** — rtok archives **`tool_use_id` blocks**, not `viking://` directories; mapping would fight determinism (T5.3).
- **MemGPT-style agent tool paging as the only upgrade path** — adds MCP/hook tool tax; rtok already has `expand` and read/memory/graph MCP.
- **Claude Code Tier-3 summarization as default L1** — lossy without store-backed L2; belongs to P28, not P33.
- **L1/L2 bodies in `inject` SessionStart** — breaks D5 budget and byte-stable prefix; inject stays L0-shaped (see `inject/PLAN.md`).
- **Tier promotion without `expand` id** — would hide expand rate; falsifies the honesty metric.
- **Ship on vendor claims** (OpenViking 34–91 %) — no `Measurement` row in this repo (`research.md` §4).
