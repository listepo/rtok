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

Since then rtok's own `RUST_SCOPED_CALL` (`src/plugins/read/outline.rs`) captures path-qualified
calls. `reference_capture_matches_the_known_misses` (`tests/graph_truth.rs`) pins the current set
on a one-file repo: plain, path-qualified and method calls found; type positions and macro
arguments missed. Re-scored 2026-09-10 after a fixture repair (T34.9): 147 sites, definitions
42/42, references 32/105, recall 0.305. The fall from 0.351 is labels the tree dropped, not the
index; `every_label_names_a_file_that_mentions_the_symbol` now catches those.

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

### Measured (T8.14, 2026-09-08)

Release, this machine. Clause (4) won: `impact(4)` on 11 110 edges is 371 ms (`lbug` path) vs
28.5 s (SQLite CTE), 77×. The CTE also lost to the Rust BFS (2.6 s) on that fixture. Clauses
(2) and (3) fail on the `graph-lbug` binary (hook p95 97 ms, warm calls 0.78–0.87 s). Default
SQLite meets them (8.07 ms / 18–27 ms). `graph-lbug` stays opt-in. Full table: `research.md` §2.

### Rejected in this round

- Replacing `rtok.db` wholesale with LadybugDB — the ledgers are relational and Diesel-typed (D13); nothing in `calls`, `usage` or `measurements` is a graph.
- A `SymbolIndex` trait with dynamic dispatch — one `impl Store` per `cfg`-selected file is the whole seam; a trait is an interface for a backend that has not earned its place.
- Exposing Cypher as a fifth tool (I-14 stays rejected) — the surface is four tools and 62 description tokens; the store is not the model's business.
- Linking the prebuilt `liblbug.a` by default — an unpinned download in `build.rs` is not a reproducible build: from source, or pinned with a checksum, or the gate fails.


## v0.4 backend spike — Grafeo (2026-09-12, T8.20)

User asked to try Grafeo (`grafeo` 0.5.42, Apache-2.0, pure Rust LPG) as an opt-in stand-in for frozen `graph-lbug`. Default stays SQLite. Ledgers stay in `rtok.db`. Feature `graph-grafeo` is **off** by default; `--all-features` prefers it over `graph-lbug`.

### What shipped

`src/store/symbols_grafeo.rs` is the `cfg` sibling of `symbols.rs`: same `symbol_*` methods, GQL/native writes only in `src/store/` (D13). Derived file `graph.grafeo` beside `rtok.db`. Crate features: `edge` + `wal` + `grafeo-file` (LPG+GQL+persist; no ONNX/AI). `Session` has no begin/commit on that feature set, so statements auto-commit. Writes use the native `create_node_with_props` / `create_edge` API — a 3-way GQL `INSERT` of `CALLS` was ~70 s on 80 nodes in debug. Path query is `MATCH p = (anc)-[:CALLS*1..n]->(start)` (`ACYCLIC` prefix is a syntax error on 0.5.42). Multi-statement GQL executes only the first statement.

### Measured / recommendation

See `research.md` §2 "Grafeo vs SQLite (P8e, T8.20)". **Recommendation after numbers:** keep investigating / abandon / adopt opt-in — filled after the release `graph_bench` run on this machine.


## P30 survey — LSP behind tags MCP (2026-09-11)

