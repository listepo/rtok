# memory — design note (D15)

## Problem

claude-mem spends tokens extracting observations; engram spends description tokens on 18 tools. Neither is measured against a recall fixture in this repo.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| claude-mem | 2026-09-01 | 2026-09-01 | progressive disclosure; SQLite+Chroma | LLM extraction; not local-LLM-free |
| engram | 2026-09-01 | 2026-09-01 | agent-written notes; FTS5 | 18 MCP tools' descriptions |
| OpenViking L0/L1/L2 | 2026-09-01 | 2026-09-01 | tiered load | AGPL; LLM compression; v0.2+ |
| mem0 / OpenMemory | 2026-09-01 | 2026-09-01 | retrieval eval culture | Docker/Qdrant; not local-first |

## Mechanism

Zero-LLM: the agent writes notes (MCP `memory_write`); FTS5 retrieves titles then bodies on demand. Store: user-approved facts, decisions, bugs. Never store secrets, raw tool dumps, or session transcripts. Recall stays inside the `inject` 800-token budget (titles first). Metric: hit rate on a fixture note set (title+body planted, query must return the id).

The property that beats the table: no extraction tax, and recall is a Check, not a banner percentage.

## Rejected

- LLM extractors in v0.1 (claude-mem, mem0 default) — they spend the tokens we are trying to save.
- Injecting bodies at SessionStart — technique #7 says titles → ids → bodies.

Target: Disable engram + claude-mem for a week; compare injection and MCP description tokens. Revert if recall is worse.

Falsified by: fixture recall below the LLM-extractor baseline on the same notes, or memory injection exceeds the 800-token budget.

## v0.2 survey — embeddings beside FTS5 (2026-09-11)

Survey for **T29.0** (`plan.md` P29). No implementation; embed path stays **off** until Gate P29
passes. Sources: mem0 OSS v3 pipeline docs and PR #4805 (2026-09-11), code-review-graph **2.3.7**
README + `docs/architecture.md` + `docs/USAGE.md` (2026-09-04 / 2026-09-11), sqlite-vec **v0.1.9**
stable + **v0.1.10-alpha.4** ANN pre-release (2026-05-18), Obsidian MCP hybrid-search write-up
(2026), `research.md` §4–§5, D6 (native), D8 (one SQLite file), D13 (`Store` owns SQL). Graph
`semantic_search_nodes_tool` shares this embed stack — survey detail for symbols lives in
`src/plugins/graph/PLAN.md` (T30.0 owns that file this round).

### Problem

v0.1 `memory` recall is **FTS5-only** (`notes_fts`, `bm25` rank). That wins on exact tokens
(T6.1 `walrus` fixture) and costs zero embed tokens, but it misses **paraphrase**: an agent asks
"why did we reject async ORM on hooks?" and no row matches if the note only says `Diesel sync` and
`≤10 ms`. mem0 and code-review-graph both ship **hybrid** retrieval (keyword + vector) as an
**opt-in** layer beside FTS5/BM25; `research.md` marks code-review-graph embeddings as optional and
mem0 as Docker/Qdrant by default. **P29** asks whether rtok can add the same semantic recall for
`mem_search` (and later `graph` symbol search) **without** making vectors mandatory, **without**
wrapping retired MCP servers (D6), and **without** splitting the ledger out of `~/.rtok/rtok.db`
(D8).

### Alternatives

