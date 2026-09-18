# Token-reduction tools for AI coding agents — research, comparison, evidence

Date: 2026-09-01. Method: 8 Haiku research agents (compression tools, code graphs, memory, host surfaces, architecture, techniques, token-optimizer plugin, session-log measurement) + 2 Haiku adversarial fact-check agents (GitHub API metadata for 19 repos; 27 documentation/blog claims). Local ground truth from `~/.claude/settings.json`, `rtk gain`, `headroom savings`, `lean-ctx gain`, `caveman status`, claude-mem banner, and 17 session transcripts (Jul 29 – Sep 1). Raw reports live in the session scratchpad `token-research/`. Features those tools have that rtok has not scheduled live in `ideas.md`.

## 1. Verdict

1. **Your stack stacks ten tools, 81 hooks and two chained proxies, and nobody measures end to end.** Vendor claims are 60–99 %; your own meters show 3–40 % on the slice each tool touches; the only independent measurement (JetBrains) found rtk cost +7.6 % to 0 % and caveman 8.5 % on agentic work.
2. **The cost lever is context × turns, not single outputs.** Cache hit rate is 98.1 %; every token that enters context is re-read (at 0.1× price, 0.025× on Fable/Mythos 5.1) on every later turn. A 17 K-token Read on turn 10 of 60 costs ~1.25 × 17 K to write and 0.1 × 17 K × 50 to keep. Shrinking early and clearing old results beats shrinking a little everywhere.
3. **Build one Rust binary with plugins, measurement first, and retire what does not pay.** Hooks cannot modify tool results after the fact, so the binary needs three surfaces: hook (PreToolUse rewrite), MCP (tool replacement), proxy (cache-safe live-zone rewriting + ground-truth usage). Plan: `plan.md` (project `~/GitHub/rtok`).

## 2. Where your tokens go (17 sessions, 43,609 transcript lines)

Estimator: 4 chars/token (heuristic). Usage counters are real API numbers.

| Item | Value |
|------|-------|
| Tool results, est. tokens | 2.83 M total; Bash 1.00 M (35 %), Read 0.41 M (15 %), Agent 23 K, MCP engram 8.8 K, MCP lean-ctx 6.2 K |
| Largest single results | Reads of 38–68 K chars (9.5–17 K tokens each, top 8 all `Read`) |
| Bash by family | `cd …&&` chains 435 K (hides the real command), sed 217 K, grep 131 K, cat 17 K, ls 13 K, pnpm 35 K, python3 34 K |
| Assistant output | 8.6 M output tokens; text is 4 % of assistant content, 96 % is tool input (the code it writes) |
| Edit `old_string` (T58.3, 2026-09-17, `rtok stats --since 90d`, 925 sessions) | 5,990 Edit/MultiEdit calls; `old_string` 1.61 MB, `new_string` 3.60 MB; `old_string` = 3.8 % of tool-input bytes, ≈ 1.3 % of output tokens (bytes/4 against API output) — under the 10 % gate, so the anchored `patch` tool (T58.4) was not built. Caveat: this machine already routes many edits through lean-ctx `ctx_patch`, so the share is a lower bound for a plain-`Edit` workload. |
| Foreign MCP results (T59.4, 2026-09-17, `rtok stats --since 30d`, 885 sessions) | `rtok stats --since 30d` (2026-09-17, 885 sessions), `mcp` table: lean-ctx 8,232 calls, 19.66 MB result bytes, mean 2.4 KB, p95 45.7 KB (≈ 4.9 M est. tokens) — ≈ 27 % of the 71.8 MB in the tool table; rtok 579 KB, engram 279 KB, t3-code 73 KB (mean 18 KB), Claude_Browser 45 KB. lean-ctx is above the 5 % gate, so the wrapper is justified for this workload; caveat: lean-ctx already compresses its own results, so the win is in the p95 tail, not the mean. |
| Cache | read 1,367 M, creation 26.7 M, uncached input 42 K → 98.1 % hit rate |
| Median final context | 167 K tokens per session |
| rtk-wrapped commands visible in transcripts | 3 of 3,658 (the PreToolUse rewrite happens after the transcript records the call, so this under-counts) |

### `graph` index accuracy (T8.8, 2026-09-04)

30 symbols of this repo labelled by hand from a plain-text scan, independent of the index that is
being scored: definition files complete per symbol, reference files a must-appear subset, no line
numbers. `tests/graph_truth.rs` re-measures on every run.

| Metric | Value |
|--------|-------|
| Definitions found | 30 / 30, recall 1.000, precision 1.000 |
| References found | 40 / 114, recall 0.351 |
| All sites | 70 / 144, recall 0.486 |
| Cause of every miss | type positions 64, macro bodies 9, path-qualified calls 1 |

The reference number is a property of the tree-sitter Rust tags query, not of rtok's storage: it
captures plain calls, field-expression method calls, macro invocations and `impl` items, nothing
else. `src/plugins/graph/PLAN.md` lists the constructs under "Known misses".

### `graph` v0.2 surface and latency (Gate P8b, 2026-09-04; surface re-measured 2026-09-18, T68.1)

Release build. The 3 000-file repo is generated, each file one function calling two others.

| Measurement | Value | P8b bar |
|-------------|-------|---------|
| Tools, description tokens | 5, 127 | ≤ 150 |
| Cold index, 3 000 files / 9 000 rows | 22.1 s | not gated |
| Warm `symbol` / `callers` / `impact` | 23 / 24 / 26 ms | < 100 ms |
| Definition recall, precision | 1.000, 1.000 | ≥ 0.9 |
| Reference recall | 0.351 | published, not gated |

The fourth clause — fewer tool calls per multi-file task on the P9 set — is not measured, so
the gate is open. `callers("estimate")` on this repo fell from 1 959 bytes at v0.1 to 793.

### `graph` composite-query chains (T52.1, 2026-09-17)

Dated command: three ad-hoc scans over all 172 transcripts in
`~/.claude/projects/*/*.jsonl` (157 sessions with tool calls; transcripts
predate the `graph` tools, so the chains are `ctx_search`/shell-`rg` shaped).

| Measurement | Value |
|-------------|-------|
| `ctx_search` calls | 615, of which 610 carry a path scope |
| Search→search refinement chains (≤3 calls apart) | 252, in 33 sessions |
| Search→read chains (≤5 calls apart) | 528, in 46 sessions |
| Shell `rg` calls with a path arg | 41 of 98 |

Verdict: GO — "X inside path Y" is the measured composite, so `symbol` /
`callers` / `impact` grew optional `path` (substring) args and `symbol` an
optional `kind` (exact) arg on the existing tools — no new tool. Surface after:
4 tools, 94 description tokens (still ≤ 150; each description ≤ 60).

Cost split with p_out = 5 × p_in (input-token equivalents):

| Component | Standard cache read 0.1× | Fable/Mythos 5.1 read 0.025× |
|-----------|--------------------------|------------------------------|
| Cache reads | 137 M (64 %) | 34 M (31 %) |
| Cache writes (1.25×) | 33 M (16 %) | 33 M (30 %) |
| Output (×5) | 43 M (20 %) | 43 M (39 %) |

Reading: on standard models, context volume dominates → compress tool results and clear old ones. On Fable/Mythos, output tokens dominate → fewer lines written (ponytail-style), fewer turns, terse prose.

### `graph` LadybugDB vs SQLite (Gate P8c, T8.14, 2026-09-08)

Release build, this machine (macOS arm64). Same generated 3 000-file repo as P8b (one `fn` calling
two others, 9 000 rows). Fan-out-10 depth-4 fixture: 11 110 call edges (10+100+1 000+10 000).
Numbers from `cargo test --release --test graph_bench -- --ignored --nocapture`.

| Measurement | default (SQLite) | `--features graph-lbug` | Bar |
|-------------|------------------|-------------------------|-----|
| (1) `tests/graph_contract.rs` | 3 passed | 3 passed | unchanged, both |
| (2) `rtok hook PostToolUse` p95, n=100 | 8.07 ms | 96.6 ms | ≤ 10 ms |
| (3) warm `symbol` / `callers` / `impact(2)` | 17.9 / 17.5 / 26.8 ms | 797 / 776 / 873 ms | < 100 ms |
| (3) cold index, 3 000 files | 13.8 s; 341 ms after T35.1, 172 ms after T35.2 (2026-09-11); **T59.3** batches 200 files/txn (was 64) — re-run `cargo test --release --test graph_bench -- --ignored` when the tree compiles | 33.7 s | not gated |
| (4) `impact(4)` on fan-out fixture | CTE 28.5 s | path 371 ms (**77×**) | lbug ≥ 2× CTE |
| (4) same fixture, Rust BFS | 2.61 s | 2.35 s | baseline |
| (5) `just check` (liblbug already built) | 16.9 s | same command (clippy `--all-features`) | ≤ 2× default |
| (6) release `rtok` bytes | 20 668 544 (19.7 MiB) | 34 002 576 (32.4 MiB) | published |
| (6) store after 3 000-file index | `rtok.db` 3.98 MB | `rtok.db` 200 KB + `graph.lbdb` 6.10 MB | published |

Clause (4) won. Clauses (2) and (3) fail on the `graph-lbug` binary (spawn/link cost, and every
warm tool call opens LadybugDB). Default SQLite meets (2) and (3). Incremental `just check` is
not 2× a default `cargo test` (18.9 s); the C++ cmake cost is paid once (T8.11: 3 min 21 s
debug from source, pinned). **Archive (P39, 2026-09-12):** after the freeze and the Grafeo
negative spike below, **LadybugDB was deleted** (`graph-lbug` / `lbug` / `symbols_lbug.rs`).
Numbers above are historical only — no live feature flag.

### `graph` Grafeo vs SQLite (P8e spike, T8.20, 2026-09-12) — abandoned / removed

