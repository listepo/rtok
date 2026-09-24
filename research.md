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
| Read delta re-reads (T58.1, 2026-09-18, `rtok stats --since 90d`, 959 sessions) | 593 native `Read` calls of a path already read in the same session with an Edit/Write/MultiEdit of that path in between; 1.79 MB of 24.61 MB Read result bytes (**7.3 %**) — above the 3 % gate, so MCP `read` returns a unified diff against the archived previous content. |
| Content-hash repeats (T65.1, 2026-09-18, `rtok stats --since 30d`, 924 sessions) | 6,649 later tool_results whose SHA-256 equalled an earlier result in the same session; 1.83 MB of 95.25 MB result bytes (**1.9 %**) — above the 1 % gate, so `cmd::run` and `read` return a pointer at the earlier archive instead of the body. |
| Extra `read` modes (T50.3, 2026-09-18, this repo's 38–68 K char sources) | 11 Rust files, 534 894 B → tree-sitter comments-stripped 433 997 B (**18.9 %**, ~25 K est. tokens) with function/type bodies kept. imports-only is 0.1–2.1 % of each file and drops those bodies. `app.slint` (38 064 B, no grammar) stays `full`. |
| Edit `old_string` (T58.3, 2026-09-17, `rtok stats --since 90d`, 925 sessions) | 5,990 Edit/MultiEdit calls; `old_string` 1.61 MB, `new_string` 3.60 MB; `old_string` = 3.8 % of tool-input bytes, ≈ 1.3 % of output tokens (bytes/4 against API output) — under the 10 % gate, so the anchored `patch` tool (T58.4) was not built. Caveat: this machine already routes many edits through lean-ctx `ctx_patch`, so the share is a lower bound for a plain-`Edit` workload. |
| Compactions (T58.2, 2026-09-18, `rtok stats --since 30d`) | 923 transcript sessions, 271 `subtype=compact_boundary` events (75 sessions compacted at least once). T2.5 fixture checkpoint body = 144 B before archive-id lines; `plugins.memory.checkpoint_tokens` = 400. Command: `rtok stats --since 30d` (header `sessions N  compact N`); detector is `compact_boundary` only so the paired `isCompactSummary` line is not double-counted. |
| Sub-agents Agent/Task (T59.6, 2026-09-18, `rtok stats --since 30d`, 939 sessions) | `rtok stats --since 30d` (2026-09-18): 42 of 939 sessions used `Agent` (449 calls); `Task` 0. Agent in 1,042,386 B / out 632,586 B (158,299 est. tokens); table line `result/tool_tokens 0.7%  input/tool_input 2.7%` (JSON 0.662 % of 23,905,777 tool-result tokens; 2.699 % of 38,626,178 tool-input bytes). Under the 5 % gate, so the `handoff` MCP tool was not built. |
| Assistant thinking blocks (T125, 2026-09-21, `rtok stats --since 30d`, 827 sessions) | 28,072 `thinking`/`redacted_thinking` blocks (unique `message.id`), 7,266,318 B ≈ 1,816,580 est. tokens = **0.0297 %** of session input (uncached + cache_create + cache_read) — two orders of magnitude under the 3 % gate. The replay share is lower still: the Anthropic API strips thinking blocks of earlier assistant turns from the context (https://docs.claude.com/en/docs/build-with-claude/extended-thinking). I-86 rejected. |
| Session checkpoints (T71.2, 2026-09-18, `rtok stats --since 30d`) | 939 transcript sessions, 0 with a `checkpoint:<id>` or `session:<id>` note, 939 without. Store has 25 legacy `kind=checkpoint` rows (no session suffix, not joinable to a stem). Header `sessions N  compact N  checkpoint N  no_checkpoint N`. Command: `rtok stats --since 30d`. |
| Foreign MCP results (T59.4, 2026-09-17, `rtok stats --since 30d`, 885 sessions) | `rtok stats --since 30d` (2026-09-17, 885 sessions), `mcp` table: lean-ctx 8,232 calls, 19.66 MB result bytes, mean 2.4 KB, p95 45.7 KB (≈ 4.9 M est. tokens) — ≈ 27 % of the 71.8 MB in the tool table; rtok 579 KB, engram 279 KB, t3-code 73 KB (mean 18 KB), Claude_Browser 45 KB. lean-ctx is above the 5 % gate, so the wrapper is justified for this workload; caveat: lean-ctx already compresses its own results, so the win is in the p95 tail, not the mean. |
| MCP description tokens × turns (T59.5, 2026-09-18, host without Tool Search) | `rtok doctor` (2026-09-18): 11 MCP servers, 8,951 description tokens (caveman 485, code-review-graph 2,295, engram 1,865, mobile 1,555, serena 1,494, lean-ctx 697, headroom 161, rtok 143+143, jscpd 113, codebase-memory-mcp 0); `mcp_tool_search likely disabled` because `ANTHROPIC_BASE_URL` is set. Transcripts `~/.claude/projects/**/*.jsonl` mtime ≥ 30 d, unique `message.id` (same rule as `measure::jsonl`): 936 sessions, 40,402 API turns, session input 5.834 B (uncached 0.670 M + cache_create 147.1 M + cache_read 5.686 B). 8,951 × 40,402 / 5.834 B = **6.2 %** of session input. Above the 3 % gate → `proxy.tools_rewrite` ships, off by default. Native-tool descriptions are extra, so 6.2 % is a lower bound. |
| Cache | read 1,367 M, creation 26.7 M, uncached input 42 K → 98.1 % hit rate |
| Median final context | 167 K tokens per session |
| rtk-wrapped commands visible in transcripts | 3 of 3,658 (the PreToolUse rewrite happens after the transcript records the call, so this under-counts) |

### `guard` read-only stems (T57.1, 2026-09-18)

Dated command: `measure::stats::collect` walk (`jsonl_paths` over `[stats] transcripts_dir`, default `~/.claude/projects`, 968 `*.jsonl`). Repeat = the same Bash `command` string (whitespace-collapsed) inside `plugins.guard.window_turns` (default 8) transcript turns. First-word family is `bash_family` (the `collect` path). Flag/subcommand counts are a token scan of those commands, not a shell grammar.

| Stem / marker | All Bash | Exact repeats in window | Action |
|---------------|----------|-------------------------|--------|
| `sed` | 2,417 | 5 | **add** (`sed -n` 2,238; `sed -i` 169 stay mutating) |
| `jq` | 20 | 0 | **add** (repeats are path/filter variants; the stem is present) |
| `awk` | 359 | 0 | **add** |
| `git rev-parse` | 8 | 0 | **add** (`git` all 1,370 / repeats 53, mostly `status` 285) |
| `cargo metadata` | 9 | 0 | **add** (`cargo` all 216 / repeats 2, none metadata) |
| `ls` `cat` `head` `tail` `grep` `rg` `find` `wc` | 1,072 / 1,793 / 147 / 308 / 3,646 / 28 / 343 / 206 | 3 / 9 / 0 / 3 / 8 / 0 / 0 / 5 | keep |
| `find -delete` / `-exec` | 0 / 12 | — | writer marker |
| `tail -f` | 8 | — | writer marker |
| `\| tee` | 44 | — | writer marker |
| redirect `>` / `>>` (token) | 13,802 | — | writer marker (greedy: fail-open vs false deny) |

No existing stem was removed: each still has a count. `tree` is 0 on this machine and stays in the list (already shipped).

Hook e2e fixture (one session, `window_turns = 64` so the fixture is not capped; PostToolUse then PreToolUse of each command, plus `ls` → `find . -delete` → `ls`). `Measurement` rows `plugin = guard`, `kind = guard`:

| | count | `before_bytes` sum |
|---|---|---|
| before (stem list, flags ignored) | 9 | 52 |
| after (flag-aware) | 8 | 32 |

Before denied writers (`find -delete`, `cat a > b`, `grep x > out`, `ls \| xargs rm`, `tail -f`) and false-denied the follow-up `ls`. After those take the mutating path (0 denies) and the new read-only repeats (`sed -n`, `jq`, `awk`, `git rev-parse`, `cargo metadata`) deny. `cat a \| grep b` and `cat f \| wc -l` deny in both. `sed -i` is never keyed.

### Extra `read` modes (T50.3, 2026-09-18)

Dated command: tree-sitter comment-node strip (newlines kept) and import-kind bytes on this
repo's files in the 38–68 K char Read-tail class (`research.md` §2), worktree `t50.3` based
on `t65.1`, 2026-09-18. Estimator 4 chars/token.

| File | full B | stripped B | save | bodies kept | imports remaining |
|------|--------|------------|------|-------------|-------------------|
| tests/proxy.rs | 59 352 | 54 433 | 8.3 % | yes | 0.4 % |
| src/agents/mod.rs | 55 932 | 44 869 | 19.8 % | yes | 0.6 % |
| src/config/mod.rs | 52 035 | 39 522 | 24.0 % | yes | 0.5 % |
| src/plugins/graph/mod.rs | 51 030 | 41 104 | 19.5 % | yes | 0.7 % |
| src/doctor.rs | 48 890 | 41 590 | 14.9 % | yes | 0.8 % |
| src/measure/stats.rs | 48 369 | 41 227 | 14.8 % | yes | 0.8 % |
| src/web/model.rs | 46 958 | 33 749 | 28.1 % | yes | 0.9 % |
| src/cli.rs | 44 534 | 34 402 | 22.8 % | yes | 1.1 % |
| src/tui/view.rs | 43 277 | 34 122 | 21.2 % | yes | 1.2 % |
| src/proxy/mod.rs | 43 052 | 33 219 | 22.8 % | yes | 2.1 % |
| src/plugins/cmd/rules.rs | 41 465 | 35 760 | 13.8 % | yes | 0.1 % |
| crates/rtok-webui/ui/app.slint | 38 064 | 38 064 | 0 % (no grammar → `full`) | n/a | 0 % |
| **11 Rust files** | **534 894** | **433 997** | **18.9 %** | **yes** | 0.1–2.1 % |

Verdict: `mode=stripped` wins; imports-only skipped (loses the bodies a Read of these files is for).

### Whole-file native Reads an outline could answer (T136, 2026-09-24)

`rtok stats --since 30d` on this machine (367 sessions, 423 630 transcript lines), `read_whole` row: native `Read` with no `offset`/`limit`, of a file `read::outline::supported` has a grammar for, result ≥ `[plugins.read] native_max_bytes` (32 768 B).

| Metric | Value |
| --- | --- |
| Calls | 6 |
| Bytes | 216 364 |
| Share of Read bytes (14 150 009) | 1.5 % |
| Share of all tool-result bytes | 0.3 % |
| Followed by an Edit of the path within `[plugins.guard] window_turns` (8) | 0 |

Under the 5 % gate: I-82 (deny such Reads, point at `outline`) is rejected with this number. The `read` plugin's advice deny (T4.6) already turns most such Reads away before they produce a result; these six got through.

### Image blocks in the live zone (T137, 2026-09-24)

`rtok stats --since 30d` on this machine (367 sessions, 424 122 transcript lines), `images` row: every `image` content block in a tool result or a user message. Bytes are decoded base64; pixel size comes from the PNG `IHDR` / JPEG `SOF` header; tokens are `⌈w/28⌉ × ⌈h/28⌉` at the high-resolution tier (long edge 2576 px, 4784-token cap), per the Anthropic vision docs ("Resolution and token cost", read 2026-09-24). `resident` multiplies each block's tokens by the API requests at or after its turn — an upper bound, since compaction drops images earlier.

| Source | Blocks | Bytes | Est. tokens |
| --- | --- | --- | --- |
| `Read` of an image file | 182 | 21 258 233 | 159 978 |
| Browser pane (`browser_batch`, `computer`) | 23 | 674 729 | 11 610 |
| iOS Simulator `control` | 3 | 606 872 | 7 128 |
| Other MCP screenshots (2 tools) | 2 | 234 584 | 2 036 |
| Pasted into a prompt | 1 | 243 375 | 370 |
| **Total** | **211** | **23 017 793** | **181 122** |

Resident: 56 020 282 tokens, **0.91 %** of session input; every block had a PNG or JPEG header. Under the 5 % gate: the multimodal token gate (§16.3 #9) stays unbuilt, with this number. Most image tokens come from the agent's own `Read` of image files, not from browser or simulator screenshots.

### Repeat native Reads by class (T179, 2026-09-24)

`rtok stats --since 30d --json` on this machine (367 sessions, 426 092 transcript lines), the new `repeat_reads` row: every native `Read` of a path already read this session (or, for `subagent`, by its parent/an earlier sibling — T127), not the first read of it, classified by why `read/dedup`+`read/delta` (the MCP `read` cache) did not shorten it. Priority order: a result already carrying a dedup/delta/archive marker → `fired`; else an Edit/Write/MultiEdit/Bash that *changed* the path since the last read → `changed`; else `offset`/`limit` differing from the last read → `ranged`; else, if nothing in the session carries any rtok marker → `hook_absent` (T174: `rtok` likely not on `PATH`); else → `declined`. `subagent` is a sub-agent's re-read of a path its parent or an earlier sibling already read — invisible to the per-session walk, so it is filled from `Subagents::reread_calls`/`reread_bytes` (T128) instead.

`changed`'s Bash signal was revised after the first pass: naming the path in a Bash command was not enough (`cat`/`grep`/`sed -n`/`head` of the path are reads and are common), so `bash_touches` now requires a write signal — output redirection, `sed -i`/`perl -i`/`-pi`, `tee`, `mv`/`cp`/`rm`/`touch`/`patch` naming the path, or a "blanket writer" (`git checkout`/`restore`/`stash`/`reset`/`rebase`/`merge`/`pull`/`apply`/`cherry-pick`, `cargo fmt`/`rustfmt`/`oxfmt`/`prettier --write`) that can rewrite any tracked file without naming it. The numbers below are the re-measurement under that narrower rule.

| Class | Calls | Share of calls | Bytes | Share of bytes |
| --- | --- | --- | --- | --- |
| `subagent` (d) | 1 017 | 41.0 % | 5 909 394 | 58.2 % |
| `changed` (b) | 732 | 29.5 % | 2 141 409 | 21.1 % |
| `ranged` (c) | 524 | 21.1 % | 1 976 031 | 19.5 % |
| `hook_absent` (a) | 111 | 4.5 % | 53 192 | 0.5 % |
| `fired` (e) | 55 | 2.2 % | 6 900 | 0.1 % |
| `declined` (f) | 44 | 1.8 % | 58 386 | 0.6 % |
| **Total** | **2 483** | **100 %** | **10 145 312** | **100 %** |

Same total calls and bytes as the first pass — only reclassification moved: 118 reads that the old, name-only `bash_touches` had called `changed` are no longer flagged as touched by a plain read-only Bash, and fall through to `ranged` (+119) or, in a few cases, `hook_absent`/`declined`; `fired` and `subagent`, untouched by the Bash signal, are unchanged. The 2026-09-22 ad hoc audit's "367 same-session re-reads / ~272 dedup+delta fires" was a narrower count (no sub-agent transcripts, and evidently a different window); this run's per-session-file walk alone (everything but `subagent`) still finds 1 466 repeats, of which only 55 (`fired`) show a dedup/delta/archive marker — consistent with that audit's headline gap even though the totals do not match exactly.

Largest class: `subagent` (d), 1 017 calls / 5.9 MB — a sub-agent re-reading a path its parent or an earlier sibling already read. Per plan, not fixed here: it is class (d), T127's territory (attribution/sharing across agent transcripts, not a `read` plugin bug), and any fix (e.g. seeding a sub-agent's dedup cache from its parent's reads) is a design decision — cross-process cache sharing, and whether a sub-agent *should* trust its parent's read of a path it cannot verify is still current — not a small, clear patch inside `src/plugins/read/**`.

Largest non-(d) class both before and after the narrower `bash_touches`: `changed` (b), now 732 calls / 2.1 MB (was 850 / 2.6 MB) — still expected behaviour by construction (an Edit/Write/MultiEdit/Bash write touched the path since the last read), not a miss to fix. `ranged` (c) likewise: a different `offset`/`limit` is a different read. `hook_absent` (a) is T174's territory (`rtok` not on `PATH`), already tracked there. The only class that is a same-session, same-path, same-range repeat the cache should have caught and did not — `declined` (f) — is also the smallest both times (44 calls now, 0.6 % of bytes): too small to justify a code change, and `--json`-scale transcript text alone cannot show *why* the MCP `read` cache missed these 44 (a native `Read` never even consults it; these are native Reads that stayed native through the read-advice gate — under `native_max_bytes`, or within the recently-edited early return of `plugins/read/hook.rs::pre_tool` that returns `None` with no marker). The class ordering did not change under the narrower Bash signal, so the fix decision stands: no class here is both largest and a small, clear `read`-plugin fix, so none was made.

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

### `graph` import-edge index time (T68.6, 2026-09-18)

Same tree (this repo checkout, 172 tagged files). Release `rtok graph index` into a fresh
`RTOK_HOME`, twice each; the second run is the comparable pair (CPU warm, still a cold store).

Command: `RTOK_HOME=$(mktemp -d) <bin> graph index <repo>`

| | Binary | Files | Rows | Cold s (1st / 2nd) |
|---|---|---|---|---|
| Before | t68.5 release | 172 | 33 312 | 0.437 / 0.270 |
| After | t68.6 release | 172 | 35 017 | 0.922 / 0.325 |

T8.8 `tests/graph_truth.rs` `labelled_symbols_are_found` (2026-09-18, after): definition recall
1.000, precision 1.000; reference recall 0.305 (floor 0.30). Imports are `kind = import` and
are excluded from `symbol_refs`.

### `graph` extra grammars and index payload size (T52.2, 2026-09-18)

This-repo debug `rtok graph index` into a fresh `RTOK_HOME` (172 tagged files, 35 231 symbol rows). The new grammars add no rows here — the tree has no Java/Kotlin/Swift/C#/Ruby/PHP sources.

Command (2026-09-18): `RTOK_HOME=/tmp/t52.2-index-home rtok graph index <worktree>` then `stat`, `sqlite3` `dbstat` / `LENGTH(...)`, `gzip -n`.

| | Bytes |
|---|---|
| `rtok.db` | 14 077 952 |
| VACUUM copy | 13 402 112 |
| `symbols` table pages | 7 503 872 |
| `symbols` indexes | 6 356 992 |
| Concatenated TEXT columns | 6 067 201 |
| gzip -n of those columns (one stream) | 197 682 |
| gzip -n of the whole db | 1 150 703 |

Most TEXT bytes are `file_sha` (2 254 784) and `root` (1 972 936) repeated on every row. One-stream gzip looks like a 30× win because it shares a dictionary across 35 231 rows; per-row gzip would grow those short fields (64-byte sha + gzip header). Live payload compression is skipped.

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

T68.9 (2026-09-18) adds `rtok bench --suite graph`: 12 architecture questions, three repos
(this tree, `bench/repos/mini-rs`, `bench/repos/mini-py`), each run twice (rtok MCP on vs
native Read/Grep only) through the T9.1 `claude -p` harness. Dry-run (no spend):
`rtok bench --suite graph --runs 1 --dry-run`. The live API run is not done (needs the
creator's go); until a dated live result lands here, codegraph's −88 % tool calls / −62 %
tokens stay a vendor claim in the §4 matrix, not an rtok number.

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
| `nudges.md` | 114 | 476 | ≤ 250 |

Aliases `cave`→`terse`, `pony`→`yagni` resolve to the same builtins at SessionStart.
`nudges` has no alias; it is opt-in via `[plugins.inject] modes` (default `[]`).

#### Coaching nudges A/B (T53.1, 2026-09-18)

I-18: short nudges (do not re-read, use `expand`, outline-first, search before Grep) may
cut waste, but they are re-read every turn. Data lives in `modes/nudges.md` (D7). Isolated
store, `[plugins.memory] recall_titles = 0`. Command:

```bash
printf '%s' '{"hook_event_name":"SessionStart","session_id":"t531","source":"startup"}' \
  | rtok --config <tmp>/rtok.toml hook SessionStart
```

| arm | `[plugins.inject] modes` | SessionStart `additionalContext` bytes | est. tokens (prose 4.2) |
|-----|--------------------------|---------------------------------------:|------------------------:|
| off (default) | `[]` | 0 | 0 |
| on | `["nudges"]` | 478 | 114 |

On-arm bytes are identical across two consecutive runs. The same config's
UserPromptSubmit `additionalContext` does not contain `# nudges`.

`rtok bench` both arms without `RTOK_BENCH_LIVE` (live `claude -p` **not** run):

| config | mean input | mean cache | mean output | mean cost USD | pass | live |
|--------|------------|------------|-------------|---------------|------|------|
| off | 0 | 0 | 0 | 0.0000 | 6/6 | false |
| on | 0 | 0 | 0 | 0.0000 | 6/6 | false |

Pass parity holds; cost is zeros.

**Live A/B (2026-09-18).** Creator approved API spend. Intended harness: `RTOK_BENCH_LIVE=1`
`rtok bench --runs 1` on the existing six-task suite, two arms (default `modes = []` vs
`modes = ["nudges"]`), same `claude -p` path and host default model, then
`rtok stats --price` per isolated store. Stopped before that pass:

```bash
claude --version
# 2.1.236 (Claude Code)
claude auth status
# {"loggedIn": false, "authMethod": "none", "apiProvider": "firstParty"}
claude -p "Reply with the single word ok and nothing else." --output-format json --max-turns 1
# is_error true; result: Failed to authenticate: OAuth session expired and could not be refreshed
# usage all zeros; total_cost_usd 0
```

`ANTHROPIC_API_KEY` was unset. `claude` CLI was present. A second probe with the
environment's gateway key against the already-set `ANTHROPIC_BASE_URL` returned HTTP 401
`Invalid API key`. No live token, cache, or USD rows. Cost per passed task cannot be
compared; the gate is **do not enable**. `nudges` stays **off** by default
(`config/default.toml` `[plugins.inject] modes = []`).

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
| Command output | lossless (`expand`), measured per family, one process ≤ 10 ms; a default rule (40 lines, head/tail, dedupe) caps every stem, so nothing passes through whole | 24 TOML rules + 13 formatters keep signal by meaning; remaining families are cut by position, so an error line can fall in the gap; rtk has 100+ per-command filters | Per-family rules chosen by measured after-bytes — **T50.1**; table formatters (`docker ps` 3147→1190, `kubectl get` 4542→1731, `ps aux` 2341→870, each beating `Rule::default()` on the same fixture) — **T58.5**. |
| Reads | 4 modes, sha256 dedup, root guard, ~223 desc tokens for 12 tools (`rtok doctor`, 2026-09-18); no banner | lean-ctx: `diff` mode; token-optimizer: delta reads; lean-ctx re-read 13 tokens (rtok's "unchanged since" line is comparable) | **Delta since last read**: rtok already keeps the sha256 and archive id of the previous read, so a changed file can return a unified diff against that archive instead of 9.5–17 K tokens again — **T58.1**. |
| Model output (the code it writes) | typed `yagni` ladder 14/14 on fixtures; modes inside the 800-token budget | nothing targets the 96 % tool-input share | **Measured 2026-09-17 (T58.3):** `old_string` is 3.8 % of tool-input bytes and ≈ 1.3 % of output tokens, so an anchored `patch` tool (serena `replace_symbol_body`, lean-ctx `ctx_patch`) would move at most ~1 % of the output slice; not built (I-43 keeps the number). The output lever that remains is fewer and smaller writes — modes (T53.1) and the read side. |
| Injection / compaction | byte-stable 800-token budget; progressive-disclosure memory; on Claude Code a `PreCompact` checkpoint (prompts, paths, errors) and modes re-injected after the summary (T2.5) — rtk, headroom and caveman have nothing here | the checkpoint exists on Claude Code only (Codex, Cursor, Gemini, Copilot events are not registered); it carries no archive ids, so `expand` of a summarized-away result depends on the model remembering the id | Register the compaction events on every host that has them and add the live archive ids to the checkpoint — **T58.2**. |
| Foreign MCP results | old ones shrink in the proxy live zone like any `tool_result` | fresh results of other servers pass whole (atlassian mcp-compressor wraps any server) | Not worth it on this workload: MCP results were 15 K of 2.83 M (§2). Idea I-44. |
| Tool descriptions | 12 tools / ~223 tokens (`rtok doctor`, 2026-09-18); `doctor` prices every server | — | **Measured 2026-09-18 (T59.5):** on this host Tool Search is off (`ANTHROPIC_BASE_URL` set); MCP description tokens × API turns = **6.2 %** of session input (§2) → `proxy.tools_rewrite` ships, off by default. |
| Memory | agent-written, FTS5, no model calls, titles-first | claude-mem/mem0 have vectors (P29 landed hash-embed; no ONNX); Claude Code auto-memory is free on that host | `doctor` should say when auto-memory makes rtok recall a duplicate injection. Idea I-47. |
| Code graph | 5 tools / 127 tokens (2026-09-18, T68.1 added `explore`), SQLite only, hook ≤ 10 ms | reference recall 0.351 vs LSP-grade (serena, codebase-memory-mcp hybrid LSP) | Already T52.5 / T30.2 (LSP optional). jCodeMunch's measured 96.5 % vs grep-read is the same claim class as `graph`; no new task. |
| Learning from history | `stats`, `report` rules (D24), `doctor --instructions` | caveman `learn`, lean-ctx mode predictor, context-budget plugin rank *sinks* and *recommend* | `report` already renders recommendations; a per-file / per-command sink ranking is idea I-48 until `stats` shows a sink the existing rows do not name. |
| Sub-agents | — | lean-ctx `ctx_handoff`/`ctx_agent`; theme "sub-agent isolation" | Agent+Task results were 0.7 % of tool-result tokens on 30d (T59.6, §2); not a lever. Idea I-46, parked with the number. |
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
| rtk smart filtering (noise, comments, boilerplate) | `keep`/`drop` patterns per rule, `BUILTIN_KEEP`, 13 formatters, raw archived first | Coverage, not mechanism: 24 rules + 13 formatters vs ~80 (rtk) / 45+ (sqz) | T50.1, T58.5 |
| rtk grouping (files by directory, errors by type) | none — `ls`/`find`/`tree` take 40 lines | generic grouping pass | T64.1 |
| rtk truncation | `max_lines`/`head`/`tail`, lossless (`expand <id>`) | rtok is ahead: rtk drops, rtok archives | — |
| rtk / sqz dedup of repeated log lines | `dedupe` folds adjacent identical lines to `(×N)` | non-adjacent, timestamp-normalised | T64.2 |
| rtk "does not break the prompt cache" paragraph | byte-stable inject, live-zone proxy rewrites, `report` cache section, 98.1 % hit rate on this machine | no page says it | T64.3 |
| sqz content-hash dedup (`§ref:HASH§`, 13 tokens) | `guard` dedups by input key only | same bytes from a different call paid twice | T65.1 (1.9 % of result bytes, 2026-09-18, `rtok stats --since 30d`, 924 sessions — above the 1 % gate) |
| sqz structural summaries (imports + signatures, ~70 %) | `read` modes via tree-sitter (`map`, `signatures`) | none | — |
| sqz JSON pipeline (nulls, arrays) | JSON compact then line cut (0.21 % of Bash bytes, 30 d, this machine) | — | T65.2 |
| sqz table compaction | none | padding collapse | T65.3 |
| sqz safe mode (traces, secrets pass whole) | single `panic`/`traceback` lines kept, frames cut; secrets never redacted | keep the block | T65.4 |
| sqz hosts: Windsurf, Cline, Gemini CLI, Kiro, Zed, Copilot CLI; browser and IDE extensions | 12 hosts in `src/agents/` (no Cline, Kiro, Gemini); no extensions | hosts on request; extensions out of scope (one binary, D21) | — |
| sqz `gain` / `stats --breakdown` | `stats`, `report`, `dashboard`, one ledger | none | — |

Order by expected effect on this workload (§2: Bash 35 % of result tokens): T65.4 and T64.3
are cheap and close a correctness / documentation hole; T65.1 measured **1.9 %** of result
bytes as same-session SHA-256 repeats (`rtok stats --since 30d`, 924 sessions, 2026-09-18)
and proceeds; T65.2 still starts with a measured share; T64.1, T64.2, T65.3 are fixture-gated.

T65.2 gate (2026-09-18, this machine, 30 d): `measurements` where `plugin = 'cmd'` and
`before_bytes > 0`, body = archive file named by `ref_id` after the first `:`,
`json.loads` of the UTF-8 body. 4 / 200 rows, 5 776 / 2 792 960 B = **0.21 %**.
(`~/.rtok/rtok.db`, read-only; the same window’s whole `archive` table is 18.66 % JSON —
MCP/read blobs, not Bash.) The rewrite still ships: the Done line is the compact pass, not
a share floor.

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
| Child model on a slice (`rlm_query`) | the host's Agent tool plus `expand <id>`; `handoff` (T59.6) closed at 0.7 % of tool-result tokens | nothing on rtok's side — rtok is not the agent loop; re-open T59.6 only if a later `rtok stats` Agent/Task share is ≥ 5 % | — |
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
| `mem_context` at session start: pinned + recent observations + sessions + prompts, 16 KiB default budget | SessionStart recall: 5 titles + ids ≤ 200 tokens; compaction checkpoint ≤ 400 tokens, same session only; SessionEnd writes `session:<id>` (T71.2); `startup_recall` restores the newest project note on `source=startup`, off by default | keep rtok's shape (D5 budget, titles not bodies); handoff stays off until a P7-style A/B. Measured 2026-09-18: `rtok stats --since 30d` → 0/939 sessions with a joinable checkpoint note (25 legacy unscoped `checkpoint` rows) | T71.2 |
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
| Claude Code hooks: SessionStart facts + conventions; UserPromptSubmit top-3 + `remember:`; PreCompact checkpoint; SessionEnd checkpoint + consolidation; errors to `hooks.log`, never break the session | SessionStart titles (T6.2); PreCompact checkpoint (T2.5); SessionEnd `session:<id>` note (T71.2, restore off by default); fail open ≤ 10 ms; nothing on UserPromptSubmit | `remember:`; per-turn recall (A/B) | T69.5; I-56 (engram `mem_context`) |
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

### 14.1 Live notes vs `recall_titles` (T69.2, 2026-09-18)

Installed `rtok 0.1.1` (`dbcc7a162`) has no `memory status`. Counted with T69.4's
`Store::memory_note_aggs` query (`kind NOT LIKE 'checkpoint%'`) on this machine:

`sqlite3 ~/.rtok/rtok.db "SELECT COALESCE(project, '-'), SUM(CASE WHEN retired IS NULL THEN 1 ELSE 0 END) FROM notes WHERE kind NOT LIKE 'checkpoint%' GROUP BY project;"`

Result: **0 rows**. Live notes: **0**. Projects with more than `[plugins.memory] recall_titles` (5): **0**.
The same file holds 25 `checkpoint` rows under project `rtok` (title `compact`, none retired);
T69.4 excludes them from the live count. SessionStart still injects those titles
(`list_note_titles` does not filter kind). Ranking order among live facts never matters here,
so T69.2 ships no scorer and no `uses` / `last_used` columns.

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
the three. The proxy stays the only path on Codex, Claude Desktop, Windsurf, Zed, ZCode
and Copilot, which have neither a plugin directory nor the events. Kimi has a plugin
store (`plugins/managed/`, T86) whose hooks and MCP server rtok's installer treats as
the singleton instead of its own tables.

### T50.1 default-rule families (2026-09-18)

Command: `rtok stats` on this machine (916 Claude Code sessions, `since` default). The `bash` table now has a `filter` column (`formatter` / `rule` / `default`). The `bash_default` table ranks stems where `cmd` `Measurement.kind = rule` still used `Rule::default()` (before the T50.1 rules landed), sorted by summed `after_bytes`.

Top 20 `bash_default` stems by filtered after-bytes:

| # | stem | `cmd` rule rows | after B |
| --- | --- | ---: | ---: |
| 1 | mise | 37 | 41,423 |
| 2 | gh | 4 | 13,809 |
| 3 | bash | 13 | 12,484 |
| 4 | cd | 4 | 6,334 |
| 5 | H=$(ls | 5 | 5,777 |
| 6 | awk | 4 | 4,954 |
| 7 | just | 2 | 4,018 |
| 8 | df | 3 | 3,451 |
| 9 | # | 3 | 3,094 |
| 10 | for | 3 | 2,881 |
| 11 | bv0thal3q.output; | 1 | 2,629 |
| 12 | head | 4 | 2,643 |
| 13 | lean-ctx | 2 | 1,552 |
| 14 | mkdir | 1 | 2,435 |
| 15 | sqlite3 | 20 | 8,556 |
| 16 | printf | 1 | 493 |
| 17 | if | 1 | 602 |
| 18 | diff | 1 | 150 |
| 19 | build.rs | 1 | 136 |
| 20 | rmcp-3.2.0 | 1 | 101 |

T50.1 added `[stem]` rules (and golden fixtures) for: `gh`, `pip`, `uv`, `python`, `python3`, `go`, `aws`, `mvn`, `gradle`, `dotnet`, `tsc`, `eslint`, `brew`, `apt`, `cmake`. T58.5 shipped table formatters (one row per object, `kind = formatter`) that beat `Rule::default()` on those fixtures: `docker ps` 3147→1190 vs rule 1311, `kubectl get` 4542→1731 vs rule 1770, `ps aux` 2341→870 vs rule 990 (`tests/cmd_golden/{docker_ps,kubectl_get,ps_aux}`).

### 15.4 MiMo Code host probe (T186, fetched 2026-09-24)

Confirmed against https://github.com/XiaomiMiMo/MiMo-Code, https://mimo.xiaomi.com/mimocode/start,
`/config-files`, `/config-overrides`, `/mcp-servers`, `/env-vars`, `/tools`: binary `mimo`
(install via `curl -fsSL https://mimo.xiaomi.com/install | bash`, `powershell -ep Bypass -c
"irm https://mimo.xiaomi.com/install.ps1 | iex"`, or `npm i -g @mimo-ai/cli`); global config
`~/.config/mimocode/mimocode.json` (`.jsonc` also accepted), moved by `MIMOCODE_HOME` /
`MIMOCODE_CONFIG`; project config `.mimocode/mimocode.json`, searched upward, parent-first
merge. MCP under `mcp.<name>`: local servers are `{type: "local", command: [..], enabled,
environment?, timeout?}` — the exact shape OpenCode kept from upstream, so `mimo`'s installer
reuses `opencode`'s `register_mcp`/`unregister_mcp` path (`src/agents/mod.rs::register_local_mcp`,
factored out in this task to keep `just dup` under budget); remote servers add
`{type: "remote", url, headers?, oauth?}`, unused here. No documented base-URL/proxy override
env var — `MIMOCODE_MODELS_URL` only relocates the model manifest fetch, not the API endpoint —
so `proxy` is `Support::No`. The only hook-shaped mention found is `tool.execute.before`/
`tool.execute.after` on the Custom Tools page, OpenCode's in-process plugin surface; no
`/mimocode/plugins` page exists (404) and no `@mimo-ai/plugin` package is documented, so v1
ships no plugin and `hooks`/`plugin` are both `Support::No` (creator instruction: do not guess
the plugin package). MiMo Desktop is confirmed to exist (early access, "powered by MiMo Code
as its core engine") but no separate config path is documented, so no Desktop variant ships.

## 16. Token savings beyond the shipped surface (2026-09-21)

Creator request: what else can save LLM tokens in an agent product like rtok (AirTalk), after inventorying what already ships. Sources: `plan.md` plugin catalogue, `ideas.md`, §§2/4/6/9–15 of this file, and the in-tree plugins under `src/plugins/`. Vendor % claims stay claims unless marked *measured*.

### 16.1 What rtok already does (short)

| Lever | Where | Mechanism |
| --- | --- | --- |
| Tool-output trim (lossless) | `cmd`, `read`, MCP wrap | Family formatters + TOML rules; archive raw → pointer + `expand` |
| Re-read / refetch dedup | `read`, `guard` | sha256 / identical read-or-command within N turns → deny or skip full body |
| Live-zone rewrite | `archive` + `proxy` | Old large `tool_result`s → head/tail + pointer; system/tools/last-2-turns untouched |
| Budgeted injection | `inject` | SessionStart / UserPromptSubmit under a token cap; modes as markdown data |
| Memory without stuffing | `memory` | FTS5 notes; SessionStart injects `id title` only; bodies via `mem_get` |
| Code navigation instead of dumps | `graph` | `symbol` / `callers` / `outline` / `impact` from tree-sitter-tags in SQLite |
| Measurement | `measure`, proxy usage | Context-token-turns + provider usage; `stats` / `report` / doctor |
| Optional tabular encode | `toon` | JSON tables → TOON (off by default) |
| Optional extractive shrink | `compress` | P28 gate; off until semantic compress clears a bench |
| Host install surface | `agents install` | Hooks + MCP + proxy so the above actually see traffic |

Routing (D9), WASM plugins (P32), embeddings (P29), semantic cache (P31), and tiered context (P33) are **decisions or Later**, not the default v0.1 path.

### 16.2 Already tracked but not the default product yet

Status as of 2026-09-21.

These are **not** greenfield — they live in `ideas.md` / `plan.md`. Listed so this scan does not reinvent them. Priority here is “still open for savings,” not “new invention.”

| Priority | Idea / task | Rough impact | Effort | Status | Why |
| --- | --- | --- | --- | --- | --- |
| P0 | **T59.5** tools[] description rewrite (I-45) | *measured* ~6.2 % of session **input** when Tool Search is off | M | shipped (off by default) | High repeat tax every turn; off-by-default rewrite is the right shape |
| P0 | **T61.2** live-zone skill bodies (I-51) | High when a large skill stays in every later request (§10.3) | M | shipped (off by default) | Same archive path as tool results; gated on skill stats (T61.1) |
| P1 | **T58.1** delta re-read (I-41) | Medium on Read-heavy sessions (Read ≈ 15 % of tool-result tokens §2) | M | shipped | MCP `read` returns unified diff when file changed since last read (§2: 7.3 % of reads) |
| P1 | **T58.2** compaction checkpoint + archive ids (I-42) | Medium on long sessions that compact | M | open | Survives host summarization; half is host-plugin work (T70.x) |
| P1 | **T59.1** per-stem `skip_wrap` (I-39) | Medium for curl/ffmpeg-class Bash if currently unwrapped | S–M | open | Fail-open; needs hang Check |
| P2 | **P28 / I-21** LLMLingua-style / extractive `compress` on | High *if* bench beats lossless; quality risk on code | L | open | Default off; costs tokens to save tokens |
| P2 | **P31 / I-23** semantic response cache | High on repeated asks; dangerous false hits | L | open | Needs false-hit Check |
| P2 | **P33 / I-25** OpenViking-style tiered context | High on very long threads | L | open | License + model path |
| P2 | **T51.1** (I-09) compress nested JSON / `data:` inside live zone | Medium when blobs dominate | M | open | Complementary to tool_result archive |
| P3 | **I-55** session token/cost budget deny | Process control, not compression | S | open | Hosts already auto-compact |
| P3 | **I-71** HTML→text curl formatter | *measured* &lt; 1 % Bash bytes here — parked | S | open | Re-open only above gate |

### 16.3 Further options not yet a first-class rtok idea (or only as a Decision)

Prioritized for an agent product like AirTalk. Effort: S &lt; 1 week, M ~1–3 weeks, L multi-phase. Impact is expected **input** token or CTT reduction unless noted.

| # | Option | Impact | Effort | Notes / sources |
| --- | --- | --- | --- | --- |
| 1 | **Explicit prompt-cache breakpoints + sticky routing** | High $ (cache-read vs input); modest unique-token cut | M | Providers bill cache hits cheaply (rtok already prices cache in T49.1). Pin stable prefix (system + tools + modes) and keep the same backend pod/region so the KV/prompt cache hits. Anthropic prompt caching docs; OpenAI prompt caching. Not the same as I-23 semantic cache. |
| 2 | **Deferred / dynamic tool declarations** | High when many MCP tools | M | Ship short tool stubs; load full schemas on first use (host Tool Search / deferred tools — doctor already warns when `ANTHROPIC_BASE_URL` disables search). Related to I-45 but schema-level, not only shorter text. |
| 3 | **Model / tier routing by job** (D9) | High $; small raw-token change | M–L | Cheap model for format/classify/expand-prep; mid for edit; expensive only after confirm. Needs a router policy + measurement so “savings” are $. |
| 4 | **Thinking / reasoning strip on replay** | Medium–High on reasoning models | S–M | Do not re-send prior chain-of-thought blocks into the next turn when the host attaches them; keep final answers + tool I/O. Host- and provider-specific. |
| 5 | **Native context-editing APIs** (I-10 / T51.2) | Medium–High | M | Let the platform shrink history (Anthropic context editing / host compaction hooks) *and* keep rtok archive ids in the checkpoint (ties to T58.2). |
| 6 | **Structured tool I/O (JSON Schema / strict)** | Medium output + easier trim | M | Force tools to return compact tables/fields instead of prose; then `toon` / formatters win more often. |
| 7 | **Sub-agent isolation + budgeted handoff** (I-46) | Medium when Task/Agent traffic grows | M | Child context starts small; parent gets a digest with archive ids — not a full transcript paste. |
| 8 | **Identifier / path dictionary in-session** | Low–Medium | L | Replace repeated long paths with short codes in tool results; expand on demand. Easy to break models; needs A/B. |
| 9 | **Multimodal token gate** | High $ when screenshots dominate | S–M | Prefer OCR/text or downscale; refuse or summarize images in the live zone. Separate from text CTT. T137 (2026-09-24): images are 0.91 % of session input (§2) — under the 5 % gate, not built. |
| 10 | **Speculative local draft → verify** | Mixed | L | Local small model proposes; cloud model verifies — can cut cloud **output** tokens, adds complexity and wrong-draft risk. |

### 16.4 Sources (non-obvious)

- Workload + technique ranking: this file §§2, 6, 9–15; comparison matrix §4.
- Parking lot (do not re-file duplicates): `ideas.md` Open / Later / Promoted.
- Plugin contracts: `plan.md` catalogue; each `src/plugins/*/PLAN.md`.
- Prompt caching (provider): Anthropic “Prompt caching”; OpenAI “Prompt caching” (billing ≠ semantic cache).
- Tool-description tax: Portkey / LiteLLM claims cited under I-45; rtok *measured* 6.2 % input (2026-09-18).
- External context + grep/slice: recursive-llm notes in §12 (I-53/I-54).
- Tiered memory / L0–L2: OpenViking row in §4 / I-25.
- Extractive / LLM compress: LLMLingua-2 family under I-21 / P28; in-tree `compress` is the off-by-default hook.
- HTML Readable path: tinyjuice / TokenJuice under I-71 (below rtok’s 1 % gate on the measured corpus).
- Host plugin ceilings: §15 (pi/Cursor/OpenCode events rtok cannot see via proxy alone).

### 16.5 Recommended next moves

1. Ship or schedule **T59.5** and **T61.2** — highest *measured* or structurally recurring input taxes.
2. Add a plan card for **prompt-cache-stable prefixes + sticky proxy upstream** if `$` savings matter as much as raw tokens (pairs with existing `stats --price` cache rates).
3. Keep P28/P31/P33 in Later until a bench beats the lossless archive lane on *code* sessions.

## 17. Sharing context between an agent and its sub-agents (2026-09-21)

Creator request: a freshly spawned sub-agent gets none of the parent's context, reads the same files again and pays for them again. Find what rtok can apply; sub-agents stay cheap (Haiku). Method: two Haiku agents (repo inventory, web scan), one Haiku docs check, an ad-hoc scan of this machine's transcripts; synthesis in the main session. Vendor numbers stay claims.

### 17.1 What T59.6 measured, and what it missed

T59.6 closed `handoff` at 0.7 % because it measured the `Agent` tool's input and result **in the parent transcript**. The cost of a sub-agent is not there: it is in `<session>/subagents/agent-<id>.jsonl` (plus `agent-<id>.meta.json`: `agentType`, `model`, `toolUseId`, `spawnDepth`), which no rtok code attributes to a parent — `src/` has no `agent_id`, `agent_type` or sidechain handling.

`rtok stats --since 30d --json` → `subagents`, run 2026-09-22 on this machine's `~/.claude/projects` (the T128 row replaces the 2026-09-21 ad-hoc scan and supersedes its numbers):

| Quantity | Value |
| --- | --- |
| Sessions with sub-agents / sub-agents | 51 / 531 |
| Tool-result bytes: sub-agents vs their parents | 32,112,993 vs 37,054,167 (46 % of the tree) |
| Sub-agent file-read result bytes | 10,262,213 (32 % of sub-agent tool-result bytes) |
| … of a path the parent also read | 2,554,602 (25 % of sub-agent read bytes) |
| … of a path an earlier sibling read | 1,460,241 (14 %) |
| Re-read total | 4,014,843 B = 39 % of sub-agent read bytes, 13 % of sub-agent tool-result bytes, 5.8 % of the tree's |
| Sub-agent usage (tokens) | input 78,320 · cache read 1,083,085,186 · cache write 40,376,428 · output 655,280 |

Largest `agentType × model` splits (the full list is `--json` `by_type`): `general-purpose | haiku` 351 agents / 1,028,835 B re-read; `general-purpose | sonnet` 63 / 1,599,793 B; `general-purpose | -` 46 / 676,366 B.

Caveats: path-level match (no range or sha), parent reads counted over the whole session (before or after the spawn), bytes are JSON-encoded `Read` result sizes. Parent-first: a path both the parent and an earlier sibling read counts as a parent re-read, so the two re-read columns are disjoint and sum to the total. Sub-agent transcripts are attributed to their parent session and are no longer counted as sessions of their own (they were before T128). Every re-read byte is also re-sent on each later sub-agent turn (the cache-read column), so the byte share understates the token share.

### 17.2 What the host gives us (Claude Code)

| Fact | Source | Status |
| --- | --- | --- |
| Hooks fired inside a sub-agent carry `agent_id` and `agent_type` | https://code.claude.com/docs/en/hooks | documented |
| They carry the **parent's** `session_id` | sub-agent transcript lines: `sessionId` = parent, `agentId`, `isSidechain: true` | observed 2026-09-21; not documented |
| `PreToolUse` may return `updatedInput`; `Agent` `tool_input` has `prompt`, `subagent_type` | https://code.claude.com/docs/en/hooks | documented (`model`, `description` in `tool_input`: unverified) |
| `SubagentStart` exists, matcher = agent type; `additionalContext` reaches the sub-agent | same page / Agent SDK hooks page | stated generically — verify on a live hook before relying on it (T130) |
| `SubagentStop` input: `agent_id`, `agent_type`, `agent_transcript_path`, `last_assistant_message` | same page | documented |
| `fork` sub-agent inherits conversation, model and prompt cache | https://code.claude.com/docs/en/sub-agents , https://code.claude.com/docs/en/prompt-caching | documented — runs on the parent's model, so it is not a cheap-Haiku path |
| Agent frontmatter: `model`, `tools`, `skills`, `memory`, `hooks`, `mcpServers`, `initialPrompt` | https://code.claude.com/docs/en/sub-agents | documented |
| Haiku 4.5: cache read 0.1× input, minimum cacheable prefix 4,096 tokens, TTL 5 min / 1 h | https://platform.claude.com/docs/en/build-with-claude/prompt-caching | documented |
| `PostToolUse` `updatedToolOutput` replaces any tool's output | Agent SDK hooks page | **unverified for CLI command hooks**; contradicts a standing rtok rule → I-91 |

### 17.3 What follows for rtok

1. **A context window is `(session_id, agent_id)`, not `session_id`.** Every "already seen" state in rtok is keyed by session: `guard::pre_tool` (`src/plugins/guard/mod.rs`), the read cache (`src/plugins/read/cache.rs`), `plugin::identical_result`. A sub-agent that reads a file its parent read inside the guard window is denied with `duplicate; rtok expand <id>` for a body it never saw: one extra round trip, the full body anyway, and a `guard` Measurement row that claims a saving. Observed: 22 such denial strings in 11 sub-agent transcripts over 30 days (how many were cross-context is unmeasured). T122 fixes the MCP `read` side with a size threshold because MCP cannot see the caller; the hook side **can** see `agent_id` → T129. **Decision (2026-09-24, T127):** MCP cannot see the caller still holds — `src/mcp.rs` builds every `Ctx::new`, never `with_agent` — so `identical_result`'s MCP `read` call keeps its session-only scoping rather than dropping the pointer outright; known gap: a body one sub-agent read through MCP `read` can still be handed to a different sub-agent (or the main window) sharing that session as a pointer it never saw the content behind. The hook surface closed the equivalent gap the same day: `cmd::run`'s dedup is now keyed on `(session, agent_id)` via the `PreToolUse(Bash)` rewrite's `--agent` flag.
2. **Content cannot be shared for free; pointers can.** A sub-agent needs the bytes in its own window. What rtok can remove is the *search and whole-file* cost: the parent's ledger already knows which paths matter, their archive ids, outlines and line ranges. A budgeted, byte-stable brief appended to the `Agent` prompt at spawn ("these files, these ranges, `read` with `range`, `expand <id>`") replaces the sub-agent's Glob/Grep/whole-file Read turns with ranged reads → T130. This is the Anthropic "pass references, not content" pattern (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) and lean-ctx `ctx_handoff` (§9), built on the existing `handoff` digest — one call path (D21).
3. **It ships only with a number.** The brief costs tokens in every spawn. T128 makes the re-read share a `rtok stats` row; T131 splits it by brief on/off; default-on only if the net is positive.
4. **A cheap scout by construction.** A shipped agent definition (`model: haiku`, tools limited to rtok MCP `read`/`search`/`outline`/`explore`/`expand`, answer = `path:line` citations) makes the cheap path the default one → T132; measured by T128's per-`agentType` split.
5. Not pursued: `fork` (parent model, parent-sized context — the opposite of cheap); shrinking the `Agent` result (0.7 %, T59.6 stands). Parked as ideas: sibling cache-prefix sharing (I-89), sibling findings board (I-90), `updatedToolOutput` (I-91).

## 18. Git worktrees: accumulation, ownership, build artifacts (2026-09-21)

Creator question: worktrees pile up, nobody knows whose they are, names are random, and they fill the disk. Not a token saving — no `Measurement` row and no public number follows from this section.

### 18.1 Measured on this repository (2026-09-21, `git worktree list --porcelain`, `du -sk`, `df -h /`)

- The data volume had 220 MiB free of 926 GiB; a sub-agent's shell failed with `ENOSPC`. A full disk also fakes test failures (integration tests exit 1 with empty output).
- 28 worktrees in 5 locations: 12 under `/private/tmp` (11 in Claude Code session scratchpads, which macOS purges), 7 as siblings `apps/rtok-*`, 5 in `.claude/worktrees/`, 3 in `_worktrees/` and `_ci-no-main-push/`, plus the main checkout.
- A worktree is 19–29 MB of source. Six held a cargo `target/` of 1.4–7.2 GB, ≈ 27 GB together; `crates/rtok-webui/target` added 5.2 GB. **All of the weight is tagged build cache** (`CACHEDIR.TAG`).
- The largest consumer was invisible to git: `.claude/worktrees/graph-perf`, 18.1 GB of `target/`, untouched since 2026-09-12. Its `.git` file pointed at `/Users/…/GitHub/rtok/.git/worktrees/graph-perf` — the repository had moved and the absolute link broke, so neither `git worktree list` nor `prune` sees it. It also held 23 edited source files that exist in no commit. Deleting only `target/` freed 17 GiB and lost nothing.
- Names carry no information: `rtok-wt-t126` holds branch `t138-inquire-prompts`, `rtok-t103` holds `t106-otel-export-tests`, agent worktrees are `agent-<hex>`; the three locked ones had no lock reason.
- `git branch --merged` cannot answer "is it merged": PRs are squash-merged, so merged branches (T138, T81) still show 1–2 commits ahead of `origin/main`.

### 18.2 What git gives and does not give (https://git-scm.com/docs/git-worktree)

Admin data is `$GIT_DIR/worktrees/<id>/` (`gitdir`, `HEAD`, `index`, `locked`); the worktree holds a `.git` *file*. No owner, description, TTL or size exists. Ignored files are never shared or cleaned. Free-text metadata fits in `git worktree lock --reason` (shown by `list --porcelain`; non-ASCII is C-quoted there, so keep it ASCII) or `git config --worktree` (needs `extensions.worktreeConfig`). `rm -rf` leaves the admin entry until `gc.worktreePruneExpire` (3 months) — and forever when the worktree was locked: checked 2026-09-22 with git 2.54, a locked worktree whose directory was deleted is not even reported `prunable`. Meanwhile its branch counts as checked out. `git worktree remove <path>` on the missing directory (after `unlock`) drops that single record → T153. `worktree.useRelativePaths` (git ≥ 2.48) would have kept `graph-perf` linked, but sets `extensions.relativeWorktrees`, which older git and possibly libgit2/gix-based tools refuse → T157.

#### Relative worktree links probe (2026-09-24, T157)

Scratch repositories only. `git config worktree.useRelativePaths true` followed by `git worktree add ../wt` writes `gitdir: ../repo/.git/worktrees/wt` in `wt/.git` and a relative back-link. It also sets `extensions.relativeWorktrees = true` and raises `core.repositoryformatversion` to 1.

| Reader | Version | Opens the worktree | How checked |
| --- | --- | --- | --- |
| git CLI | 2.54.0 | yes | `git status`, `git worktree list` |
| cargo (VCS dirty check) | 1.97.1 | yes | `cargo package --list` inside the worktree reports the one uncommitted file |
| gh | 2.101.0 | yes | `gh repo view --json name` inside the worktree resolves `listepo/rtok` |
| lazygit, delta, editors (VS Code, Zed, Cursor) | — | not tested | interactive; lazygit shells out to the git CLI, delta never opens a repository |

Move test: the parent directory holding `repo/` and `wt/` was moved with `mv a b`. With relative links, `git -C b/wt status` and `git -C b/repo worktree list` work unchanged, and `git worktree repair` is not needed. The absolute-link control breaks on the same move: `fatal: not a git repository: (null)`, and the worktree is listed as `prunable`. Renaming only one side (the repository or the worktree) breaks the relative link too, as expected.

Conclusion: every non-interactive reader on this machine opens a relative-link worktree. The editors are untested. The setting stays opt-in until they are checked: the T157 Check asks for "every reader passes" before the setting is added to the `worktrees` skill and AGENTS.md.

### 18.3 Hosts (vendor docs, fetched 2026-09-21, not re-verified by running each host)

| Host | Location | Naming | Automatic cleanup |
| --- | --- | --- | --- |
| Claude Code (https://code.claude.com/docs/en/worktrees) | `<repo>/.claude/worktrees/<name>` | random words or `agent-<hex>`; branch `worktree-<name>` | only when unchanged; sub-agent worktrees by a `cleanupPeriodDays` sweep |
| Cursor (https://cursor.com/docs/configuration/worktrees) | `~/.cursor/worktrees/` | undocumented | max 25, 6-hourly sweep, oldest evicted |
| Codex (https://learn.chatgpt.com/docs/environments/git-worktrees) | `$CODEX_HOME/worktrees` | `thread-N`, detached HEAD | keeps the 15 most recent |
| Conductor (https://www.conductor.build/docs/concepts/git-worktrees) | `~/conductor/workspaces/<repo>/` | workspace name | archive script only |

No host accounts for build output. Claude Code exposes `WorktreeCreate`/`WorktreeRemove` hooks that replace the default create/remove (https://code.claude.com/docs/en/hooks) → T156. The same four complaints are filed upstream: anthropics/claude-code#46098 (names), #24207 (unbounded growth), #56639 (archive deletes uncommitted work).

### 18.4 Libraries and tools

- `git2` 0.21: add, list, lock, prune — no `move`, `repair`, or dirty-checked `remove`; adds libgit2. `gix` 0.87: worktrees read-only (create/move/remove/repair open in its `crate-status.md`). worktrunk and `git-worktree-runner` shell out to `git`. rtok already shells out to `git` (`git_changed_files`, `src/plugins/graph/mod.rs`) and has no shared git helper; `git_root` exists twice (`src/config/layers.rs`, `src/doctor.rs`). **Decision: `git worktree list --porcelain -z` through one helper, no new dependency.**
- worktrunk (https://github.com/max-sixty/worktrunk): path templates, merge-and-remove, `--copy-ignored` reflink seeding of `target/`. It does not record owners, find orphans, or clean idle caches — the three things measured in §18.1.
- Squash-aware "merged" needs no GitHub call: `git merge-tree --write-tree <base> <branch>` equals `<base>^{tree}` when merging the branch would change nothing.
- A shared `CARGO_TARGET_DIR` is rejected: ~5 parallel agents would serialize on the build lock. `sccache` does not cache incremental builds. Reflink seeding (`reflink-copy`) only lowers the cost at creation; cleaning idle caches removes it → measure before adopting (T156).
- First data point for T156 (2026-09-22, APFS, `cp -c -R <other-worktree>/target <new-worktree>/target`, disk delta from `df -k`, not `du`): an 8.1 GB `target/` cloned in 8.8 s for 17 MiB of physical disk; the first `cargo nextest run --lib --test worktree` in the seeded worktree (T150) rebuilt only the four workspace crates, 24 s, with no dependency recompiled. A cold build was not run for comparison — the disk had under 8 GiB free, which is why the clone was tried at all.

### 18.5 What follows for rtok

1. rtok's hooks already fire in every session on every host and carry `session_id` and `cwd`; that is an ownership record no worktree manager has, at no cost to the agent → T154.
2. The always-safe operation is "delete tagged caches, keep the worktree"; `git worktree remove` cannot express it (it refuses the whole worktree when anything is uncommitted) → T152.
3. Inventory first, then deletion: T150 → T151 → T152/T153. Conventions ship as a skill so they cost one description line, not `AGENTS.md` budget → T155.
4. Every measured problem starts at creation (location, name, reason-less lock), and creation is the only moment the owner is known for certain. rtok creates the worktree itself → T158. A skill is advice; on Claude Code the `WorktreeCreate`/`WorktreeRemove` hooks are the one place the rules cannot be skipped, at the price of a decision on what fail-open and the 10 ms budget mean for a hook that must spawn git → T159.

## 19. Hook wall-clock time as Claude Code sees it (2026-09-23)

T178. Machine: the creator's Mac (Apple silicon, macOS, `/bin/sh` → bash), shared with other agents' cargo builds, so every run states its load average. Release build of `68760c6` (`target/release/rtok`, 26,831,904 bytes). All runs use an isolated `RTOK_HOME` and two payloads recorded in the store (a 968-byte `PreToolUse` Bash call and a 2,692-byte `PostToolUse` Bash call, `cwd` rewritten to the worktree). Not a token saving: no `Measurement` row follows.

### 19.1 What Claude Code records

Claude Code writes every hook run into the session transcript as an `attachment` (`hook_success`, `hook_cancelled`, `hook_non_blocking_error`) with `durationMs` and `command`. Command: `find ~/.claude/projects -name '*.jsonl' -mtime -3 | xargs cat | jq 'select(.type=="attachment") | .attachment | select(.durationMs!=null)'`, grouped by `command`.

| `command` | runs | p10 | p50 | p95 |
| --- | ---: | ---: | ---: | ---: |
| `"${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh" PreToolUse` (plugin) | 5,997 | 18 ms | 23 ms | 71 ms |
| `"${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh" PostToolUse` (plugin) | 7,976 | 18 ms | 20 ms | 52 ms |
| `rtok hook PreToolUse` (settings-file install) | 3,246 | 14 ms | 17 ms | 78 ms |
| `rtok hook PostToolUse` (settings-file install) | 4,566 | 13 ms | 16 ms | 80 ms |
| another vendor's `/bin/sh` hook (reads stdin, prints `{}`, may POST to a local port) | 11,699 | 13 ms | 19 ms | 53 ms |

### 19.2 Reproducing it: node `spawn(cmd, {shell: true})`, payload on stdin, clock stops on `close`

A 30-line node harness spawns each command the way Claude Code does, round-robin so load drift hits every command equally. 300 rounds, load average 12.6 → 21.6:

| command | p50 | p95 | delta |
| --- | ---: | ---: | --- |
| `true` | 5.19 ms | 7.27 ms | node + `/bin/sh` floor |
| `rtok --version` | 10.81 ms | 13.59 ms | +5.6 ms: loading the 27 MB binary, clap |
| `rtok hook PreToolUse` | 13.61 ms | 19.63 ms | +2.8 ms: the hook itself |
| `"${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh" PreToolUse` (shipped) | 20.01 ms | 24.88 ms | +6.4 ms: the launcher |

The harness lands on Claude Code's own p50 (20–23 ms), so the gap between the 0.3 ms in-process `calls.ms` and what Claude Code waits for is: shell floor 5 ms, binary start 5.6 ms, launcher 6.4 ms, hook work 2.8 ms. `hyperfine -N -w 10 -r 200` (no shell): `/bin/echo` 2.0 ms, `rtok --version` 5.3 ms, `rtok hook PreToolUse` 8.4 ms, `rtok hook PostToolUse` 8.6 ms.

### 19.3 Root causes, largest first

1. **The launcher, 6.4 ms.** Claude Code runs `/bin/sh -c`, which execs `hook.sh`, a second `/bin/sh` (bash in sh mode, ~5 ms to start on macOS), which forks once more for `$(command -v rtok)` before it execs `rtok`.
2. **Binary start, 5.6 ms over the shell floor** (3.3 ms over `/bin/echo` in hyperfine). Two static initialisers only (`__mod_init_func` is 16 bytes); dyld maps 25 MB of `__TEXT`, rebases 860 KB of `__DATA_CONST` and loads Security, CoreFoundation and CoreServices. `DYLD_PRINT_STATISTICS` prints nothing on this macOS, and samply is not installed, so this was not split further.
3. **In-process, 2.9 ms.** Temporary `Instant` marks (not committed), p50 of 200 runs, with a second connection held open as a live MCP server holds one: clap parse 0.23 ms, `Config::load_lenient` 1.07 ms (figment: defaults, user TOML, legacy fold, env), `Store::open` 0.84 ms (connection, pragmas, settled-migration check, host row), registry 0.01 ms, `calls` row and plugins 0.6 ms, `set_call_ms` and `call_io` 0.08 ms, drop 0.08 ms. With no other connection open, drop costs 1.1–2.1 ms more: the last connection checkpoints the WAL on close. Migrations do not run on a settled store (`migrate` returns after one `COUNT`).

### 19.4 The 5 s cancellations and UserPromptSubmit

`~/.claude/settings.json` has no `hooks` key; the only enabled plugin is `rtok@rtok`, whose `hooks.json` owns every `UserPromptSubmit` hook, with `timeout: 5` from `setup.hook_timeout_s`. The ten cancelled rtok hooks (5 `PreToolUse`, 5 `UserPromptSubmit`, 2026-09-14 to 2026-09-21) all ran the older settings-file command `rtok hook <event>`. The store does hold `UserPromptSubmit` rows (905, mean 0.52 ms in-process). For four of the ten, the matching `calls` row carries a timestamp 0–1 s before Claude Code logged the cancellation, 5 s after it started the hook, and recorded 0.3–0.6 ms in-process; the other six left no row in that window. So the stall came before `record_call`, which is `Config` load or `Store::open`. `Store::open` waits `busy_timeout = 1000` ms on a locked database and retries a locked open up to ten times (~11 s at worst), so this points at the SQLite write lock. Not reproduced here.

### 19.5 Change and result

`plugins/claude/hooks/hooks.json` now runs `command -v rtok >/dev/null 2>&1 && exec rtok hook <event>; exec "${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh" <event>`. `command -v` is a shell builtin, so with `rtok` on PATH Claude Code's own shell execs it directly; without it, `hook.sh` keeps the ketch-store lookup and the fail-open hint. Same harness, 500 rounds, load average 40 → 30:

| event | before (`hook.sh`) p50 / p95 | after p50 / p95 | `true` floor p50 |
| --- | --- | --- | ---: |
| PreToolUse | 20.89 / 62.26 ms | 14.63 / 41.65 ms | 5.38 ms |
| PostToolUse | 18.99 / 33.33 ms | 13.30 / 22.24 ms | 4.86 ms |

About −6 ms, or 30 %, on every hook call. The T178 Check (p50 under 10 ms as Claude Code sees it) is **not met**: the shell floor plus `rtok --version` alone is 10.8 ms. What is left cannot come from trimming the hook path. Parse, config and store open together are 2.1 ms. Reaching 10 ms needs a process that starts in ~1–2 ms: a small hook client with no TLS or framework dependencies, talking to a resident process over a socket, and falling open when the process is absent.

### 19.6 A locked store (2026-09-23)

Where the waits came from: the hook opens the store fine with another writer holding the lock (WAL readers never wait), and its first write, `record_call`, waited out `busy_timeout = 1000` ms. The error was dropped, so the plugins ran on and each of their writes could wait another second. A migration run held on a fresh or upgraded store waits up to 30 s. Fix: `Store::open_with` takes a `LockWait` (per-statement `busy_timeout`, connect attempts, migration wait). `Store::open` keeps 1 s × 10 / 30 s; `rtok hook` passes 5 ms × 1 / 5 ms. When `record_call` comes back "database is locked", the hook returns `{}`: input unchanged, no row written, one `rtok: hook <event> skipped: store locked` line on stderr.

`tests/hook_fail_open.rs` `a_locked_store_fails_the_hook_open_in_ms`: another thread holds `BEGIN IMMEDIATE` on the store, and `rtok hook` (debug build) runs with a Bash payload that the unlocked control run rewrites. Wall time of the whole process, start to exit:

| event | before | after (3 runs) |
| --- | --- | --- |
| PreToolUse | 1.06 s, command still rewritten | 19–31 ms, `{}` |
| PostToolUse | 2.13 s | 21–23 ms, `{}` |
| UserPromptSubmit | 1.07 s | 21–22 ms, `{}` |
| SessionStart | 1.07 s | 20–30 ms, `{}` |

This does not reproduce a full 5 s cancellation. With the lock held, one event wrote at most two statements that waited, but UserPromptSubmit injection and a migration's 30 s wait can add more. After the fix, none of these waits exceeds 5 ms on the hook path.

## 20. WebSearch, WebFetch and browser page text: size, reach, what would cut it (2026-09-23)

T180. Corpus: `~/.claude/projects/**/*.jsonl` modified in the last 7 days — 347 files, 33,599 tool results, deduplicated by `tool_use_id` (resumed sessions copy history, which inflated the 2026-09-22 audit's figures). Bytes are the result text the model received, after any rtok shrinking. Claude Code 2.1.267. Scan scripts stayed in scratch.

### 20.1 Size

| Tool | Calls | Bytes | Share of all tool-result bytes | p50 | p90 | max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| all tools | 33,599 | 31,948,245 | 100 % | | | |
| `WebSearch` | 552 | 1,544,421 | 4.83 % | 2,803 | 3,310 | 4,827 |
| `WebFetch` | 513 | 1,039,970 | 3.26 % | 1,266 | 2,835 | 41,232 |
| `Claude_Browser` `get_page_text` | 16 | 150,143 | 0.47 % | 4,561 | 25,477 | 40,447 |
| `Claude_Browser` `read_page` | 13 | 95,083 | 0.30 % | 4,911 | 20,292 | 20,301 |
| Bash calling `curl`/`wget`/`gh api` | 160 | 231,431 | 0.72 % | 575 | 3,920 | 17,258 |

Web results together: 3.06 MB, 9.6 % of tool-result bytes — third after Bash (41.8 %) and file reads (`Read` + MCP reads, 34.7 %). `claude-in-chrome` did not appear in the window.

### 20.2 Anatomy (400-result samples per tool)

- **`WebSearch`**: a fixed header, a `Links: [...]` JSON array (title + url, usually 10 entries), prose written by the search sub-call, and a `REMINDER` line about citing sources. The `Links` array is **48.4 %** of the bytes. Only **12 %** of listed links (mean per result) are cited by url or host in the prose; the rest are often off-topic (a search for cargo cleanup tools lists Wikipedia pages on a spacecraft and a vacuum cleaner). 615 of 3,665 link mentions repeat a url already listed in an earlier result.
- **`WebFetch`**: already reduced by the host — a small model answers the agent's prompt over the page, so p50 is 1.3 KB. The long tail is pages returned nearly verbatim (Markdown docs such as `code.claude.com`): 12 of 400 results above 8 KB hold 220 KB.
- **Browser page text**: page text or an accessibility tree with `[ref_N]` handles the agent needs for clicks; 415 duplicate-line bytes in the largest page. Below the 1 % gate on its own.

### 20.3 Offline estimate of candidate reductions

| Reduction | Tool | Saving on sample | Lossless? |
| --- | --- | ---: | --- |
| `Links` JSON → one `- title <url>` line per link | `WebSearch` | 5.3 % | yes (format only) |
| same, plus keep only links cited in the prose; the rest behind `expand <id>` | `WebSearch` | 42.8 % | via `expand` |
| head/tail above 8 KB, rest behind `expand <id>` | `WebFetch` | 15.9 % (12 of 400 hit) | via `expand` |
| head/tail above 4 KB | `WebFetch` | 25.8 % (34 of 400 hit) | via `expand` |

Scaled to the week: WebSearch 42.8 % × 1.54 MB ≈ 661 KB, WebFetch (8 KB cap) 15.9 % × 1.04 MB ≈ 165 KB — ≈ 826 KB, **2.6 % of all tool-result bytes**, before counting that each result is re-sent (cached) on every later turn of its session. These are estimates on a scratch script, not `Measurement` rows; a built filter must record its own.

### 20.4 Surfaces that can reach these results

| Surface | Reaches | Today | Notes |
| --- | --- | --- | --- |
| `rtok proxy` `proxy_filter` on the Anthropic wire | every tool result in the request; tool name from the preceding `tool_use` | works for proxy users; **not the creator** — their `ANTHROPIC_BASE_URL` is `https://api.anthropic.com` | byte-stable rewrite on first sight keeps the cache prefix (the `archive` invariants); fail open on any parse error |
| PostToolUse `updatedToolOutput` | native `WebSearch`/`WebFetch` in every Claude Code session | unknown — T134 is the probe | if honoured, the widest reach with no proxy |
| PostToolUse `updatedMCPToolOutput` | MCP tools only (browser page text) | documented for MCP tools | browser text is 0.77 % — below the gate |
| rtok MCP `fetch` replacing `WebFetch` (I-92) | pages the agent is steered to fetch through rtok | not built | loses the host's per-prompt summary, so a readable-text page (p50 several KB) would usually be **larger** than WebFetch's answer (p50 1.3 KB); wins only on the verbatim tail |
| rtok replacing `WebSearch` | — | no | `WebSearch` is a server-side search; rtok has no search backend and should not add one |

### 20.5 Recommendation

One pure formatter for web results — `WebSearch`: compact the `Links` array and keep only cited links, the full array archived behind `expand <id>`; `WebFetch`: head/tail above a byte cap, archived — wired first behind whichever hook surface T134 opens for native tools, and into the proxy as a second consumer for proxy users. Do T134 before building anything: without `updatedToolOutput` the creator's own sessions see no saving. I-92 as written would grow context on the common case. Browser page text and Bash network calls stay below the gate. Proposed as I-97 in `ideas.md` for creator approval.

## 21. General HTTP(S) interception as a surface: measured, not built (2026-09-23)

T165. Question: would a local MITM proxy (`HTTPS_PROXY` + a CA the user trusts) reach agent tokens that `rtok proxy`, the hooks and MCP cannot? Corpus and method as §20: `~/.claude/projects`, last 7 days, 33,599 tool results deduplicated by `tool_use_id`, 31.9 MB of result text; Claude Code 2.1.267.

### 21.1 How much agent context arrives over HTTP outside the model API

| Result source | Share of tool-result bytes | How the bytes travel | Already reachable by |
| --- | ---: | --- | --- |
| `WebSearch` | 4.83 % | server-side search inside a model API call — not outside the API | `rtok proxy` (tool result in the next request); PostToolUse if T134 |
| `WebFetch` | 3.26 % | the host fetches the page itself, then a small model answers the agent's prompt over it; some Markdown pages come back verbatim | `rtok proxy`; PostToolUse if T134 |
| Bash `curl` / `wget` / `gh api` | 0.72 % | the command's own HTTP | Bash PreToolUse rewrite (`cmd`, `[curl]` rule) |
| Browser page text (`Claude_Browser`) | 0.77 % | a separate browser renders the page; text returns as an MCP result | `updatedMCPToolOutput`; `rtok proxy` |
| **Reachable only by interception** | **≈ 0 %** | | |

Non-API HTTP carries 4.75 % of the bytes (WebFetch, Bash, browser), but every one of those results enters the context as a tool result that an existing surface already sees, and in its final form. An interceptor would see the raw page instead, and for WebFetch only before the host's summarizing call — so what it could shrink is that side call's input, not the agent's context. Below the card's 1 % gate: **do not build.** Creator approved the stop on 2026-09-23.

Re-open when a host appears that fetches content client-side and places it in context through no hook, MCP or `*_BASE_URL` surface, and that share reaches ≥ 1 % of tool-result bytes on a 7-day scan.

### 21.2 Survey (read 2026-09-23), kept for a re-open

| Option | Version / date | Fit for rtok | CA install and removal | Pinning, HTTP/2, streaming |
| --- | --- | --- | --- | --- |
| mitmproxy | 12.2.3 (PyPI) | a second runtime (Python) beside the single rtok binary | own CA in `~/.mitmproxy`; the user trusts it per OS (Keychain, `update-ca-certificates`, `certutil`) | HTTP/1, 2, 3 and WebSockets; `ignore_hosts` passes hosts through untouched; TLS-failure hooks allow excluding a host after a pinning failure |
| `hudsucker` (Rust) | 0.25.0, crates.io 2026-07-15 | in-process library on hyper + rustls, `rcgen` authority; fits the binary | rtok would generate the CA and script trust and removal itself | HTTP/2 feature, WebSocket interception; bypass for pinned hosts is rtok's job |
| `http-mitm-proxy` (Rust) | 0.18.0, crates.io 2026-01-24 | lower-level library, smaller user base | same as `hudsucker` | SSE and WebSocket passed raw, no parsers |
| Proxyman / Charles | desktop apps | not embeddable; GUI-first, commercial | `proxyman-cli install-root-cert … --trust` (macOS Keychain) | per-host SSL proxying toggles |
| No MITM: `*_BASE_URL` proxy + hooks + MCP | shipped | the current design | none | nothing to pin; covers every row of §21.1 |

Claude Code trusts its bundled Mozilla set plus the OS store by default (`CLAUDE_CODE_CERT_STORE=bundled,system`) and honours `HTTPS_PROXY`/`NO_PROXY` (code.claude.com network-config page, read 2026-09-23), so an interceptor would work for it without extra flags; clients that ship their own root set would reject the CA and must pass through untouched. Latency was not measured, since nothing is built.

### 21.3 Privacy rule recorded for any future interception work

Default-deny: no host is decrypted unless it is on an explicit allow-list of hosts that carry agent-visible text; everything else is a plain CONNECT tunnel. The CA key lives in `RTOK_HOME` with owner-only permissions, is never exported, and one command removes both the key and the trust entry. Nothing is stored beyond what `archive` already keeps under its retention. Creator choice, 2026-09-23.

## 22. Host junk map (T182) (2026-09-24)

Nothing here is deleted by rtok until the creator reviews the map; all 17 hosts (`HOSTS` in `src/agents/mod.rs`) were re-verified against official docs or source repos via WebSearch/WebFetch on 2026-09-24, and a cell reads "not documented" rather than a guess whenever no host-specific official source names a path — a wrong row here can destroy a user's real data. Evidence: "documented" = the host's own docs site; "source" = the host's own repo (file cited); both give the full URL, never a site name.
Never junk, on any host: settings/config files, credentials and auth tokens, session or conversation history (e.g. pi's `~/.pi/agent/sessions`, aider's `.aider.chat.history.md`/`.aider.input.history`), installed extensions/plugins, and a whole config/state directory named as if it were all junk (`~/.gemini/`, `~/.kimi-code/`, `~/Library/Application Support/Zed`, `~/.config/Code/` are never junk as a whole).

| Host | Temp | Logs | Cache | Evidence | Checked |
| --- | --- | --- | --- | --- | --- |
| claude | `~/.claude/shell-snapshots/` (cleared on clean exit; sweep clears the rest) | `~/.claude/debug/`; legacy `~/.claude/logs/`, `todos/`, `statsig/` are no longer written | `~/.claude/paste-cache/` | documented: https://code.claude.com/docs/en/claude-directory | 2026-09-24 |
| cursor | not documented | not documented | not documented | not documented — closed-source app; docs confirm a `/logs` command exists but name no fixed path (https://cursor.com/changelog/04-14-26) | 2026-09-24 |
| codex | not documented | `$CODEX_HOME/log` (default `~/.codex/log/`) | not documented | documented: https://learn.chatgpt.com/docs/config-file/config-reference; source: https://github.com/openai/codex/blob/main/codex-rs/core/src/config/mod.rs#L4045 | 2026-09-24 |
| opencode | not documented | `~/.local/share/opencode/log/` (macOS/Linux), `%USERPROFILE%\.local\share\opencode\log` (Windows), auto-rotated | `~/.cache/opencode/` (macOS/Linux), `%USERPROFILE%\.cache\opencode` (Windows) | documented: https://opencode.ai/docs/troubleshooting/ | 2026-09-24 |
| kilo | not documented | not documented | not documented | not documented — CLI exposes a `kilo debug paths` runtime lookup instead of a fixed default (https://kilo.ai/docs/code-with-ai/platforms/cli-reference) | 2026-09-24 |
| pi | not documented | `~/.pi/agent/pi-debug.log` (written by `/debug`) | not documented | source: https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/usage.md | 2026-09-24 |
| omp | not documented | not documented | `~/.omp/cache/github-cache.db` only — the sibling `~/.omp/cache/auth-broker-snapshot.enc` is an encrypted credential snapshot; never delete the `cache/` folder itself | source: https://github.com/can1357/oh-my-pi/blob/main/docs/environment-variables.md | 2026-09-24 |
| zcode | `~/.zcode/cli/exec` (past terminal output; ZCode recreates it) | not documented | not documented | documented: https://zcode.z.ai/en/docs/qa | 2026-09-24 |
| kimi | not documented | `~/.kimi-code/logs/` | `~/.kimi-code/bin/` (downloaded `rg`/`fd`, redownloaded on use); `~/.kimi-code/updates/latest.json` | source: https://github.com/MoonshotAI/kimi-code/blob/main/docs/en/configuration/data-locations.md | 2026-09-24 |
| grok | not documented | `~/.grok/logs/` (`unified.jsonl`, MCP server logs; override `GROK_LOG_FILE`) | not documented | source: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/05-configuration.md | 2026-09-24 |
| vscode | not documented | not documented | not documented | documented (mechanism only, no path given): https://docs.github.com/en/copilot/troubleshooting-github-copilot/viewing-logs-for-github-copilot-in-your-environment | 2026-09-24 |
| copilot | not documented | `~/.copilot/logs/` (recreated every session) | macOS `~/Library/Caches/copilot`, Linux `$XDG_CACHE_HOME/copilot` (or `~/.cache/copilot`), Windows `%LOCALAPPDATA%/copilot`, override `COPILOT_CACHE_HOME` | documented: https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference | 2026-09-24 |
| aider | not documented | not documented | not documented | not documented — only documented files are `.aider.conf.yml` and the chat/input history, both excluded (https://aider.chat/docs/faq.html, https://aider.chat/docs/config/options.html) | 2026-09-24 |
| windsurf | not documented | not documented | not documented | not documented — Devin Desktop's Windsurf-migration FAQ names generic Electron `logs/`/`crashpad/`/cache categories but no Windsurf-specific path (https://docs.devin.ai/desktop/devin-desktop-faq) | 2026-09-24 |
| zed | not documented | macOS `~/Library/Logs/Zed/Zed.log`; Linux `~/.local/share/zed/logs/Zed.log` (or `$XDG_DATA_HOME/zed/logs/Zed.log`) | macOS `~/Library/Caches/Zed`; Linux `$XDG_CACHE_HOME/zed` — caution, issue #17835 shows Zed has also written `~/.cache/zed` on macOS by bug; verify the installed version first | documented: https://zed.dev/docs/troubleshooting, https://zed.dev/docs/macos; source (bug): https://github.com/zed-industries/zed/issues/17835 | 2026-09-24 |
| gemini | not documented — `~/.gemini/tmp/<hash>/` exists but holds checkpoints/shell history, which is session history, not junk | not documented | not documented | documented: https://geminicli.com/docs/cli/settings/, https://geminicli.com/docs/resources/troubleshooting/ | 2026-09-24 |
| codewhale | not documented | not documented — `audit.log` is tied to session reconciliation, not a pure rotating log | `~/.codewhale/update-check.json` (single file; caches the update-check result, reused for `check_interval_hours`) | source: https://github.com/Hmbown/Codewhale/blob/main/docs/CONFIGURATION.md | 2026-09-24 |

## 23. Subagent-start context injection per host (T262.2) (2026-09-24)

Question: which hosts let a hook add context to a sub-agent before it runs, the way Claude Code's `SubagentStart` returns `hookSpecificOutput.additionalContext` (the T130 spawn brief)? Checked from each host's hook docs or source; the two yes rows re-read first-hand.

| Host | Verdict | Event, output | Source |
| --- | --- | --- | --- |
| Claude Code | yes | `SubagentStart`, `additionalContext` | wired in T130.2 |
| VS Code Copilot Chat | yes | runs `plugins/claude` hooks as-is | `src/agents/vscode/mod.rs` |
| Codex | yes | `SubagentStart`; plain stdout or hook-specific context becomes developer context for the subagent | https://learn.chatgpt.com/docs/hooks |
| Copilot CLI | yes | `subagentStart` (matcher on agent name), `additionalContext` prepended to the subagent's prompt; the built-in general-purpose agent emits no event | https://docs.github.com/en/copilot/reference/hooks-reference |
| Kimi | event-only | `SubagentStart` fires; the result of `runner.trigger` is discarded | MoonshotAI/kimi-code `packages/agent-core-v2/src/features/externalHooks/session/sessionExternalHooksService.ts` |
| Cursor | event-only | `subagentStart` output has only `permission` / `user_message` | https://cursor.com/docs/hooks |
| Grok | event-only (weak) | `SubagentStart` / `SubagentStop` fire; no output schema documented | https://docs.x.ai/build/features/hooks |
| CodeWhale | event-only | `subagent_spawn` is an observer event; result discarded | `src/agents/codewhale/README.md` |
| Gemini CLI | no | no subagent event (`BeforeAgent` / `AfterAgent` are the parent turn) | https://geminicli.com/docs/hooks/reference/ |
| ZCode | no | no subagent event | https://zcode.z.ai/en/docs/hooks |
| OpenCode, Kilo | no | plugin events have no subagent spawn | https://opencode.ai/docs/plugins |
| Pi, omp | no | no hookable spawn; subagents are an extension of their own | badlogic/pi-mono `docs/extensions.md` |
| Windsurf | no | no subagent event among the documented hooks | https://docs.devin.ai/desktop/cascade/hooks |
| Cline | no | `new_task` hands off in the same conversation, no child agent | https://docs.cline.bot/customization/hooks |
| Antigravity | no (weak) | no hook on `invoke_subagent` | https://antigravity.google/docs/hooks/ |
| MiMo | no | no hook system | mimo docs |

Follow-ups: T262.3 (Codex) and T262.4 (Copilot CLI). Grok and Antigravity rest on missing docs, so a docs change there is worth a recheck. Found on the way: Copilot CLI `subagentStop` accepts `modifiedResponse`, which replaces the subagent's answer to the parent (idea I-98).
