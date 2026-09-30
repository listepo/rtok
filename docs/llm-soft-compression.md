# LLM soft compression (P28)

**Status: Later / design + extractive shipped; LLM lane still gated.**  
Research and roadmap only — no new runtime behaviour in this document.
Branch context: `docs/batch-flex-pass` (token-saving research alongside
Batch/Flex and model routing).

Sources of intent: `ideas.md` (I-21, “LLM soft compression (P28)”),
`plan.md` / `done.md` **P28** (T28.0–T28.2),
`src/plugins/compress/PLAN.md` (D15 survey), `roadmap.md` Later,
`docs/prompt-cache.md`, `docs/batch-flex.md`, `docs/model-routing.md`,
`research.md` §2 / §6.9 / §9.2.

## 1. What “soft compression” means here

In rtok, **LLM soft compression** is an **optional, measured, archive-first
rewrite of in-context text** (typically old `tool_result` bodies already
held under `expand <id>`), produced by a **model call** (or a small
specialist compressor), **not**:

- KV-cache / activation quantization inside a provider GPU
- Attention sinks, sliding-window, or RoPE scaling in the serving stack
- Host auto-compact (`PreCompact` / `preCompact`) — that is T58.2 /
  checkpoint territory
- The **deterministic extractive** summary already shipped in
  `plugins.compress` (T28.2 / T127)

“Soft” = the **view** in the prompt may drop or paraphrase bytes; the
**hard** contract (D4) keeps the original in `~/.rtok/archive/` so
`rtok expand <id>` / MCP `expand` recovers non-regenerable sources.

### Already shipped (not the Later item)

| Piece | Behaviour today |
|-------|-----------------|
| Lossless archive + `expand` | Always on for filtered/archived payloads |
| `plugins.compress` extractive ranker | Default **on** (`enabled = true`); runs only when `proxy.mode = "compress"`; no network / no LLM; `Measurement` rows |
| Live-zone / cache-preserving proxy rewrites | Old tool results shrink without churning system/tools/live edge (`docs/prompt-cache.md`) |
| Output / prose compress modes | Extractive + caveman-style (`modes`, comparison benches) |

The item is still tracked in `ideas.md` (I-21) as **To add** because
the **LLM lane** (LLMLingua-class or claude-mem-style observation extraction
behind an explicit flag, Gate P28) is **not** the default extractive path.
T28.2 landed the extractive implementation and deferred the memory
observation LLM extractor.

### I-21 → P28 (verbatim intent)

From `ideas.md` Later:

> **I-21** (LLMLingua-2, claude-mem extraction) → **promoted P28** —
> LLM-based compression and/or observation extraction. Default off.
> Costs tokens; quality risk on code. Ship only if a bench beats v0.1
> lossless.

From `roadmap.md` Gate P28:

> `rtok bench` cost per passed task against the v0.1 lossless path must
> not rise, and `expand` still recovers a non-regenerable original.

Canonical design note: `src/plugins/compress/PLAN.md` (T28.0).

## 2. Approach survey (what fits a proxy / agent runtime)

rtok is middleware: hooks, MCP, and especially **`rtok proxy`** on sync
chat wires. It does **not** own the model’s KV cache or sampler. Approaches
fall into three buckets.

### A. Realistic for rtok (HTTP / archive plane)

| Approach | Idea | Fit | Caveat |
|----------|------|-----|--------|
| **Archive-first semantic summary (chosen)** | After lossless archive, replace in-context blob with structured summary (type / title / facts / files); `expand` keeps original | **Primary P28 mechanism** (`compress/PLAN.md`) | Costs compressor tokens; code/diff quality risk |
| **Extractive token/span keep-drop (LLMLingua-2 class)** | Small classifier scores tokens; drop low-info spans | Useful as **local / offline** lane or bench baseline; idea already surveyed; Microsoft package itself rejected (D6) | Not lossless without archive back-pointer; weak on identifiers |
| **Observation extraction → `memory`** | Post-tool LLM writes notes; progressive disclosure | T28.2 optional half; ties I-73 / I-79 “LLM half stays P28” | Wrong facts poison recall; default off |
| **Host compact hooks + archive ids** | Survive provider summarisation | T58.2 (separate card); complements soft compress | Does not shrink bytes itself |
| **Model routing for compressor calls** | Cheap model for summarize / classify | Orthogonal lever (`docs/model-routing.md`); use for **compressor** spend, not agent coding model | Needs router Check; do not conflate with Batch/Flex |