Release build, this machine (macOS arm64). Same `tests/graph_bench.rs` harness as T8.14 plus
focused `p8e_impact4_*` tests. `grafeo` 0.5.42 (`edge`+`wal`+`grafeo-file`, no ONNX/AI).
Spike lived on `feat/graph-grafeo` (PR #21 draft / PR #22 abandon); **not merged**; code removed
with P39.

| Measurement | default (SQLite) | `--features graph-grafeo` | Bar |
|-------------|------------------|---------------------------|-----|
| (1) `tests/graph_contract.rs` | 3 passed | 3 passed (debug) | unchanged, both |
| (2) `rtok hook PostToolUse` p95, n=100 | 11.7 ms | 76.9 ms | ≤ 10 ms |
| (3) warm `symbol` / `callers` / `impact(2)` | 15.4 / 15.6 / 22.8 ms | 39.3 / 42.1 / **498.6 s** | < 100 ms |
| (3) cold index, 3 000 files | 127 ms | 19.0 s | not gated |
| (4) `impact(4)` on fan-out fixture | CTE 30.5 s | path query **DNF >14 min** | grafeo ≥ 2× CTE |
| (4) same fixture, Rust BFS | 2.65 s | 59.4 s | baseline |
| (5) build | default features | pure Rust, no cmake C++ | not catastrophic |
| (6) release `rtok` bytes | 23 733 888 (22.6 MiB) | 27 626 000 (26.3 MiB) | published |
| (6) store after 3 000-file index | `rtok.db` 4.08 MB | `rtok.db` 213 KB + `graph.grafeo` 3.72 MB | published |

Clause (1) and the cmake-free build were the only wins. Warm `impact(2)` ~22 000× slower than
SQLite (CALLS re-materialized per call); fan-out path query never finished in 14 min.
**Decision: abandon** — then **delete** under P39 (SQLite only).

### `graph` watcher idle cost (Gate P8d (2), T8.16, 2026-09-08)

Release binary, macOS arm64, this machine. `rtok mcp` idle for 60 s (stdin held open, no
requests), 50-file fixture root, `ps -o time=,rss=` sampled at t+60 s. Watcher thread inside
the server process (D18 one-writer rule).

| Measurement | `watch = "off"` | `watch = "notify"` | Bar |
|---|---|---|---|
| CPU time over 60 s idle | 0:00.03 (30 ms) | 0:00.02 (20 ms) | Δ ≤ 50 ms |
| RSS at t+60 s | 11 552 KB (11.3 MB) | 12 720 KB (12.4 MB) | Δ ≤ 2 MB |

Clause (2) **passed**: the FSEvents-backed watcher costs less CPU than the noise floor of the
measurement and +1.14 MB RSS. Per the Gate P8d decision rule the watcher is not forced to
`"off"`; it stays opt-in anyway because T8.15 shipped `watch = "off"` as the default and no
task in P8d changes it.

### `graph` watchman backend (Gate P8d (3)+(5), T8.17, 2026-09-09)

Release binary, macOS arm64, this machine, `watchman 2026.07.27.00` on PATH.
Gate P8d (1) under `watch = "watchman"`: daemon edit visible in `symbol`
within 1 s while the call reads 0 files — `watchman_sees_daemon_edit_within_1s_reading_nothing`
green (`tests/` poll the daemon's `watch-list` for the root before the timed
write, so the check measures delivery, not connect + subscribe).
Gate P8d (3): `mcp_watchman_watch_list_names_the_root` green — the MCP cwd
registers with the daemon (the first version slept a fixed 400 ms and flaked
when the daemon needed longer; it now polls `watch-list` ≤ 5 s); with no
socket the server prints exactly one `watchman: … falling back to notify`
line and serves through `notify` (`mcp_watchman_without_daemon_falls_back_once`).
Gate P8d (1) latency, single probes (debug binary, one live `rtok mcp` per
backend, `auto_index = false`, write-then-poll to `symbol`): `notify` ~250 ms,
`watchman` ~500 ms — both under the 1 s bar; the daemon path pays connect +
round-trip on top of the same 250 ms quiet period, so it cannot beat `notify`
here. Not a bench (n=1 each); the committed tests assert the bar, not the gap.
Gate P8d (4): hook path is untouched by this phase (no `hooks/` file changed;
the default binary links neither `notify` nor `watchman_client`), so no new
p95 is owed by the change itself. Measured anyway on this machine:
`cargo test --release --test latency` p95 14.8/14.9 ms (Pre/PostToolUse,
n=200) at load ~9, and 10.1/10.6 ms at load ~11 — both over the 10 ms bar
with p50 ~8.3 ms and 633 ms scheduler-stall maxima, i.e. machine load, as in
Gate P17 (7–8 ms p95 at load ~3). Resolved 2026-09-09: the straddle is the
harness itself — cargo runs both latency tests in parallel (2×200 spawns
contend). Serialized (`-- --test-threads=1`) at load ~4–6: Pre p50 7.22 ms
p95 8.25 ms, Post p50 7.18 ms p95 8.25 ms — pass with margin (row in the
Gate P17 section). Gate P8d (4) takes that row.
Gate P8d (5): release `rtok` 19 764 144 B pre-`notify` (scratch worktree at
`54f2445^`) vs 19 867 968 B (18.9 MiB) default with `notify` (+103 824 B,
+0.5 %) vs 20 429 392 B (19.5 MiB) with `--features graph-watchman`
(+561 424 B over default, +2.8 %). The feature is **not** in `default`:
(3) passes but watchman does not beat `notify` on (1) latency — same ≤ 1 s
bar, same quiet-period loop, plus a daemon the user must run — so per the
Gate P8d decision rule the `watchman_client` crate stays behind opt-in
`graph-watchman` and `watchman` stays a documented value, not the default.

### OpenTelemetry export (Gate P16, 2026-09-04)

Release binary, macOS arm64, 100 runs per event, spawn-to-exit measured from Python.
"Endpoint set" points at a local OTLP receiver that answers 200.

| Hook event | No endpoint | Endpoint set | Delta | Bar |
|---|---|---|---|---|
| `PostToolUse` p95 | 8.89 ms | 9.70 ms | +0.81 ms | 10 ms |
| `Stop` p95 | 8.36 ms | 8.97 ms | +0.61 ms | 10 ms |

`Stop` is the event that spawns the detached `rtok otel flush`; 0.61 ms is what that spawn
costs. `PostToolUse` never touches the exporter, so its delta is the extra `[otel]` section in
the config plus noise.

Payload, checked by an independent receiver that re-implements the OTLP JSON rules
(`tools/otlp_validator.py`, not the Rust encoder): **0 problems** over one session's
traffic — 205 spans, 3 metric streams, ids 32/16 lowercase hex, every int64 a decimal string,
numeric `kind` / `severityNumber` / `aggregationTemporality`. Bodies: 93.8 KB for the first
batch, 889 B per incremental flush, 1.4 KB per metrics post. Span names seen:
`execute_tool Read`, `hook UserPromptSubmit`, `hook Stop`, `hook SessionEnd`,
`invoke_agent agent` (the root, once `SessionEnd` sets `ended_at`).

**Gate P16 clause (3), 2026-09-04: open.** Jaeger, Grafana, SigNoz and Maple were not
exercised: Docker was blocked by this machine's shell allowlist. What was proven is that the
bytes satisfy the OTLP/HTTP JSON spec as an independent implementation reads it.

**Clause (3), 2026-09-07: two of four backends verified; two remain.** Docker allowed
(`lean-ctx allow docker`), the two `docs/otel.md` recipes run against a copy of the live
ledger (the `p17-bench` session: 780 hook calls, no `usage` or measurement rows), one
`rtok otel flush` each, checked through the backends' own APIs rather than by eye:

| Backend | Traces | Logs | Metrics | Flush report |
|---|---|---|---|---|
| Jaeger 2.11.0 (`jaegertracing/jaeger`) | 2 traces, 780 spans; `execute_tool` spans carry `gen_ai.tool.call.arguments` / `result` | 404 | 404 | `780 spans · 0 logs · 0 metric points · 1 posts · not served: logs, metrics` |
| Grafana `otel-lgtm` (Tempo, Loki, Prometheus) | trace `ca4564baad2314a1f53c431f7a5dc802` in Tempo | accepted, 0 rows sent (the ledger had none pending) | `rtok_calls_total` in Prometheus; no `rtok_tokens_total` because the ledger has no `usage` rows | `780 spans · 0 logs · 2 metric points · 2 posts` |

Two findings. Jaeger 2.x has no logs or metrics pipeline over OTLP/HTTP: `/v1/logs` and
`/v1/metrics` answer 404. Before this run the exporter treated that as a failure, logged one
`logs` row per flush, and re-sent that row on the next flush — pending grew by one per flush
(every 5 s under `mcp` / `proxy`), unboundedly. A 404 is now "stream not served": skipped,
watermark kept, nothing logged, named in the report (`tests/otel.rs`,
`a_404_stream_is_skipped_not_logged`). Second, `rtok_tokens_total` needs `usage` rows, which
only the proxy or an imported transcript write; hooks alone produce `rtok_calls_total`.

Still open: the clause asks for one real Claude Code session (hooks + MCP + proxy) as one
trace with an `invoke_agent` root and `chat {model}` spans — this ledger has no ended session
and no proxy traffic, and rtok is not on this machine's PATH — and for SigNoz and Maple, which
need an account or an API key.


### WASM bundle (`rtok web`, T60.7, 2026-09-18)

| What (date, command) | Result |
| --- | --- |
| Before (`ls -l crates/rtok-webui/pkg/rtok_webui_bg.wasm`, 2026-09-17) | 10,560,601 B, default `wasm-pack --release`, no `wasm-opt` |
| After (`wasm-pack --release` + wasm-opt -Oz, 2026-09-18) | 4,130,017 B |

### Build size (T17.1, Gate P17, 2026-09-04)

macOS arm64, this machine. Every "before" is a cold build of the same commit with the profile
lines removed; every "after" a cold build with them in. Sizes are `stat -f%z` bytes.

**Release, default features** (`[profile.release] strip = "symbols"`):

| Artifact | Before | After | Δ |
|---|---|---|---|
| `target/release/rtok` | 22 374 576 B (21.34 MiB) | 19 157 712 B (18.27 MiB) | **−14.4 %** |

Release build time is unchanged at 1m30s — stripping happens after linking.

Latency, since Gate P17 asks for it: `rtok hook PostToolUse`, 100 spawn-to-exit runs after 10
warmups, the two binaries interleaved run-for-run so they see the same machine.

| Release binary | p50 | p95 | max |
|---|---|---|---|
| stripped (this profile) | 8.16 ms | 10.07 ms | 19.21 ms |
| unstripped (same commit, `strip = "none"`) | 8.33 ms | 9.83 ms | 12.91 ms |

Stripping does not cost latency — it is marginally faster at the median, and the p95 gap is inside
the noise (four repeat rounds of the stripped binary gave p95 9.83, 10.07, 10.70, 10.83, 10.91 ms).
What the table does not show is a comfortable margin: p95 sits *on* the 10 ms bar today, against
8.89 ms recorded by the same harness at Gate P16 earlier the same day. The interleaved A/B places
that drift outside the profile change — the unstripped binary drifted with it — so it is the
machine or the grown `rtok.db`, and it is the p95 that P8/P16 own, not P17.

Re-measured 2026-09-05 with `cargo test --release --test latency -- --nocapture`, which now times
`PostToolUse` beside `PreToolUse` (200 runs each, fresh `RTOK_HOME`, so the grown database is out
of the picture). Three rounds at a 1-minute load average of 14 → 36 (other sessions compiling):

| Event | p50 | p95 | max |
|---|---|---|---|
| `PostToolUse` | 8.51 / 7.93 / 7.58 ms | 13.58 / 11.06 / 11.98 ms | 39.9 / 19.4 / 16.5 ms |
| `PreToolUse` | 7.45 / 8.09 / 7.73 ms | 10.29 / 11.65 / 12.28 ms | 22.3 / 21.0 / 17.0 ms |

The median holds where T17.1 measured it and the empty-home `PreToolUse` test drifts by the same
amount as `PostToolUse`, so the drift is the machine, not the database and not the profile.

**Gate P17 p95 clause: passed 2026-09-07** on a quiet machine (1-minute load 2.9–3.4), same
test, three rounds, release profile:

| Event | p50 | p95 | max |
|---|---|---|---|
| `PostToolUse` | 5.63 / 5.60 / 5.64 ms | **8.17 / 7.24 / 7.86 ms** | 16.2 / 10.3 / 17.9 ms |
| `PreToolUse` | 5.49 / 5.54 / 5.46 ms | **6.41 / 6.94 / 5.79 ms** | 8.6 / 13.4 / 6.7 ms |

Re-measured 2026-09-09: the straddle is the harness running both tests in
parallel (cargo default; 2×200 spawns contend with each other). Serialized
(`-- --test-threads=1`), one round at load ~4–6, release, fresh homes:
`PreToolUse` p50 7.22 ms p95 8.25 ms max 14.5 ms, `PostToolUse` p50 7.18 ms
p95 8.25 ms max 10.6 ms — both pass with margin. Parallel rounds on the
same machine straddle the bar (9.5–11.2 ms): scheduler noise, not the
binary — the hook path is unchanged since Gate P17 passed it. Gate P8d (4)
takes this row: hook p95 ≤ 10 ms holds when the harness does not load the
machine it measures.

Where a hook's milliseconds go (spawn-to-exit p50, same harness, same quiet machine; the
in-process figures are `Instant` around the call):

| Step | p50 | How measured |
|---|---|---|
| harness floor (`/usr/bin/true`) | 1.3–1.5 ms | same `Command` + three pipes |
| empty Rust binary | 1.9 ms | scratch crate, `strip = "symbols"` |
| … linking Security.framework + CoreFoundation | **3.2–3.5 ms** | same crate, one `#[link]` block |
| `rtok --version` (release) | 3.3–4.2 ms | exits inside `Cli::parse` |
| `rtok --version` (dist: thin LTO, 1 cgu, 17.4 MB) | 3.3 ms | −0.1 to −0.2 ms vs release |
| `Config::load_lenient`, real 7 KB file | 0.26 ms | in-process |
| `Store::open` + close (WAL) | 0.52–0.59 ms | in-process; `TRUNCATE` would be 0.24 ms |
| `hooks::run PostToolUse` (in-process, whole hook) | 1.07–1.30 ms | includes the store line |
| `rtok hook PostToolUse` (release, spawn-to-exit) | 5.5–5.7 ms | |

