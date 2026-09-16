# rtok compared with the alternatives

Every tool named here is real, useful, and was measured before rtok was written. The raw
survey — 19 repositories, GitHub API metadata, 27 vendor claims fact-checked, 17 local
session transcripts — is
[`research.md`](https://github.com/listepo/rtok/blob/main/research.md) §4–§7. This page is the reading of
that survey: what each category does well, what rtok does differently, and which of rtok's
claims are backed by a row in a database rather than by a README.

Survey date 2026-09-01, refreshed 2026-09-04/09/10 (modes bench). Star counts and versions move; the
mechanisms do not.

## 1. The short version

Ten tools that each shrink one slice of the context, stacked on one machine, produce a
stack that nobody measures end to end. The author's own machine, before rtok:
**88 hook entries across 16 events, nine MCP servers, ~8 600 tool-description tokens paid on
every single turn, and two chained proxies.** Every one of those tools reported a saving.
The bill did not move, because no meter covered the whole path.

rtok is the opposite bet: **one binary, one ledger, one number.** Ten methods live as
plugins behind one trait, they share one SQLite file, and a saving that is not a
`measurements` row does not exist — including rtok's own, which is why the A/B table in the
README is currently all zeros instead of a marketing figure.

## 2. What each category does well

### Command-output filters — rtk

~80 hand-written filters that shrink `git status`, `cargo test`, `npm` and friends, wired
into `PreToolUse`. The idea is correct and rtok copies it: Bash output was 35 % of the
author's tool-result tokens (1.00 M of 2.83 M).

Two problems. It drops lines with no way to get them back, and the only independent
measurement of it — JetBrains — found it cost **+7.6 % to 0 %** on agentic work against a
claimed 60–90 %. rtok's `cmd` plugin keeps the filtering (9 rule families plus per-family
formatters for `cargo`, `git`, test runners, `ls`/`find`) and adds the two things that were
missing: the raw output goes to `~/.rtok/archive/<id>` before anything is cut, and the
before/after byte counts go to `measurements`.

### Read, search and shell MCP servers — lean-ctx, token-optimizer

Bounded reads, outline modes, deduplicated re-reads, compact directory trees. Also correct,
also copied: `read` ships 3 MCP tools with 4 read modes and a `read_cache` table.

The cost these tools do not count is their own presence. lean-ctx injects ~3.1 K tokens per
turn of banner and instruction text; token-optimizer installs 27 Python hooks, and a full
event chain on this machine fires up to ~30 subprocesses. Both are paid on every turn,
cached or not, whether or not the agent reads a file.
rtok's whole injection budget is **800 tokens per turn**, byte-stable so it never busts the
prompt cache.

### Compressing proxies — headroom, caveman, bifrost

The proxy is the only surface that can rewrite what is already in the conversation, and
headroom's cache-aligned live zone is the sharpest idea in the field: shrink the tail,
leave the cached prefix byte-identical. rtok's `archive` plugin implements the same
invariant, and `proxy` captures provider `usage` as ground truth.

Measured against their claims: headroom claims up to 95 %, the author's own meter showed
11.3 % over 30 days; caveman claims 65 %, JetBrains measured 8.5 % on agentic work, and its
issue #112 corrupts inline code. bifrost's semantic cache is a category error for coding
agents — agent contexts never repeat, so a cache hit is a wrong answer.

### Code graphs — serena, codegraph, code-review-graph, codebase-memory-mcp, graphify

Ask "who calls this" instead of grepping and reading five files. The best of them are very
good: serena's LSP backend is the most precise symbol resolution available, and
code-review-graph's impact analysis is genuinely clever.

What they cost is the tool list. Measured with `rtok doctor` on 2026-09-09 — every MCP
server on the author's machine, not only the code graphs:

| MCP server | tools | description tokens, every turn |
|---|---:|---:|
| code-review-graph | 30 | ~2 295 |
| engram | 18 | ~1 865 |
| mobile | 32 | ~1 507 |
| serena | 22 | ~1 494 |
| lean-ctx | 12 | ~697 |
| caveman | 5 | ~485 |
| headroom | 3 | ~161 |
| **rtok** (read + memory + graph + expand) | **11** | **~143** |

That column is not a one-off: it is re-sent on every request of every session. Two of those
servers together cost more per turn than rtok's entire injection budget. `graph` answers
`symbol` / `callers` / `impact` / `outline` in 62 description tokens.

