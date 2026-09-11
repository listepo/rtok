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

Target: Two days of usage rows.

Falsified by: p95 added latency ≥ 20 ms, or a documented cache miss caused by rtok rewriting the prefix.

## v0.2 survey — semantic response cache (2026-09-11)

Promoted from I-23 → P31. v0.1 rejected bifrost's semantic cache on evidence (`research.md` §4,
`docs/comparison.md`); this survey defines what a native, opt-in rtok cache would look like before
T31.1–T31.2. Sources: bifrost docs (getbifrost.ai, 2026-09-11), GPTCache 0.1.44 (2024-08-01),
Redis semantic-cache docs / RedisVL 0.15.0 (redis.io, 2026-09-11), Helicone semantic-cache docs
(maintenance mode under Mintlify, 2026-03), `plan.md` P31/P9, `bench/tasks.toml`.

### Problem

Agent coding sessions send 65 K–100 K+ token contexts per turn (`research.md` §2); the last user
message is often a short paraphrase, but the full message list (system, tools, prior tool results)
does not repeat byte-for-byte. A **semantic** cache embeds a normalized prompt, finds a near
duplicate in vector space, and returns a stored LLM response without calling upstream — saving cost
and latency when it hits, but returning **another turn's answer** when it false-hits.

v0.1 evidence: bifrost at cosine **0.9** in the retired Docker chain produced wrong answers for
agent workloads (`docs/comparison.md`: "agent contexts never repeat; a hit is a wrong answer").
That is different from Anthropic **prompt caching** (provider-side, prefix-exact, D11-aligned) and
from rtok's **archive** live-zone rewrite (lossless, prefix-stable). P31 asks whether any semantic
tier can ship with **zero false hits on the P9 task set**; if not, the flag stays off.

### Alternatives

| Tool | Version / docs date | Mechanism | Threshold / hit rule | Fit for rtok P31 |
|------|---------------------|-----------|----------------------|------------------|
| **bifrost** (maximhq) | ent-v2.1.1-base · docs 2026-09-11 | Opt-in `semantic_cache` gateway plugin; embed via configured provider (`text-embedding-3-small`, 1536-d); vector store Redis/Valkey, Weaviate, Qdrant, or Pinecone; **direct** (hash) and **semantic** (cosine) paths | Default cosine **0.8**; per-request `x-bf-cache-threshold`; skips when `messages` > `conversation_history_threshold` (default 3); partitions on `x-bf-cache-key` + model + provider | **Spec only (D6)** — behaviour to re-implement, not wrap. Proven false-hits on agents at 0.9. |
| **GPTCache** (zilliztech) | **0.1.44** · 2024-08-01 | Python library: pluggable embedder (OpenAI, ONNX, SentenceTransformers), vector store (FAISS, Milvus, SQLite), similarity evaluator (distance, ONNX pair model) | `Config.similarity_threshold` default **0.8**; published QA sweep: at 0.95 → 25 "negative" (wrong) hits / 999 queries; at 0.8 → 92 negatives with 904 positives | **Spec only (D6)** — same pattern as bifrost, explicit false-positive accounting in examples. Not linked at runtime. |
| **Redis semantic cache** (RedisVL / LangCache) | RedisVL **0.15.0** docs · redis.io 2026-09-11 | `SemanticCache`: embed prompt, `FT.SEARCH` KNN on stored prompt vectors, return stored LLM response; metadata TAG filters (tenant, model) | **Distance** threshold on Redis cosine distance **[0, 2]** (0 = identical); `CacheThresholdOptimizer` sweeps threshold vs accuracy; docs recommend tuning from hit/miss logs | **Spec only** — bifrost's recommended backend; rtok should not require Redis for v0.2 (D6, one SQLite store). Same embed→ANN→respond shape. |
| Helicone | cloud/OSS · maintenance 2026-03 | External proxy; `Helicone-Semantic-Cache-Enabled: true`; dashboard similarity knob | Community reports **0.85–0.95** band; 0.92 cited as practical for FAQ | **Rejected** — third-party proxy (D6), sunset risk, not wire-normalizable inside `rtok proxy`. Listed for threshold folklore only. |

**P9 task set** (Gate P31 corpus): `bench/tasks.toml` — six tasks (`add-fn`, `fix-bug`,
`write-test`, `rename`, `explain`, `run-tests`) × `rtok bench` configs A/B × three runs
(`research.md` A/B bench, T9.1). Live runs use `RTOK_BENCH_LIVE=1`; proxied chat bodies land in
`call_io` via the existing usage-capture path.

### Mechanism (proposed native design — T31.2, not implemented here)

Wire-agnostic **cache prompt** (D11): after parsing Anthropic or OpenAI chat JSON, build one
canonical struct — `provider`, `model`, optional `system` text, ordered `(role, content_text)`
messages (tool blocks reduced to stable text or excluded), optional `tools_fingerprint` (hash of tool
**names** only). Responses are stored as completed assistant text + usage metadata, then re-encoded
on the **same wire** the request arrived on. No bifrost binary, no GPTCache import, no Helicone hop.

**Two-tier lookup** (bifrost direct + semantic, without wrapping bifrost):

1. **Direct tier** — SHA-256 of canonical JSON (`CachePrompt`); hit only on byte-stable equality.
   Zero false hits by construction; covers exact replays and bench reruns.