### B. Adjacent but not “soft compress”

| Approach | Why it is a different lever |
|----------|-----------------------------|
| **Batch / Flex** | Changes *when / which bill tier* the same bytes ride (`docs/batch-flex.md`). Soft compress changes *how many* prompt bytes sync wires send. |
| **Prompt cache sticky prefix** | Soft compress must **never** rewrite system / tools / live edge (T14.5 / D). |
| **Semantic response cache (P31)** | Caches **answers**, not prompt shrink; false-hit risk. |
| **TOON / cmd formatters / tools_rewrite** | Deterministic lossless or allowlisted rewrites — already v0.1. |

### C. Out of scope for rtok (serving-model internals)

| Approach | Why rtok cannot ship it as a plugin |
|----------|-------------------------------------|
| **KV-cache compression / eviction** | Lives in vLLM / TensorRT-LLM / provider; rtok never sees tensors |
| **Activation / weight quantization for inference** | Deploy-time serving; not proxy middleware |
| **Attention sinks / sliding window / LongRoPE** | Model architecture / serving config |
| **Speculative decoding / Medusa** | Latency, not prompt-token economy on the agent wire |

If a provider exposes an API flag for “compressed context”, treat it as a
**Wire prepare** experiment with a Measurement — still not KV soft-compress
inside rtok.

## 3. Mechanism to keep (decision)

Reaffirm `src/plugins/compress/PLAN.md`:

1. **Lossless lane** — unchanged, always available.
2. **Extractive lane** — shipped; deterministic; default on when
   `proxy.mode = "compress"` and `plugins.compress.enabled`.
3. **LLM lane (Later)** — default **off**; only after archive row exists;
   only on eligibility matching archive (e.g. older than N turns or over
   size cap); never touch cached prefix; write `Measurement` with
   `bytes_in` / `bytes_out` / `tokens_spent` / `plugin = compress` (or
   `memory` for observation extractor).
4. **Gate** — P9-style `rtok bench`: **cost per passed task ≤** lossless
   baseline (compressor tokens included); pass rate must not fall;
   `expand` recovers non-regenerable originals.

Rejected (still): wrapping Microsoft LLMLingua / claude-mem worker /
headroom (D6); default-on LLM; pruning without archive; rewriting prefix.

## 4. Roadmap (phases)

Priority vs other Later items in `ideas.md`: **below** Batch/Flex pass-through
and **below or beside** model-routing measurement — soft compress spends
tokens to save tokens; Batch/Flex and routing are often cheaper first
dollars. Promote cards into `plan.md` before coding.

### Phase 0 — Inventory (done)

| | |
|--|--|
| **DoD** | T28.0 PLAN exists; I-21 → P28; Gate text in `roadmap.md`; extractive lane + flag in tree (T28.1/T28.2); this doc + `ideas.md` I-21 pointer |
| **Deps** | — |
| **Insert** | `src/plugins/compress/PLAN.md`, `done.md` P28 |

### Phase 1 — Measurement & eligibility (docs + benches, no LLM yet)

| | |
|--|--|
| **Goal** | Know *where* LLM spend could win: share of archived tool-result bytes still shown live; CTT vs bill; code vs prose mix |
| **Do** | Dated `research.md` §2 rows from `rtok stats` / archive replay; define eligibility YAML/fixture (turn age, size, MIME/family); golden set of “must not drop” spans (paths, error codes, rustc spans) |
| **DoD** | Public numbers with dates; eligibility checklist; falsifier cases listed; no production LLM calls |
| **Deps** | Phase 0 |
| **Insert** | `research.md`, optional `tests/fixtures/p28_*`, bench harness notes under `src/plugins/compress/` |
| **Risk** | Optimising for CTT while Gate is **$/passed task** |

### Phase 2 — LLM summarize lane (default off)

| | |
|--|--|
| **Goal** | Optional host LLM (or configured cheap model) produces structured summary view for eligible archived blobs |
| **Do** | Config keys e.g. `plugins.compress.llm.enabled` (default false), model / max tokens / timeout; call path only from `proxy_filter` after archive id exists; store summary as view metadata if useful; `Measurement.tokens_spent` |
| **DoD** | Flag off → byte-identical to extractive/lossless today; flag on → fixture shrinks + `expand` recovers; Gate P28 bench green or **do not merge** |
| **Deps** | Phase 1; ideally cheap model via existing proxy upstream (may use **model routing** later for compressor-only rewrite) |
| **Insert** | `src/plugins/compress/mod.rs`, `src/config/`, `config/default.toml`, `docs/config.md`, proxy only (`src/proxy/mod.rs` already runs `proxy_filter`) |
| **Risk** | Code corruption; cache bust if eligibility wrong; compressor cost dominates savings |