| Tool | Version / docs date | Embed + keyword mechanism | Where vectors live | Fit for rtok P29 |
|------|---------------------|---------------------------|--------------------|------------------|
| **mem0** (mem0ai/mem0) | OSS v3 pipeline · docs + PR #4805 · **2026-09-11** | LLM fact extraction → batch embed → **pluggable vector store**; search fuses **semantic + BM25 + entity boost** (additive score; `score_breakdown.semantic` for raw vector term) | **Split:** SQL metadata DB + separate vector DB (Qdrant default in OpenMemory Docker) | **Spec only (D6).** Hybrid scoring shape is the takeaway; runtime needs Qdrant/Pinecone/Chroma and LLM extraction — violates D8 one-file ledger and v0.1 zero-LLM `memory` lane. |
| **code-review-graph** (tirth8205) | **2.3.7** · README/architecture/USAGE · **2026-09-04 / 2026-09-11** | Tree-sitter index + **`nodes_fts` FTS5** + optional **`embeddings` table**; `semantic_search_nodes_tool` runs **vector cosine** when rows exist else FTS5/`LIKE`; **RRF** merges ranked lists | **Same SQLite file** (`.code-review-graph/graph.db`); `embeddings(qualified_name, vector BLOB, text_hash, provider)`; refresh **default-off**, explicit `--embedding-provider` + `--embedding-model` | **Spec only (D6)** — closest native pattern: optional vectors beside FTS5, local sentence-transformers or cloud embedder with egress warning. rtok copies the **storage + hybrid + opt-in refresh** shape for `notes`, not the 30-tool MCP surface. |
| **sqlite-vec** (asg017/sqlite-vec) · *outside retired stack* | **v0.1.9** stable brute-force KNN · **v0.1.10-alpha.4** DiskANN/rescore ANN · **2026-05-18** | `vec0` virtual table: `embedding MATCH ?` KNN in-SQL; joins to relational `notes` via `rowid`/`note_id`; pure C extension, no server | **Same SQLite database file** as relational tables (Obsidian MCP 2026: FTS5 + `chunk_vecs` + `chunks` in one DB) | **Chosen storage engine (T29.2).** Keeps D8: vectors are not a second process or Qdrant container. Pre-v1 ANN stays behind a feature until measured; note counts start on **brute-force KNN** (fine for agent-scale corpora). |
| **LanceDB** (lancedb/lancedb) · *outside* | **0.15.x** docs · **2026-09-11** | Embedded columnar vector index; Rust/Python SDK; ANN at large scale | **Sidecar directory** (`*.lance` beside or away from `rtok.db`) | **Rejected for P29** — second on-disk format and backup surface; D8 allows a justified sidecar (cf. `graph.lbdb`) but notes already live in `rtok.db` and note cardinality does not need a columnar ANN store yet. Revisit only if Gate P29 brute-force KNN fails latency on a measured corpus. |
| **OpenAI embeddings API** (remote-only path) | `text-embedding-3-small` · API docs · **2026-09-11** | HTTPS embed call; 1536-d vectors; no local model | Vectors wherever the app stores them | **Optional embedder only, not the store.** Cloud egress + API key; acceptable behind explicit `embed.provider = "openai"` (same opt-in bar as code-review-graph `CRG_ACCEPT_CLOUD_EMBEDDINGS`). Default embedder must be **local ONNX** (fastembed-class) so `mem_save` works offline. |

### Mechanism (proposed native design — T29.1/T29.2, not implemented here)

**FTS5 remains the default path.** `mem_search` bytes and ranking stay identical to v0.1 when
`[embed] enabled = false` (absent key counts as false).

**Chosen backend:** **`sqlite-vec` `vec0` table inside `~/.rtok/rtok.db`**, keyed by `notes.id`,
behind Cargo feature **`embed-vec`** (off in default builds until Gate P29). One migration adds:

- `note_embeddings_meta(note_id, model, dims, text_hash, embedded_at)` — Diesel-modelled, joinable.
- `note_embeddings_vec` — `vec0(embedding float[D])` with `rowid = note_id` (or equivalent mapping
  documented in the migration); queried only from `src/store/` (D13).

**Embed text** at index time: `title + "\n" + body` (same lexical content as `notes_fts`, so FTS5 and
vectors stay aligned). **Index on write, not on the hook path:** `mem_save` queues or inline-embeds
only when `[embed] enabled = true`; hooks stay ≤ 10 ms fail-open (D1). Re-embed when `text_hash`
changes; delete vector row on note delete (mirror FTS5 triggers).