Survey for **T30.0** (`plan.md` P30). No implementation; feature stays **off** until Gate P30 passes with a recorded `Measurement` row. Sources: `research.md` §4 (serena, codebase-memory-mcp), `docs/comparison.md` §5, serena `solidlsp` (`rust_analyzer.py`, PR #1173 PATH detection, 2026), rust-analyzer book + `lsp/ext.rs` (`workspace/symbol` scope/kind filters), clangd 20 release notes (outgoing call hierarchy, 2026), typescript-language-server `lsp-server.ts` (NavTree + References, `didOpen` / `projectLoadingFinish`, 2026). Complexity: 3/5. Claimed: Composer 2.5.

### Problem (what tags cannot see)

T8.8 measured reference recall **0.305–0.351** on this repo (`research.md` §2; `PLAN.md` "Known misses"). Definitions are complete (30/30); **74 reference misses** cluster on three constructs the tree-sitter Rust tags query never captures: **type positions** (64), **macro argument bodies** (9), and (before `RUST_SCOPED_CALL`) path-qualified calls (1). The gap matrix already names **dynamic dispatch, traits, generics** as over-approximation by name on the tags path (I-24). Serena's LSP backend is the precision ceiling the field offers (`research.md`: 22 tools ~1 494 desc tokens; times out at 30 s here; most precise). rtok's v0.1 answer is four tools in **62 description tokens** and warm calls in **23–26 ms** — P30 must not trade that surface for serena's tool list.

### Serena / field reference (behaviour spec, not a dependency)

Serena (oraios, MIT, ~28.7 k★) implements **solidlsp**: a Python JSON-RPC client that **spawns** a language server per language (`RustAnalyzer` → `rust-analyzer` from rustup, validated `PATH`, or common install paths). Its MCP tools are thin wrappers over LSP — e.g. `find_symbol` → `workspace/symbol` + `textDocument/definition`, `find_referencing_symbols` → `textDocument/references`, `get_symbols_overview` → `textDocument/documentSymbol`. It ships **22 tools** (rename, replace body, …) rtok deliberately does not expose. **D6 forbids spawning serena** (rejected in v0.1 `PLAN.md` §Rejected); P30 re-implements the *precision* of that stack natively: an in-tree LSP client talking to **standard language-server binaries**, not to serena or codebase-memory-mcp.

codebase-memory-mcp's **hybrid LSP dispatch** (11 langs) is the same idea at a different scale: tree-sitter index plus LSP for precision gaps. rtok already has the tree-sitter half; P30 adds the LSP half behind one config flag.

### Backend alternatives (≥ 3)

| Alternative | Version / date | Gets right | Gets wrong for rtok |
|-------------|----------------|------------|---------------------|
| **A. Native Rust LSP client + spawned language-server binaries** | rust-analyzer (rust-lang, 2026); clangd 20+ (LLVM, 2026); typescript-language-server (master, 2026) | **D6-native** client in `src/plugins/graph/`; serena-grade type resolution; one long-lived server per `(root, language)` over stdio; maps cleanly onto the four existing MCP tools | Cold start + project load (serena **30 s timeout** here); must `didOpen` files and wait for `projectLoadingFinish` (tsserver returns partial refs until then); proc-macros / `build.rs` need a compilable workspace; three binaries to detect on `PATH`, not one index |
| **B. Full backend swap (`tags` default, `lsp` optional)** | rtok config (T30.1) | Gate P30 is honest: **flag off → byte-identical tags answers**; flag on → every `symbol` / `callers` / `impact` / `outline` call routes through LSP only; no duplicate tool names | No fast path when LSP is slow; tags index work is wasted while `lsp` is on unless we also skip indexing (T30.2 choice) |
| **C. Tags-first with LSP fallback on miss** | codebase-memory-mcp hybrid pattern (spec) | Keeps **23 ms** warm tags for the common case; LSP only when `symbol_refs` is empty or a precision bit is set | **Fails Gate P30 "off → tags-only bytes"** if fallback runs with the flag nominally off; two divergent code paths per tool; harder to test (`graph_contract.rs` needs deterministic mode) |
| **D. Rust-only LSP ship, other languages later** | rust-analyzer only at T30.2 | Smallest T30.2; rtok's own repo is the gate fixture language; clangd/tsserver adapters share the same client trait | TS/C++ repos get no lift until a follow-up; risks a Rust-shaped API in the client |

**Chosen direction (T30.1+):** **A + B** — native in-process JSON-RPC client, spawn **rust-analyzer** / **clangd** / **typescript-language-server** from `PATH` (with `--version` smoke test, serena PR #1173 pattern), `plugins.graph.backend = "tags"` (default) vs `"lsp"`. Language picked by file extension and project markers (`Cargo.toml`, `compile_commands.json`, `tsconfig.json`). **C** rejected for gate honesty; **D** is an acceptable T30.2 slice if scope bites, but the survey prices all three servers now so adapters do not surprise a later task.

### LSP method → MCP tool mapping (stable names)

Agents, hooks, and `tests/graph_contract.rs` pin **`symbol`**, **`callers`**, **`impact`**, **`outline`** — not serena's `find_symbol`, not a fifth tool. LSP is an implementation detail behind `graph` `invoke`; `mcp_tools()` and `tools/call` stay unchanged (T8.9 lesson).

| MCP tool (unchanged) | Tags backend today | LSP backend (when `backend = "lsp"`) | Notes |
|----------------------|-------------------|--------------------------------------|-------|
| **`symbol(name)`** | `symbol_defs` + body lines from disk | `workspace/symbol` (query = name; rust-analyzer `searchScope` / `searchKind` when supported) → `textDocument/definition` or hover range → read definition body from disk; same cap + `expand` | Trait/default items: `textDocument/implementation` enriches impl sites serena exposes as separate tools — fold into `symbol` lines, no new tool |
| **`callers(name)`** | `symbol_ref_groups` (scope edges from tags) | Resolve definition → `textDocument/references` (`includeDeclaration: false`) → map each ref to enclosing `documentSymbol` for `path  scope ×N (Lline)` shape (T8.5) | rust-analyzer `find_all_refs` excludes imports/tests per config; same grouping contract as tags |
| **`outline(path)`** | `read` map mode / per-file tags | `textDocument/didOpen` if needed → `textDocument/documentSymbol` (hierarchical) | tsserver: NavTree requires open buffer + loaded project (issue #256); wait on `projectLoadingFinish` progress |
| **`impact(name, depth)`** | BFS over stored `scope` edges | `textDocument/prepareCallHierarchy` at definition → repeated `callHierarchy/incomingCalls` to `depth` (rust-analyzer + clangd 20+; same BFS shape as `impact_bfs`) | Outgoing direction is callees, not callers — do not add a tool; depth cap unchanged (max 4) |

### MCP-name stability rule

1. **No renames, no aliases, no serena names in `tools/list`.** Hosts hardcode `symbol`, `callers`, `impact`, `outline`.
2. **No fifth graph tool** for LSP-only affordances (rename, `find_implementations`, Cypher). Extra precision is folded into the four responses or left to `read`.
3. **`plugins.graph.backend = "tags"` path is tags-only** — when `backend` is `"tags"` (default), responses are **byte-identical** to today's tags implementation (Gate P30 clause 1). Enabling `"lsp"` may change bytes; disabling must restore them.
4. **`graph_contract.rs` is the contract** — output shape changes only in a task commit that updates expected strings; both backends must pass or the backend is not done.

### Mechanism (T30.1 / T30.2 sketch)

1. **Config** (T30.1): `plugins.graph.backend = "tags" | "lsp"` (default **`tags`**); optional `plugins.graph.lsp.server.<lang>.path` overrides per language (serena `ls_specific_settings` shape, spec only).
2. **Lifecycle**: one LSP session per MCP server process per workspace root; `initialize` with `rootUri`, `didOpen` on demand; shutdown on MCP exit. **Not** on the hook path (D1 ≤ 10 ms).
3. **Dispatch** in `mod.rs`: `backend == "tags"` → existing `index` + `symbol_*` store methods; `backend == "lsp"` → `lsp::` module, no tags walk on that call (index may still run for `tags` mode only).
4. **Measurement**: each LSP tool call records `plugin=graph`, `method=lsp.<tool>`, latency ms — compare against tags warm **23–26 ms** in `research.md`; unrecorded precision claims do not exist (D3).

### Gate P30 fixture (tags miss, LSP hit)

**Fixture:** `tests/graph_truth.rs` → `reference_capture_matches_the_known_misses` (`truth-constructs` temp repo). Source pinned in-test:

```rust
pub struct OnlyTyped;
pub fn user(r: Recv, t: Vec<OnlyTyped>) { /* … */ }
```

| Query | Tags (`backend = "tags"`) | LSP (`backend = "lsp"`, rust-analyzer) |
|-------|---------------------------|----------------------------------------|
| Reference to `OnlyTyped` in `user`'s signature | **Miss** — `symbol_refs("OnlyTyped")` empty (type position; 64/74 T8.8 misses) | **Hit** — `textDocument/references` on the `OnlyTyped` struct definition returns the `Vec<OnlyTyped>` site in `user` |
| `callers("OnlyTyped")` after grouping | Empty / no rows | At least one row naming `user` as the enclosing scope |

T30.2 adds `tests/graph_lsp_gate.rs` (or extends `graph_contract.rs`) that runs the same file with `backend = "lsp"` against a functional `rust-analyzer` on `PATH`; skips when absent. **Secondary** candidate (not the gate): `&dyn Trait` dispatch — tags over-approximate by name; LSP resolves the trait method's `references` — but the gate uses the already-measured **type-position miss** so the row cites T8.8 evidence.

**Gate P30 (review):** Same four MCP names; `backend = "tags"` → tags-only bytes unchanged; `backend = "lsp"` → fixture above hits on LSP and misses on tags; one `Measurement` row recorded.

### Rejected (P30)

- **Spawning serena / solidlsp / codebase-memory-mcp** — D6; already rejected in v0.1 §Rejected; circular measurement.
- **Exposing serena-shaped MCP tools** (`find_symbol`, `find_referencing_symbols`, …) — breaks MCP-name stability and `graph_contract.rs`.
- **Default-on LSP** — tags stay default; cold index **1.3 s** debug / **341 ms** release on 3 000 files (P35) beats LSP startup for most calls.
- **Hybrid tags+LSP per call without a mode flag (alternative C)** — cannot satisfy "off → tags-only bytes".
- **LSP on the hook path** — violates D1 ≤ 10 ms; graph stays MCP-only for LSP.
- **Embedding rust-analyzer as a library** — no stable in-process API; subprocess is the portable seam (serena, VS Code, clangd all spawn).
- **A fifth tool for call hierarchy / implementations** — `impact` and richer `symbol` lines absorb it; surface stays four tools / ≤ 150 description tokens (`doctor`).
- **Claiming serena's precision without a `Measurement` row** — research.md already notes serena timeout here; gate must be local fixture + dated command.