Reading: the hook's own work is ~1.3 ms; process startup is ~4 ms, of which 1.3–1.5 ms is the
dyld cost of Security.framework and CoreFoundation, linked because reqwest 0.13's `rustls`
feature hard-depends on `rustls-platform-verifier` (the crate has no webpki-roots feature any
more). Nothing on the hook path uses them. Dropping the link means a different TLS root story
for `proxy` and `otel` — an `ideas.md` entry (I-32), not a P17 task. Config loading and the
store are already small; `WAL` costs 0.3 ms per short-lived process over `TRUNCATE`, kept
because `mcp` and `proxy` write concurrently with hooks.

**T53.3 (2026-09-18).** Decision D30: one binary, webpki Mozilla roots via
`ClientBuilder::use_preconfigured_tls` — not a second hook binary. reqwest
0.13.4's `rustls` feature still references `rustls_platform_verifier::Verifier::new`
ungated under `__rustls`, so the verifier is never-called-but-linked.

`otool -L` on release `rtok`, `cargo build --release --bin rtok`, this machine:

| Binary | bytes | Security.framework |
|---|---|---|
| before (`533d68a`, pre-webpki) | 25,124,800 | linked (also CoreFoundation, CoreServices) |
| after (HEAD `0731efd` + T53.3 crates already in tree) | 25,562,032 | linked (same dylib set) |

`nm -u` on the after binary still lists `SecTrustCreateWithCertificates` and
the other `SecTrust*` imports. Security.framework did **not** disappear.

Hook spawn, n=200, nearest-rank p95, fresh `RTOK_HOME`, same spawn-to-exit
harness as `tests/latency.rs`, sequential arms, 2026-09-18, 1-minute load 50
(other agents compiling — not a P17 quiet run):

| Event | before p50 / p95 | after p50 / p95 |
|---|---|---|
| `PreToolUse` | 34.14 / 79.71 ms | 32.91 / 80.82 ms |
| `PostToolUse` | 28.99 / 66.47 ms | 47.33 / 92.60 ms |

No dyld win, as expected while the frameworks stay linked. Absolute p95 is
scheduler noise against the quiet P17 row (2026-09-07: Pre 5.79–6.94 ms, Post
7.24–8.17 ms). `cargo test --release --test latency -- --nocapture --test-threads=1`
on the after binary during the same compile storm: Pre p95 235 ms, Post 336 ms
(gate fail; load, not the hook path). Corporate CAs: `SSL_CERT_FILE`
(`docs/config.md`, TLS and corporate CAs).

**Dev, `--features graph-lbug` (archived; feature removed P39).** The whole debug footprint was one C++ library. `lbug` builds
`liblbug` through `cmake-rs`, which reads `OPT_LEVEL`/`DEBUG` from the profile: at cargo's dev
defaults that is `CMAKE_BUILD_TYPE=Debug`, `-O0 -g`. `[profile.dev.package.lbug] opt-level = 2,
debug = false` flips it to Release, `-O3 -DNDEBUG`.

| Artifact | Before | After | Δ |
|---|---|---|---|
| `liblbug.a` | 2 169 748 672 B (2.02 GiB) | 83 940 608 B (80.1 MiB) | **−96.1 %** (25.8×) |
| `lbug` build directory | 4.3 GiB | 358 MiB | **−92 %** |
| cold `cargo build --features graph-lbug` | 3m36s | 8m36s | +5m00s |

Five minutes of `-O3` bought back 3.9 GiB, once per feature set. One `cargo clean -p lbug -p rtok`
before the measurement freed **24.5 GiB across 34 028 files** on a volume that was 98 % full.

**Dev, Rust debuginfo, default features.** Measured as an isolated A/B — three cold builds into
three empty `CARGO_TARGET_DIR`s, `--config profile.dev.debug=…`, nothing else varied:

| `debug` | `target/debug/rtok` | target dir | cold build |
|---|---|---|---|
| `true` (cargo default) | 62 500 520 B | 1 479 MiB | 1m50s |
| `"line-tables-only"` (chosen) | 60 378 600 B | 1 211 MiB | 2m00s |
| `false` | 54 364 592 B | 945 MiB | 1m13s |

`line-tables-only` gives up 268 MiB of the 534 MiB that `debug = false` would, and keeps what the
fail-open rule depends on. Checked, not assumed: a crate compiled with exactly that flag still
returns `Err` from `catch_unwind`, and `RUST_BACKTRACE=1` still prints `panicked at src/lib.rs:1:14`
with `at ./src/lib.rs:1:14`, `:2:14`, `:6:18`, `:5:42` on the frames. Only variable names and types
are gone. **No profile may set `panic = "abort"`** — `hooks::dispatch` fails open through five
`catch_unwind` sites, and an abort would exit non-zero.

The first "before" for the dev binary was misleading: `target/debug/rtok` held 45 135 464 B from an
older feature set, which made the new binary look 12 MB *larger*. Only the isolated A/B above is
the real comparison. Any size measurement in a shared `CARGO_TARGET_DIR` is a stale artifact until
proven otherwise.

**Packaging `liblbug` differently does not help.** A `.framework` is a directory around the same
Mach-O — same bytes, plus a plist. An `.xcframework` is strictly larger by construction: one slice
per platform/arch, and it has no representation for the Linux x86_64 target `dist` ships (T10.4).
A `.dylib` (`LBUG_SHARED=1`) is the only variant that removes bytes — static linking with
`+whole-archive` copies lbug into `rtok` *and* into each of ~12 test binaries — but it costs the
single static binary of D1, `lbug`'s `build.rs` emits no `-rpath` so those test binaries would not
find it at runtime, and `LBUG_SHARED` is a build-time env var while `.cargo/config.toml [env]` is
global, so "dylib in dev, static in release" is not expressible and toggling it forces a full C++
rebuild (`rerun-if-env-changed`). At 80 MiB rather than 2.02 GiB the premise is gone anyway.

### `rtok stats --save-baseline before-rtok` (2026-09-03)

Gate P1. Default `[stats] since = 30d` (not the 17-session slice above). File: `~/.rtok/measurements/before-rtok.json`. `--compare before-rtok` → all Δ0. Estimator: 4 chars/token. No rtok hooks in `settings.json` at save time. Proxy `usage` empty (`api` {}).

| Item | Value |
|------|-------|
| Sessions / lines | 580 / 181 303 (0 malformed) |
| Tool results, est. tokens | 13.03 M total; Bash 7.71 M (59 %), Read 3.07 M (24 %), WebSearch 0.70 M, MCP lean-ctx `ctx_read` 0.48 M |
| Bash by family | sed 1.71 M, cd 1.20 M, cat 0.93 M, grep 0.93 M, git 0.35 M |
| MCP groups | lean-ctx 0.86 M, engram 36 K |
| Cache | read 8 124 M, creation 235 M, uncached input 1.52 M → 97.2 % hit rate |
| Median final context | 65 882 tokens per session |
| Archive replay (estimate) | CTT 11.81 G → 8.42 G (−28.7 %); 1 803 candidates |

### A/B bench (T9.2, 2026-09-02)

Config A = `bench/configs/legacy.json` (81-hook + dual-proxy baseline described above). Config B = `bench/configs/rtok.json` (7 rtok hooks, `rtok mcp`, `ANTHROPIC_BASE_URL` :8790). Six tasks × 3 runs. Live `claude -p` is gated on `RTOK_BENCH_LIVE=1`; this commit ran without it, so usage/cost are zeros and pass rate is the tasks' `check = true`.

| config | mean input | mean cache | mean output | mean cost USD | pass |
|--------|------------|------------|-------------|---------------|------|
| a (legacy) | 0 | 0 | 0 | 0.0000 | 6/6 |
| b (rtok) | 0 | 0 | 0 | 0.0000 | 6/6 |
| **delta (b−a)** | 0 | 0 | 0 | 0.0000 | 0 |

Source: `bench/results/a.json`, `bench/results/b.json`. Re-run with `RTOK_BENCH_LIVE=1` to fill cost.

Your local meters (each measures a different slice, none the bill):

| Tool | Own meter | Note |
|------|-----------|------|
| rtk | 40.4 % of bash output over 5,725 cmds (1.1 M of 2.8 M) | `rtk read` only 6.4 %; grep 18.5 %; diff 90 % |
| headroom | 3.2 % today, 3.5 % 7 d, 11.3 % 30 d (14.35 M of 126.7 M) | proxy on :8788 chained to caveman :8787 |
| lean-ctx | "75 % ratio, 6.1 M difference" | +3.1 K tokens/turn fixed injection; 0 output tokens saved; "not a provider bill" (its words) |
| caveman | proxy in record mode; this session uncompressed | Pro/Max streaming sessions pass through |
| claude-mem | "87 % savings" | ratio of its own retrieval reads (21 K) vs. work it indexed (159 K) |
| token-optimizer | author's sessions: 14.44 M tokens / 30 d, "$313/mo measured" | 3.3 chars/token estimator; 27 Python hooks here |

## 3. Host surfaces (verified against code.claude.com/docs/en/hooks and env-vars, 2026-09-01)

- 32 hook events. PreToolUse may return `permissionDecision` (allow/deny/ask) and `updatedInput`; exit 2 blocks. **PostToolUse cannot modify or replace the tool result**; it only adds context. SessionStart/UserPromptSubmit inject via stdout or `additionalContext`. Command hooks support `async: true`. Timeouts are per event (600 s default, 30 s UserPromptSubmit, 10 s MessageDisplay).
- `ANTHROPIC_BASE_URL` routes traffic through a proxy; docs say it disables MCP tool search by default (check `rtok doctor`). `BASH_MAX_OUTPUT_LENGTH` default 30,000 chars, max 150,000. `autoCompactWindow` is set to 300000 in your settings; the env var name for it is undocumented.
- API side: prompt caching 1.25× (5-min write), 2× (1-h write), 0.1× read (0.025× Fable/Mythos 5.1). Context editing strategies `clear_tool_uses_20250919`, `clear_thinking_20251015`, `compact_20260112` (trigger 100 K, keep 3). `count_tokens` is free and rate-limited. Memory tool `memory_20250818`. Claude Code issue #81967: tools-array mutation invalidates the cache (up to −274 K tokens observed).
- Other hosts: Cursor `hooks.json` (before/after shell), OpenCode plugin API `tool.execute.after` (the one host that can replace results), Codex (MCP; proxy needs Responses API), Gemini CLI (MCP).

### `modes` terse/yagni vs caveman/ponytail-style (2026-09-10)

Branch `feat/modes-cave-pony`. Native helpers in `src/modes/` + enriched `modes/terse.md` /
`modes/yagni.md` via `plugins::inject`. **Not** a wrap of JuliusBrussee/caveman or
DietrichGebert/ponytail (D6); prompt text stays data (D7). Re-run:
`cargo test --test mode_bench -- --nocapture`.

Estimator: prose 4.2 chars/token (same `Estimator` as the rest of the suite). Baselines are
honest and weak on purpose — not the vendors' marketing figures from §4.

#### Compress (caveman-style)

17 agent-reply fixtures with fluff, ``` fences, backtick error strings, and critical
negations. Weak **caveman-lite** strips only `Sure!` / `I'd be happy to help…`. Ours is
`compress_prose(…, CaveIntensity::Full)`.

| Metric | Weak caveman-lite | rtok Full | Notes |
|--------|-------------------|-----------|-------|
| Avg chars saved / fixture | 12.2 | **36.2** | |
| Char save % (corpus) | 11.7 % | **34.9 %** | |
| Est. tokens saved (sum) | 50 | **147** | |
| Fence bodies | n/a (lite may leave fluff outside) | **byte-identical** to input | Hard fail if mutated |
| Negation tokens | — | **kept** (`not`/`never`/`no`) | |