**Search when enabled:** run **both** legs, merge with **RRF** (code-review-graph
`search.py:150–173` pattern): (1) existing `notes_fts MATCH ? ORDER BY bm25`; (2) embed query →
`vec0` KNN `LIMIT k`. Progressive disclosure unchanged — `mem_search` still returns ids, titles,
snippets; `mem_get` still returns full body. Shared `Store::embed_*` helpers are what **`graph`
semantic search** and **P31 proxy semantic cache** reuse later (`proxy/PLAN.md` already points at
P29 for tier-2 vectors).

**Config sketch (T29.1):**

```toml
[embed]
enabled = false              # Gate P29: default off
provider = "local"           # "local" | "openai"
model = "all-MiniLM-L6-v2"  # local default; provider-specific
dimensions = 384             # must match model + vec0 column
hybrid = true                # when true: RRF(fts5, knn); when false: knn only (discouraged)
```

**Optional stays optional:** (1) `enabled = false` → no `vec0` table reads, no embed calls, no MCP
tool renames; (2) `embed-vec` feature unset → binary contains no sqlite-vec extension (link error if
enabled without feature); (3) cloud provider requires explicit provider + key env, logged once
(code-review-graph egress warning pattern).

### Rejected in this round

- **Wrapping mem0, code-review-graph MCP, or OpenMemory Docker** — D6; measurement would be circular.
- **Qdrant / Chroma / Pinecone as the default vector store** — second service; breaks D8 one SQLite
  ledger file (`research.md`: mem0 not local-first by default).
- **LanceDB sidecar for `memory` notes** — unjustified while note count ≪ graph symbol count; D18
  sidecar precedent is for the **derived** graph index, not ledgers (`calls`, `notes`, …).
- **Default-on embeddings or hybrid-only search** — P29 gate requires FTS5-only behaviour with flag off.
- **LLM memory extraction to feed vectors** (mem0/claude-mem default) — extraction tax; stays P28
  optional lane, not P29 index input.
- **sqlite-vec ANN (DiskANN) in the first slice** — alpha stability (v0.1.10-alpha.4, 2026-05-18);
  ship brute-force KNN first, promote ANN only with a `Measurement` latency row.

### Gate P29 — fixture shape

**Measured against:** v0.1 FTS5-only `mem_search` on the same in-memory store (flag off = byte-identical
ranking for the FTS5 query).

| Field | Value |
|-------|--------|
| **Fixture file** | `tests/fixtures/p29_memory.toml` (new in T29.2) — checked-in note rows + queries; no live API. |
| **Planted note** | `kind = decision`, `title = p29-gate-arctic-tern`, `body = Hooks must exit in ≤10 ms fail-open; async ORM rejected — Diesel stays sync on the hook path (D13).`, `project = rtok`. |
| **Decoys** | ≥ 2 notes in the same project with unrelated bodies (same pattern as T6.1 `banana` / `other`). |
| **FTS5 query** | `Diesel sync` — must rank `p29-gate-arctic-tern` **first** with `[embed] enabled = false`. |
| **Embed query** | `why not use an async database library for hooks` — shares **no** FTS token with the body; must rank the same note **first** with `[embed] enabled = true` and `hybrid = false` (vector leg alone). |
| **Hybrid query** | `hook database latency budget` — with `enabled = true` and `hybrid = true`, RRF must rank the planted note **first** (proves both legs compose). |
| **Negative** | Flag off + embed query → **empty** or FTS miss (proves embed is not implicit). |
| **Measurement** | One `Measurement` row per gate run: `plugin = memory`, `kind = p29_hybrid_recall`, `bytes_in` = query len, `bytes_out` = hit id present (1/0). |

**Gate P29 (review):** FTS5 remains default; embed path is a config flag; the fixture note above is
found by **both** isolated legs (FTS5 query + embed query) and by hybrid when enabled.