2. **Semantic tier** (opt-in) — embed `CachePrompt` summary (last user text + model + provider;
   **exclude** system and tool-result bodies from the vector until measured safe). SQLite table in
   `rtok.db` holds `(embedding BLOB, response BLOB, metadata)` with TTL; brute-force or small HNSW
   later if row count warrants it. Reuse P29 embed backend when present; until then `embed_backend =
   "hash"` keeps tier 2 off and tier 1 testable.

**Eligibility guards** (reduce false-hit surface; derived from bifrost + P9 shape):

- `require_empty_tools = true` — do not cache turns where `tools[]` is non-empty (agentic turns).
- `max_messages = 1` initially — only single-turn user prompts (no prior tool-result context in the
  vector). Raise only after the false-hit audit passes at a higher count.
- `cache_by_model` and `cache_by_provider` true (bifrost defaults).
- Never cache incomplete SSE streams; write only after upstream completes (contrast v0.1 rejected
  full-stream buffer — here the cache path is off by default and adds no latency when disabled).
- Record `Measurement { plugin: "proxy", kind: "semantic_cache_hit", … }` with `similarity` and
  `ref_id` to archived request/response for audit.

**Opt-in shape** (T31.1; default off — unchanged proxy bytes when disabled):

```toml
[plugins.proxy.semantic_cache]
enabled = false                       # Gate P31: must stay false until audit passes
threshold = 0.99                      # cosine similarity; semantic tier only
ttl_s = 300
max_messages = 1                      # bifrost uses 3; start stricter for agents
require_empty_tools = true
embed_backend = "hash"                # "hash" = direct tier only until P29 embeddings
cache_by_model = true
cache_by_provider = true
```

**Threshold proposal:** ship semantic tier at **cosine ≥ 0.99** (not bifrost's 0.8 default and not
the 0.9 setting that false-hit on agents). Rationale: GPTCache's own QA table shows dozens of wrong
hits at 0.95–0.8; coding-agent contexts are **harder** than FAQ (report.html: contexts do not
repeat at 0.9). Expect **near-zero hit rate** on the P9 set at 0.99 — that is acceptable; the phase
goal is **zero false hits**, not maximum savings. Lower the threshold only after the offline audit
reports 0 false pairs on a production `call_io` sample and a human signs off in `research.md`.
Optional later: bifrost-style per-request threshold header (`x-rtok-cache-threshold`) for
experiments; not in T31.1.

### Rejected in this round

- **Wrapping or subprocess to bifrost, GPTCache, or Helicone** — D6 native; those repos are specs.
- **Default-on semantic cache** — wrong answer is worse than cache miss; opt-in only.
- **Threshold ≤ 0.95** for initial enablement — contradicts v0.1 agent evidence and GPTCache
  negative-hit tables.
- **Caching multi-turn agent contexts** (`max_messages` > 1 with tool results in thread) — primary
  false-hit source; defer until audit proves safety.
- **Caching requests with `tools[]` present** — tool schemas change semantics; exact-match provider
  caching already covers stable prefixes.
- **Mandatory Redis / external vector DB** — D8 one SQLite file; optional Redis backend is not P31.
- **Wire-specific cache keys** without `CachePrompt` normalization — breaks D11 dual-wire proxy.
- **Helicone as the implementation** — external proxy, maintenance mode, duplicates `rtok proxy`.

v0.1 bullet "Semantic response cache (bifrost 0.9) in v0.1" stays rejected; P31 is a **measured
revisit** with stricter guards, not a reversal.

### Gate P31 — false-hit Check protocol

**Definition:** a **false hit** is any proxied chat completion where semantic cache **serves**
response *R* for request *Q*, but the passthrough upstream response for *Q* (ground truth) is
*R′* with *R ≠ R′* (byte compare on normalized assistant text). Direct-tier hash hits where
*Q* is byte-identical to a prior request are never false hits.

**Step 1 — Corpus (cache off):** `RTOK_BENCH_LIVE=1 rtok bench` with config B (`bench/configs/rtok.json`),
`[plugins.proxy.semantic_cache] enabled = false`, `proxy.mode = "passthrough"`. Export every chat
`call_io` request/response pair from the 6 tasks × 3 runs to a frozen fixture
(`tests/fixtures/p9_semantic_cache_corpus/`).

**Step 2 — Offline audit (code-closable, runs in CI without API):** For each pair of corpus
entries (*i*, *j*), *i ≠ j*:

1. Build `CachePrompt` for each; compute embedding (or hash-only mode skips tier 2).
2. If `cosine_sim(i, j) ≥ threshold` **and** `canonical_hash(i) ≠ canonical_hash(j)` **and**
   `response(i) ≠ response(j)` → count one **false-hit pair** (would have served the wrong answer).

Gate requires **false-hit pair count = 0** at the configured `threshold` (initially 0.99) on the P9
corpus. Print hit rate and pair count; append numbers to `research.md` §2 with date.

**Step 3 — Online confirmation (optional live re-run):** Enable semantic cache; rerun P9 with
`RTOK_BENCH_LIVE=1`. Require `pass` equal to Step 1 (6/6 per config B); every `semantic_cache_hit`
row must reference a corpus entry with `similarity ≥ threshold` and `false_hit = 0` in the store.

**Step 4 — Ship rule:** If Step 2 count > 0 **or** Step 3 pass rate drops → feature stays
`enabled = false` in `default.toml`; document failure in this note. If both pass → Gate P31 closed;
T31.2 may ship with documented hit rate (likely ~0 % on P9 at 0.99).

**T31.2 Check:** `enabled = false` → proxy bytes identical to today; `enabled = true` → Step 2
audit runnable via `just check` and reports 0 false-hit pairs on the P9 fixture.
