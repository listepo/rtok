# graph — design note (D15)

## Problem

Four graph servers (~85 MCP tools) fight for description tokens. tree-sitter-tags miss dynamic dispatch, macros, and generated code; that is acceptable at v0.1 if hit rate on a fixture symbol set is measured.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| codebase-memory-mcp | v0.7.0 · 42.1 k★ | 2026-09-04 | 158 grammars; hybrid-LSP dispatch for 11 langs; 15 tools incl. `trace_path`, `get_impact_radius`, `detect_dead_code`; zstd 8–13:1 | 281 MB binary + 141 MB cache on this machine and it exits on start (0 tools in `doctor`); 99.2 % on 5 queries unrepeated; Cypher surface |
| codegraph | 69.4 k★ | 2026-09-04 | one `codegraph_explore` tool: verbatim source + call paths + blast radius in one call (vendor: −88 % tool calls, −62 % tokens, 7 repos) | 97 MB `.codegraph/` for one repo here; 73.8–100 % coverage by language; dispatch / DI unresolved |
| code-review-graph | 2.3.7 · 31.2 k★ | 2026-09-04 | impact radius, `get_minimal_context`, communities; publishes its own accuracy tables | 30 tools (~2 295 desc tokens measured); F1 0.69 against its own edges; “graph context can exceed naive file reads for trivial edits” |
| graphify | v8 · 114 k★ | 2026-09-04 | 37 grammars; Leiden communities; HTML map | JSON graph without an index; docs/PDF layer needs LLM calls (not D6-able); not installed here |
| serena | serena-agent · 28.7 k★ | 2026-09-04 | LSP precision; rename / replace body | Python + LSP subprocess; 22 tools ~1 494 desc tokens; times out at 30 s here |
| aider repo map | 0.82 | 2026-09-02 | tree-sitter tags + PageRank under a fixed 1 K budget | file-level ranking; a map, not a query |
| Universal Ctags | 6.x | 2026-09-02 | cheap tags; many langs | no call graph; regex-ish |

## Mechanism

tree-sitter tags index in-process. Three MCP tools only: `symbol`, `callers`, `outline`. Refresh on mtime of indexed files, not every request. Output capped (N lines) with `expand` for the rest. Dynamic dispatch / macros / generated code are known misses; LSP is v0.2 (`ideas.md` Later).

The property that beats the table: three tools, < 2 s index of this repo, measured hit rate — not 85 descriptions.

## Rejected

- Shipping serena as a subprocess (D6).
- Cypher-like query language in v0.1 — three named tools are enough.

Target: Description-token savings vs the four servers; index this repo in < 2 s.

Falsified by: index of this repo ≥ 2 s, or fixture symbols that tags should see (plain fn/impl) are misses.

## v0.2 survey — what the field does that v0.1 does not (2026-09-04)

Re-surveyed after Gate P8. Four tools, ~4 500 description tokens retired; rtok's `graph` costs ~117 and indexes this repo in 0.48 s. What is left is quality, not quantity. Sources: `research.md` §4/§6, `scratchpad` reports of 2026-09-04.

### Gap matrix

| Capability | Who has it | rtok v0.1 | Verdict |
|------------|-----------|-----------|---------|
| Source in the answer ("explore" in one call) | codegraph `codegraph_explore`, codebase-memory-mcp `get_code_snippet` | `symbol` returns `path:line kind`; the agent's next call is `read` | **T8.6** — fold the definition body into `symbol` |
| Caller = enclosing function, not a line | all four | reference lines grouped by file; `callers("new")` is noise | **T8.5** — scope edges at index time |
| Transitive impact / blast radius | codebase-memory-mcp, code-review-graph, codegraph | none | **T8.7** — one BFS over T8.5 edges, fourth tool |
| One store, many repos | all four keep a DB per repo | `symbols` has no root column: indexing repo B deletes repo A's rows (`delete_symbols_missing`), `src/main.rs` collides, `mark_symbols_stale` suffix-matches across repos | **T8.3** — correctness bug, first |
| Freshness without re-reading | code-review-graph 2.5 s per edit on 3 k files; codebase-memory-mcp RAM-first | every tool call walks the repo and reads + sha256s every supported file (0.03 s on 61 files; seconds on 3 k) | **T8.4** — stat gate (mtime + size) before sha |
| A published accuracy number | code-review-graph (own edges, F1 0.69); codebase-memory-mcp (5 queries) | none; this note promised a "measured hit rate" and T8.2 shipped without one | **T8.8** — hand-labelled fixture, recall printed by a test |
| Dynamic dispatch, traits, generics | codebase-memory-mcp hybrid LSP | over-approximation by name | I-24 (LSP backend), v0.2+ |
| 30–158 languages | all four | 7 grammars | I-16; grammars are data, add when a measured repo needs one |
| Communities, hubs, wiki, HTML map | code-review-graph, graphify | none | rejected — no tool validates communities against an architecture; nothing an agent can act on under a 2 K cap |
| Cypher / query language | codebase-memory-mcp | none | I-14; promote only when a measured query misses the named tools |
| Embeddings / semantic search | code-review-graph (optional) | FTS5 via `read search` | I-22, v0.2+ |
| Compressed index | codebase-memory-mcp zstd | plain rows | not needed: 6 625 rows for this repo; revisit above 10⁶ rows |
| Dead-code list | codebase-memory-mcp `detect_dead_code` | none | I-29, idea only (pub API and trait impls make it noisy) |