### Phase 3 — Memory observation extractor (optional)

| | |
|--|--|
| **Goal** | claude-mem-style facts into `memory` with archive back-pointer (I-21 second half; I-73 LLM half) |
| **Do** | Async or deferred extract; progressive disclosure search → expand; share Measurement taxonomy |
| **DoD** | Default off; notes cite `expand <id>`; no silent drop of tool payloads |
| **Deps** | Phase 2 (or shared LLM client helper); `memory` plugin |
| **Insert** | `src/plugins/memory/`, compress→memory hook boundary |
| **Risk** | Hallucinated facts; double-count with host auto-memory (T59.7 doctor advice) |

### Phase 4 — Native extractive upgrade (optional, still no Microsoft wrap)

| | |
|--|--|
| **Goal** | Improve deterministic ranker (LLMLingua-2 *idea*) using in-tree or ONNX weights under D6 rules — only if Phase 2 loses Gate on net $ |
| **DoD** | Bench vs Phase 2; D6 satisfied; Measurement rows |
| **Deps** | Phase 1 numbers showing extractive headroom |
| **Risk** | Shipping weights / license; little gain vs T127 ranker |

### Phase 5 — Cross-lever integration (Later+)

| | |
|--|--|
| **Batch / Flex** | Soft compress stays on **sync** agent wires; Batch JSONL remains byte-identical (no compress) — see `docs/batch-flex.md`. Do not attach Batch to sync `Wire`. |
| **Model routing** | Prefer routing **compressor** calls to a cheap model; do not silently change the agent’s coding model for P28. |
| **Prompt cache** | Eligibility must exclude prefix; document bust cases next to `docs/prompt-cache.md` ledger. |
| **DoD** | Integration notes + Checks that Batch/pass-through and prefix stability hold with LLM lane on |

## 5. Integration map (where code would change)

| Area | Role |
|------|------|
| `src/plugins/compress/mod.rs` | `proxy_filter`, extractive today; LLM lane behind new flag |
| `src/plugins/compress/PLAN.md` | Normative design; keep in sync when phases promote |
| `src/plugins/archive/` | Archive id + eligibility; expand path |
| `src/plugins/memory/` | Phase 3 observation extractor |
| `src/proxy/mod.rs` | `compress()` pipeline order (`record` → compress → …); never Batch |
| `src/config/` + `config/default.toml` + `docs/config.md` | Flags; default LLM **off** |
| `src/measure/` / `rtok bench` | Gate P28 cost-per-passed-task |
| Hooks / MCP | Do **not** implement LLM soft compress on hook JSON; proxy owns chat bodies |

## 6. Risks and falsifiers

1. **Cost per passed task rises** after counting compressor tokens → do not ship.
2. **`expand` fails** on non-regenerable tool/read/diff → D4 failure; block.
3. **Claimed saving without `Measurement`** → D1; saving does not exist.
4. **Prompt-cache bust** from rewriting system/tools/live turns → regressions in `docs/prompt-cache.md` hit rates.
5. **Code-shaped text** quality (research.md §6.9 / caveman lessons) → require golden “must keep” spans in Phase 1.
6. **Confusing extractive (on) with LLM (Later)** in docs/marketing → this file and `ideas.md` naming.

## 7. Priority (summary)

| Priority | Item | Rationale |
|----------|------|-----------|
| P0 done | Lossless + extractive compress | Already measured path |
| P1 research | Phase 1 measurement | Without it Gate is guesswork |
| P2 Later | Phase 2 LLM summarize | Real I-21 / `ideas.md` item |
| P3 Later | Phase 3 memory extractor | Same P28 umbrella; optional |
| P4 optional | Phase 4 native ranker upgrade | Only if LLM loses Gate |
| Parallel | Batch/Flex, model routing | Usually better $/effort first |

## 8. Related docs

- `src/plugins/compress/PLAN.md` — T28.0 survey and mechanism
- `docs/batch-flex.md` — sync vs Batch (no soft compress on Batch)
- `docs/model-routing.md` — cheap model for mechanical jobs
- `docs/prompt-cache.md` — prefix / live-edge rules
- `docs/comparison.md` — lossless vs competitor banners
- `ideas.md` I-21, I-73, I-79; `roadmap.md` Gate P28