Why this shape: caveman's Go shrink path is out-of-process and has corrupted inline code
(#112). A deterministic in-process stripper that refuses to touch fences is the reversible,
CI-checkable analogue — and it still beats the weak lite baseline by ~3× on chars and
tokens on this fixture set.

#### Ladder (ponytail-style)

14 labelled contexts (speculative skip, reuse, stdlib, native-before-dep, security
`must_not_simplify`, …). Naive baseline = always [`LadderDecision::Minimum`] (unstructured
“be lazy” without a ladder).

| Metric | Naive always-Minimum | rtok `evaluate_ladder` |
|--------|----------------------|------------------------|
| Correct rung | 4 / 14 (**29 %**) | **14 / 14 (100 %)** |

Why better: a prompt-only YAGNI mode has no typed state; models default to “write the
smallest new code,” which is wrong when the right answer is skip, reuse, or native
platform. Security work forces Minimum even when `speculative` is set.

#### Mode markdown budgets (inject)

| Mode | Est. tokens | Chars | Cap |
|------|------------:|------:|-----|
| `terse.md` | 162 | 679 | ≤ 250 |
| `yagni.md` | 145 | 613 | ≤ 250 |

Aliases `cave`→`terse`, `pony`→`yagni` resolve to the same builtins at SessionStart.

**Reading vs §4 vendor claims.** We still do **not** claim caveman's 65 % or ponytail's
−54 % LOC against a live bill. This gate shows the native path wins the re-runnable
fixture contest and stays inside the inject budget — the same honesty bar as the offline
T9.2 A/B zeros.

## 4. Comparison matrix

Stars/language/license from the GitHub API on 2026-09-01. "Claimed" is the vendor's number; "Measured" is yours or an independent source.

| Tool | Layer | Mechanism | Surface | Lang · License · Stars | Local · LLM-free · Lossless | Claimed | Measured |
|------|-------|-----------|---------|------------------------|-----------------------------|---------|----------|
| rtk (rtk-ai) | command output | ~80 filters, TOML custom filters, `gain` (bytes/4) | PreToolUse rewrite | Rust · Apache-2.0 · 78.2 k | ✓ · ✓ · ✗ (drops lines) | 60–90 % | 40 % of bash bytes (yours); JetBrains bill +7.6 %/0 % |
| lean-ctx (yvgude) | file reads, search, shell | 78 MCP tools, 10 read modes, 95+ shell patterns, dedup re-reads | MCP + hooks (deny Grep/Glob) | Rust · Apache-2.0 · 3.7 k | ✓ · ✓ · ~ (expand) | 75 % (own) | +3.1 K/turn injection; 0 output saved |
| headroom (headroomlabs-ai) | API request | JSON crusher, code compressor, cache-aligned live zone, CCR retrieve | proxy + MCP + wrap | Python 82 %/Rust 13 % · Apache-2.0 · 68.3 k | ✓ · ✓ · ✓ (retrieve) | up to 95 % | 11.3 % 30 d, 3.2 % today (yours) |
| caveman (JuliusBrussee) | prose + request | terse mode, shrink-hook, proxy record/compress, TOON, MCP compress/retrieve | prompt + proxy + MCP | Go · custom · 102 k | ✓ · ✓ · ~ | 65 % output | 8.5 % agentic (JetBrains); 0 here (proxy inert); issue #112 corrupts inline code; rtok native Full beats weak-lite 34.9 % vs 11.7 % chars on 17 fixtures (2026-09-10, see modes subsection) |
| ponytail (DietrichGebert) | model output | YAGNI ladder prompt | prompt file | JS · MIT · 120 k | ✓ · ✓ · n/a | −54 % LOC, −22 % tokens (own bench, Haiku 4.5, n=4) | none independent; rtok typed ladder 14/14 vs naive Minimum 4/14 on 14 fixtures (2026-09-10, modes subsection) |
| token-optimizer (alexgreensh) | reads, bash, archive, compaction, coaching | delta reads, structure maps, bash compress (111 cmds), archive >4 KB, checkpoints, quality nudges | hooks (Python subprocess) | Python · PolyForm-NC · 2.1 k | ✓ · ✓ · ✓ (archive) | "$313/mo" | author's own meter only; 27 hooks on your machine |
| codebase-memory-mcp (DeusData) | code graph | tree-sitter 158 grammars + hybrid LSP (11 langs) → SQLite (zstd), 15 tools, Cypher-like queries | MCP | C · MIT · 42.1 k (v0.7.0, 2026-09-04) | ✓ · ✓ · n/a | 99.2 % on 5 queries; Linux kernel 3 min | exits on start here (0 tools, `doctor` 2026-09-02); 281 MB binary, 141 MB cache |
| codegraph (colbymchenry) | code graph | `.codegraph/codegraph.db` (SQLite+FTS5), one `codegraph_explore` tool | CLI + MCP | C/TS · MIT · 69.4 k (2026-09-04) | ✓ · ✓ · n/a | −88 % tool calls, −62 % tokens (7 repos) | 97 MB `.codegraph/` for one repo (cross-code); no binary here |
| code-review-graph (tirth8205) | code graph | tree-sitter → SQLite, impact radius, minimal context, communities | MCP (30 tools) | Python · MIT · 31.2 k (2.3.7, 2026-09-04) | ✓ · optional embeddings · n/a | 65× median (36–376×, 6 repos) | 30 tools ~2 295 desc tokens (`doctor`); F1 0.69 against its own edges (own README) |
| graphify (safishamsi) | code graph | 37 tree-sitter grammars + LLM extraction for docs → JSON, Leiden communities, HTML map | CLI + MCP + plugin | Python · Apache-2.0/MIT · 114 k (v8, 2026-09-04) | ✓ · ✗ (docs layer) · n/a | “code maps for free” | not installed; none |
| serena (oraios) | symbols | LSP-backed symbol tools | MCP | Python · MIT · 28.7 k (2026-09-04) | ✓ · ✓ · n/a | — | 22 tools ~1 494 desc tokens; times out at 30 s here; most precise |
| claude-mem (thedotmack) | memory | LLM-extracted observations, SQLite+Chroma, progressive disclosure | plugin + MCP + daemon | JS · Apache-2.0 · 92.9 k | ✓ · ✗ (uses Claude) · n/a | 87 % (own banner) | costs tokens to build memory |
| engram (Gentleman-Programming) | memory | agent-written notes, SQLite FTS5, HTTP+MCP | MCP | Go · MIT · 6.3 k | ✓ · ✓ · n/a | — | 18 tools' descriptions per session |
| mem0 / OpenMemory | memory | LLM extraction + vectors | MCP (Docker, Qdrant) | Python · Apache-2.0 · 64.5 k | ~ · ✗ · n/a | — | not local-first by default |
| OpenViking (volcengine) | context DB | L0/L1/L2 tiered loading, session compression | SDK + MCP | Python 78 %/Rust 14 % · AGPL-3.0 · 34.9 k | ✓ · ✗ · n/a | 34–91 % | none |
| TOON (toon-format) | data format | tabular JSON → TOON | library | TS · MIT · 25.3 k | ✓ · ✓ · ✓ | −42.6 % tokens; accuracy 72.2 vs 71.4 | vendor bench |
| LLMLingua-2 (microsoft) | prompt compression | small-model token pruning | library | Python · MIT · 6.6 k | ✓ · needs model · ✗ | 2–20× | quality risk on code |
| bifrost (maximhq) | gateway | semantic cache (redis, 0.9 threshold) | proxy | Go · Apache-2.0 · 7.7 k | ✓ · embeddings · ✗ | — | agent contexts never repeat; a hit is a wrong answer |
| Anthropic native | platform | prompt caching, context editing, memory tool, deferred tools, auto-compact | API/Claude Code | — | ✓ · — · ✓ | — | free; align with it, do not fight it |
| **Your stack today** | all layers | 10 tools, 81 hooks, 2 proxies (+bifrost in Docker) | hooks + MCP + proxy | mixed | ✓ · mostly · mixed | — | 3–40 % per slice; no end-to-end number |
| **rtok `modes` (terse/yagni)** | model output / prose | enriched prompt modes + native `compress_prose` / `evaluate_ladder` | inject + `src/modes` | Rust · (in-tree) | ✓ · ✓ · ✓ fences | none claimed on bill | fixture bench 2026-09-10: 34.9 % chars / 147 tok vs lite; ladder 14/14; budgets 162/145 |
| **Proposed `rtok`** | all layers | 10 plugins, 1 binary, 3 surfaces, measurement + bench | hooks + MCP + proxy | Rust | ✓ · ✓ · ✓ by default | none until measured | `rtok stats`, `rtok bench` |

## 5. Your stack vs. the proposed app

| Aspect | Today | Proposed |
|--------|-------|----------|
| Hooks | 81 across 16 events (token-optimizer 27 Python, orca 12, caveman-proxy 11, holdmylid 10, lean-ctx 9, cbm 7, tokenbar 2, rtk 1, caveman shrink 1, codegraph 1) | ≤ 8 token-related (`rtok hook <event>`), non-token hooks untouched |
| Per-tool-call overhead | up to ~30 subprocesses per event chain, several Python | one Rust process < 10 ms |
| Bash filtering | rtk + lean-ctx ctx_shell + token-optimizer bash_compress | `cmd` (delegates to rtk filters, archives raw, measures) |
| Reads | lean-ctx (78 tools, 3.1 K/turn) + token-optimizer read_cache + headroom audit | `read` (5 tools, no banner, dedup) |
| Proxies | headroom :8788 → caveman :8787 (inert on Max) → Anthropic; Docker: headroom → bifrost | `rtok proxy :8790` (passthrough → compress), chainable for A/B |
| Memory | claude-mem (LLM) + engram | one, agent-written, FTS5 |
| Code graph | code-review-graph + codebase-memory-mcp + serena (+codegraph stale) ≈ 85 MCP tools | `graph`: native tags index, 3 tools (D6; was “adapter” before D6 was rewritten) |
| Injection per turn | lean-ctx 3.1 K + engram + claude-mem + ponytail + caveman + token-optimizer nudges | one budget (800 tokens), byte-stable |
| Measurement | 5 incompatible meters, none the bill | usage from proxy + transcripts; context-token-turns; A/B bench |
| Reversibility | partial (headroom retrieve, token-optimizer expand) | every rewrite has `expand <id>` |

## 6. Technique ranking (evidence-weighted, for this workload)

1. Clear or shrink old tool results in context (archive live zone; context editing) — compounds over turns.
2. Keep the cached prefix byte-stable (no tools-array or system-prompt churn; stable injections).
3. Read less: outline/signature modes on first read, dedup re-reads, deny giant native reads.
4. Fewer output tokens: YAGNI/terse modes (measure), fewer turns via better first reads.
5. Command output filtering — real but small in the bill (JetBrains); keep it lossless.
6. Tabular JSON → TOON where results are tables (−40 %).
7. Memory with progressive disclosure (titles → ids → bodies), never bodies at SessionStart.
8. Code graph queries instead of grep-and-read chains — plausible, unmeasured; adapter first.
9. LLM-based compression — negative until proven; **v0.2+**, not v0.1 (`ideas.md` I-21).



### Plugin design surveys (P14, 2026-09-02)

Every alternative in `src/plugins/*/PLAN.md` with the survey date. Stars/versions for retired tools are from §4 (GitHub API 2026-09-01) unless noted.

| Alternative | Version / date | Cited by |
|-------------|----------------|----------|
| rtk gain | rtk-ai · 2026-09-01 | measure, cmd, guard |
| headroom savings / live zone / proxy / audit | 2026-09-01 | measure, read, archive, proxy |
| token-optimizer dashboard / bash_compress / structure map / archive / refetch | 2026-09-01 | measure, cmd, read, archive, guard |
| lean-ctx ctx_shell / read modes / banner / deny Grep | 2026-09-01 | cmd, read, inject, guard |
| caveman shrink / proxy / TOON | 2026-09-01 | archive, proxy, toon, inject/modes |
| engram | 2026-09-01 | inject, memory |
| claude-mem | 2026-09-01 | inject, memory |
| ponytail | 2026-09-01 | inject, `src/modes` ladder |
| rtok modes terse/yagni (native) | fixture bench 2026-09-10 | inject, `src/modes`, `tests/mode_bench.rs` |

| codebase-memory-mcp | v0.7.0 · 2026-09-04 | graph |
| codegraph | 2026-09-04 | graph |
| code-review-graph | 2.3.7 · 2026-09-04 | graph |
| graphify | v8 · 2026-09-04 | graph |
| serena | 2026-09-04 | graph |
| OpenViking | 2026-09-01 | memory |
| bifrost | 2026-09-01 | proxy |
| TOON (toon-format) | 2026-09-01 | toon |
| Langfuse generation usage | 3.x · 2026-09-02 | measure (outside retired stack) |
| cargo --message-format=json | rustc 1.90 · 2026-09-02 | cmd (outside) |
| aider repo map (tree-sitter + PageRank) | 0.82 · 2026-09-02 | read (outside) |
| Anthropic context editing / tool-result clearing | docs 2026-09-01 | archive (outside) |
| LiteLLM | 1.7x · 2026-09-02 | proxy (outside) |
| Lost in the Middle (Liu et al.) | arXiv 2307.03172 · 2023-07 | inject (outside) |
| LangGraph recursion limit | 0.2x · 2026-09-02 | guard (outside) |
| mem0 / OpenMemory | 2026-09-01 | memory (outside) |
| Universal Ctags | 6.x · 2026-09-02 | graph (outside) |
| minified JSON / CSV / JSONL | RFC 8259 / 4180 · 2026-09-02 | toon (outside) |

## 7. Fact-check ledger

Checked 27 claims + 19 repos. Refuted: JetBrains rtk post (rtk did not save; +7.6 % on low effort), TOON numbers (42.6 %, 72.2 vs 71.4), "headroom is Rust" (82 % Python), "OpenViking is Rust" (78 % Python), "claude-mem is TypeScript" (JS per API), "37 hook events" (32). Partial/unverifiable: `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (undocumented; settings key exists), cache invalidation order and 20-block lookback (not in docs), `ENABLE_TOOL_SEARCH` value range and 10 % trigger (undocumented), offline Claude tokenizer (docs silent; `count_tokens` is the only official path). Confirmed: PostToolUse cannot modify results; PreToolUse `updatedInput`; caching multipliers incl. 0.025× Fable/Mythos; context-editing names; memory tool name; issue #81967; caveman #112; ponytail bench figures; codebase-memory-mcp and codegraph README figures; lean-ctx README figures. GitHub reports NOASSERTION for caveman and token-optimizer licenses; token-optimizer's local LICENSE file is PolyForm Noncommercial 1.0.0.

## 8. Open questions

- Does `ANTHROPIC_BASE_URL` really disable MCP tool search on your setup (deferred tools are visible in this session, so something enables it)? `rtok doctor` T1.4 answers it.
- Does headroom's live-zone compression keep the prefix byte-stable across turns in your traffic? T5.5 cache-health report answers it before rtok compress replaces it.
- Pricing for Fable 5.1 output tokens: the cost split above assumes p_out = 5 × p_in; adjust in `rtok stats --price` once known.

## 9. Competitive gap review (2026-09-17)

Method: five Haiku web-scan agents (rtk/headroom/caveman; MCP read+graph servers; memory and repo-map tools; host-native features; new 2026 entrants) and one Sonnet agent that inventoried rtok's own surface from code and config. Synthesis by Fable 5.1. Scan cost: ~470 K subagent tokens. Everything below is from READMEs, release notes and docs as of 2026-09-17 unless marked *measured*; vendor numbers are quoted as claims. Two scan results were discarded as wrong targets (an `engram` by AcidicSoil, an `OpenViking` mirror by LoicHmh) — the §4 rows for those two tools stand un-refreshed. `token-optimizer-mcp` (ooples, v7.0.0, TypeScript, 74 tools) is a different project from the `token-optimizer` hook pile in §4; both are listed.

### 9.1 What moved since §4 (2026-09-01)

| Tool | Then | Now (2026-09-17) | Source |
|------|------|------------------|--------|
| rtk | ~80 filters | v0.47.0 / 0.50.0-rc: 100+ filters (ctest, mvnd, spring-boot, liquibase, ssh, AWS, containers), Gemini `BeforeTool`, OpenCode/OpenClaw/Hermes adapters, Windows native hook; maintainers now say "cuts bash output, not necessarily the bill by 90 %" | github.com/rtk-ai/rtk/releases |
| headroom | proxy + CCR | adds ASGI middleware, TS library, Codex WS fixes, hosted proxy; claims 21–57 % on own benchmarks, 20 % "for coding agents" | github.com/headroomlabs-ai/headroom/blob/main/CHANGELOG.md |
| caveman | skill + proxy | v2.7.0: `caveman learn` (ranks token sinks from history), hosted proxy, per-request accounting; engine relicensed BSL-1.1 | github.com/JuliusBrussee/caveman/releases |
| lean-ctx | 78 tools | v3.10.2: 30+ tools incl. `ctx_handoff`/`ctx_agent`, 10 read modes incl. `diff`/`density`, mode predictor learned from past sessions, ~13-token re-reads, still ~3.0 K fixed per-session overhead (own README) | github.com/yvgude/lean-ctx |
| token-optimizer-mcp | — | v7.0.0: default switched from `enforce` to `assist`; publishes a randomized 16-task holdout: cheaper on 11/16, median 0.926× cost, quality 0.994 vs control (own bench) | github.com/ooples/token-optimizer-mcp/releases/tag/v7.0.0 |
| codebase-memory-mcp | v0.7.0 | v0.10.0, LZ4 store, arXiv:2603.27277 (31 repos, 83 % answer quality, "99.2 % fewer tokens" on 5 queries) | github.com/DeusData/codebase-memory-mcp |
| serena | — | v1.6.0 (2026-07-16), 40+ LSP languages; no token measurement | github.com/oraios/serena |
| claude-mem | — | v13.25.1, 4 MCP tools, SQLite+FTS5+Chroma, cloud sync; one third-party "~10×" retrieval comparison (mindstudio.ai blog, not a bill delta) | github.com/thedotmack/claude-mem |
| mem0 | — | v3.1.8; states its own overhead: 6.7–7.0 K tokens + ~1 s per add | github.com/mem0ai/mem0 |
| New entrants | — | jCodeMunch-MCP (symbol retrieval, published 96.5 % vs grep-read bench, 2026-09-03); atlassian-labs/mcp-compressor (lossless wrapper that compresses *any* MCP server's results, levels low…max, 40–60 % claimed, no bench); Portkey/LiteLLM gateways ("tool description compression + allowlist", 18–28 % claimed); billion-context (prefix-cache-friendly reversible history compression proxy); tool-result-cache-rs / TVCACHE (content-hash tool-result caches) | URLs in the scan; all claims unverified |

### 9.2 Host-native features that make third-party work redundant (docs, 2026-09-17)

| Host | Native now | Effect on rtok |
|------|-----------|----------------|
| Claude Code | Tool Search defers MCP tools (~3 K tokens loaded per query instead of every schema); auto-memory `MEMORY.md` (v2.1.59+, on by default); `PreCompact` / `PostCompact` / `Setup` / `CwdChanged` / `FileChanged` / `PreModelSwitch` hooks; `promptCacheTtl`; images/PDFs auto-dropped near the limit | Description-token compression (Portkey-style) is not worth building — `doctor` already flags `mcp_tool_search_disabled`. Auto-memory overlaps `memory` recall on this host. rtok already registers `PreCompact`/`PostCompact` here (T2.5: checkpoint of prompts, paths, errors; modes re-injected on `source = compact`); the checkpoint has no archive ids, and no other host registers its compaction event (T58.2). |
| Claude API | context editing (`clear_tool_uses`, `clear_thinking`), server-side compaction, memory tool (~2.5 K overhead), 1h cache TTL at 2× write, Fable/Mythos 5.1 cache read 0.025× | `archive` and context editing do the same job; T51.2 (emit native context editing) is the reconciliation. |
| Cursor | `afterMCPExecution` fires after the tool response and before it enters context; `preCompact`; "Dynamic Context" (v3.11, claims 46.9 %, no method published) | A hook that can see an MCP result before context is the surface `PostToolUse` lacks on Claude Code — verify whether it may modify the result (unverified). |
| Codex CLI | `PreCompact`/`PostCompact`, `SubagentStart/Stop`, hooks may call MCP tools | Same compaction surface as Claude Code. |
| OpenCode | Two-phase compaction: non-destructive "marking" of old verbose tool outputs (trigger when >20 K freed, keeps newest 40 K), then LLM summary; `experimental.session.compacting` hook | Overlaps `archive` on this host; a pointer inside a marked result is harmless (fail open) but the saving is double-counted unless measured per host. |
| Gemini CLI / Copilot CLI | context-compression hook before summarization; Copilot auto-compacts at 80 % and has `/context` | Same as above; `/context` is what `rtok doctor` prints. |

### 9.3 Better / worse / missing, by category

Grounded in §2 (this workload: tool results 2.83 M est. tokens, Bash 35 %, Read 15 %; assistant output 8.6 M tokens of which **96 % is tool input**, i.e. the code and `old_string`s the model writes; on Fable/Mythos 5.1 output is **39 %** of the bill).

| Category | rtok better | rtok worse | Missing, and whether it is worth building |
|----------|-------------|------------|-------------------------------------------|
| Command output | lossless (`expand`), measured per family, one process ≤ 10 ms; a default rule (40 lines, head/tail, dedupe) caps every stem, so nothing passes through whole | 9 TOML rules + 10 formatters keep signal by meaning; every other family (docker, kubectl, gh, aws, pip, mvn, gradle, dotnet, tsc, eslint) is cut by position, so its error lines can fall in the gap; rtk has 100+ per-command filters | Per-family rules chosen by measured after-bytes — **T50.1** (data only); formatters for table/grouped outputs — **T58.5**. |
| Reads | 4 modes, sha256 dedup, root guard, ~223 desc tokens for 12 tools (`rtok doctor`, 2026-09-18); no banner | lean-ctx: `diff` mode; token-optimizer: delta reads; lean-ctx re-read 13 tokens (rtok's "unchanged since" line is comparable) | **Delta since last read**: rtok already keeps the sha256 and archive id of the previous read, so a changed file can return a unified diff against that archive instead of 9.5–17 K tokens again — **T58.1**. |
| Model output (the code it writes) | typed `yagni` ladder 14/14 on fixtures; modes inside the 800-token budget | nothing targets the 96 % tool-input share | **Measured 2026-09-17 (T58.3):** `old_string` is 3.8 % of tool-input bytes and ≈ 1.3 % of output tokens, so an anchored `patch` tool (serena `replace_symbol_body`, lean-ctx `ctx_patch`) would move at most ~1 % of the output slice; not built (I-43 keeps the number). The output lever that remains is fewer and smaller writes — modes (T53.1) and the read side. |
| Injection / compaction | byte-stable 800-token budget; progressive-disclosure memory; on Claude Code a `PreCompact` checkpoint (prompts, paths, errors) and modes re-injected after the summary (T2.5) — rtk, headroom and caveman have nothing here | the checkpoint exists on Claude Code only (Codex, Cursor, Gemini, Copilot events are not registered); it carries no archive ids, so `expand` of a summarized-away result depends on the model remembering the id | Register the compaction events on every host that has them and add the live archive ids to the checkpoint — **T58.2**. |
| Foreign MCP results | old ones shrink in the proxy live zone like any `tool_result` | fresh results of other servers pass whole (atlassian mcp-compressor wraps any server) | Not worth it on this workload: MCP results were 15 K of 2.83 M (§2). Idea I-44. |
| Tool descriptions | 12 tools / ~223 tokens (`rtok doctor`, 2026-09-18); `doctor` prices every server | — | Portkey-style description compression is redundant with Tool Search deferral. Idea I-45, parked. |
| Memory | agent-written, FTS5, no model calls, titles-first | claude-mem/mem0 have vectors (P29 landed hash-embed; no ONNX); Claude Code auto-memory is free on that host | `doctor` should say when auto-memory makes rtok recall a duplicate injection. Idea I-47. |
| Code graph | 5 tools / 127 tokens (2026-09-18, T68.1 added `explore`), SQLite only, hook ≤ 10 ms | reference recall 0.351 vs LSP-grade (serena, codebase-memory-mcp hybrid LSP) | Already T52.5 / T30.2 (LSP optional). jCodeMunch's measured 96.5 % vs grep-read is the same claim class as `graph`; no new task. |
| Learning from history | `stats`, `report` rules (D24), `doctor --instructions` | caveman `learn`, lean-ctx mode predictor, context-budget plugin rank *sinks* and *recommend* | `report` already renders recommendations; a per-file / per-command sink ranking is idea I-48 until `stats` shows a sink the existing rows do not name. |
| Sub-agents | — | lean-ctx `ctx_handoff`/`ctx_agent`; theme "sub-agent isolation" | Agent results were 23 K of 2.83 M here (§2): not a lever. Idea I-46, parked with the number. |
| Gateways / caches | 4 wires, usage capture, semantic cache off (P31: 0 hits at 0.99) | — | Nothing to add; bifrost/Portkey/LiteLLM are routing products. |
| Hosts | 11 hosts with a reversible installer; per-host `support()` table | rtk/caveman list 30+ hosts (Windsurf/Cline/Aider/Qwen/OpenClaw/Hermes) | T48.8 (VS Code) is the only one with a measured user; the rest wait for a request. |

### 9.4 Ranking of the gaps by expected effect on this workload

Estimates, not measurements — each task's first step is the measurement that replaces the estimate.

1. **Anchored patch — measured and dropped (T58.3).** Output is 39 % of the Fable/Mythos bill and 96 % of output is tool input, but `old_string` is only 3.8 % of tool-input bytes (≈ 1.3 % of output tokens) over 925 sessions, so T58.4 was not built. `new_string` is 2.2× `old_string`: the model's output is the code it writes, not what it quotes back.
2. **Delta reads (T58.1).** Read is 15 % of tool-result tokens and the top-8 single results are all Reads; the dedup already handles unchanged re-reads, so the win is the changed-file re-read after an Edit — count it from transcripts before building.
3. **Compaction checkpoint everywhere (T58.2).** Cheap (the T2.5 checkpoint and restore exist; the work is host registration and one more field); the effect is keeping the measured mode savings and `expand` reachability alive after compaction on Codex, Cursor and Copilot the way they already are on Claude Code.
4. **Filter families (T50.1, then T58.5).** Real but small: JetBrains measured rtk at +7.6 % to 0 % on the bill, and rtok's default rule already caps every stem; the win is the error line that the positional cut drops. Data first (TOML rules), Rust only for table and grouped outputs.

Promoted: I-41 → T58.1, I-42 → T58.2, I-43 → T58.3 (measured; T58.4 dropped with the number). Not promoted: I-44 foreign-MCP compression, I-45 description compression, I-46 handoff, I-47 doctor overlap audit, I-48 sink ranking — each with the number that parks it in `ideas.md`.

## 10. Skill loading: where the tokens go (2026-09-17)

Question from the creator: how to spend fewer tokens and requests on loading skills (the
`SKILL.md` folders every host now reads). Sources: host docs (10.1, Haiku web survey), and
three dated measurements on this machine (10.2). Estimates are bytes/4 unless a row says
otherwise.

### 10.1 How each host loads a skill

| Host | Where | At session start | On invocation | Knobs that shrink the listing |
| --- | --- | --- | --- | --- |
| Claude Code | `~/.claude/skills/<n>/SKILL.md`, `.claude/skills/`, `<plugin>/skills/` (listed as `/plugin:skill`) | name + description of every listed skill in the system prompt (docs: "~100 tokens per skill") | the whole `SKILL.md` body; `references/`, `scripts/`, `assets/` only when the model reads them (script output enters context, script code does not) | `disable-model-invocation: true` (only `/name` by a human), project-scoped skills, plugin enable/disable |
| Cursor | `.cursor/skills/`, `.agents/skills/`, user equivalents | name + description | body on demand, resources lazily | `paths:` globs scope a skill to matching files; `disable-model-invocation` |
| OpenCode | `.opencode/skills/`, `~/.config/opencode/skills/`, Claude paths | Agent Skills standard (not documented in detail) | body on demand | `opencode.json` permission `allow` / `deny` / `ask` per skill pattern |
| Copilot CLI / VS Code | `.github/skills/`, `.agents/skills/`, `~/.copilot/skills/` | metadata for discovery | body when relevant or on `/name` | not documented |
| Gemini CLI | `~/.gemini/skills/`, `.gemini/skills/`, `.agents/skills/`, extensions | metadata only | `activate_skill` tool loads the body | `/skills disable <n>` per session; precedence built-in > extension > user > workspace |
| Codex / ChatGPT | `.agents/skills/`, plugins | not documented | not documented | not documented |

Docs: https://code.claude.com/docs/en/skills, https://cursor.com/docs/skills,
https://opencode.ai/docs/skills/, https://docs.github.com/en/copilot/concepts/agents/about-agent-skills,
https://geminicli.com/docs/cli/skills/, https://agentskills.io (spec: description ≤ 1024
chars, body ≤ 500 lines recommended). Every host does the same two-level load: a listing
that rides in every request, and a body that lands once per invocation and then stays in
the conversation for every later request of that session.

### 10.2 Measured on this machine

| What (date, command) | Result |
| --- | --- |
| Skills on disk (2026-09-17, `skillscan` over `~/.claude/{skills,plugins}`, `~/.codex/skills`, `~/.cursor/skills`, `~/.config/opencode`, `~/.copilot`, `~/.agents`) | 243 `SKILL.md`, 1,452,088 B (≈ 363 K tokens) of bodies; description median 200 chars; largest bodies 19–33 KB (`skill-creator`, `m5-onboard`, `skill-development`, `monitor-ci`, `imagegen`). Most of the 195 plugin-cache copies are marketplace clones, not installed. |
| What Claude Code actually lists (2026-09-17, `enabled.py` over `installed_plugins.json` + `~/.claude/skills`) | 25 user skills (2,888 description chars) + 41 skills from 5 enabled plugins (9,948 chars; claude-mem 18, claude-obsidian 15, ponytail 6, engram 1, slint 1) = 66 listed skills, 12,836 description chars ≈ 3.2 K tokens of descriptions, ≈ 4–5 K tokens with names and paths, in the system prompt of every request. Body bytes of the listed set: 214,605 (plugins) + user skills — loaded only on invocation. |
| Invocations (30 d to 2026-09-17, 890 transcripts, 81 sessions) | 26 `Skill` tool calls, 8 distinct skills (`slint` 8, `artifact-design` 7, `update-config` 4, `claude-api` 3, four × 1); 12 of 81 sessions (15 %) invoked any skill; 8 direct `Read`s of a `SKILL.md`; 115 slash-command messages (`<command-name>`), median 143 B — negligible. |
| Where the body lands (2026-09-17, `skillinj.py` over 173 transcripts in `~/.claude/projects`) | The `Skill` tool_result is 22 B (`Launching skill: <n>`); the body arrives as the **next user message** (`Base directory for this skill: …`): 17 bodies, median 8,863 B (≈ 2.2 K tokens), max 248,175 B (`update-config`, ≈ 62 K tokens in one message). `rtok stats` counts tool results, so it sees 2.5 KB where ≈ 150 KB entered. |
| What `rtok stats` now folds (2026-09-18, `rtok stats --json --since 30d`, T61.1 skills section over `~/.claude/projects`) | 24 injected bodies, 792,820 B (≈ 198 K tokens); `update-config` 391,824 B over 2 invocations, `claude-api` 248,816 B, `slint` 35,997 B over 8. The `resident` column — body bytes × the API requests that carried them — totals ≈ 96.8 MB over the 30 d window (the context-token-turns view of §10.3's "heavy tail"). |
| rtok hub skill (2026-09-18, `skills/rtok/SKILL.md` frontmatter; T71.3) | description **112 chars** (≤ 120); body **780 B** (≤ 2 KB); `disable-model-invocation` unset. `doctor` lists it from the same §10.1 user roots as any other skill once `rtok agents install <host>` has copied the hub. |

### 10.3 Where the cost is, ranked for this workload

1. **The listing, every request.** ≈ 4–5 K tokens of the cached prefix per request. At the
   measured 97.5 % cache-hit rate (§2, `rtok stats --since 30d`) it is mostly cache-read,
   but every change to the listed set — a plugin auto-update (`lastUpdated` 2026-09-15 on
   the installed plugins), a new user skill, an edited description — rewrites the whole
   prefix once per open session. Two levers: fewer listed skills, shorter descriptions.
2. **Bodies with a heavy tail.** Invocation is rare (26 in 30 d) but one body of 248 KB
   costs more than the listing does in 50 requests, and it stays in the session's context
   for every later request. A body over ~8 KB is almost always documentation pasted into
   `SKILL.md` instead of a `references/` file the model reads only when needed.
3. **Resources loaded through `Read`** are ordinary tool results: rtok's `read` plugin
   (dedup, modes) and the archive live zone already apply. Skill bodies do not pass through
   any rtok surface today: they are not tool results and not hook output.
4. **Requests** are not the cost: a skill invocation is one tool call inside the turn, and
   the listing adds zero requests. The only request-shaped waste is a `Read` of a
   `SKILL.md` the host would have injected anyway (8 in 30 d).

### 10.4 Techniques, with the lever each pulls

| Technique | Lever | Evidence / limit |
| --- | --- | --- |
| Description ≤ 120 chars, one sentence: what it does and when to pick it | listing | median here is 200 chars; a 120-char cap on 66 skills is ≈ −1.3 K tokens per request, byte-stable once set |
| `disable-model-invocation: true` for skills only a human runs (setup, onboarding, release checklists) | listing | Claude Code and Cursor document it; the skill keeps working as `/name` |
| Project-level skills for project-only knowledge; user-level only for cross-project ones | listing | Claude Code lists project skills only inside that project; Cursor `paths:` scopes further |
| Enable plugins per project, not globally | listing | 41 of the 66 listed skills here come from 5 plugins; a plugin unused in a repo still lists all its skills there |
| Body ≤ 2 K tokens: hub `SKILL.md` + `references/*.md` read on demand; scripts in `scripts/` (only their output enters context) | body | the 248 KB `update-config` body is the ceiling case; agentskills.io recommends ≤ 500 lines |
| One skill per task family, not per sub-step | listing + body | fewer lines in the listing; the body loads once instead of three times |
| Do not restate `CLAUDE.md` in a skill | body | `CLAUDE.md` is already in every request; a skill that repeats it pays twice |
| Pin plugin versions / update in one batch | cache | each listing change is a full prefix rewrite for every open session |
| Measure before trimming | all | `rtok stats` cannot see skill bodies today (10.2); I-49 makes them a row |

### 10.5 What rtok can add (ideas I-49–I-52)

- **I-49 `stats` skill row.** Count the user message that follows a `Skill` tool_use (marker
  `Base directory for this skill:` or the `/plugin:skill` header) as a `skill` family: calls,
  bytes, mean, p95, per skill name. Without it the 150 KB measured above is invisible.
- **I-50 `doctor` skill audit.** For every skill the host lists: description chars, body
  bytes, invocations in the window; flag descriptions > 200 chars, bodies > 8 KB, skills
  never invoked in 30 d, and skills that duplicate a rtok surface (T59.7 already does the
  host-feature half). Output is advice, never an edit.
- **I-51 live-zone shrink for skill bodies.** The `archive` live zone already replaces old
  tool results with an `expand <id>` pointer; the same matcher on a skill-body user message
  older than N turns would drop the 248 KB case to a pointer for the rest of the session.
  Gate: measure how many requests carry a skill body (I-49 first).
- **I-52 rtok's own skill (the creator's request).** Design constraint from this section:
  description ≤ 120 chars, body ≤ 2 KB hub pointing at `docs/`, `disable-model-invocation`
  off (the model must pick it), installed and removed by `rtok agents install/remove`
  together with the host plugin, one per host that supports the format (10.1).

### 10.6 Open questions

- **T71.4 (2026-09-18):** `cargo test --test skill_listing` on
  `tests/fixtures/proxy/skills_listing_request.json` captured through `rtok proxy`
  (`call_io`): `<available_skills>` block **452 B** for **3** listed skills;
  **92 B** framing per skill beyond its description (name + `fullPath` + tags; not the
  docs' "~100 tokens per skill"). `doctor::SKILL_LISTING_FRAMING_BYTES` carries the
  constant; listing bytes per request = description bytes + `N × 92`.
- Whether hosts other than Claude Code and Cursor honour `disable-model-invocation` in the
  listing is not documented (10.1).

### 10.7 Working around the blind spot (2026-09-17)

Two facts fix it. In the transcript the injected body is its own record: `type: "user"`,
`isMeta: true`, `turnCompanion: true`, `sourceToolUseID: <id of the Skill tool_use>`, text
`Base directory for this skill: <path>\n\n<SKILL.md body>` (`skillrec.py`, 2026-09-17,
two invocations checked). On the wire it is a plain user text block that follows the
`tool_result` `Launching skill: <name>` of that same `tool_use_id`, and it is re-sent whole
in every later request of the session. Three surfaces can act, in this order:

| Surface | What it can do | Limit |
| --- | --- | --- |
| `stats` (transcripts) | Count the body exactly: join the `isMeta` record to its `Skill` tool_use through `sourceToolUseID`; family `skill`, one row per skill name, bytes + est. tokens, and a "resident" column = bytes × later requests of the session (what the model actually paid for). | Claude Code only; other hosts' transcripts are not read (T49.2). |
| `proxy` / `archive` | Shrink the body outside the live zone the way old tool results are shrunk: key = the preceding `Launching skill` `tool_use_id` (byte-stable pointer), archive the body once, replace it with `[archived <id>: skill <name> · N lines · expand(<id>)]`. Lossless: `expand <id>`, or the model re-invokes the skill. The `keep_turns` boundary already decides "old". | Only when the proxy is in the chain (`ANTHROPIC_BASE_URL`); hooks never see the body (`UserPromptSubmit` carries the human prompt only, `PostToolUse(Skill)` fires before the injection). |
| `doctor` (advice) | Prevent at the source: list what the host lists, flag description > 200 chars, body > 8 KB, never invoked in 30 d, and say which lever applies (`references/`, `disable-model-invocation`, project scope). | Advice only — rtok never edits a user's skills. |

What does not work: a hook cannot intercept or rewrite the injection (it is not a tool
result, and PostToolUse can only add context); the MCP surface never sees it; a compaction
checkpoint (T2.5) does not carry skill bodies, so after auto-compaction the body is gone
and the model re-invokes — which is the cheap outcome, not a loss.

Order: measure first (T61.1), shrink behind the measurement (T61.2, gate: the `resident`
column shows skill bodies above 2 % of input tokens on a real window), advise in parallel
(T61.3). The 248 KB `update-config` body alone is ≈ 62 K tokens resident in every request of
that session; at the measured 97.5 % cache hit it is cache-read, at each cache miss it is
a full re-send.

### 10.8 What the host plugins can do (2026-09-17)

§10.7 said hooks never see the injection. They do not see it, but on two hosts a plugin
can act before it or on its carrier, and on one host the compaction checkpoint can carry
the fact that a skill was loaded. Checked against `src/agents/claude/mod.rs` (`ENTRIES`:
`PreToolUse` on `Bash`/`Read`, `PostToolUse *`, `PreCompact`, `PostCompact`, `SessionStart`)
and `plugins/opencode/rtok.ts` (`tool.execute.after` already rewrites bash output).

| Host | Interception point | What rtok can do there | Task |
| --- | --- | --- | --- |
| Claude Code | `PreToolUse` with matcher `Skill` (`tool_input.skill = <name>`), fires before the body is injected; the hook may answer `permissionDecision: deny` with a reason the model reads | for a body over a byte cap: archive it, answer deny with a digest (headings + first line per section) and the `expand <id>` trailer — the model gets the map, not the 248 KB, and pulls sections on demand. Off by default: a denied skill does not apply its frontmatter (`allowed-tools`, `model`, `context`), so skills carrying those keys always pass | T62.1 |
| Claude Code | `PreCompact` reads the transcript (T2.5 checkpoint already extracts prompts, paths, errors); the `isMeta` + `sourceToolUseID` records name the skills loaded so far | the restore line after compaction lists them with sizes so the model re-invokes only what the next step needs, instead of guessing which skill it had | T62.2 |
| OpenCode | `tool.execute.after` (`plugins/opencode/rtok.ts`) receives every tool's output, including the tool that loads a skill if OpenCode delivers skills as a tool call | shorten the body the way bash output is shortened, with an archive id so the full text is one `expand` away | T62.3 (step 1 verifies the delivery path) |
| Cursor, Codex, Copilot, Gemini | no hook fires on skill activation (Cursor hooks: shell, MCP, file read; Codex: none; Gemini: `activate_skill` is a tool, hooks not documented for it) | nothing on the plugin side; the proxy path (T61.2) is the only lever | — |

`PostToolUse(Skill)` stays useless for this: it can only add context, and the body is
already on its way. `UserPromptSubmit` carries the human prompt only.

## 11. rtk's four strategies and sqz, each against rtok (2026-09-18)

Sources: rtk README ("four strategies", "Does RTK break Claude's prompt cache?") and sqz README
(github.com/ojuschugh1/sqz, fetched 2026-09-18: Rust, ELv2, 625 stars, 265 commits; self-reported
"178,442 tokens saved across 3,003 compressions, 24.7 % avg reduction, up to 92 % with dedup").
Every claim on their side is vendor-reported; nothing here was re-measured. rtok side checked
in `src/plugins/cmd/{rules,formatters}.rs`, `src/plugins/guard/mod.rs`, `src/agents/`.

| Their feature | rtok today | Gap | Task |
| --- | --- | --- | --- |
| rtk smart filtering (noise, comments, boilerplate) | `keep`/`drop` patterns per rule, `BUILTIN_KEEP`, 10 formatters, raw archived first | Coverage, not mechanism: 9 rules + 10 formatters vs ~80 (rtk) / 45+ (sqz) | T50.1, T58.5 |
| rtk grouping (files by directory, errors by type) | none — `ls`/`find`/`tree` take 40 lines | generic grouping pass | T64.1 |
| rtk truncation | `max_lines`/`head`/`tail`, lossless (`expand <id>`) | rtok is ahead: rtk drops, rtok archives | — |
| rtk / sqz dedup of repeated log lines | `dedupe` folds adjacent identical lines to `(×N)` | non-adjacent, timestamp-normalised | T64.2 |
| rtk "does not break the prompt cache" paragraph | byte-stable inject, live-zone proxy rewrites, `report` cache section, 98.1 % hit rate on this machine | no page says it | T64.3 |
| sqz content-hash dedup (`§ref:HASH§`, 13 tokens) | `guard` dedups by input key only | same bytes from a different call paid twice | T65.1 (gated on a measured share) |
| sqz structural summaries (imports + signatures, ~70 %) | `read` modes via tree-sitter (`map`, `signatures`) | none | — |
| sqz JSON pipeline (nulls, arrays) | line cut; `toon` is wire-side and off | JSON-aware cut in the hook path | T65.2 (gated) |
| sqz table compaction | none | padding collapse | T65.3 |
| sqz safe mode (traces, secrets pass whole) | single `panic`/`traceback` lines kept, frames cut; secrets never redacted | keep the block | T65.4 |
| sqz hosts: Windsurf, Cline, Gemini CLI, Kiro, Zed, Copilot CLI; browser and IDE extensions | 12 hosts in `src/agents/` (no Cline, Kiro, Gemini); no extensions | hosts on request; extensions out of scope (one binary, D21) | — |
| sqz `gain` / `stats --breakdown` | `stats`, `report`, `dashboard`, one ledger | none | — |

Order by expected effect on this workload (§2: Bash 35 % of result tokens): T65.4 and T64.3
are cheap and close a correctness / documentation hole; T65.1 and T65.2 start with a
measured share and only proceed above it; T64.1, T64.2, T65.3 are fixture-gated.

## 12. recursive-llm (RLM), against rtok (2026-09-18)

Source: github.com/grishahq/recursive-llm (Python library, MIT, 604 stars, v0.4.0, last commit
2026-09-03); one Haiku agent read the README, `src/rlm/{core,repl,prompts,budget,stats}.py` and
`DOCUMENT_EVALUATION.md`. Self-reported numbers, not re-measured here: on 100 K-character
documents RLM cut tokens 61–78 % against direct completion (gpt-4-mini $0.0089 → $0.0048 with
3/3 correct vs 0/3; DeepSeek V4 Flash −61 %, 2/3 vs 0/3) at 5–10× the latency.

The idea: the document never enters the prompt. It sits as a `context` string in a sandboxed
Python REPL; the model writes code (`len(context)`, `re.search`, slicing) and gets only the
results back; `llm_query` / `rlm_query` run a child model on a slice, bounded by depth,
iterations and a `RunBudget` (calls hard; tokens and cost soft; wall clock). The system prompt
forbids answering before searching the context. Library only: no CLI, no MCP, no hooks.

| RLM feature | rtok today | Gap | Task |
| --- | --- | --- | --- |
| Context outside the prompt, a pointer with its size in the prompt | `cmd` trailer `[rtok <id> · N lines]`, `archive` live-zone pointer with head/tail and est. tokens, `read` cap | same shape | — |
| `re.search` over the context | `expand --grep` is a substring match that prints bare lines | a hit has no position, so nothing can follow but a full expand | T67.1 |
| Slice around a hit (`context[i-500:i+500]`) | `expand --lines a-b` | with T67.1 it takes two calls; every call is a turn that re-reads the prompt | T67.2 |
| Child model on a slice (`rlm_query`) | the host's Agent tool plus `expand <id>`; `handoff` (T59.6) parked at 23 K of 2.83 M | nothing on rtok's side — rtok is not the agent loop; RLM's accuracy-up / tokens-down result is the reason to re-measure the sub-agent share when T59.6 reopens | — |
| `RunBudget` hard/soft caps on calls, tokens, cost, time | none; hosts auto-compact; `stats --price`, `report` | not a saving lever for a tool outside the loop; parked as I-55 | — |
| "Search before you answer" system prompt | `inject` modes and T53.1 nudges; the T62.1 skill digest tells the model to `expand --grep <heading>` | none | — |
| REPL snapshot cap (1 MB) | `mcp.max_result_chars`, `read` cap with archive id | none | — |
| Per-depth usage tracker, trajectory JSONL | `calls` / `measurements` ledgers, `--json` (T60.1) | none | — |
| Benchmark: accuracy and cost vs direct | `rtok bench` cost per passed task | none | — |

What transfers is the loop, not the runtime: rtok already externalises every large payload
behind an id; T67.1 gives a grep hit a position and T67.2 folds the slice into the same call.

## 13. engram, feature by feature against `memory` (2026-09-18)

Source: github.com/Gentleman-Programming/engram `README.md` and `DOCS.md` read on 2026-09-18
(Go, MIT; the 18-tool / ~1 865-description-token row in §4 and `docs/comparison.md` is the
`rtok doctor` measurement of 2026-09-09). Creator request: what is missing in rtok, add what
is useful. engram's own docs carry no token-saving number; its value is recall, not bytes.

| engram feature | rtok `memory` today | Verdict | Where |
| --- | --- | --- | --- |
| `topic_key` upsert: same `project + scope + topic_key` updates the row, `revision_count++` | every `mem_save` inserts; a re-saved decision leaves two rows with one title in the 5-title recall | adopt, zero-LLM, no schema: the title is the key, upsert on `(project, kind, title)` | T66.1 |
| Git Sync: gzipped JSONL chunks + manifest, `engram sync --import` | `memory import <file.jsonl>` exists (T6.3); nothing produces that file from `rtok.db` | adopt the missing half: `memory export` in the shape `import` reads; no chunk manifest (a file in git is the manifest) | T66.2 |
| `mem_context` at session start: pinned + recent observations + sessions + prompts, 16 KiB default budget | SessionStart recall: 5 titles + ids ≤ 200 tokens; compaction checkpoint ≤ 400 tokens, same session only | keep rtok's shape (D5 budget, titles not bodies); cross-session handoff off by default behind an A/B | I-56 → T71.2 |
| `pinned` observations first in context | recency only | parked; `kind = "pin"` would do it without a column | I-57 |
| project identity from the normalised `origin` remote, `.engram/config.json` override, child-repo scan | git-root basename | parked; one checkout per repo is the workflow here | I-58 |
| `mem_update(id)` | none | covered by T66.1: re-save the same title | — |
| `normalized_hash` dedupe on save | `import` dedupes by body sha256; `mem_save` did not | covered by T66.1 (identical re-save is a no-op update) | — |
| `scope` project / personal / global | `project` column, `NULL` = no project | not needed: recall filters by project; a global note is a `project = NULL` row | — |
| `mem_judge` / `mem_compare` / `mem_review`: relations (`supersedes`, `conflicts_with`, …) judged by the model, `judgment_required` envelopes | none | rejected: every judgment is model output spent on bookkeeping, and the tool descriptions ride every turn (D15 target: fewer description tokens than engram) | — |
| `mem_session_summary` (mandatory before "done": goal, discoveries, next steps, files) | PreCompact checkpoint extracted mechanically from the transcript (prompts, paths, errors, skills) | rejected as a protocol; an agent may still `mem_save` a summary note by hand | — |
| `mem_capture_passive` (`## Key Learnings:` sections of the model's own output) | none | rejected: parses text the model already paid for; the agent-written note is the same bytes without a parser | — |
| `expires_at`, `review_after`, `duplicate_count`, `last_seen_at` lifecycle columns | none | not needed at note counts recall shows (5 titles); revisit with a measured stale-hit rate | — |
| HTTP API, TUI, cloud replication, Postgres backend | `rtok web` / `rtok tui` read the same store (D27); no cloud | out of scope (D8: one SQLite file) | — |
| optional embeddings beside FTS5 | `[plugins.memory.embed]`, off by default (P29) | parity | — |

Net: two tasks (T66.1, T66.2), three ideas (I-56–I-58), nothing that adds an MCP tool — the
description column stays at 3 memory tools.

## 14. graymatter, against rtok's `memory` (2026-09-18)

Source: github.com/angelnicolasc/graymatter (Go, MIT, one ~10 MB static binary; bbolt +
chromem-go in `.graymatter/gray.db`; MCP server, CLI and importable library; README fetched
2026-09-18). Every number below is graymatter's own (`go run ./benchmarks/token_count`,
keyword embedder, no LLM); nothing was re-measured here. rtok side checked in
`src/plugins/memory/mod.rs`, `src/store/mod.rs` (`notes`, `list_note_titles`, `search_notes`),
`src/store/embed.rs`, `migrations/0001.sql`, `src/agents/claude/mod.rs` (`ENTRIES`),
`src/web/model.rs` (`config_fields`).

Its claims: tokens per session against full-history injection ~80 → ~80 (1 session),
~630 → ~550 (10), ~1 880 → ~550 (30), ~6 960 → ~670 (100, "90 %"); a fact planted 96 sessions
ago retrieved 83 % of the time; superseded facts returned 0 %. The baseline is "re-inject the
whole history", which no coding host does, so the 90 % is not a bill delta; the two recall
numbers are the useful ones. rtok's own plant-and-recall numbers are the T69.3 table below — never graymatter's 83 %.

| Their feature | rtok today | Gap | Task |
| --- | --- | --- | --- |
| Hybrid recall: vector + keyword + recency, top-8, per-signal receipts | FTS5 BM25; optional hash-embed RRF (P29); SessionStart = newest 5 ids of the project; no recency, no receipts | ranking by age and use | T69.2 |
| 30-day decay half-life; never hard-delete; pinned facts exempt | none: every note is live forever, no pin | lifecycle | T69.1 (pin, retire), T69.2 (decay) |
| `revise` / `forget` as tombstones; corrections recorded | insert-only; the in-place update by title is the memory card "`mem_save` updates a note in place" | retire + supersede | T69.1 |
| Benchmark: tokens/session vs full injection, plant-and-recall, superseded = 0 | FTS5 and P29 hybrid 20/20 at N=1/10/30/100; superseded 0; SessionStart 100 B vs 371 866 B full injection at N=100 (`tests/memory_bench.rs`, 2026-09-18) | — | T69.3 |
| Claude Code hooks: SessionStart facts + conventions; UserPromptSubmit top-3 + `remember:`; PreCompact checkpoint; SessionEnd checkpoint + consolidation; errors to `hooks.log`, never break the session | SessionStart titles (T6.2); PreCompact checkpoint (T2.5); fail open ≤ 10 ms; nothing on UserPromptSubmit; SessionEnd registered, unhandled | `remember:`; per-turn recall (A/B); SessionEnd | T69.5; I-56 (engram `mem_context`) |
| `context-sync`: budgeted managed block in CLAUDE.md / AGENTS.md, hand-edit detection, backup | none (hook injection only; hosts without a SessionStart hook get no recall) | a sync command | T69.6 |
| `status` / 4-tab `tui`: facts, KB, recall counts, health, weights | Memory page shows two config keys; no `memory status` | store rows on the page | T69.4 |
| Knowledge graph: entities, co-mentions, Obsidian export, HTML force graph | `graph` is the code index | — | I-72 |
| Consolidation: summarise + decay + prune + extract (Ollama; OpenAI / Anthropic / keyword fallback) | no LLM (P28 is Later) | the mechanical half only | T69.1 / T69.2; I-73 |
| Embedding chain Ollama → OpenAI → Voyage → keyword | hash-embed local or `openai` (P29) | — | I-75 |
| `export --format obsidian` | JSONL import (T6.3); JSONL export is the memory card "`rtok memory export`" | markdown | I-74 |
| MCP wiring: Claude Code, Cursor, Codex, OpenCode, Antigravity, Windsurf, VS Code Copilot | 12 hosts in `src/agents/`; VS Code is T48.8; Windsurf / Antigravity on request | — | — |
| Security: loopback + bearer on network surfaces; recalled facts fenced, never in the system prompt | `rtok mcp` is stdio; recall is `id title` lines in the hook's `additionalContext`, bodies only via `mem_get` | — | — |
| Go library in three lines | `rtok-plugin-sdk` (D25) | — | — |

### T69.3 memory recall bench (2026-09-18)

`cargo test --test memory_bench -- --nocapture`. Seeded in-memory store: N sessions × 6 filler notes of realistic length, 20 planted facts at known offsets, 5 revised later (T69.1). Query = eight content words from the live body. `search_limit` = 5. No network, no LLM.

`half_life_days = 30` is N/A: T69.2 closed without ranking code (0 live notes on that machine; no `uses` / `last_used` columns, no scorer). P29 hybrid (`embed.enabled`, hash-embed RRF) ran.

| N | FTS5 hit | hybrid hit | superseded returned | SessionStart recall bytes | full live-body injection bytes |
| --- | --- | --- | --- | --- | --- |
| 1 | 20/20 | 20/20 | 0 | 95 | 6 331 |
| 10 | 20/20 | 20/20 | 0 | 95 | 39 566 |
| 30 | 20/20 | 20/20 | 0 | 100 | 113 240 |
| 100 | 20/20 | 20/20 | 0 | 100 | 371 866 |

T69.2's default stays off: FTS5 already hits 20/20 at N=100 without extra recall bytes, and there is no scorer to turn on. Floors are the FTS5/hybrid columns in `tests/memory_bench.rs`; a drop fails the test.

Where rtok is ahead: one ledger — `Measurement` rows plus proxy `usage` — where graymatter's
numbers are its own bench; FTS5 in the same SQLite file as every other plugin (D8) and three
memory tools inside the measured 12-tool / ~223-token surface (`docs/comparison.md` §2,
`rtok doctor` 2026-09-18); the
`expand` path and the compaction checkpoint with modes re-injected (T2.5); titles → ids → bodies
where graymatter injects the top-K bodies.

Order by expected effect: T69.1 first (a wrong fact recalled is worse than a missing one),
T69.3 (landed 2026-09-18: FTS5/hybrid 20/20, Gate P6 now has a floor), T69.4 (cheap; feeds T69.2 step 1), then T69.2 / T69.5 /
T69.6 behind their gates.

## 15. What a host plugin can do that rtok's own surfaces cannot (2026-09-18)

Creator request: go through §1–§13 and `ideas.md` for everything parked because rtok's three
surfaces (D2: hook, MCP, proxy) cannot reach it, and check whether a **host plugin** —
`plugins/<host>/`, linked by `rtok agents install <host>` (D21) — can.

Method: one Haiku web agent read the three plugin APIs rtok already links against
(https://pi.dev/docs/latest/extensions, https://opencode.ai/docs/plugins/,
https://cursor.com/docs/agent/hooks) and answered, per event, whether a return value may
replace a tool result, block a call, change the messages sent to the model, or run at
compaction. Vendor docs only — nothing re-measured here, and the scan disagrees with
`src/agents/cursor/mod.rs` on Cursor's event names (the installer writes
`beforeShellExecution` / `afterShellExecution`; the scan also reports `preToolUse` /
`postToolUse`). **Every task below therefore starts with a step that re-verifies the API
against the host's current docs and one real session, and closes with that finding if the
capability is not there.**

### 15.1 The three constraints that park work today

| Constraint | Where it is stated | What it blocks |
| --- | --- | --- |
| PostToolUse can only add context, never modify a tool result | §3, D2, `plan.md` working agreement | On Claude Code only `Bash` shrinks (PreToolUse rewrite → `rtok run`); `Read`, `Grep`, `Glob`, `WebFetch`, `Task` and every foreign MCP result enter context whole |
| The live zone needs the proxy | §9.3, `archive` plugin docs | A host with no base-URL setting (Cursor, Claude Desktop, pi, Windsurf, Zed, ZCode, Kimi, Copilot) never shrinks an old tool result — the lever §1 ranks first |
| A host without hook events reaches no hook plugin | `src/agents/<host>/README.md` module tables | `inject`, `guard` unreachable on pi, OpenCode, Codex; `guard` unreachable on every MCP-only host |

### 15.2 What each plugin API offers against those constraints

Scan of 2026-09-18, unverified against a running host. "—" is "not documented".

| Capability | pi extension | OpenCode plugin | Cursor plugin |
| --- | --- | --- | --- |
| Replace a tool result | `tool_result` returns `content` for **every** tool | `tool.execute.after` mutates `output` (rtok uses it for bash; other tools — not documented) | MCP results only, per the scan (`updated_mcp_tool_output`); shell output not replaceable |
| Block a call with a reason | `tool_call` → `{block, reason}` | `tool.execute.before` (throw) | `beforeShellExecution` / `beforeMCPExecution` → `permission: deny` |
| Rewrite the messages sent to the model | `context` fires before **each** LLM call with the message array | — | — |
| Change the system prompt | `before_agent_start` | — | — |
| Inject context at session start | — | — | `sessionStart` → `additional_context`, `env` |
| Act at compaction | `session_before_compact` may supply the summary or cancel | `experimental.session.compacting` may replace the prompt | `preCompact` observational |
| Register a tool without MCP | `pi.registerTool` | — (MCP entry does it) | — (MCP entry does it) |

### 15.3 What that unblocks, and what it does not

| Parked item | Why it was parked | Host plugin that reaches it | Task |
| --- | --- | --- | --- |
| Shrink results of tools other than Bash on a host with no proxy | PostToolUse cannot modify results (§3) | pi `tool_result` (all tools) | T70.1 |
| `archive` live zone without a proxy | proxy-only (§9.3) | pi `context` rewrites the message array per call — the same job the proxy live zone does | T70.2 |
| pi reaches only `measure`, `cmd` (`src/agents/pi/README.md`) | "pi philosophy is no MCP" | `pi.registerTool` is not MCP: `read` / `search` / `graph` / `memory` can be pi tools | T70.3 |
| Foreign MCP results the **host** launched | T59.4 wraps only servers rtok itself spawns (`rtok mcp -- <argv>`); lean-ctx measured at ≈ 27 % of tool-result bytes over 30 d (§2) | Cursor's post-MCP output replacement | T70.4 |
| `guard` unreachable on pi and OpenCode | no hook events on either host | pi `tool_call` block, OpenCode `tool.execute.before` | T70.5 |
| Compaction outside Claude Code (T58.2 (a)) | T58.2 registers host **hook** events; pi and OpenCode have none | pi `session_before_compact`, OpenCode `experimental.session.compacting` — both stronger than a note: they own the summary | T70.6 |
| `inject` claimed reachable on Cursor | `reaches()` counts declared surfaces, and Cursor supports hooks — but the installer registers only the two shell events, which never carry a session start or a prompt | Cursor `sessionStart` / `beforeSubmitPrompt` | T70.7 |
| Skill bodies on pi | §10.8 lists Claude Code (T62.1) and OpenCode (T62.3) only | pi `context` (T70.2) drops an old body from the array like any other block; no separate task | — |
| Sub-agent handoff (I-46), strict-mode Read deny (I-82), `RunBudget` (I-55) | parked on a **measured share**, not on a missing surface | — | stay parked |

Reading: two of the three constraints are host-plugin-shaped, and pi is the host where the
gap is widest — it reaches two plugins today and its extension API is the most capable of
the three. The proxy stays the only path on Codex, Claude Desktop, Windsurf, Zed, ZCode,
Kimi and Copilot, which have neither a plugin directory nor the events.