The trade is real, though, and §5 states it: serena resolves references that rtok's
tree-sitter tags index misses.

### Memory — claude-mem, engram, mem0

Notes that survive compaction. claude-mem extracts them with an LLM, which costs tokens to
build the thing that saves tokens; mem0 wants Docker and a vector database. rtok's `memory`
is agent-written notes in SQLite FTS5 with progressive disclosure — titles, then ids, then
bodies, never bodies at `SessionStart` — for 3 tools and no model calls.

### Prompt-level — ponytail, terse modes

The cheapest lever in the field, because output tokens are 20 % of the bill on standard
models and **39 % on Fable/Mythos 5.1** where cache reads cost 0.025×. ponytail's own bench
claims −54 % LOC and −22 % tokens; no independent check exists. caveman claims 65 % output
shrink; JetBrains measured **8.5 %** on agentic work, and issue #112 has corrupted inline
code.

rtok ships the same *ideas* as optional prompt modes
(`rtok agent setup claude --mode terse,yagni`, aliases `cave`/`pony`) plus native helpers in
`src/modes/` — not a wrap of either tool (D6). Modes are markdown data under `modes/`
injected once per session inside the shared **800-token** budget (D7); measured mode sizes
2026-09-10: `terse` **162** est. tokens, `yagni` **145** (cap 250 each).

Structured A/B against honest weak baselines (`cargo test --test mode_bench -- --nocapture`,
2026-09-10; full write-up in [`research.md`](../research.md) modes subsection):

| Slice | Baseline | rtok | Why better |
|-------|----------|------|------------|
| Prose compress (17 fixtures) | weak caveman-lite: **11.7 %** chars, **50** tok saved | Full: **34.9 %** chars, **147** tok saved | Fence bodies byte-identical; negations (`not`/`never`/`no`) kept — caveman #112 class of bug is a hard fail here |
| YAGNI ladder (14 fixtures) | naive always-Minimum: **4/14 (29 %)** | typed ladder: **14/14 (100 %)** | Prompt-only “be lazy” collapses to Minimum; the ladder can Skip / Reuse / Stdlib / Native first |
| Injection cost | caveman MCP ~485 desc tokens + separate prompt; ponytail another file | modes share inject budget; aliases resolve to the same builtins | One binary, byte-stable injection, no Go/JS on the path |

These are **fixture benches**, not a live bill delta — same honesty rule as the README A/B
zeros. They show the native path beats the weak baselines we can re-run in CI; they do not
replace an end-to-end `rtok bench` with a provider.

### Formats — TOON, LLMLingua-2

TOON's −42.6 % on tabular JSON is a vendor bench that survived fact-checking, so rtok has a
`toon` plugin — **off by default**, because most agent payloads are not tables. LLMLingua-2
prunes tokens with a small model; on code that is a quality risk with no measured upside,
so it is v0.2+ at the earliest.

### The platform itself — prompt caching, context editing, auto-compact

Free, first-party, and the thing every tool above is really competing with. The author's
cache hit rate is 98.1 %. The correct move is to align with it, not fight it: rtok's
byte-stable injection and cache-preserving proxy rewrites exist for exactly this reason.

## 3. Where rtok is different

| | The field | rtok |
|---|---|---|
| Processes per tool call | up to ~30 subprocesses per event chain, several Python | one Rust process, p95 **8.25 ms** |
| Hook entries | 88 across 16 events (10 tools) | **7**, one binary |
| MCP description tokens/turn | ~8 600 across 10 servers | **~143** across 11 tools |
| Injection per turn | lean-ctx 3.1 K + engram + claude-mem + nudges | one **800-token** budget, byte-stable |
| Reversibility | partial (headroom retrieve, token-optimizer expand) | every rewrite has `rtok expand <id>` |
| Measurement | 5 incompatible meters, none of them the bill | one ledger: `measurements` + provider `usage` |
| Failure mode | varies; some block the tool call | fail open: exit 0, unmodified input, ≤ 10 ms |
| Runtime | Python, Node, Go, Docker, daemons | one static binary, 18.9 MiB, no daemon |
| Third-party code on the hot path | adapters and wrappers | none (decision D6) |
| Observability | per-tool dashboards | OTLP to Jaeger / Grafana / SigNoz / Maple |

## 4. The advantages, with their evidence

1. **The tool list is context too.** 11 tools for ~143 description tokens, against 30 tools
   for ~2 295 in one competing server. Source: `rtok doctor`, 2026-09-09.