Design lessons kept from the survey: one rich tool beats thirty thin ones (codegraph vs code-review-graph); a fixed per-response budget is a feature (aider 1 K, code-review-graph 2–3.5 K, rtok 2 K); accuracy measured against a tool's own edges is an upper bound, not a number; small edits must stay cheaper than a graph call (code-review-graph's own caveat).

### Proposed tasks (plan.md format; promote by moving them under P8)

Order: T8.3 → T8.4 → T8.8 → T8.5 → T8.6 → T8.7. Measure (T8.8) before the edges land so their effect on recall is visible. Each ≤ 200 LOC, ≤ 3 files, one commit on `main`.

**T8.3 per-root index** · T8.1 · `migrations/0004.sql`, `src/store/mod.rs`, `src/plugins/graph/index.rs`
Do: column `root` on `symbols` (canonical root of the call, today the cwd of `rtok mcp`); every symbol query, `delete_symbols_missing`, and `mark_symbols_stale` are scoped to it; stale marks resolve the absolute path against the root instead of suffix `LIKE`; `keep` becomes a `HashSet`.
Check: index two fixture roots into one store; indexing B leaves A's row count unchanged; `symbol("main")` from A never lists B; stale-marking A's `src/main.rs` keeps B's rows.

**T8.4 stat-gated freshness** · T8.3 · `migrations/0005.sql`, `src/store/mod.rs`, `src/plugins/graph/index.rs`
Do: store `mtime` and `size` per file (git's index rule); a file whose stat matches is skipped without being read; sha256 only when the stat differs; unchanged content with a new mtime re-hashes but inserts 0.
Check: generated 3 000-file fixture, release build: warm `run` reads 0 bodies and returns in < 100 ms; one edited file re-parses only itself; `touch` alone inserts 0 rows.

**T8.8 labelled hit rate** · T8.2 · `tests/graph_truth.rs`, `tests/fixtures/graph_truth.toml`, `research.md`
Do: 30 symbols of this repo labelled by hand (definition sites + reference sites, cross-checked with `rg`); the test computes precision and recall of `symbol` and `callers`; the numbers go to `research.md` §3 with a date.
Check: recall ≥ 0.9 on plain fn / struct / impl symbols; precision printed; every miss listed in this note under "Known misses" with the tree-sitter construct that caused it.

**T8.5 call edges** · T8.3 · `src/plugins/read/outline.rs`, `src/plugins/graph/index.rs`, `migrations/0006.sql`
Do: `TagHit` carries the definition's end line (tree-sitter-tags `range`); at index time each reference gets `scope` = innermost enclosing definition in the same file (`''` at file level). `callers(name)` groups by scope with a count and first line — `src/plugin.rs  fn estimate ×3 (L41)` — under the same cap.
Check: fixture `fn a(){b()} fn b(){c()}`: `callers("c")` → `b`, `callers("b")` → `a`; a file-level call reports `''`; bytes of `callers("estimate")` on this repo ≤ v0.1 bytes.

**T8.6 `symbol` returns the definition** · T8.5 · `src/plugins/graph/mod.rs`, `docs/config.md`
Do: after each `path:line kind`, the definition's source from `line` to `end_line`, at most `plugins.graph.body_lines` (default 40) per definition, whole definitions first; the same cap and `expand <id>` for the rest. This is codegraph's one-call explore without a new tool.
Check: `symbol("cap")` returns the body of `cap` verbatim; a 500-definition fixture is still capped with an archive id; one `graph` measurement per call; on the P9 task set the `calls` table shows fewer `read` calls in the two turns after a `symbol` call than with v0.1.

**T8.7 `impact(name, depth)`** · T8.5 · `src/plugins/graph/mod.rs`, `src/store/mod.rs`
Do: BFS over `scope` edges up to `depth` (default 2, max 4); lines `depth  path  fn`, capped. Fourth and last tool; description ≤ 25 tokens.
Check: chain fixture `impact("c", 2)` → `b` (1), `a` (2); `depth = 1` omits `a`; `rtok doctor` reports the graph surface ≤ 4 tools and ≤ 150 description tokens.

Gate P8b (after T8.7): graph surface ≤ 150 description tokens (`doctor`); warm tool call < 100 ms on a 3 000-file repo; T8.8 recall ≥ 0.9; on the P9 bench, tasks touching ≥ 3 files use fewer tool calls with v0.2 `symbol` than with v0.1 (`calls` table). Revert T8.6 if tool calls do not fall; revert T8.7 if `impact` is never called across a week of `calls`.

### Rejected in this round

- A separate `explore` tool (codegraph) — the same result is `symbol` with a body; a fifth tool is description tokens for nothing.
- Communities / hub nodes / wiki generation (code-review-graph, graphify) — unvalidated, and useless under a 2 K cap.
- Per-repo databases (all four) — D8 is one SQLite file; a `root` column gives the same isolation.
- Storing line text in the index — `callers` reads it from disk on demand; the file is the source of truth.
