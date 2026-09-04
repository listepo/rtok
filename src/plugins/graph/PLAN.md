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

### Known misses (T8.8, measured 2026-09-04)

`tests/graph_truth.rs` scores 30 hand-labelled symbols of this repo, 144 sites in all. Definitions
are found completely, 30/30 with precision 1.000. References are found for 40 of 114, recall 0.351.
All 74 misses come from three constructs the tree-sitter Rust tags query does not capture:

| Construct | Misses | Example |
|-----------|--------|---------|
| Type positions | 64 | `Vec<ToolDef>`, `Surface::Mcp`, `Manifest { .. }`, `fn f(m: Manifest)` |
| Anything inside a macro body | 9 | `assert_eq!(cx.store.measurement_count("graph"), 1)` |
| Path-qualified calls | 1 | `crate::measure::stats::plugin_json(&cfg, x)` |

The query captures references only for a plain call, a field-expression method call, a macro
invocation, and an `impl` trait or type. Macro arguments parse as an opaque `token_tree`, so no
query can reach into them; type positions and `scoped_identifier` calls would need rtok's own
query on top of the grammar's. Recorded as I-31; not attempted in v0.2, where every task is about
what the index already holds.

### Rejected in this round

- A separate `explore` tool (codegraph) — the same result is `symbol` with a body; a fifth tool is description tokens for nothing.
- Communities / hub nodes / wiki generation (code-review-graph, graphify) — unvalidated, and useless under a 2 K cap.
- Per-repo databases (all four) — D8 is one SQLite file; a `root` column gives the same isolation.
- Storing line text in the index — `callers` reads it from disk on demand; the file is the source of truth.

## v0.3 backend survey — LadybugDB (2026-09-04, D18)

The user asked for LadybugDB under the graph plugin. This is the D15 survey for that decision:
what it is, what it costs, the one thing it can win, and the gate that decides.

### What it is

LadybugDB is the MIT community fork of Kùzu, an embedded property-graph database with Cypher.
Kùzu Inc. archived `kuzudb/kuzu` on 2025-10-10 at 0.11.3 (Apple acquired the company); the fork
was created 2025-10-07 under the `LadybugDB` org and is led by Arun Sharma. As of 2026-09-04:
core 0.20.2 (released 2026-09-02, five releases in 40 days, 1.7 k stars, 88 open issues); Rust
crate `lbug` 0.20.2 (2026-09-01), repo `LadybugDB/ladybug-rust`. Storage is one `.lbdb` file
plus a WAL; one read-write `Database` per process; a `Connection` is not thread-safe; in-memory
when the path is empty. Cypher has `-[:R*1..4]->` variable-length paths, `SHORTEST` and
`ACYCLIC`. Not to be confused with Ladybug Tools (PyPI `ladybug`, daylighting) or the Ladybird
browser.

### Backend alternatives

| Backend | Version | Date | Gets right | Gets wrong |
|---------|---------|------|------------|------------|
| LadybugDB `lbug` | 0.20.2 | 2026-09-04 | active fork; Cypher path patterns; single file + WAL; MIT | `build.rs` compiles ~212 K SLoC of C++ (cmake) or pulls a 78 MB prebuilt `liblbug.a` from a mutable branch with no checksum (ladybug-rust #27, closed "not planned"); docs.rs broken since 0.16.1; one RW process per DB; a second file beside `rtok.db` (D8); a run of fixed segfaults through 0.17–0.20 |
| Kùzu `kuzu` | 0.11.3 | 2025-10-10 | the same engine with more history | archived; no fixes |
| SQLite `WITH RECURSIVE` | bundled (Diesel) | 2026-09-04 | zero new dependency; one query for a depth-bounded walk; `Store` already owns it | per-hop joins on a name index, not adjacency lists; no path semantics the CTE does not spell out |
| petgraph | 0.8.2 | 2025-06 | pure Rust; BFS and shortest path in memory | no persistence: rebuild 9 000 rows per process or serialise them yourself |
| indradb | 5.0.0 | 2025-08-16 | pure-Rust API; pluggable stores | last commit 13 months ago; `sled` is "not production-ready", RocksDB is C++ again |
| cozo | — | 2024-12-04 | Datalog with recursive rules | 21 months without a commit |

### What a graph store can and cannot win here

The index is 9 000 rows for 3 000 files; every v0.2 query is one indexed lookup, and a warm call
is 23–26 ms, almost all of it the directory walk no store can remove. `symbol`, `callers` and
`outline` cannot get faster by changing the store. `impact` can: the Rust BFS issues one
`symbol_ref_groups` per frontier definition, so a fan-out-10 walk at depth 4 is 10 000 lookups,
while a path pattern is one query. That is the single clause where LadybugDB can beat SQLite —
and a `WITH RECURSIVE` can contest it with no dependency at all, so the gate measures both.

### Mechanism (P8c)

Same four tools, byte for byte: `tests/graph_contract.rs` (T8.9) pins them through `rtok mcp`,
so the acceptance test never names a store. The seam is one file: the eleven `symbol_*` methods
move to `src/store/symbols.rs` (T8.10) and `src/store/symbols_lbug.rs` is the `cfg`-selected
sibling under feature `graph-lbug` (T8.11–T8.12). No trait — one `impl Store` per file. Cypher
lives in `src/store/` only; the plugin calls the same methods (D13). Ledgers stay in `rtok.db`;
`graph.lbdb` is a derived cache beside it (D8 as narrowed by D18). `impact` becomes one query on
both sides (T8.13); T8.14 measures and Gate P8c decides.

Bar for Gate P8c — clauses (1)–(6) in `plan.md` P8c: contract byte-identical under both builds;
hook p95 ≤ 10 ms; warm calls < 100 ms; `impact(4)` on 10 000 edges ≥ 2× faster than the SQLite
CTE; clean `just check` ≤ 2× and a reproducible build; sizes published.

P8c is falsified by clause (4) lost or tied: then the graph store adds a C++ toolchain and a
second file for nothing the CTE does not do, and its code is deleted.

### Rejected in this round

- Replacing `rtok.db` wholesale with LadybugDB — the ledgers are relational and Diesel-typed (D13); nothing in `calls`, `usage` or `measurements` is a graph.
- A `SymbolIndex` trait with dynamic dispatch — one `impl Store` per `cfg`-selected file is the whole seam; a trait is an interface for a backend that has not earned its place.
- Exposing Cypher as a fifth tool (I-14 stays rejected) — the surface is four tools and 62 description tokens; the store is not the model's business.
- Linking the prebuilt `liblbug.a` by default — an unpinned download in `build.rs` is not a reproducible build: from source, or pinned with a checksum, or the gate fails.