2. **Nothing is lost.** `cmd`, `read` and `archive` write the raw payload to
   `~/.rtok/archive/<id>` *before* shortening it; `rtok expand <id>` returns it, with
   `--lines` and `--grep`. rtk drops lines outright.
3. **A saving is a row or it is nothing.** `Ctx::record(&Measurement)` is the only way for a
   plugin to claim one, and `rtok stats --plugin <id>` prints them. The proxy writes
   provider-reported `usage`, which is the actual bill rather than a chars/4 estimate.
4. **It admits what it has not proven.** The committed A/B bench ran offline and reports
   zeros; the README says so. Every vendor number in §2 that was independently checked came
   in 5–10× below its claim.
5. **It cannot break your agent.** A hook exits 0 with unmodified input on any error or
   panic, inside 10 ms. A half-installed or crashing rtok is a no-op, not an outage.
6. **It does not fight the cache.** Injections are budgeted and byte-stable across turns;
   the proxy never rewrites system instructions, tool definitions, or the newest tool
   results. At a 98.1 % hit rate, busting the prefix costs more than any filter saves.
7. **Three surfaces, because one is not enough.** `PostToolUse` cannot modify a tool result
   — verified against the hook docs — so shrinking what is already in context needs the
   proxy, and replacing a tool needs MCP. Single-surface tools have a ceiling that is a
   property of the host, not of their code.
8. **One config, with provenance.** Every flag is a config key; `rtok config show --sources`
   names the layer each value came from. No tool in the survey can answer that question.
9. **Reversible install.** `rtok agent setup claude --dry-run` prints the exact edits, the real run
   backs up the settings file before writing it, and `rtok agent remove claude` takes the hooks,
   the MCP registration and the proxy variable back out (foreign entries stay).
10. **Your ledger, in your observability stack.** `rtok otel flush` projects calls, logs and
    metrics as OTLP/HTTP JSON — verified against Jaeger 2.11 and Grafana `otel-lgtm`, and
    against an independent validator that re-implements the spec. Cost when an endpoint is
    configured: +0.81 ms p95 on the hook path.

## 5. Where rtok is behind

Stated plainly, because §4 is only worth reading if this section exists.

- **No live end-to-end cost win has been demonstrated.** The A/B harness runs; it has not
  been run against live traffic. Until it is, rtok's own claim is "measurable", not "cheaper".
- **`graph` finds every definition and misses most references.** Definition recall and
  precision are 1.000 over a 30-symbol hand-labelled set; reference recall is **0.351**,
  because the tree-sitter tags query does not capture type positions or macro bodies.
  serena's LSP backend is more precise here. An LSP backend for `graph` is v0.2.
- **No LLM compression, no embeddings, no semantic search.** claude-mem and mem0 do those
  today. In rtok they are v0.2+ and gated on beating the lossless path in a bench.
- **Smaller filter library than rtk.** 9 rule families plus a handful of per-family
  formatters, against rtk's ~80 filters.
- **Younger, and a single maintainer.** Several tools in §2 have five-figure star counts and
  years of edge cases baked in. rtok is at v0.0.1.
- **`toon` and `graph-watchman` are opt-in** because they lost their gates on this machine.
  LadybugDB and Grafeo were measured then **removed** (P39, 2026-09-12): Ladybug won depth-4
  impact by 77× but missed the hook ≤10 ms bar; Grafeo abandoned after warm `impact(2)` ~22 000×
  slower than SQLite. The symbol index is SQLite only.

## 6. Check any of it yourself

```bash
rtok doctor                       # your hooks, MCP servers and their description-token cost
rtok plugins                      # what is enabled, and on which surface
rtok stats --since 7d             # transcript estimates plus provider usage
rtok stats --save-baseline before # then change something, then --compare before
rtok bench --dry-run              # the A/B schedule that has to beat the baseline
rtok expand <id>                  # the original of anything rtok shortened
```

`rtok doctor` reads the settings of whatever is already installed, so it will price your
current stack before you install anything of rtok's.

## 7. When not to use rtok

- You want the most precise symbol resolution available and do not mind 22 tools of
  description on every turn — use serena.
- You want a big, mature filter library for shell output and can live with lossy cuts — use
  rtk.
- You need LLM-extracted memory or vector search today — use claude-mem or mem0.
- You are on a plan where tokens are not the constraint. Then none of this pays, including
  rtok.
