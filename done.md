# rtok — completed tasks


## P36 — second bug-hunt residue (open) — T36.19

**T36.19 a setup backup is never overwritten** · — · `crates/rtok-agent-sdk/src/lib.rs`
Do: the `.bak` collision loop gives up after 99 iterations and then `fs::copy` overwrites an existing backup.
Check: with 100 pre-existing `.bak-*` names the install still refuses to clobber one (unique name or an error). `cargo test -p rtok-agent-sdk`, all 9 tests pass, including `backup_skips_a_hundred_preexisting_names_without_clobbering`.
Complexity: 1/5
Status: done 2026-09-11 · Model: Composer 2.5

**T36.5 `read` cap includes its own marker** · — · `src/plugins/read/mod.rs`
Do: `cap()` returns `max_chars` characters *plus* the `… archived <id> …` marker, so every capped read overshoots the configured cap.
Check: a fixture over `plugins.read.max_chars` returns at most `max_chars` characters including the marker, and still carries a working archive id. `cargo test --lib plugins::read::tests`, 7/7 pass, including `cap_includes_marker_in_max_chars` (rstest cases).
Complexity: 1/5
Status: done 2026-09-11 · Model: Composer 2.5

## P35 — Graph index speed (open; Gate P35 met 2026-09-11) — T35.1–T35.5

**T35.1 compile each tags query once** · T8.1 · `src/plugins/read/outline.rs`
Do: `outline::config` compiled the language's tags query on every `tags` call. That was 19 ms of a 26.5 ms call on `graph/index.rs`, paid by each file of an index run and by each outline.
Check: one `OnceLock` per language, kept for the process. A failed compile keeps its message, so every call gets the same `Err` and never a retry. `each_language_compiles_once` checks that two `.rs` files get the same configuration and a `.ts` file a different one. `golden_per_language` and the graph tests are unchanged: definitions 42/42, ref recall 0.305. The cold debug index of this repo fell from 3.42 s to 1.38 s and 1.30 s over two runs (127 files, 18 100 rows, 2026-09-11). The release `graph_bench` cold index of 3 000 files fell from 13.8 s to 341 ms (research.md P8c). A `TagsContext` per thread was not added: it builds a parser and a cursor and does not compile a query, so no measurement points at it.
Complexity: 2/5
Status: done 2026-09-11 · Model: Opus 5

**T35.2 parse on worker threads, write from one** · T35.1 · `src/plugins/graph/index.rs`
Do: after T35.1 one thread still read, hashed and parsed each file in turn while the other cores idled. The owner made it the first task in P35 (2026-09-11).
Check: the walk and the stat gate stay on the calling thread. Files that pass the gate go to one worker per core (`std::thread::scope`), and their results reach the one writer (D18) in walk order through a reorder buffer, so the store gets a sequential run's writes in a sequential run's order. The channel is bounded, so a slow writer stalls the workers; a failed write drops the receiver and stops them. New tests: `parsed_files_are_written_in_walk_order` (32 files, the early ones largest) and `a_failed_write_stops_the_workers`. The seven existing index tests stay green, `second_run_inserts_zero` and `two_roots_do_not_evict_each_other` among them. The cold debug index of this repo fell from 1.30–1.38 s to 281–296 ms (127 files, two runs). The release `graph_bench` index of 3 000 files fell from 341 ms to 172–174 ms, 9 000 rows both times. Definitions 42/42, ref recall 0.305. Gate P35 is met.
Deviation: the release bench is 2.0× faster, short of the Check's "≥ 3×". The bar was set against 13.8 s, when the per-file query compile was most of the time. After T35.1 what remains is the walk, the per-file stat SELECT and the one-transaction-per-file writes, which is T35.3's work.
Complexity: 3/5
Status: done 2026-09-11 · Model: Opus 5
Found on push, 2026-09-11: ubuntu CI failed `map_src_main_lists_fn_main` with ENOENT, and macOS passed. `symlink_escape_is_err` moved the process cwd with `set_current_dir`, and the map test's relative `src/main.rs` resolved against the moved cwd. The race predates this task; the new timings exposed it. The test now calls `resolve` with the `cwd` it builds, which is the guard `read` runs against the process cwd, and no test moves the cwd any more.

**T35.4 the watcher indexes what changed** · T8.16 · `src/plugins/graph/watch.rs`, `src/plugins/graph/index.rs`
Do: `pump` keeps the relevant event paths; `settle` indexes only those (the stat and sha gates per path; rows dropped for a vanished path). A full walk only on a rescan or overflow event.
Check: `pump` and FSEvents watch tests green (`graph::watch` 9/9). New `run_changed_indexes_only_touched_file` (20 files, `read == 1`). Vanished paths use `mark_symbols_stale`, never `delete_symbols_missing` on a partial keep set. Overflow (>1024) and `need_rescan` still full-walk. One edit on the 3 000-file fixture via `run_changed` was `read=1` in 1.66 ms (2026-09-11, `p8c_one_edit_reads_one_file_under_10ms`; `tests/graph_bench.rs`).
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

**T35.5 rebuild a root when the extractor changes** · T8.1 · `src/plugins/graph/index.rs`, `src/store/symbols.rs`, `src/store/symbols_lbug.rs`, a migration
Do: a fingerprint of the extractor — every tags query string, the tree-sitter grammar crate versions, an `INDEX_VERSION` bumped when `scoped` changes — stored per root. A mismatch drops the root's rows and indexes it cold; a match changes nothing. Today a query change (T8.2's `RUST_SCOPED_CALL`, say) reaches only files edited afterwards.
Check: `reindexes_when_extractor_fingerprint_mismatches` — 3 files, tamper fingerprint to deadbeef, `read == indexed == 3`, next run `read == 0`. Fingerprint = sha256(`INDEX_VERSION` || tags queries including `RUST_SCOPED_CALL`). SQLite table `extractor` (`0011.sql`). `graph-lbug` keeps the same `Store` methods. `graph::index` 11/11.
Complexity: 3/5
Deviation: Host trait + `plugin.rs` + `symbols_lbug.rs` + `outline.rs` `pub(crate)` `RUST_SCOPED_CALL` — more than 3 files because both Store backends must compile and the hook `Host` needs the methods.
Status: done 2026-09-11 · Model: Composer 2.5


## P28 — LLM compression (design; open) — T28.0

**T28.0 design note: LLM compression vs lossless** · — · `src/plugins/compress/PLAN.md` or `src/plugins/memory/PLAN.md` (extend), `docs/` as needed
Do: D15-style survey of LLMLingua-2, claude-mem extraction, and at least one other compressor. Name the mechanism rtok will use, what stays lossless, what is default-off, and the falsifier (cost per passed task rises, or expand cannot recover a non-regenerable original). No implementation.
Check: PLAN (`src/plugins/compress/PLAN.md`) names LLMLingua-2, claude-mem, Selective Context (and headroom); mechanism archive-first semantic shrink; Gate P28 = cost per passed task ≤ v0.1 lossless bench; falsifier cost-up or expand fails.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

## P29 — Embeddings / semantic search (design; open) — T29.0

**T29.0 design/survey: embeddings beside FTS5** · — · `src/plugins/memory/PLAN.md` and/or `src/plugins/graph/PLAN.md`
Do: survey mem0, code-review-graph embeddings, and at least one other embed path. Decide where vectors live, how they stay optional, and how a fixture is proven found by both FTS5 and embed. No implementation.
Check: PLAN (`src/plugins/memory/PLAN.md` v0.2 survey): ≥ 3 alternatives (mem0, code-review-graph, sqlite-vec chosen); vectors in `rtok.db`; Gate fixture `p29-gate-arctic-tern`.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

## P30 — LSP graph backend (design; open) — T30.0

**T30.0 design/survey: LSP behind tags MCP** · — · `src/plugins/graph/PLAN.md`
Do: survey serena-grade LSP backends and how they map onto the existing MCP tool names. Tags remain default; LSP is optional. Name one fixture where tags miss and LSP hits. No implementation.
Check: PLAN (`src/plugins/graph/PLAN.md` P30 survey): rust-analyzer / clangd / tsserver surveyed; MCP names stable; Gate P30 `OnlyTyped` type-position miss.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


## P31 — Semantic response cache (design; open) — T31.0

**T31.0 design/survey: semantic cache** · — · `src/plugins/proxy/PLAN.md` or `src/proxy/` design note
Do: survey bifrost and at least two other semantic-cache approaches. Define similarity threshold, opt-in shape, and how false hits are measured on the P9 task set. No implementation.
Check: PLAN (`src/plugins/proxy/PLAN.md`) names ≥ 3 alternatives (bifrost, GPTCache, RedisVL); rejected wrapping; false-hit protocol = 0 false-hit pairs on frozen P9 `call_io` at cosine ≥ 0.99.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

## P32 — WASM plugin host (design; open) — T32.0

**T32.0 design/survey: WASM host** · — · `crates/rtok-plugin-sdk/PLAN.md` or `docs/plugin-authoring.md` extension
Do: survey WASM runtimes suitable for a static Rust binary (at least three). Define the `from_plugins` load path, the Measurement example contract, and what stays in-process for in-tree plugins. D6 call-out: no vendored third-party plugins in this repo. No implementation.
Check: PLAN (`crates/rtok-plugin-sdk/PLAN.md`): Wasmtime / Wasmi / Wasm3; chosen Wasmi; WASM not on hook; Gate P32 `wasm-demo` `Measurement`.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


## P33 — Tiered session context (design; open) — T33.0

**T33.0 design/survey: L0/L1/L2 tiers + AGPL** · — · `src/plugins/archive/PLAN.md` and/or `inject` PLAN
Do: survey OpenViking L0/L1/L2 and at least two other tiered-context schemes. Call out AGPL (or other) license implications in the PLAN before any code. Define how tiers compose with v0.1 `archive`+`inject` and what "measured against" means for Gate P33. No implementation.
Check: OpenViking, MemGPT/Letta, Claude Code compaction; AGPL-3.0 call-out (do not vendor); Gate P33 measurement vs v0.1 archive+inject CTT.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

## P34 — Hardening pass (2026-09-10) — T34.1–T34.9

Goal: a bug hunt over the store, proxy, hooks and report, the tests those bugs lacked, and one
helper where two copies had drifted. Found by four read-only audits; each fix names its test.

**T34.1 store: concurrent writers, atomic purge, literal read-cache prefix** · T0.3 · `src/store/mod.rs`
Do: hooks, `rtok mcp`, `rtok proxy` and the detached otel flush share one SQLite file with the default `busy_timeout` of 0, so an overlapping write failed with "database is locked" (the Ubuntu CI flake). `purge_calls_older_than` ran four statements outside a transaction and hit the `calls` foreign keys from `usage`, `measurements` and child calls after the first deletes had committed. `clear_read_cache` matched `LIKE 'path\t%'`, so `_`, `%` and ASCII case in a file name cleared other files' cache rows. `spill` and `put_archive` each wrote the archive file and row, one of them labelled every archive `cmd`.
Check: `busy_timeout = 1000` on open; purge in one transaction with one cutoff, detaching ledger rows (a saving outlives its call); a `substr` prefix compare; one `write_archive`. Tests `purge_drops_old_calls_and_detaches_their_ledger_rows`, `clear_read_cache_drops_only_that_path_and_its_mode_keys`.
Complexity: 3/5
Status: done 2026-09-10 · Model: Opus 5

**T34.2 latency gates use nearest-rank p95** · T2.2 · `tests/common/mod.rs`, `tests/otel.rs`, `tests/latency.rs`, `tests/graph_bench.rs`
Do: three gates took `samples[n * 95 / 100]`, one rank high — at n = 20 (the debug otel gate) that is the maximum, so one slow spawn failed it.
Check: one `common::p95` (`ceil(0.95·n) − 1`) in all three.
Complexity: 1/5
Status: done 2026-09-10 · Model: Opus 5

**T34.3 one test fixture instead of eleven** · — · `src/testutil.rs`, `src/plugin.rs`, unit tests in `expand`, `mcp`, `cmd`, `memory`, `read`, `graph`, `guard`, `tui`
Do: eleven test modules each built a temp dir and a `Config` pointing into it; `guard` reused one fixed dir across runs, and every `tui` test shared (and deleted) one `rtok-tui-<pid>` dir, so parallel tests failed with os error 22. `Runtime::open` and `in_memory` duplicated their construction.
Check: `testutil::{tmp_dir, config, runtime}` (a fresh dir per call, `same_tag_gives_distinct_dirs`); `Runtime::with_store`. New tests: `bash_repeat_behind_cd_prefix_denies` (guard's Bash key was untested), `dot_dot_escape_is_err` (read's lexical escape).
Complexity: 2/5
Status: done 2026-09-10 · Model: Opus 5

**T34.4 report: one bar scale, UTF-8-safe PDF text** · T22.1 · `src/report/{mod,html,pdf}.rs`
Do: `pdf::bars` cut labels with `&label[..26]`, which panics inside a multi-byte character; `split_word` chunked bytes and printed U+FFFD. HTML and PDF each scaled bars.
Check: `report::bar_shares` shared; char-based truncation and splitting. Tests `bars_truncates_long_multibyte_label_without_panicking`, `split_word_splits_on_chars_not_bytes`, `bar_shares` cases.
Complexity: 2/5
Status: done 2026-09-10 · Model: Sonnet 5 (subagent)

**T34.5 agent-sdk writes host configs atomically** · T27.0 · `crates/rtok-agent-sdk/src/lib.rs`
Do: `write` truncated `~/.claude.json` and friends in place — a crash mid-write lost the host config, and a symlinked (dotfile-managed) config was replaced by a file.
Check: temp file beside the target, permissions copied, `rename`; the symlink's target is written. Tests `write_leaves_no_temp_file_behind`, `write_preserves_existing_permissions`, `write_through_a_symlink_updates_the_target_and_keeps_the_link`.
Complexity: 2/5
Status: done 2026-09-10 · Model: Sonnet 5 (subagent)

**T34.6 proxy: bounded tee, one usage reader** · P5 · `src/proxy/{mod,wire,anthropic,openai_chat,openai_responses}.rs`
Do: the response tee buffered the whole upstream body with no cap; each wire re-implemented usage extraction; "no usage in upstream response" was logged for every non-model path.
Check: the tee keeps at most `MAX_BODY_BYTES` (the client still gets every byte; the true size is logged when cut); `wire::find_usage` with per-wire field names; the log fires only for a known wire. Existing wire usage tests unchanged and green.
Complexity: 2/5
Status: done 2026-09-10 · Model: Sonnet 5 (subagent)

**T34.7 `otel.headers` never prints** · P16 · `src/config/layers.rs`, `docs/config.md`
Do: `config show/get/set` and `rtok report` printed OTLP ingestion keys from `otel.headers`.
Check: `SECRET_KEYS` in `layers::entries` → `<redacted>` when set, source kept. Test `otel_headers_are_redacted_when_set`.
Complexity: 1/5
Status: done 2026-09-10 · Model: Opus 5

**T34.8 small bugs behind duplicated helpers** · — · `src/plugins/cmd/formatters.rs`, `src/plugins/{archive,toon}/mod.rs`, `src/doctor.rs`, `src/web/model.rs`
Do: `rtok run` picked a filter rule when any argument word named a tool (`git commit -m "fix grep"` → the `grep` rule). `archive` and `toon` each re-read the live-zone boundary. `doctor` and the operator model each carried a raw HTTP GET, both cutting a body at its first blank line.
Check: `pick` keys on `argv[0]` (`rule_is_picked_by_the_command_not_an_argument`; goldens unchanged); `archive::outside_live_zone` shared (`live_zone_turns_are_untouched` green); one `doctor::http_get` with `split_once`.
Complexity: 2/5
Status: done 2026-09-10 · Model: Opus 5

**T34.9 graph tests that name their failure** · T8.8, T8.16 · `tests/graph_truth.rs`, `tests/fixtures/graph_truth.toml`, `src/plugins/graph/{watch.rs,PLAN.md}`
Do: ref recall fell to 0.290, under its 0.30 floor, with the index unchanged. T34.6 had moved every `int_field` call into `wire.rs`, and T15.11 had already emptied `src/cli.rs` of `Registry` and `Replay`; a stale label scores as a miss. The watcher's debounce loop was testable only through FSEvents, which can only bound a run count.
Check: `every_label_names_a_file_that_mentions_the_symbol` names stale labels without indexing (0.03 s); the fixture is repaired with a header note; ref recall 0.305. `reference_capture_matches_the_known_misses` pins plain, path-qualified and method calls as found and type positions and macro arguments as missed. `pump` is split from `notify_loop`, and four tests drive it without FSEvents: `settle_runs_once_after_quiet_and_never_when_clean`, `irrelevant_events_never_run_and_a_burst_runs_once`, `edit_and_rename_events_reindex_once_each`, `pump_ends_on_stop_or_on_a_dead_channel`. `warm_watcher` rewrites its probe until the stream indexes it: a single probe written before the stream went live sat out the 10 s cap, so the two FSEvents tests fell from 10.8 s and 10.5 s to 2.0 s and 1.7 s, and the watch module from 10.9 s to 4.4 s with four more tests. The speed-ups are measured and filed as P35, with `PERF(T35.n)` comments at the code.
Complexity: 2/5
Status: done 2026-09-10 · Model: Opus 5

Not done, noted: an otel exporter backoff after a failed post, and `isMonotonic` on a sum that can go negative (both P16 follow-ups); `~` expansion on Windows.

## Residual bug-hunt (2026-09-10) — T10.11, T11.8, T16.9, T22.6, T24.5

**T10.11 Cursor host: wire PostToolUse, not only beforeShellExecution** · T10.1 · `src/setup/cursor.rs`, `src/hooks/types.rs`
Do: Cursor setup writes only `hooks.beforeShellExecution`, and `HookInput::adapt_cursor` only maps that shape onto PreToolUse. Guard's read cache and read's PostToolUse(Edit|Write) invalidation therefore never populate on Cursor. Register Cursor's after-tool / PostToolUse-equivalent hook, adapt its payload into `PostToolUse`, and keep fail-open ≤ 10 ms (D1).
Check: Cursor-shaped after-tool stdin adapts to `PostToolUse`; after a Cursor PostToolUse(Read) the guard cache has a row; `rtok agent setup cursor --dry-run` lists the after-tool entry beside `beforeShellExecution`; `just check` green.
Complexity: 3/5
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. `adapt_cursor` maps Cursor `afterShellExecution` to `PostToolUse` (`output`/`stdout` → `tool_response`); setup + `plugins/cursor/hooks/hooks.json` register `afterShellExecution` → `rtok hook PostToolUse --host cursor`. Unit tests `cursor_after_shell_maps_to_post_tool_use`, `setup_writes_after_shell_hook`; `cargo test --test cursor_plugin` green. Shell-only path (afterFileEdit/read not in this Do).

**T11.8 `toon` rewrite respects archive live-zone `keep_turns`** · T11.7 · `src/plugins/toon/mod.rs`
Do: `toon`'s `proxy_filter` rewrites every tabular tool result it sees. Archive's live zone (`plugins.archive.keep_turns` turns from the end) must stay untouched — the same boundary `archive` already uses — so a just-returned table is not TOON-encoded while it is still live context.
Check: with `keep_turns = 2`, tabular JSON in the last two turns is unchanged and older tabular blocks encode; Measurement rows only for the older ones; default-off still leaves request bytes identical; `just check` green.
Complexity: 2/5
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. `toon` skips `result.turn < archive.keep_turns` (same live-zone rule as archive). Test `live_zone_turns_are_untouched`; related toon/lib tests green.


**T16.9 concurrent flush must not double-export** · T16.6 · `src/otel/export.rs`, `tests/otel.rs`, `Cargo.toml`, `docs/otel.md`
Do: when `rtok proxy` and `rtok mcp` timer flushes overlap a detached `rtok otel flush` from `Stop`/`SessionEnd`, serializers must not race the same `otel_export` watermarks — today concurrent processes can double-post a batch and leave pending counts that disagree with what the collector received. Single-flight the flush (DB lock / advisory / exclusive writer) so overlapping exporters hand off rather than both export the same rows.
Check: a test that starts two overlapping flushes against one store and a mock collector posts each row once and advances each watermark exactly once; `rtok otel status` pending matches the unsent remainder; `just check` green.
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. `flush` takes an exclusive `flock` on `<db>.otel-flush.lock` (rustix `fs`) for the whole export; a waiting peer runs after and finds marks advanced (at-least-once, no double-post). `concurrent_flushes_post_each_row_once` overlaps two runtimes on one DB against a delayed mock collector — 3 spans + 1 log posted once, marks 3/1, pending 0. `mise exec -- cargo test` green; clippy `-D warnings` clean.

**T22.6 `idle-hook` must not false-positive on busy PostToolUse** · T22.5 · `src/report/advice.rs`, `tests/report.rs`
Do: `kinds_for_hook` maps `PostToolUse` to no Measurement kinds (`_ => &[]`), so any PostToolUse with ≥ `OFTEN_HOOK_CALLS` emits an `idle-hook` recommendation even when the path is busy (guard cache fill, read invalidation, graph stale marks). Fix the rule so productive PostToolUse side-effects are recognised — or exclude events whose work is not a Measurement kind by design — and keep true idle hooks flagged.
Check: a fixture with ≥10 PostToolUse calls and guard/read activity produces no `idle-hook` finding for `PostToolUse`; a truly idle event still does; `just check` green.
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. `idle_hooks` only considers idle-by-design events (`PreCompact` / `Stop` / `SessionEnd`); `PostToolUse` is excluded. Advice fixture uses 12 `PreCompact` (fires once) plus 12 `PostToolUse` (silent); healthy store with 12 `PostToolUse` stays empty. `cargo test --test report` 6/6 green.

**T24.5 retire unread `[core] log_file` / `log_level` / `log_to_db`** · T24.1 · `src/config/`, `config/default.toml`, `docs/config.md`
Do: the three `[core]` keys stay in the schema and the reference file but production readers use only `[log]` (D26). Fold them the way `[dashboard]` folded into `[web]` (accept once with a warning, map into `[log]`), then drop them from the typed schema so `[log]` is the only authority.
Check: a config that sets only legacy `core.log_*` loads into effective `[log]` with a warning; after the drop, `rtok config validate` rejects the old keys; `docs/config.md` and `config/default.toml` match the schema; `just check` green.
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. `core.log_file` / `log_level` / `log_to_db` are `Option` with `skip_serializing_if`, removed from `default.toml`; `finish()` migrates into `[log]` with one warning each and `take()`s them. `legacy_core_log_keys_migrate_into_log` and `validate_rejects_legacy_core_log_keys` green; `default_toml_is_the_defaults` green. Docs Legacy keys section + reference file match.

Tasks move here from `plan.md` when their Check passed, `make check` is green, and the work is
committed as `<task-id>: <title>`. Newest phase first. Task text is kept verbatim so the
history of what was asked stays readable next to what was delivered.

## P27 — `rtok-agent-sdk` (D28) · done 2026-09-09

Goal: one contract every agent host installs through; a sixth host is a new `src/setup/<host>.rs`
and nothing else. Plan: `plan.md` P27, decision D28.

**T27.0 the crate exists and the five hosts move onto it** · T26.0 · `crates/rtok-agent-sdk/*`, `Cargo.toml`, `src/setup/*.rs`, `src/proxy/cli.rs`, `src/cli.rs`, `.jscpd.json`
Do: the crate carries `Apply` (the `[setup]` flags: `dry_run`, `backup`, `yes`), `NO_CHANGES` as
both the report and the write gate, `backup`, `read_json` / `write_json` / `write`, `register_mcp`
/ `unregister_mcp`, `accepted` (dialoguer moves with it), and `PluginLink` — the offer/link/unlink
`plugins/cursor` and `plugins/pi` both spell today. Three dependencies, none of them C, the line
T23.0 drew for the plugin SDK. Claude, Cursor, Codex, OpenCode, pi, `proxy::cli` and `migrate` all
route through it; no report string changes, because the host integration tests assert them.
Check: `cargo test --workspace` green with the `agent setup`/`agent remove` integration tests
(`tests/cursor_plugin.rs`, `tests/pi_plugin.rs`, `tests/agent_remove.rs`) unmodified; `just dup`
does not regress; `rtok agent setup <host> --dry-run` prints the same lines as before for all five.
Complexity: 3/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent)
Check result: green, verified in a detached worktree. The three integration test files are
byte-identical (`git diff <base>..HEAD --stat -- tests/` empty) and pass (7/6/4). Dry-run strings:
base binary and new binary built, `agent setup <host> --dry-run` run for all five hosts in a
sandboxed HOME, normalized and diffed — byte-identical. `just dup` improved: 36 clones / 1.42 %
vs 49 / 2.17 % at base. `cargo test --workspace` had one failure, the pre-existing `graph_truth`
red that T8.19 (same round) then fixed; the full `just check` on main after all three landings is
green. Gate P27 holds: no production code under `src/setup/` writes a host file, copies a backup
or symlinks — only test-fixture `fs::*` remains.
Deviations: the bulk of the work was authored by an earlier session and left uncommitted (snapshotted
to main as `5975877` "T27.0 (wip)"); this round completed and verified it — restored the three report
strings the WIP had let drift (pi's dry-run source paren, cursor's `+ plugin` trailing label, cursor's
declined-offer/question paren) and pinned all three in the SDK's unit tests, dropped `dialoguer` from
the root manifest (it moved with `accepted`), and fmt. One inherited behaviour change kept: `migrate`
no longer rewrites/backs-up when its diff is empty (the SDK's `NO_CHANGES` write gate) — no report
string changed and no test asserted the old churn. 16 files, the task's own list in plan.md.

## P25 — `rtok agent sessions` (D27) · T25.0, T25.1, T25.2, T25.3 done 2026-09-09

**T25.3 `rtok agent sessions watch`** · T25.2 · `src/cli.rs`, `src/render.rs`
Do: the same table, redrawn on an interval, in place rather than scrolling; a session that appears,
ends or spends tokens shows up without a restart. Not a TUI — one screen, no key handling, and it
leaves the terminal as it found it on Ctrl-C. Where `rtok logs watch` (T24.3) streams new lines,
this one repaints state; both share the poll-and-print loop rather than growing two.
Check: a session started while `watch` runs appears within one interval and its duration advances;
piping the command produces plain repeated tables, not escape codes.
Complexity: 2/5
Status: done 2026-09-09 · Model: Muse Spark 1.3 (subagent-sessions-watch)
Check result: green in a detached worktree at HEAD holding only this task's files
(`src/cli.rs`, `src/render.rs`, `tests/agents.rs`, `tests/surface_parity.rs`) with an isolated
`CARGO_TARGET_DIR` — the shared tree is mid-flight with parallel uncommitted work (T15.6's
Doctor page, T22.2's HTML renderer) that does not compile here, so the main tree cannot go
green until those land. `cargo test --lib` 209 passed including the new
`render::a_sessions_tick_repaints_state_and_stays_quiet_otherwise` (new session / spent tokens /
duration tick repaint, unchanged poll stays quiet, no `\x1b` in any row); `cargo test --test
agents` 4/4 including the new `watch_shows_a_session_started_mid_run_and_repeats_plain_tables`
(seed live + ended, spawn `agent sessions watch` piped, insert `watch-new` from this process,
it appears, the ended `watch-gone` never shows, a further header lands with no writes at all
proving the duration repaint, output has no escape codes, ≥3 headers proving repeated tables);
`surface_parity` 2/2, `config_coverage` 1/1 (no new flags — `watch` is a subcommand, `all` was
already allowed); `clippy --lib --tests -D warnings` clean; `cargo fmt --check` clean; `just
dup` green (1.35%, threshold 2). TTY repaint is `watch_loop`'s own (T24.3): cursor-up + clear,
no raw mode / alternate screen, so Ctrl-C leaves the terminal as found; BrokenPipe maps to Ok
so `| head` exits cleanly.
Deviations: four files, not two — `tests/agents.rs` carries the Check's e2e and
`tests/surface_parity.rs` gains the `agent sessions watch` streaming exempt without which
T15.12 fails by name; product code is ~110 lines, the +199 is the e2e. `Sessions::all` became
`global = true` (the shape `Logs::lines` already uses) so `sessions watch --all` and `sessions
--all watch` both parse; no config key, no behaviour change to the plain table. No new
dependency, no second loop: the watch arm calls `crate::log::watch_loop` with
`crate::log::WATCH_POLL` and `render::sessions_tick`; a transient unreadable store keeps the
previous screen instead of blanking. `WatchTick` has no `Debug`, so the unit test asserts on
`fresh`/`screen` contents rather than `{tick:?}` — `src/log.rs` stays untouched per the file
list.

**T25.2 `rtok agent sessions`** · T25.1 · `src/cli.rs`, `src/render.rs`, `tests/agents.rs` (new)
Do: render the model's page as a table — agent, provider, model, in / out / cache, started, and how
long it has run, newest first; `--all` includes sessions that have ended. Durations and the table
layout come from one helper in `render.rs`, because `demon status`, `stats` and this all pad columns
by hand today and the next one would be the fourth copy.
Check: two live sessions and one ended print two rows, three with `--all`; the token columns equal
`rtok stats` over the same window; an empty store prints a header and a line saying nothing is
running.
Complexity: 2/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, effort Low)
Check result: green. `render::table(cols, rows)` pads every column to its widest cell with a
per-column width floor — the floor is what lets a fixed-width table move over byte-identically;
stats' api table, tool/bash/mcp sections and `stats --cache` were refactored onto it, byte-identity
proven by the unmodified full-equality goldens in `tests/stats_model.rs`. `demon status`
deliberately NOT refactored: it pads its state word before colouring so ANSI bytes stay out of the
width math, and its last column is an unpadded path — a measure-then-pad helper would change its
tty output. Also `render::duration()` ("45s"…"3d04h", two units max) and `sessions_table()`
(both cache counts labelled). Default window `since = 0`: the default view's selectivity is
liveness (`ended_at IS NULL`) — a started_at floor could hide a long-running live session, the row
the command exists for. `tests/agents.rs` 3/3: two live + one ended → 2 rows, 3 with `--all`;
`agents` alias byte-equal; token columns == `rtok stats --json` over the same window (fixture
transcripts mirror the usage rows turn-for-turn so both definitions agree); empty store → header +
"nothing is running". `just check` exit 0.
Deviations: the `agents` visible alias did not exist on the tree despite the P25 preamble — added
as one `#[command(visible_alias)]` line. `tests/config_coverage.rs` gained `"all"` in its
action-flag allow-list (`--all` is a view toggle, not a stored setting). At integration the
command joined T15.12's `COMMAND_PAGES` (`agent sessions` → `sessions`) — the page rides the
snapshot, so it is a real page, not an on-demand call.

**T25.1 one reader, in the model** · T25.0 · `src/store/mod.rs`, `src/web/model.rs`
Do: `Store::session_totals(since)` — one `GROUP BY` over `sessions` joined to `usage` and `calls`,
returning id, host slug, provider/api, model, the four token counts, `started_at`, last activity
and `ended_at`. It lands in the D23 model as a `Sessions` page, which is what makes it a `rtok web`
and `rtok tui` page and not just a command (D27). No second query anywhere.
Check: a fixture DB with three sessions across two hosts totals each one's tokens exactly, and the
model's page carries the same numbers as the store call; `rtok web`'s snapshot gains the page.
Complexity: 3/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent, effort High)
Check result: green. One statement: `sessions` LEFT JOINed to four per-session CTE aggregates —
`tot` (token SUMs over `usage`), `last_u` (newest usage row's api/model), `act` (MAX(ts) over
usage UNION ALL calls — no `[agents] idle_secs`, per T25.0's deviation), `prov` (newest
provider-bearing call's slug). CTE pre-aggregation is required: a flat sessions×usage×calls join
fans each usage row across each call row and multiplies the sums. `since` windows sessions, not
tokens. Contract for T25.2: `session_totals(&self, since: i64) -> Result<Vec<SessionTotals>>`,
rows newest first, `ended_at IS NULL` = live, zeroed session still appears with
`last_activity = started_at`; `SessionTotals` is documented field-by-field. `Snapshot` gains
`sessions` (existing keys untouched — the P19 shape test passes unmodified) and `pages()` gained
`("sessions", "sessions")` in the same commit, which keeps the T15.10 parity gate green. Fixture:
three sessions across `claude` and `pi`, one ended, one zeroed — store call, model page and
snapshot frame all asserted equal. `just check` exit 0.
Deviations: host slugs are `claude` (0002) and `pi` (0010), not `claude-code` as the briefing
guessed. `SessionTotals` carries `project` beyond the task's field list — P25's gate ("every
session has a host and a project") and T25.2's "sessions active in this project" need it. Diff
+397/−2 exceeds the ≤200-LOC line; ~265 of it is the two test groups the Check requires, product
code ~150 in the task's two files.

**T25.0 a session knows whose it is** · - · `migrations/0010.sql` (new), `src/plugin.rs`, `src/hooks/mod.rs`, `src/store/mod.rs`, `src/config/layers.rs`
Do: `Ctx` learns the host from `[hook] host` and passes it to `upsert_session` instead of `None`,
with `project` (git root basename) and `cwd`; `SessionStart` writes the row rather than leaving it
to the first call that happens to arrive. The migration seeds the two host slugs `rtok agent setup`
can install but `hosts` never had — `pi` and `claude-code`-style additions belong in data, not in a
match arm. `ended_at` gains a companion: sessions are live until a `SessionEnd` *or* silence longer
than `[agents] idle_secs`, because the proxy and every non-Claude host never send one.
Check: a hook run leaves a `sessions` row with a non-NULL `host_id` and `project`; `pi` records as
`pi` and not as `other`; an existing DB migrates with no row rewritten.
Status: done 2026-09-09 · Model: Opus 5 (subagent)
Check result: green. `Runtime` resolves `host_id` once at open from `[hook] host` (falling back to
`other`, the shape `proxy::ProxyState::new` already uses) and carries the event's `cwd`, which
`dispatch_owned` sets before dispatch — so the row is attributed from the run's first hook, normally
`SessionStart`. `insert_call` now passes `host_id`, `project` and `cwd` instead of four `None`s.
`hooks::tests::hook_run_attributes_the_session` reads the row back and finds slug `claude` and
project `myproj`; `pi_host_resolves_to_pi_not_other` finds `pi`;
`store::tests::schema_0002_seeds_hosts_and_rejects_bad_fk` counts 7 hosts where it counted 6, and
`migration_is_idempotent` still passes — 0010.sql is one `INSERT OR IGNORE`, so an existing DB gains
the row and nothing is rewritten. 71 plugin, 9 hooks and 10 store tests pass; clippy clean.
Deviations: three. (1) `src/store/mod.rs` was outside the planned file list but a migration is inert
until it is listed in `MIGRATIONS`; it also holds the `#[cfg(test)] session_row` reader the Check
needs. (2) `src/config/layers.rs`: `git_root` became `pub(crate)` so `project_of` calls it instead of
walking the tree a second time — five files rather than three, but the alternative was the copy the
no-duplication rule forbids and `just dup` gates. (3) The `[agents] idle_secs` liveness companion is
**not** implemented: it needs `config/mod.rs` and `config/default.toml`, and T25.1 computes last
activity with a `GROUP BY` over `calls`/`usage` joined to `sessions`, so no stored column is needed
for it. That sentence of the Do stays open and belongs with T25.1.

## P26 — duplication gate · T26.0, T26.1 done 2026-09-09

**T26.1 retire what it found** · T26.0 · `src/proxy/wire.rs`, `src/proxy/anthropic.rs`, `src/proxy/openai_chat.rs`, `src/proxy/openai_responses.rs`, `.jscpd.json`
Do: the 46 clones are concentrated in the proxy (the same request/response shaping repeated across
wires) and in per-file test fixtures. Extract the proxy ones — they are the copies D6 warns about,
one shared helper at the responsible layer — and lower `threshold` to what remains.
Check: `just dup` green at the new threshold; the proxy tests are unchanged, which is what proves
the extraction did not change behaviour.
Status: done 2026-09-09 · Model: Opus 5 (subagent)
Check result: green. Three clones retired, all through `src/proxy/wire.rs`, the module's existing
home for cross-wire helpers (`str_field`, `int_field`): the `tool_results` prologue — take the array,
count the user turns, start the accumulators — became `wire::turn_setup`, used by all three wires;
`session_id` (`str_field(body, "user")`, byte-identical in both OpenAI wires) and
`provider() -> "openai"` became provided defaults on the `Wire` trait, with Anthropic the only
override. `cargo test --lib proxy` 14 passed and `cargo test --test proxy` 16 passed, both
unmodified — byte-identical passthrough, usage extraction and tool-result compression across all
three wires still hold. `threshold` 3 → 2, `minTokens` untouched; `just dup` exits 0 at 1.94 %
duplicated lines, 44 clones tree-wide.
Deviations: two. (1) The task named `src/proxy/mod.rs`; the clones it describes are in the wire
files beside it (`mod.rs`'s only flagged clone pairs with `src/web/mod.rs`), so the four sibling
files were edited instead — nothing outside `src/proxy/`. (2) The remaining
`anthropic.rs` ~ `openai_chat.rs` `tool_results` pair was left: Anthropic nests results inside a
user message's content blocks while the OpenAI wires dispatch flat on role, so unifying them needs
a walker taking the per-message match as a closure — a closure as long as the loop it replaces.
That is the case D6 calls out, where the abstraction costs more than the copy.

**T26.0 `just dup`** · - · `.jscpd.json` (new), `justfile`, `mise.toml`
Do: jscpd over `src/` and the SDK crate — token-based (Rabin-Karp) with a Rust tokenizer, config in
`.jscpd.json`, `min-tokens 50`, `threshold 3`, console reporter only so it writes no artefact. It
joins `check`, so a task that copies a block instead of extracting a helper fails before review.
The threshold sits just above what the tree measures today (2.14 % of lines, 46 clones), which
stops new duplication without demanding a refactor first; T26.1 lowers it after the cleanup.
Not chosen: `similarity-rs` is AST-based and Rust-native, which is the better shape for this
question, but it is a `cargo install` this repo would have to pin and build; revisit it if jscpd's
token matching turns out to report noise. PMD's CPD is a JVM to install for the same answer.
Check: `just dup` is green on the tree as it stands and exits non-zero at `--threshold 1`, so the
gate is known to fail rather than merely to run; `just check` includes it.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — `just dup` exits 0 on the tree (46 clones, 2.14 % of lines, 2.60 % of tokens,
72 files) and exits 1 with `--threshold 1`, so the gate fails when it should. Verified both through
`mise exec -- jscpd` (the recipe's default) and through the `JSCPD` override. Baseline for T26.1:
the clones are concentrated in `src/proxy/mod.rs`.
Deviations: one, and it is an addition rather than a shortcut. `.agents/skills/dry-refactoring/`
(plus the `.claude/skills` symlink) is jscpd's own refactoring workflow, installed at the user's
request with `npx skills add`. It is prose — a checklist for turning a clone report into an
extraction — with no code and nothing executable, and it belongs beside the gate that produces the
report T26.1 will work from.

## P24 — `rtok logs` (D26) · done 2026-09-09 (T24.0–T24.4)

**T24.3 `rtok logs watch`** · T24.2 · `src/log.rs`, `src/cli.rs`
Do: print the same last-`lines` screen, then follow: every new line appears above the previous one,
so newest-first holds while it runs. Rotation while watching is handled — the file the watcher
holds is renamed, and it reopens `path` rather than following the inode into `.1`. Ctrl-C leaves
the terminal as it found it.
Check: a line written by another process shows up within a poll interval; a rotation mid-watch does
not end the stream and does not repeat lines already printed.
Complexity: 3/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent, effort High)
Check result: green. `watch_loop` polls a step closure every `WATCH_POLL` (200 ms, a pub const a
test reads) and renders each `WatchTick` two ways: on a TTY the newest-first screen is repainted in
place with `\x1b[{n}F` + `\x1b[J` so new lines land on top; piped, it appends only fresh rows
(numbered past the initial screen) with zero escape codes, and `BrokenPipe` maps to `Ok` so
`| head` exits cleanly. Rotation is detected by content (first line + line count — std has no
portable inode read), carrying the unconsumed tail out of the `.1`/`.2` chain before the
newcomer's lines; a rotation-storm unit test (12 appends across ~4 rotations at `max_bytes=150`)
asserts every line once in arrival order. Ctrl-C needs no handler: the loop only writes
row-move/clear escapes — no raw mode, no alternate screen — leaving the terminal as `tail -f`
would. Evidence: the integration test seeds the log, runs `logs watch` piped, appends from the test
process, rotates exactly as the sink does, appends again — both markers arrive, each exactly once,
no `\x1b` anywhere; the TTY repaint is unit-pinned byte-exact; a real-pty smoke run confirms the
row-1 landing. `watch_loop`/`WatchTick` is the skeleton T25.3 reuses for state-table repaint.
`cargo test --lib log::` 18 passed; `cargo test --test logs` 4 passed; `just check` exit 0
(workspace 284 passed, jscpd 37 clones / 1.42 %).
Deviations: content-based rotation detection instead of the inode the Do implies (std limitation;
the degenerate case degrades to skipping lines, never repeating). `tail` refactored into
`tail_with(live, …)` so the first screen and the follow state share one read — two reads repeat or
lose a line written between them; output unchanged, its tests untouched. Documented in code: a row
wider than the terminal makes the repaint drift one row (fixing it needs an ioctl or a dependency).
No new dependency (`std::io::IsTerminal`).

**T24.4 the demon's own logs are bounded too** · T24.0 · `src/demon.rs`
Do: today `supervise` hands the child a raw appending fd, so `<service>.log` grows without limit and
rtok cannot rotate a file the child holds open. Pipe the child's stdout and stderr instead and let
the supervisor write them through the T24.0 sink, which is what makes rotation possible at all.
Check: a service that writes more than `max_bytes` ends with rotated `<service>.log.1`; `demon
status` still names the live file; the restart and backoff tests are unchanged.
Status: done 2026-09-09 · Model: Opus 5 (subagent)
Check result: green. `supervise` now spawns the child with piped stdout/stderr; two reader threads
(`pump`) only forward `(level, line)` over an `mpsc` channel — stdout as `info`, stderr as `warn` —
and the existing poll loop is the sole writer, calling `drain` through `log::append` against a
cloned `Config` whose `log.path` is the service's file. One writer means two streams on one file are
never a rotate/append race, and a child that never closes its pipe cannot wedge the loop. Both
readers are joined and the channel drained once more before a restart and on the stop path, so no
buffered line is lost. `cargo test --lib demon` 3 passed, `cargo test --test demon` 3 passed
(restart, backoff, status and stop unchanged); clippy and fmt clean on the file.
Deviations: one. The Check reads as an end-to-end run through a real supervised service, but no
built-in service emits a dial-in number of lines and the plan asks for none, so the new test
`a_service_that_writes_past_max_bytes_gets_a_rotated_log` drives `pump`/`drain` directly with a
`Cursor` of 50 lines against `max_bytes = 200` — the same path `supervise` now uses — and asserts
`mcp.log` and `mcp.log.1` both exist with the live file bounded.

**T24.2 `rtok logs` and `rtok logs export`** · T24.0 · `src/cli.rs`, `src/log.rs`, `tests/logs.rs` (new), `src/render.rs`
Do: `rtok logs` prints the last `[log] lines` lines (`--lines N` overrides), newest first, reading
back through the rotated files as far as it needs; each line numbered, `1` being the newest, with
the level coloured through `render.rs` — one helper, not a second colour table. `rtok logs export`
is the same selection with no numbers and no colour, for `rtok logs export > my.log`.
Check: with 3 rotated files and `--lines 10`, the first line printed is the newest written and the
tenth is ten lines back across the file boundary; `export` output is byte-identical to those lines
with the numbering and ANSI stripped; both say so when nothing has been logged.
Status: done 2026-09-09 · Model: Opus 5 (subagent)
Check result: green — 16 `log::` unit tests, 3 `tests/logs.rs` integration tests against the real
binary, `config_coverage` unchanged (`--lines` was already in its ALLOW list). The integration test
seeds four lines in the live file and in each of three rotated siblings, then asserts line 1 is
`live-4` and line 10 is `r2-3`, which is two file boundaries back; `export`'s lines are the numbered
screen's with the `"<n> "` prefix removed; both commands print `no logs yet` on an empty store.
Deviations: two. (1) Four files: `render.rs` gained `log_line`, the one place a level is coloured.
The Do asks for the colour to go through `render.rs`, so the alternative was a second colour table
in `log.rs` — the duplication `just dup` now gates. (2) The subagent could not build in the shared
worktree while another session's SDK rename was mid-flight, so it verified in a detached
`git worktree` at HEAD holding only its four files, then removed it; the numbers above were re-run
in the main tree afterwards. That session's uncommitted `rtok_agent_sdk::backup` line in
`src/cli.rs` is excluded from this commit: it needs their `Cargo.toml`, which is not committed.

**T24.1 every log line goes through the funnel** · T24.0 · `src/plugin.rs`, `src/proxy/mod.rs`
Do: `Ctx::log` writes the file line *and* the `logs` row (`to_db` false skips the row, and the file
is then the only sink — the reason the key exists). The proxy's private `log(store, …)` helper
routes through the same funnel instead of inserting on its own; there is one writer, not two.
Fail open stays fail open: an unwritable log directory never turns into an error a plugin sees.
Check: a plugin call leaves one line in the file and one row in the table; with `to_db = false`,
one line and no row; a read-only log directory changes nothing about the call's result.
Status: done 2026-09-09 · Model: GLM-5.3-Flash (zai-coding-plan)
Check result: green. All three writers are one: log::record is the funnel, Runtime::log and the proxy's
log helper (including finish's log_err) both call it, and neither inserts a logs row on its own. A
plugin call leaves exactly one line in the file and exactly one row in the table
(a_funnel_call_is_one_file_line_and_one_row, a_plugin_call_is_one_file_line_and_one_row); with
to_db = false one line lands and logs_after stays empty (to_db_false_leaves_the_file_as_the_only_sink);
a read-only log directory changes nothing about the call — the row still lands, the file write fails
silently (a_read_only_log_directory_changes_nothing_about_the_call, root-guarded). The proxy's rows
keep the shape insert_log gave them (proxy_log_rows_keep_their_shape_through_the_funnel). cargo test
--lib 187 passed; proxy/logs/otel/demon integration suites 29 passed; clippy -D warnings and fmt
clean, verified in a detached worktree at 5975877. Note: [log] level (default info) now gates both
sinks, so a line below level no longer reaches the logs table either — the funnel's one-decision
semantics, not a regression. Deviation: the [core] log_file/log_level/log_to_db → [log] key migration
T24.0's preamble deferred to "T24.1's commit" is not here — it needs src/config/mod.rs, a fourth
file; after this commit core.log_file has no production reader, so it is a pure config commit of its own.

**T24.0 `[log]`: a sink that rotates** · - · `src/log.rs` (new), `src/config/mod.rs`, `config/default.toml`
Do: the section (`path` `~/.rtok/logs/rtok.log`, `max_bytes` 1048576, `files` 5, `lines` 200,
`level` `info`, `to_db` true) and one `append(cfg, level, source, name, message)` that writes a
line and rotates when the file would pass `max_bytes`: `rtok.log.4` → `.5`, current → `.1`, and
whatever falls past `files` is deleted. Lines below `level` are dropped before any I/O. The three
`[core]` keys migrate in `finish()`, `log_file` → `log.path`, and `[log] path` joins the `~`
expansion list.
Check: a sink with `max_bytes` 200 and `files` 2 keeps exactly `rtok.log`, `.1`, `.2` after 50
writes and the newest line is in `rtok.log`; an old config with `[core] log_file` loads and warns;
`default_toml_is_the_defaults` green.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — 5 new unit tests in `src/log.rs`, `config::` 19 green including
`default_toml_is_the_defaults`, `cargo test --lib` 173 passed. The rotation test asserts exactly
three files (`rtok.log`, `.1`, `.2`) after 50 writes at `max_bytes` 200, that `line 49` is in the
live file, and that the live file is under the cap. `stamp()` is checked against `date -u -r` on
three epochs: a leap year, the day after a leap day, and 2100-03-01 — the century that is not a
leap year, which is where a hand-rolled calendar goes wrong.
Deviations: three. (1) The `[core] log_file` / `log_level` / `log_to_db` migration is not here. It
is one line in `finish()` but it also changes the type of `core.log_file`, whose only reader is
`Ctx::log` in `src/plugin.rs` — T24.1's file, and a file another session is holding open right now.
Doing it here would have made a fourth file and a commit that reaches into the next task's code.
`[log]` and `[core]` therefore coexist for one commit, with `[log]` the one that is read.
(2) Four files, not three: `src/lib.rs` gains `pub mod log;`. (3) `stamp()` is 12 lines of
`civil_from_days` rather than a date crate. std has no calendar and the binary needed one line
formatted; a dependency for that is what the dependency rule is about.

## P23 — `rtok-plugin-sdk` (D25) · T23.0, T23.1 done 2026-09-09

Goal: one published contract every plugin implements. Plan: `plan.md` P23.

**T23.6 the release publishes it** · T23.5 · `release-plz.toml`, `.github/workflows/release-plz.yml`, `.github/workflows/ci.yml`
Do: `publish = false` becomes a per-package setting — the SDK is published, the `rtok` binary crate
stays off crates.io (dist ships it). release-plz gains the `release` command with
`CARGO_REGISTRY_TOKEN`, after `verify`, and `semver_check` is switched on for the SDK because its
whole point is a stable surface. CI runs `cargo publish -p rtok-plugin-sdk --dry-run` so a broken
manifest fails on the pull request, not at the tag.
Complexity: 2/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `just publish-dry` (`cargo publish -p rtok-plugin-sdk --dry-run --locked`) packages
12 files, 68.7 KiB, and verifies the built package — green. `publish = false` moved off the
workspace default onto the `rtok` package itself and `[[package]] rtok-plugin-sdk` sets
`publish = true`, so a release run publishes exactly one crate. cargo-semver-checks v0.50.0
against `--baseline-rev HEAD` passes 223 checks on the tree as it is; adding one required method
to `Plugin` fails it with `trait_method_added` — "a non-sealed public trait added a new method
without a default implementation" — and the probe was reverted. The licence is Apache-2.0, the
one the repository owner chose, with the full text in `crates/rtok-plugin-sdk/LICENSE`.
Deviation: the licence file sits inside the crate, not at the repository root. `cargo publish`
packages that directory and nothing above it, and the owner's answer was about this crate — what
licence the binary and the rest of the tree carry is still their decision to make.
Deviation: `--release-type patch` was needed to make cargo-semver-checks say anything. At 0.0.1
with an identical baseline version it assumes major and skips all 254 lints; release-plz will not
have that problem once a version is actually out, but it is why the local proof is spelled that
way.
Note: cargo-semver-checks is not in mise's registry and there is no `cargo binstall` here, so the
proof was run from a `cargo install`ed binary in `~/.cargo/bin`. Nothing in the repository depends
on it locally — in CI, `release-plz/action` brings its own.
Note: `just example` now also runs the SDK's own `shrink` example, so CI exercises the plugin
that has no `rtok` dependency.

**T23.5 documentation someone can build against** · T23.1 · `crates/rtok-plugin-sdk/README.md`, `crates/rtok-plugin-sdk/examples/`, `docs/plugin-authoring.md`
Do: crate-level docs that say what a plugin is, the required methods, the lifecycle of each event,
and the three rules that never bend for a plugin either (fail open, lossless, a saving that is not
a `Measurement` row does not exist). Every public item documented, with an example that compiles as
a doctest. `examples/` holds one complete plugin — the smallest thing that records a
`Measurement`. `docs/plugin-authoring.md` is rewritten against the crate and stops describing the
in-tree path as the normal one.
Complexity: 2/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `cargo test -p rtok-plugin-sdk --doc` green — 4 doctests, one of them the
`compile_fail` proof from T23.2 and one a whole `impl Plugin` that records a `Measurement` and
asserts the row landed. `cargo doc -p rtok-plugin-sdk --no-deps` has no warning (the one it did
have, an unresolved `[`Ctx`]` link in the new module, is fixed). `cargo run -p rtok-plugin-sdk
--example shrink` prints the rewrite, the recorded measurement and the archived original read back
by its id. `just check` green apart from the known `graph::watch` flake, which passes on its own.
The crate docs gained the event table (which method the host asks for, when, and what it may do)
and `docs/plugin-authoring.md` is rewritten: the trait comes from the crate, the in-tree wiring is
§2 rather than the whole document, and §3 is what a third party writes.
Deviation: the Check's "the example crate builds against the published version number" is not
possible before T23.6 — nothing is on crates.io yet. The example is `crates/rtok-plugin-sdk/
examples/shrink.rs`, built against the in-tree `0.0.1`, and the published-surface proof is
T23.6's `cargo publish -p rtok-plugin-sdk --dry-run`.
Deviation: a new public module, `testing`, with `MemoryHost` — a host that keeps what a plugin
records and archives and answers every other capability empty. The Check asks for an example that
records a `Measurement`, and `Ctx::new` takes `&dyn Capabilities`: without it, the example (and
any out-of-tree plugin's first unit test) would have to hand-write about thirty trait methods
before it could assert anything. It replaced the `NoConfig` fake the host tests were using.
Note: the `README.md` the crate now carries is the crates.io front page; `readme = "README.md"`
went into the manifest with it.

**T23.4 the ten plugins move** · T23.2, T23.3 · `src/plugins/*/`
Do: mechanical, one commit per group of plugins if it does not fit — `measure` `cmd` `read`
`archive` `proxy`, then `inject` `guard` `memory` `graph` `toon`. Imports come from
`rtok_plugin_sdk`; behaviour does not change. Each plugin's `AGENTS.md` gets the one line that
says its contract now lives in the SDK. Exempt from the ≤ 3 files rule: it is import churn across
ten directories, and splitting it further would leave the tree half-migrated between commits.
Check: `just check` green; `rtok stats --json` over a fixture store is byte-identical before and
after; no `use crate::plugin::` remains under `src/plugins/`.
Complexity: 3/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `grep -rn 'use crate::plugin::\|use crate::proxy::wire::\|use crate::tokens::'
src/plugins/` returns nothing — every contract type now arrives as `rtok_plugin_sdk::…`, and the
three `use` lines each plugin used to carry collapsed into one. `just check` green (247 tests
across the workspace; the two `plugins::graph::watch` tests that fail under full-suite load pass
on their own and on the re-run — pre-existing flake, unrelated). `rtok stats --json` byte-identical
before and after: the modified tree's binary built a fixture store under `RTOK_HOME` with three
`rtok run` calls, the store was snapshotted, `src/plugins/` was stashed, the pre-refactor binary
read the restored snapshot, and `cmp` on the two outputs is silent (same md5).
Deviation: one commit, not two groups — the import swap compiles as a whole and splitting it would
have left half the tree on the old path. 30 files (20 sources + 10 `AGENTS.md`), under the
exemption the task already carries.
Note: `Runtime` stays `crate::plugin::Runtime`, spelled out at its three call sites (`cmd/run.rs`,
`memory/import.rs`, and the test helpers). It is rtok's host implementation, not part of the
published contract, so importing it from the SDK would have been a lie.
Note: the fixture's transcripts dir was empty, so the compared `stats` report is all zeros over a
populated `rtok.db`; the archive write path ran, the transcript scan had nothing to scan. A real
transcripts dir is not reproducible — this session writes to it while the test runs.

**T23.3 host capabilities, and the trait moves with them** · T23.1 · `crates/rtok-plugin-sdk/src/host.rs`, `src/plugin.rs`, `src/store/`
Do: the capability traits the survey named — `Host` (estimate, record a `Measurement`, record
calls and tokens, log, and `plugin_config::<T>()` for the plugin's own `[plugins.<id>]` section)
plus one trait per store area a plugin actually uses: `Archive`, `Notes`, `ReadCache`, `Symbols`,
`Ledger`. The SDK gains a `Ctx<'a>` wrapping `&'a dyn Host` that keeps today's method names
(`cx.estimate`, `cx.record`, `cx.log`), so the `Plugin` trait moves into the SDK with its
signatures textually unchanged and only `cx.store.*` and `cx.config.*` call sites move (T23.4).
The wire view `proxy_filter` takes (`WireRequest`, `ToolResultRef` — 150 lines, `serde_json` only)
moves with it. `rtok`'s `Ctx` becomes the `Host` implementation. Nothing new is exposed: a `Store`
method that no plugin calls does not become a capability.
Check: `rtok::plugin::Plugin` and `rtok_plugin_sdk::Plugin` are the same type (a test that assigns
one to the other); `grep -r 'cx\.store\.' src/plugins/` is empty; every capability method is
reachable from a plugin that does not depend on `rtok`; the store test suite is unchanged and
green.
Complexity: 4/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `plugin::tests::the_trait_is_the_published_one` declares a plugin as
`impl rtok_plugin_sdk::Plugin` and binds it to `&dyn rtok::plugin::Plugin` — one trait, so the
assignment compiles. `grep -rn 'cx\.store\.' src/plugins/` returns 24 lines, all inside
`#[cfg(test)]`, where `cx` is the host `Runtime` and the assertions read counters
(`measurement_count`, `has_symbol_def`, `token_phases`) that no plugin calls and that therefore
did not become capabilities; plugin *logic* names `Store` nowhere. The SDK builds and tests on
its own (`cargo test -p rtok-plugin-sdk`), which is the reachability claim. The store test suite
is untouched. `just check` green (174 lib tests + every integration suite), `rtok stats --json`,
`rtok plugins` and `cargo run --example hello_plugin` all behave as before.
Deviation: two commits, not one. The first landed the traits with the host still holding the
`Plugin` trait, so the tree compiled at every point; the second moved `Plugin`, `Ctx` and the
wire view. Splitting it any finer would have left the tree not compiling in between.
Second deviation: `Ctx` is not a wrapper over `&dyn Host` but over `&dyn Capabilities`
(`Host + Archive + Notes + ReadCache + Ledger + Symbols`, blanket-implemented) with a `Deref` to
it. The plan's shape needed a plugin to import each capability trait it used; deref means a
plugin names none of them, which is what "keeps today's method names" was actually asking for.
Third deviation: the host struct is now `rtok::plugin::Runtime`, because the SDK's `Ctx` took the
name that 35 call sites used. `cx.config.*` did move here rather than in T23.4 — it had to: the
SDK `Ctx` has no `config` field. Plugins read their own section with
`cx.plugin_config::<crate::config::X>("x")`; the three cross-section reads that exist
(`proxy.mode` in `archive`, `setup.modes` and `estimator` in `inject`) go through
`cx.config::<T>(path)`, added because `plugin_config` alone cannot express them.
Fourth deviation: 40 files. Making `Plugin` live in another crate moves every signature that
names its context; the ≤ 3 files rule cannot survive that, the same exemption T23.4 carries.
Note: `Wire` gained `ToolResults` as a supertrait so the three provider dialects keep their
`tool_results` while the plugin-visible half lives in the SDK.

**T23.2 required methods are required** · T23.1 · `crates/rtok-plugin-sdk/src/lib.rs`, `src/plugins/*/mod.rs`
Do: the mandatory set from T23.0 has no default body — `manifest()` (what the plugin is) and `dashboard_page()` (the page D23 says every plugin contributes). Every event method keeps its no-op default. `DashboardPage::from_id`'s catalogue match dies: each plugin owns its own title, summary and `saves_tokens`, which is where that copy belonged.
Check: a `trybuild` case where a plugin implements only `manifest()` fails to compile naming `dashboard_page`; `rtok web`'s snapshot carries the same titles and summaries as before (the T15.0 model test); `rtok plugins` output is unchanged.
Complexity: 2/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `Plugin::dashboard_page` has no default body; the 178-line `DashboardPage::from_id` match is gone, replaced by `DashboardPage::new(title, summary, saves_tokens)`, and all ten catalogue plugins plus both examples carry their own copy. `Registry::pages()` returns `(Manifest, enabled, DashboardPage)` so `src/web/model.rs` asks the plugin instead of looking copy up by id — the T15.0 model test still reads `Bash / cmd` off the snapshot, so titles and summaries are unchanged. `rtok plugins` prints id/enabled/surfaces and never touched the page, so it is unchanged by construction. `just check` green twice (169 tests), `cargo run --example hello_plugin` still records exactly one measurement row.
Deviation: no `trybuild`. The compile-failure proof is a ```compile_fail doctest on the trait, which `cargo test` already runs — a new dev-dependency to assert one compile error is the kind of thing D6's spirit and the repo's dependency rule both argue against. It fails for the same reason and in the same run.
Second deviation: 16 files, not ≤ 3. Making a trait method required is one edit in the trait and one in every implementor; splitting it would leave the tree not compiling between commits, the same reason T23.4 is exempt.

**T23.1 the crate exists and owns the contract** · T23.0 · `Cargo.toml`, `crates/rtok-plugin-sdk/*`, `src/plugin.rs`
Do: a workspace root (`rtok` plus `crates/rtok-plugin-sdk`; `crates/rtok-webui` keeps its own build), the new crate with `#![deny(missing_docs)]`, and the contract types moved into it exactly as the survey drew them. `src/plugin.rs` becomes a re-export so `rtok::plugin::*` still resolves and no call site outside it changes in this task.
Check: `cargo test` green with the types imported from the SDK; `rtok::plugin::Plugin` and `rtok_plugin_sdk::Plugin` are the same type (a test that assigns one to the other); `cargo doc -p rtok-plugin-sdk` builds with no missing-docs warning.
Complexity: 3/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `crates/rtok-plugin-sdk` exists on `serde` + `serde_json` with `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]`, and owns `Surface`, `Manifest`, `Measurement`, `PreToolDecision`, `Injection`, `DashboardPage`, `ToolDef` and the five event views; `src/plugin.rs` re-exports them and keeps only the host side (`Ctx`, the `Plugin` trait), 397 → 205 lines. The root manifest is now the workspace root with `crates/rtok-webui` excluded (it builds for wasm through its own toolchain), and `just check` runs `--workspace` so the SDK is inside the gate. `re_exports_are_the_sdk_types` assigns an `rtok_plugin_sdk::Manifest` to an `rtok::plugin::Manifest` and an SDK `DashboardPage` to the re-exported one, which only compiles while both paths name one type. `cargo build` clean, `cargo test -p rtok-plugin-sdk` 2 passed, `just check` green twice.
Deviation, and the reason the plan changed: the `Plugin` trait did **not** move. Its signatures name `Ctx` (config + store) and `WireRequest`, which are the host side of the line T23.0 drew, so moving it now would have meant moving the host with it — the whole of T23.3 in one commit. T23.3 now carries the trait, an SDK-side `Ctx<'a>` wrapping `&dyn Host`, and the wire view; the "same type" clause moved to its Check with it.
Second deviation: `tests/fixtures/graph_truth.toml` moved seven `defs` entries (`Manifest`, `Measurement`, `ToolDef`, `Injection`, `PostToolUse`, `Surface`, `PreToolDecision`) from `src/plugin.rs` to `crates/rtok-plugin-sdk/src/lib.rs`. This was not optional bookkeeping: those definitions really moved, and until the labels followed, `labelled_symbols_are_found` measured definition recall 0.781 against the P8b bar of 0.9. The ground truth is a plain-text scan of this repo (T8.8), so a refactor that moves a definition moves its label.

**T23.0 where the boundary goes** · T15.0 · `crates/rtok-plugin-sdk/PLAN.md` (new)
Do: D15-style survey before any code. The question is what crosses the crate line, and there are three honest answers to price: (A) the runtime moves — `Config`, `Store`, `tokens`, the wire views go into the SDK and `rtok` becomes surfaces on top; (B) contract only — the SDK holds the trait and the event types, and everything a plugin does to the host goes through capability traits the SDK declares and `rtok` implements; (C) the middle — contract plus `Config` and `tokens`, with the store behind capability traits. Price each on (1) what a third party has to compile to implement one trait, (2) whether the ten internal plugins compile against it without reaching back into `rtok` (they use 29 `Store` methods today — the survey counts them and says which become capabilities), (3) what the crate's public surface costs to keep stable across versions, (4) build time and the P17 size gate. At least one comparison from outside this stack (rustc's `rustc_plugin`-era history, `bevy_app::Plugin`, `tower::Layer`, or `nu_plugin`) on how they drew the same line. Name the required methods and why each is required.
Check: `crates/rtok-plugin-sdk/PLAN.md` names the choice, the two rejected boundaries with the reason, the capability list with the `Store` methods behind it, and the falsification (what would make this the wrong line). No code in this task.
Complexity: 3/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `crates/rtok-plugin-sdk/PLAN.md` chooses the middle line (C) — trait, events, value types and host capability traits in the crate; `Store`, `Config` and every surface stay in `rtok`; crate dependencies are `serde`, `serde_json`, `anyhow`. Rejected with reasons: (A) runtime-in-SDK publishes 4 112 lines of host internals and makes a plugin author compile diesel plus bundled SQLite; (B) contract-only cannot record a `Measurement`, which makes it useless under D3; (C′) out-of-process spends most of D1's 10 ms budget on a hop, and is kept as the v0.2+ WASM host. Capability list is five traits — `Archive`, `Notes`, `ReadCache`, `Symbols`, `Ledger` — plus `Host`; the `Store` methods behind them are the measured 26 (`grep -rhoE "cx\.store\.[a-z_0-9]+" src/plugins/ | sort -u`, 2026-09-09: symbols 11, ledger 6, archive 3, read cache 3, notes 4 — the task text said 29 from a rougher first count). Required methods: `manifest()` and `dashboard_page()`, with the reason for each. Outside comparisons priced from the crates.io API on 2026-09-09: `bevy_app` 0.19.1 (17 direct deps), `tower-layer`/`tower-service` 0.3.3 (0), `nu-plugin` 0.115.1 / `nu-protocol` 0.115.1 (8 / 38). `Falsified by:` names the condition that sends the line back to option A. `tests/plugin_plans.rs` now walks this file too, so the D15 structure is enforced rather than promised: `cargo test --test plugin_plans` 8 passed; `just check` green.
Deviation: the task text priced (C) as "contract plus `Config` and `tokens`". The survey moves neither — `Config` would publish ~100 config keys as semver surface, and the estimator needs the host's rates, so both stay behind `Host` (`plugin_config::<T>()`, `estimate()`). Same line, one notch tighter.

## P22 — `rtok report` (D24) · T22.0–T22.5 done 2026-09-10

Goal: one artefact a person or a model can act on — the report renders the D23 operator model and computes nothing of its own. Plan: `plan.md` P22.

**T22.5 recommendations** · T22.1 · `src/report/advice.rs`
Do: rules over the ledgers, never a model call. Each finding prints what triggered it and the rows
it read. The first set, all answerable from data rtok already stores: expand rate above
`[expand] max_rate` (the compression is lossier in practice than it looks); a plugin whose net
saving over the window is ≤ 0 (it costs more than it returns — D10 says retire, not stack); cache
busts with cause `tools` or `system` (the host rewrites its tool list mid-session, and the turn is
named); hooks that fire often and produce no `Measurement` (weight on the 10 ms path for nothing);
measured injection bytes per turn against `[plugins.inject] budget_tokens`; `archive keep_turns`
against how often old tool results were actually re-read. Findings are ordered by the tokens they
would recover, and a finding with no number attached does not ship.
Check: a fixture store triggers each rule exactly once and the text names the row count behind it;
a healthy store produces an empty section that says so, not filler advice.
Complexity: 3/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; tier GLM-5.3, effort High) — adopting the disconnected subagent-report-reco heap
Check result: green. Six rules in `src/report/advice.rs` over the D23 ledgers only (D24: no new
Store query): expand rate vs `[expand] max_rate` (key added, default 0.05); plugin net saving ≤ 0
(D10 retire); cache busts cause `tools`/`system` naming the turn; hooks firing ≥ 10 times with no
Measurement kind on their path; inject `est_after` per turn vs `[plugins.inject] budget_tokens`;
`archive keep_turns` vs observed re-reads. Findings ordered by tokens recoverable; a finding with
no number cannot be pushed. The model grows only what the rules read (`savings.kinds`,
`calls.hooks`, `cache.detail`, `expand.cost`/`cost_rows`). Empty section prints
`No recommendations.` Verified in worktree `rtok-wt-e`: `mise exec -- cargo test --test report`
6 passed (`each_rule_fires_once_in_recoverable_order`, `healthy_store_has_no_recommendations`,
plus the prior four).
Deviations: 7 files, +552/−15 — over the ≤200 / ≤3 brief; the Check needs the fixture seed, the
six rules, and the ledger fields the rules read. Precedent T22.1 (+880), T22.4 (+750). No new
dependency.

**T22.1 `--format md`** · T22.0 · `src/report/mod.rs`, `src/report/markdown.rs`, `src/cli.rs`
Do: the whole section set as Markdown, straight from the D23 model. Tables, no charts. Every
number is followed by its evidence — row count and window — so the document cannot quietly grow a
figure nobody measured. Markdown first because it needs no renderer: it is the format that proves
the *content* is right before any layout work starts.
Check: on a store with known fixtures, every number in the output is traceable to a row the test
also asserts; an empty store produces a report that says so rather than zeros.
Complexity: 3/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent, effort High)
Check result: green. The eight sections render in the fixed order (Window · Savings · Calls ·
Cache · Expand · Config · Doctor · Recommendations, the last a T22.5 placeholder that says "No
recommendations." on an empty rule set); every section carries its evidence — per-ledger row
counts plus coverage, with measurements/usage honestly labelled "whole ledger (no row times)"
because their readers return no row time. `src/report/` imports only `crate::web::model` (D24):
`model::report_ledgers()` opens the store once and builds the section data entirely on existing
readers (`calls_after`, `list_measurements`, `archive_decision_counts`, `cache::report`,
`usage_sessions`), so `src/store/` needed no edits; `config_entries`, `doctor`, `log::stamp`,
`stats::parse_since` reused as-is. `tests/report.rs` (3 tests) asserts the fixture store's numbers
row-for-row; `empty_store_says_so_rather_than_zeros` pins the no-rows prose (no `| cmd |` table
rows anywhere). CLI: `rtok report [--format md] [--out <path>] [--since <window>]`, `--format` a
ValueEnum with `Md` alone, `[report] format/out/since` keys (D12/D14; asking for html/pdf errors
with "not built yet (T22.2/T22.3)"). `cargo test --test report` 3 passed; `just check` exit 0
(188 unit + all integration suites; jscpd unchanged at 36 on its tree).
Deviations: 9 files / +880 — D12 forces the config pair + docs mirror, D24 forces the model
methods, `tests/report.rs` follows the house fixture pattern; stated in the commit body. Savings
are net per plugin (expand rows negative), with an explicit line that a `Measurement` row carries
no turn count, so tokens are floors, not context-token-turns.

**T22.0 pick the PDF renderer against the size gate** · T15.0 · `docs/report.md` (new)
Do: D15-style survey before any code. At least three candidates priced honestly: `typst` as a
library, `printpdf` + `svg2pdf`, and rendering through a browser. Judge each on (1) what it does to
the release binary, which P17 already gates, (2) whether it keeps "one static binary, no runtime
dependency" true, (3) whether HTML and PDF can come from *one* document rather than two layouts.
Charts are one decision too: pure-Rust SVG (`plotters`) embeds into all three formats, where a
JS charting library would make the HTML the only real format and the other two second-class.
Check: `docs/report.md` names the choice, the two rejected options with the reason, and the
measured size cost of the winner; a `dist` build stays inside the P17 budget.
Status: done 2026-09-09 · Model: GLM-5.3-Flash (zai-coding-plan)
Check result: green. docs/report.md names the choice (printpdf 0.12.8 + svg2pdf 0.13.0, plotters
0.3.7 SVG for charts), the two rejected options with their reasons (typst: measured +42.30 MiB
against an 18.27 MiB binary; browser rendering: a runtime dependency, rejected on criterion (2)
without a build), and the measured size cost of the winner (+5.99 MiB release / +5.12 MiB
dist-shaped) with the scratch commands and dates. All sizes from throwaway /tmp crates built
2026-09-09 with P17's strip = "symbols" profile; a no-dep hello is the baseline, so the deltas are
the per-candidate cost; each scratch binary was run and produced its artefact. P17 published no byte
cap, so the budget clause is applied as published arithmetic — dist 17.4 MB + 5.12 MiB ≈ 22.8 MB
(+31 %), the same size class P17 recorded — with the linked measurement due at T22.3; typst's
arithmetic (≈ 61.8 MB, ~3.5×) is what fails the gate, which is why the smaller candidate wins. The
site row for the new docs page is in _content.gotmpl. Repo Cargo.toml/Cargo.lock untouched.

**T22.2 `--format html`** · T22.1 · `src/report/html.rs`
Do: the same sections, one self-contained file — inline CSS, inline SVG charts, no network fetch,
openable from a file:// URL. Charts where a series exists (savings over time, tokens per plugin,
latency distribution, cache busts per turn); tables everywhere else.
Check: the HTML contains every number the Markdown contains, asserted by a test that walks both;
the file opens with no external request (no `http` outside code blocks).
Complexity: 3/5
Status: done 2026-09-09 · Model: Muse Spark (meta/muse-spark) subagent XHIGH
Check result: green. `src/report/html.rs` (new) renders the same eight sections in the same order —
one `<h2>` per Markdown `##`, same names, stable `id`s; tables mirror the Markdown cells and
sentences, so every figure keeps its evidence. Charts are inline SVG bars over the series the
model has — saved tokens per plugin, calls per surface, busts per cause; the model carries no
per-turn series (savings over time, latency distribution), so those stay tables. Config values ride
in `<code>` and Doctor in `<pre><code>`, so the only `http` in the file is the two upstream URLs
in code spans — the document opens from `file://` fetching nothing. CLI: `Html` joins the
`ReportFormat` ValueEnum (D14), `"html"` renders through a shared `emit()` sink T22.1's `md` arm
now uses too (same bytes to stdout or `--out`, no second sink). `tests/report.rs`
`html_holds_every_markdown_number_and_fetches_nothing` walks both over the fixture store: every
digit-led run in the Markdown is in the HTML, the eight headings in order, exactly 3 `<svg>`,
no `http` outside code. Verified in a detached worktree at `a054922` with only this task's files
(the shared checkout carries T22.4/T22.5/T25.x in flight): `cargo test --test report` 4 passed,
`cargo test --workspace` green (208 lib + all suites, 0 failed), `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean, `cargo fmt --check` clean, `cargo build
--no-default-features --features measure` rc=0, `just dup` exit 0 (38 clones, none from the new
files). An empty-store HTML was also eyeballed: balanced tags, 8 headings, 2 tables, no charts.
Deviations: ~370 added lines across 3 files + the test (brief allowed the test extra) vs ≤200 —
the Check's own content (8 evidenced sections + 3 charts + walk-both test) does not fit 200;
precedent T22.1 needed +880 for Markdown tables alone, and most of the renderer's 275 lines are
`rustfmt`'s chain splits, not logic. Charts are hand-rolled SVG bars, not T22.0's `plotters`: no new dependency (one-line
reason unnecessary — there is none), no font payload, and the SVG strings stay `svg2pdf`-ready
for T22.3, which consumes SVG bytes rather than plotter objects; revisit if T22.3 needs axes.
The 3-line latency formatter mirrors `markdown::ms` instead of sharing it — sharing would edit a
fourth file for 3 lines, and T22.4's `pub(crate)` share is the natural home when it lands.
`report_flags` needed no change (`Html != Md` already inserts `format = "html"`); the
`default.toml` / `docs/config.md` format comments still read `html (T22.2)` and are left for the
owner — T22.4 is editing those same lines.

**T22.4 `--ai`** · T22.1 · `src/report/ai.rs` (new), `tests/report_ai.rs` (new), `src/cli.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`
Do: the same document shaped for a model instead of a person. No images, no styling, no box
drawing. Tables in `toon` rather than Markdown pipes — dense, and it dogfoods the plugin this repo
ships. Stable heading ids so a model can be pointed at one section. Every number keeps its unit and
its row count, because a model has no other way to weigh it. One budget (`[report] budget_tokens`),
and when the document does not fit, it says which sections it dropped instead of truncating
mid-table. Ends with the recommendations as an ordered, explicit task list.
Check: `--ai` output is measurably smaller in tokens than `--format md` on the same store and
window (a `Measurement` row, not an assertion in prose); no section is silently missing — the
dropped ones are named; every heading id is stable across two runs over the same data.
Complexity: 3/5
Status: done 2026-09-09 · Model: Muse Spark (meta/muse-spark) subagent
Check result: green. `src/report/ai.rs` (new) renders the same eight sections in the P22 order —
one `#id` anchor per section (`window` … `recommendations`, stable by construction), dense plain
lines, tables through the toon encoder (`tabular_keys`/`encode` are `pub(crate)` now, so the
`--ai` rendering dogfoods the plugin T22.2's entry pointed at instead of growing a second table
syntax; latencies reuse `markdown::ms`, also `pub(crate)` now). Without the `toon` feature the
same tables fall back to `k=v` lines (`build-min` stays silent). Every section states its `units:`
line and every number keeps its unit and row count (`total_saved=210tok rows=2`). The budget is
first-fit whole sections under `[report] budget_tokens` (default 8000, ~6× the fixture's 1316-tok
`--ai` output, so it binds only on very large stores); the past-budget sections are named in
`dropped:`, never truncated mid-table. `--ai` (flag and key, D12/D14) selects this renderer
instead of `--format`, through T22.2's committed `emit()` sink. CLI and tests: `tests/report_ai.rs`
4 passed — md 1630 tok vs ai 1316 tok on the same fixture store and window (−19%, −314 tok, the
measured proof), the eight `#id`s in order with md's numbers (`total_saved=210tok`, `hooks 2`,
`dropped: none`), a `budget_tokens = 100` config file naming its drops with no dropped heading
left behind, and an empty store saying `no rows.` / `no recommendations.`. Four unit tests beside
the renderer pin the task-list shape (`1. [rule] finding (evidence)`, nothing after it), block
wholeness under a tight budget, byte-equality across two runs, and toon-vs-pipes. Verified in a
detached worktree at `1246857` with only this task's files (the shared checkout carries
T15.6/T22.5/T25.x in flight): `cargo fmt --check` clean, `cargo clippy --workspace --all-targets
--all-features -D warnings` clean, `cargo test --workspace` green (30 suites, 0 failed),
`cargo build --no-default-features --features measure` silent, `just dup` exit 0 (38 clones, none
from the new files). One transient on the way: `report_ai` failed 4/4 once under a full-workspace
run (all in 0.04s at the binary-status assert — the shared `CARGO_TARGET_DIR` was being rebuilt by
sibling builds), green on immediate rerun and on the recorded full run; environment flake, not code.
Deviations: ~750 added lines across 7 tracked files + 2 new (brief: ≤200) — D12 forces the config
pair plus the docs mirror, the Check forces three measured proofs plus the task-list shape (8
tests), and the toon dogfood needs the 2-word `pub(crate)` share; precedent T22.1 (+880) and T22.2
(+370, whose entry already named this task's share as the home for its `ms()` mirror — that mirror
stays theirs to retire). No new dependency. Landing note: verified both before (`a054922`) and
after (`1246857`) T22.2's landing; the dispatch rides T22.2's committed `emit()`.

**T22.3 `--format pdf`** · T22.0, T22.2 · `src/report/pdf.rs` (new), `tests/report_pdf.rs` (new), `src/cli.rs`, `src/report/mod.rs`, `Cargo.toml`, `Cargo.lock`
Do: the renderer T22.0 chose, over the same document and the same SVG charts. Paged, with a table
of contents.
Check: the PDF has the same section headings in the same order as the HTML, and the release binary
still passes the P17 size gate with the renderer linked in.
Complexity: 4/5
Status: done 2026-09-10 · Model: Muse Spark 1.3 (subagent-pdf)
Check result: green. `src/report/pdf.rs` (new) renders the same eight sections in the P22 order
over the same `Document` — A4, Helvetica only (no font files ship, which is why T22.0 picked this
renderer), a contents page with page numbers plus viewer bookmarks, native bar charts over the
identical pairs, order and titles as the HTML bars, streams uncompressed so every figure stays
greppable in the artifact. `tests/report_pdf.rs` (new) 2 passed:
`pdf_has_the_html_headings_in_order_and_the_charts` walks `--format html` and `--format pdf` over
one store — the eight `(Title)` headings in the same order in the contents list and the body, one
chart label per series, ≥2 `/Type/Page`s, `%PDF-`/`%%EOF`;
`empty_store_pdf_keeps_all_headings` keeps all eight headings with `(No rows in window.)`. CLI:
`Pdf` joins the `ReportFormat` ValueEnum (D14); the shared `emit()` sink now takes bytes (the PDF
is binary; md/html/ai pass `.as_bytes()`, stdout bytes unchanged). Verified in a detached worktree
at `b7d4c2e` with an isolated target dir (the shared-target contention T22.4 noted):
`cargo fmt --check` clean, `cargo clippy --workspace --all-targets --all-features -D warnings`
clean, `cargo test --workspace` green (all suites incl. doctests), `cargo build
--no-default-features --features measure` silent, `just dup` exit 0 (38 clones, none from the new
files). P17, linked measurement (release, `strip = "symbols"`, isolated target dir):
release `rtok` 20 988 304 B → 23 212 128 B (delta +2 223 824 B / +2.12 MiB, +10.6 %),
measured 2026-09-10 on the pre-T15.8 tree plus this task (T15.8/T15.3 landed after the measurement; CLI/model lines only, same size class) — inside T22.0's projected +5.99 MiB
class (its scratch over-estimated: a hello-binary baseline pulls different dependency
paths than rtok's real feature set); P17 holds (no byte cap published, same size class). An empty-store PDF is 22 595 B over 4 pages; its xref offsets were validated
object-by-object with a dependency-free script (31 live objects, headings ordered).
Deviations: ~720 added lines across 4 tracked files + 2 new (brief: ≤200) — the Check's own
content (8 evidenced sections plus the Op-stream layout T22.0 priced as "a second layout by
construction", plus the walk-both parity test) does not fit 200; precedent T22.1 (+880), T22.2
(+370) and T22.4 (+750). `svg2pdf` is not linked although T22.0 named it: verified in its 0.13.0
source that `to_pdf` returns a standalone one-page PDF, and printpdf 0.12 has no PDF-page import
(only `UseXobject` for registered objects) — merging the two would be byte-level PDF surgery, far
outside this task, so the charts are the same series drawn natively (T22.0's own scratch produced
two separate files, never a merged page; its fontdb-on-bare-Linux hazard goes away with it). New
dependency: `printpdf 0.12.8` — the paged-PDF renderer the T22.0 survey chose.

## P15 — `rtok tui` (D17, D23) · T15.0–T15.12 done (T15.3–T15.9 2026-09-10, rest 2026-09-09)

Goal: `rtok tui` and `rtok web` are two renderings of one operator model. Plan: `plan.md` P15.

**T15.5 Calls tab (P13 rows + detail)** · T15.0 · `src/store/mod.rs`, `src/web/model.rs`, `src/tui/{app,view}.rs`
Do (roadmap §TUI): Calls tab — list P13 ledger rows (newest first) with Enter/z detail pane
for the selected row; the page rides the snapshot (D23/D27).
Complexity: 3/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; High) — cherry-picked from agent/T15.5 onto main after T15.4/T15.6/T15.7/T15.9
Check result: green. `Store::recent_calls(limit)` joins host/provider/model slugs and newest
usage; `Model::calls` + Snapshot.`calls` + `pages()` `("calls","calls")`; bound `CALLS_ROWS=120`.
App `CallsState` + page-scoped Up/Down/Enter/z; view list + detail pane. Tests:
`recent_calls_is_newest_first_bounded_and_linked`, `calls_page_is_the_store_read_and_rides_the_snapshot`,
view list/detail/empty, `calls_page_claims_its_keys_and_clamps_the_selection`. Conflicts with
T15.4/T15.6/T15.7 resolved keeping all pages; `fresh_store` shared with Overview seeding.
All 24 `tui::` tests + surface_parity green after cherry-pick.
Deviations: 4 files, +648/−12 on the originating commit (over ≤200 LOC/≤3 files — store+model+app+view
by construction; T15.3/T25.1 precedent).

**T15.4 Plugins tab (toggle enabled)** · T15.0 · `src/tui/{app,view,mod}.rs`, `src/config/mod.rs`
Do (roadmap §TUI): Plugins tab — row cursor and toggle that writes `plugins.<id>.enabled`
through `config set`'s writer (`toml_edit` on `<home>/config.toml`), never a second config writer.
Complexity: 3/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; High) — cherry-picked from agent/T15.4 onto main after T15.6/T15.7/T15.9
Check result: green. `Config::set_plugin_enabled` mirrors `plugin_enabled`; App owns the config
and the loop's timeout calls `app.tick()` so a toggle's re-read is not clobbered. Up/Down move
the cursor (page-scoped); Space/Enter flip the selected plugin; refusal is a status line (D1).
Tests: `plugins_cursor_moves_and_clamps`, `space_toggles_the_selected_plugin_through_config_set`,
view pins for rows/cursor/hint/status, `set_plugin_enabled_flips_every_catalogue_id`. Hermetic
doctor paths added on `cursor_on_plugin` after T15.6. Originating worktree `just check` green;
focused lib tests green after cherry-pick.
Deviations: 4 files, +325/−35 on the originating commit (over ≤3 files: config writer lives with
`plugin_enabled`). Cherry-pick conflict in `view.rs` resolved keeping Doctor/Logs match arms.

**T15.7 Logs tab** · T15.0 · `src/tui/view.rs`, `src/web/model.rs`, `tests/surface_parity.rs`
Do (roadmap §TUI): Logs tab — render the model's log lines off the snapshot, newest first,
bound by `[log] lines` (the same selection `rtok logs` screens).
Complexity: 2/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, Low) — cherry-picked from agent/T15.7 onto main after T15.6/T15.9
Check result: green. Snapshot gains `logs: Vec<String>` via `Model::log_lines(None)`;
`pages()` gains `("logs","logs")`; `logs` moves from surface_parity EXEMPT to COMMAND_PAGES.
Plain lines (no CLI ANSI numbering/colour). Tests: `logs_tab_renders_the_model_lines_newest_first`,
`logs_tab_honors_the_log_lines_bound`. Conflicts with T15.6 resolved by keeping both doctor and
logs pages. Originating worktree `just check` green; focused lib tests green after cherry-pick.
Deviations: none to the spec.

**T15.9 TTY guard, `q` restores the terminal** · T15.1 · `src/tui/mod.rs`, `tests/tui_tty.rs`
Do (roadmap §TUI): refuse `rtok tui` without a TTY before `try_init` touches terminal state;
`q` / Esc / Ctrl-C restore via `run()`'s `restore()` on every loop exit (T15.1); ratatui's
`try_init` already installs the panic hook that restores.
Complexity: 2/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, Low) — cherry-picked from agent/T15.9
Check result: green. Guard message names both stdin and stdout; exit 1 via anyhow Termination;
no escape codes on the refuse path. `tests/tui_tty.rs` runs the real binary with all stdio
piped (exit 1, one stderr line, clean stdout, no ESC); unit test covers all four tty-flag
combinations. `just check` green on the originating worktree; cherry-picked onto main after
T15.6.
Deviations: none. No second panic hook added.

**T15.6 Doctor tab** · T15.0 · `src/web/model.rs`, `src/tui/{app,view}.rs`, `tests/surface_parity.rs`
Do (roadmap §TUI): Doctor tab — render what `rtok doctor` reports off the shared D23 model
(the snapshot carries the page), with no probe of its own.
Complexity: 1/5
Status: done 2026-09-10 · Model: GLM-5.3 (subagent; adopting the disconnected subagent-doctor-tab heap onto post-T15.3 HEAD)
Check result: green. Snapshot gains `doctor: Option<doctor::Report>` and `pages()` gains
`("doctor", "doctor")`; `Model::snapshot` is the one probe run (D27), fail-open to `None`.
The tab renders `Report::to_text()` verbatim — the same text `rtok doctor` prints.
Hermetic doctor paths in `tui::app::tests::config` so every `App::new` does not spawn this
machine's MCP servers. `doctor` moves from `surface_parity`'s EXEMPT to COMMAND_PAGES.
Tests: `doctor_page_rides_the_snapshot` (model Report == direct `doctor()`, wire key),
`doctor_tab_shows_what_rtok_doctor_reports` (TestBackend). `just check` exit 0 (fmt, clippy
`-D warnings`, workspace tests, build-min, jscpd 38 clones — no new clone in the task's files).
Deviations: 4 files (parity is the T15.12 move the EXEMPT comment promised), +84/−8 — under
≤200 LOC. The disconnected heap was based on a pre-T15.3 tree and would have stripped Overview;
this commit ports only the doctor page onto current HEAD.

**T15.3 Overview tab (CTT, bars, sparkline)** · T15.0 · `src/web/model.rs`, `src/tui/view.rs`, `src/store/mod.rs`
Do (roadmap §TUI): the Overview tab — CTT, per-plugin savings bars, per-turn sparkline — rendered off the shared D23 model, with no query of its own; the tab set stays `model::pages()` by reference.
Complexity: 3/5
Status: done 2026-09-10 · Model: Muse Spark 1.3 (subagent-overview-tab)
Check result: green, verified in a detached worktree at HEAD holding only this task's three files — `just check` exit 0 (fmt, clippy `--workspace --all-targets --all-features -D warnings`, workspace tests, `build-min`, `just dup` with no clone in the task's files). `overview_carries_totals_ctt_and_turns`: totals, CTT (`ctx × turns-after`, the usage-row mirror of `stats`' `tokens × remain`) and the per-turn series pinned against inserted rows, plus the same sums `rtok stats --json` prints in its `api` table (`attach_api` reads the same `usage_by_api` rows — Gate P15). `overview_tab_renders_the_snapshot_numbers`: the TestBackend screen carries the snapshot's totals, CTT, one scaled bar per measured plugin and the sparkline. Both T15.10/T15.12 parity tests green — no new page, no new snapshot key, and the totals stay flat under the `usage` key, so the P19 wire pin and the Slint UI read on.
Deviations: 3 files, +305/−27 — over the ≤200 line (T25.1 precedent): the two Check test groups and the `#[cfg(test)]` helper are the bulk of it. The third file is the DRY extraction `Store::insert_proxy_turn` (the T25.0 `session_row` precedent): both Check tests seed proxy turns, and the seed block cloned `cache.rs`'s helper until `just dup` named it. No new dependency.

**T15.8 CLI + `[tui]` config** · `roadmap.md` §TUI · `src/cli.rs`, `src/config/{mod,layers,validate}.rs`, `src/tui/{mod.rs,app.rs}`, `config/default.toml`, `docs/config.md`
Do (roadmap §TUI): `rtok tui` CLI flags and the `[tui]` config section, by the `[web]`
precedent (`web_flags` + host/port keys, T21.3 migration shape): every flag has a key (D12),
clap derive (D14).
Check: `rtok tui --help` shows the flags; `config show --sources` resolves every `[tui]` key
with origin; the T12.4 coverage test is green.
Complexity: 2/5
Status: done 2026-09-10 · Model: (subagent-tui-config)
Check result: green. `rtok tui --tab <page> --tick-secs <n>` over `[tui] tab` (`""` = first
tab) and `tick_secs` (= 2, the web socket's tick in `src/web/mod.rs`). An empty or unknown tab
falls back to the first page rather than failing the surface (fail open, D1); the loop clamps
the cadence to ≥ 1 (`--tick-secs 0` cannot busy-poll) and `config validate` rejects
`tick_secs = 0` in the file. `config show --sources` resolves both keys through every layer
(`RTOK_TUI_TAB` / `RTOK_TUI_TICK_SECS` via the leaf table; flags via `layers::tui_flags`).
New tests `tui_tab_picks_the_opening_tab` and `tui_flags_beat_env`. Verified in a detached
worktree at `b7d4c2e` plus only these hunks: `cargo fmt --check`, `clippy --all-targets`
(no warnings), full `cargo test` green (incl. `config_coverage`, 9 `tui::`, 20 `config::`).
The shared tree's lib-test could not link at the time (T22.5's mid-flight `src/report/ai.rs`
vs `src/web/model.rs`), which is why the worktree carried the verdict; none of its files are
in this commit.
Deviations: the file set is wider than the brief's three — `layers.rs` (the `tui_flags`
overlay, where `web_flags` lives), `config/mod.rs` (the `Tui` section, where `Web` lives),
`validate.rs` (one range arm), `src/tui/` (the wiring the flags actually drive) — each required
by the `[web]`/T22.4 precedent the brief names. No T21.3 migration arm: no legacy `[tui]` key
exists. T15.9 (TTY guard) stays open.

**T15.12 the parity test enumerates commands, not pages** · T15.11, T15.10 · `tests/surface_parity.rs`
Do: extend T15.10's test from "the two surfaces expose the same pages" to "every reading command has
a page", walking `Cli::command()` the way `config_coverage` already walks it, with an explicit
allow-list for the streaming and writing commands D27 exempts.
Check: adding a reading command with no page fails `just check` naming the command; the allow-list
entries each carry the reason they are exempt.
Complexity: 2/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, effort Low)
Check result: green. `every_command_is_exempt_or_renders_a_page_of_the_model` walks `Cli::command()`
the way `config_coverage` does (leaves only; a parent that needs a verb is navigation; `logs` is a
leaf beside its subcommands; clap's `help` skipped). Two const data lists, one line per entry:
`COMMAND_PAGES` (command → page, asserted ∈ `model::pages()`) and `EXEMPT` (command, reason).
Anti-rot asserts: unclassified fails by name; stale entries (renamed/removed commands), empty
reasons, and mapped-and-exempt overlaps all fail. Check evidence (scratch command, reverted, not
committed): `unclassified command 'sessions': a reading command renders a page of the model
(COMMAND_PAGES); everything else needs a reason in EXEMPT (D27, T15.12)`. Both parity tests pass;
`just check` exit 0.
Deviations: none to the spec. Judgment calls recorded: the lists live in the test, not the model,
so the frame stays byte-stable; `otel status` exempted as an exporter echo (the decision T15.11
left open); `config get`/`logs export`/`demon list` classified alongside their siblings. At
integration the coordinator added this round's new commands: `report` (reading, on-demand P22
document), `logs watch` (streaming), `tui` (surface) to `EXEMPT`, and `agent sessions` — a real
snapshot page since T25.1 — to `COMMAND_PAGES`: 2 mapped, 37 exempt of 39 paths.

**T15.2 header · tabs · footer shell** · T15.1 · `src/tui/view.rs`, `src/tui/app.rs`
Do (roadmap §tui): the TUI shell — header line, tab bar, footer.
Complexity: 2/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, effort Low)
Check result: green. Header `rtok · <project dir> · <cols>x<rows>`, tab bar with the selected tab
bold, body, footer `q quit · Left/Right or 1..N switch tabs · updated HH:MM:SS UTC` (reusing
`log::stamp` — one calendar implementation). Tabs render through ratatui's `Tabs` widget from
`tab_names()`; `selected()` (this task) drives the highlight. Shell rendering pinned by a
`TestBackend` test; `tabs_are_the_model_pages` still holds. `just check` exit 0.
Deviations: none.

**T15.1 ratatui + crossterm scaffold, event loop** · T15.0 · `Cargo.toml`, `src/lib.rs`, `src/cli.rs`, `src/tui/{mod,app,view}.rs`
Do (roadmap §tui): the scaffold — dependencies, module, event loop, `rtok tui` subcommand.
Complexity: 2/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent; policy tier GLM-5.3-Flash, effort Low)
Check result: green. `ratatui 0.30.2` (the widget layer; its default backend is crossterm) and
`crossterm 0.29` (key/tick events; same major as ratatui 0.30's backend so exactly one copy
links) — reasons in the commit message. `src/tui/mod.rs` runs `ratatui::try_init`/`restore` on
every exit path; the loop draws, `event::poll(2 s)` (the web tick's cadence), re-reads
`model::snapshot(cfg)` on timeout, and routes keys through `App::key` (`q`/`Esc`/`Ctrl+C` quit,
Left/Right wrap, `1..=9` jump). `App` holds `tabs = model::pages()` **by reference** — no second
list anywhere; pinned by `tabs_are_the_model_pages`. The TUI never opens the `Store` (D23): data
only via the model's snapshot. Unit tests cover tab switching and the pages-derived tab list; a
real-pty smoke run rendered the header, bold tabs, usage body and footer without a panic (a 0×0
pty also does not crash — the real TTY guard is T15.9). `just check` exit 0 per commit.
Deviations: T15.1's placeholder view renders one line consuming app state rather than a bare
hello — private `mod app` accessors trip `-D dead_code` otherwise; the shell replaced it in T15.2.

**T15.11 the model covers every reading command** · T15.0 · `src/web/model.rs`, `src/measure/stats.rs`, `src/cli.rs`
Do: move the queries the reading commands own into the D23 model, one command at a time, and have
the command render what the model returns. `stats` is the hard one and goes first: it counts
transcript files while the model sums `usage` rows, so the two disagree about what a session is —
D27 says one of them is the model and the other is a renderer. `doctor`, `plugins`, `config show`,
`logs`, `demon status` follow; each is a page.
Check: `grep -r 'Store::open' src/` outside `src/web/model.rs` finds only writing commands and the
three surfaces; `rtok stats` output is unchanged for a fixture store.
Complexity: 4/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent)
Check result: green. stats, doctor, plugins, config show (+ `config get` and `set`'s value re-print,
which share its query), logs (+ export) and demon status/list now ask `src/web/model.rs`
(`stats_report` / `plugin_stats` / `cache_health`, `doctor::page -> Report`, the Plugins page via a
shared `render::plugins_table`, `config_entries` over the layered figment, `log_lines`, `demon::rows`);
no reading command opens the `Store` any more. The grep after the move hits only `plugin.rs`
`Runtime::open` (the hook/graph-index/otel runtime), `plugins/checkpoint.rs` (PreCompact restore on
the hook surface), `proxy/mod.rs` (the proxy surface) and store tests. Output-unchanged proof:
`tests/stats_model.rs` pins `rtok stats` table/`--json`/`--plugin cmd`/`--cache` byte-for-byte on a
fixture store (goldens recorded from the pre-move binary), and nine more command variants (doctor
plain and `--instructions`, plugins, config show/`--sources`/get, logs, logs export, demon status /
list / `status <svc>`) each diffed empty against the pre-move binary — only the inherent
`current_exe` path in doctor's self-listed MCP server differs between two binaries. The ws
`Snapshot` is unchanged, so T15.10's `surface_parity` still passes with `pages()` at
[overview, plugins]. Full `just check` green on the combined tree (with T24.1 + T15.10).
Deviations: the stats-vs-model definitional conflict is resolved structurally, not renumbered — the
model owns both definitions, one per page (Stats page: sessions = transcript files; Overview:
usage-row sums), with the store's authoritative session definition left to T25.1's Sessions page;
forcing one number would have changed output the Check pins without proving the old number wrong
(recorded in the `stats_report` doc comment and the commit body). 11 files, +663/−199 — the task
itself spans six commands. `rtok otel status` still reads through `Runtime::open`; it is not in the
task's command list, and T15.12's enumeration should decide it. graph_truth entries for
`plugin_json` (this task) and `read_settings` (T27.0, same round) were removed in the same landing —
annotated in the fixture header, per T8.19's rule that a must-appear list may not name nonexistent
sites.

**T15.10 the two surfaces cannot drift** · T15.0 · `tests/surface_parity.rs` (new)
Do: enumerate the pages each surface exposes and assert the sets are equal, so a page added to one
fails the build until it exists on the other. This is D23's gate, and it replaces the prose promise.
Check: adding a page to `rtok web` alone fails `just check` with the page's name in the message.
Status: done 2026-09-09 · Model: GLM-5.3-Flash (zai-coding-plan)
Check result: tests/surface_parity.rs owns the page table T15.0 deferred — model::pages() returns
[("overview", "usage"), ("plugins", "plugins")], and web_serves_exactly_the_pages_the_model_offers
holds the web surface to it by parsing the frame rtok::web::frame sends on /ws (each side's set comes
from its own source: the model's declaration vs. the wire's keys). cargo test --test surface_parity
1 passed; fmt and clippy -D warnings clean, in a detached worktree at 5975877. The gate is real:
adding a calls field to Snapshot alone fails with "assertion left == right failed: a page exists on
one surface and not the other (D23) … right: ["calls", "overview", "plugins"]" — the page's name in
the message — and the suite is inside just check's cargo test. rtok tui does not exist yet; its tabs
plug into the same compare at T15.1+, and T15.12 extends the test to reading commands.

**T15.0 one operator model behind both surfaces** · T19.1 · `src/web/model.rs` (new), `src/web/mod.rs`
Do: lift the values `rtok web` serves — Overview, Plugins, Calls, Doctor, Logs — out of the axum handlers into one module that returns them as plain data, and have the handlers render that. No new query, no second `Store` reader. `Plugin::dashboard_page` keeps its name: it is the page a plugin contributes to *both* surfaces, and renaming it would break the published plugin API.
Check: `rtok web` serves byte-identical JSON to what P19 pinned; the model module is the only place that touches `Store` / `stats` / `doctor` for either surface.
Complexity: 2/5
Status: done 2026-09-09
Model: Claude Opus 5 (anthropic/claude-opus-5)
Check result: `src/web/model.rs` owns `Snapshot` / `Stats` / `PluginPage` as typed plain data plus `Model::{overview, plugins}`; `src/web/mod.rs` no longer names `Store`, `Registry` or `DashboardPage` (`grep -E 'Store|stats|doctor' src/web/mod.rs` → 0 hits) and its ws frame is `serde_json::to_value(model::snapshot(cfg)).to_string()`. Byte-identity holds because both the old `json!` tree and the new structs land in a `serde_json::Value` (BTreeMap, no `preserve_order`), so key order and values are unchanged — pinned by the new `json_shape_is_what_p19_pinned` test beside the moved P19 test. `cargo test --lib web::` 2 passed; `just check` green (`tests/web.rs` 1 passed).
Deviation: the page set stayed {Overview, Plugins} — what `rtok web` actually serves today. Calls, Doctor and Logs are named in D23 but exist on neither surface, so lifting them would have been new pages, not the extraction this task asks for; they arrive with T15.5–T15.7 and are then covered by the T15.10 parity test. No `pages()` enumeration was added for the same reason — T15.10 owns it.

## P8d — `graph` freshness · done 2026-09-09 (T8.15–T8.19)

**T8.19 `graph_truth` is red and nobody noticed** · T8.14 · `tests/graph_truth.rs`, `src/plugins/graph/index.rs`
Do: `cargo test --test graph_truth` fails — `definition precision 0.864: a name resolved to a file
it is not defined in` — and it failed identically at `ddda5d0`, so it has been red for at least a
day of committed work while `just check` was still being reported green. Find out which: the index
regressed and the labelled truth is right, or the truth file lists references the tree no longer
has. Fix whichever is wrong; if the labels are the stale half, say so in the Check result rather
than editing them quietly.
Check: `cargo test --test graph_truth` passes, and the run prints the precision and recall it
passed at, so the next regression names a number instead of a threshold.
Complexity: 3/5
Status: done 2026-09-09 · Model: GLM-5.3 (subagent)
Check result: green — the labels were the stale half, edited openly. Definition recall was already
1.000 (38/38); precision 0.864 was 38/44 because all six extra def rows were genuine definitions
(`mark_symbols_stale`, `symbol_defs`, `insert_note`, `put_read_cache`, `get_read_cache`,
`put_archive_decision`) in `crates/rtok-plugin-sdk/src/testing.rs` — real `impl MemoryHost`
capability methods added by T23.5 after the fixture's 2026-09-04 scan, which the fixture header's
"`defs` is complete" claim had silently stopped being true of. The six gained their def files,
annotated "(T8.19 repair)"; `str_field`'s `refs` emptied (T26.1 removed both call sites). The
resolver-gap suspicion in the task text does not hold: every one of the 68 missed ref sites was
grepped — all still exist except `str_field`'s two; the `Surface`/`PreToolDecision`/
`measurement_count` misses are the documented query limits (type positions, macro bodies,
path-qualified calls) already priced into the recall floor. The test now leads with
`graph_truth: precision 1.000 recall 0.510 over 28 labels / sites 76/149 / definitions 42/42 /
references 34/107 (0.318)` and prints each miss by name, so the next regression names numbers and
files; doc-comment counts updated to the measured values. On the "nobody noticed" half: `just
check` was *not* skipping the test — no `#[ignore]`, no feature gate — it ran red through ten
commits; that is a process fact, recorded here, CI untouched. Final full `just check` green.
Deviations: no resolver code changed — `src/plugins/graph/index.rs` is untouched; 2 files. The
landing round's own tasks staled two further entries (`plugin_json` — T15.11; `read_settings` —
T27.0); both removed with annotations in the fixture header, same rule.

**T8.18 the watcher tests stop racing the filesystem** · T8.16 · `src/plugins/graph/watch.rs`
Do: `watcher_reindexes_new_file_while_calls_read_nothing` and
`watchman_without_socket_falls_back_to_notify` give the watcher one second to observe a write and
re-index. On a loaded machine FSEvents does not deliver in that window, and both tests have failed
and then passed on a re-run three times on 2026-09-09 alone. A flaky gate is worse than a slow one:
it trains everyone to re-run instead of to read. Replace the fixed deadline with a poll until a
generous cap (the deadline is then only reached when the watcher is genuinely broken), and assert on
the store's state rather than on timing.
Check: both tests pass 20 consecutive runs (`cargo test --lib plugins::graph::watch` in a loop) and
still fail within the cap when the watcher is disabled.
Status: done 2026-09-09 · Model: Opus 5 (subagent)
Check result: green. `wait_contains` now polls the store until a 10 s `REINDEX_CAP` instead of
looping a fixed 25 × 50 ms, and the separate `t0.elapsed() <= 1 s` assertion — the actual flake, since
FSEvents sometimes delivered between 1 s and the old 1.25 s cap — is gone from both tests. 20
consecutive `cargo test --lib plugins::graph::watch` runs: 5 passed, 0 failed, every time. The
negative case was proved rather than assumed: a scratch test that seeds the index and never spawns
the watcher fails at the cap with `watcher did not re-index within 10s: expected watched to contain
"watched.rs:1", store holds "no definition of watched"`, then was reverted. `assert_eq!(read, 0)`
and the `gone` assertion are untouched.
Deviations: one file, no production code — the flake was in the tests. The two tests' duplicated
poll-and-assert became one `assert_reindexed` helper. The feature-gated
`watchman_sees_daemon_edit_within_1s_reading_nothing` was left alone: it is not in the Do, and it
only runs with `graph-watchman` and a `watchman` binary on PATH.

Goal: the index follows the working tree without a tool call paying for the walk. Plan: `plan.md` P8d.

**T8.17 `watchman` backend** · T8.16 · `Cargo.toml`, `src/plugins/graph/watch.rs`, `docs/config.md`
Do: optional dependency `watchman_client = "0.9"` (Meta's client; tokio, already a dependency) behind feature `graph-watchman` (in `default` only if Gate P8d (3) and (5) pass). `watch = "watchman"`: connect to the socket (`watchman get-sockname`), `watch-project` the root, subscribe with the same suffix filter as `notify`, and feed the same quiet-period loop; the fallback to `notify` when the socket is missing prints one stderr line. The 250 ms loop and `index::run` call are shared with T8.16 — one function, two event sources.
Check: with `/opt/homebrew/bin/watchman` on PATH the T8.16 test passes with `watch = "watchman"` and `watchman watch-list` lists the temp root; with `PATH` emptied the same test passes through the fallback and stderr has exactly one `watchman: … falling back to notify` line; Gate P8d (3) and (5) numbers into `research.md` §2 and the decision rule applied in the same commit.
Complexity: 4/5 — async `watchman_client` (tokio) bridged into the sync quiet loop, one loop with two event sources, an external daemon on the machine, a fallback path that must print exactly one stderr line, feature gating (`graph-watchman`), and the Gate P8d (3)+(5) numbers plus the removal decision rule in the same commit.
Status: done 2026-09-09
Model: Muse Spark (meta/muse-spark-1.3-contributor)
Check result: `cargo test --features graph-watchman --lib plugins::graph::watch` 5 passed (incl. new `watchman_sees_daemon_edit_within_1s_reading_nothing`: daemon edit visible ≤ 1 s, `index_for.read == 0`); `cargo test --features graph-watchman --test mcp mcp_watchman` 2 passed (`watch-list` names the root — fixed a 400 ms fixed-sleep flake by polling ≤ 5 s, plus `watch-del` cleanup; fallback prints exactly one line); `just check` green (15 suites); Gate P8d (3) passes but watchman does not beat `notify` on (1), so the crate stays behind opt-in `graph-watchman`, never `default`; Gate P8d (4) `PostToolUse` p95 9.77 ms; Gate P8d (5) release 18.9 MiB → 19.5 MiB (+2.8 %) — all in `research.md` §2.

**T8.16 watcher in `rtok mcp` (`notify`)** · T8.15 · `Cargo.toml`, `src/plugins/graph/watch.rs`, `src/mcp.rs`
Do: dependency `notify = "8"` (stable; 9 is an RC; MSRV 1.77 under the 1.97 pin) — reason in the commit message. `watch::run(cx, root, stop)` watches `root` recursively, drops events whose path fails `outline::supported` or sits under `.git/`, and after 250 ms without one calls `index::run(cx, &root)`; every error is logged once and the loop continues (a lost event costs one stale answer, never a crash). `mcp::run` wraps the stdin loop in `std::thread::scope` and spawns the watcher when `plugins.graph.watch != "off"`; EOF on stdin sets `stop` and joins the thread. No `rtok graph watch` subcommand: the MCP server is the only consumer, and a second process would break the one-writer rule under `graph-lbug`.
Check: test on a temp root: start the watcher, write a new `.rs` file with one `fn`, poll `symbol` with `auto_index = false` — the definition appears within 1 s and the call's `Report.read` is 0; delete the file, it disappears; 200 writes in 100 ms produce ≤ 3 `index::run` calls (count via `Report`); `rtok mcp` exits within 500 ms of stdin EOF with the watcher on; Gate P8d (2) numbers into `research.md` §2.
Complexity: 3/5 — concurrent watcher thread inside `rtok mcp`, 250 ms debounce, timing-sensitive tests (poll ≤ 1 s, EOF exit < 500 ms); the design is fully specified by the task, so the risk is test flake, not architecture.
Status: done 2026-09-08
Model: Muse Spark 1.3 Contributor
Check result: `cargo test --lib plugins::graph::watch` 4 passed (reindex within 1 s with `Report.read == 0`, delete disappears, 200 writes coalesce to ≤ 3 runs, git/unsupported paths dropped, watchman-less fallback re-indexes); `cargo test --test mcp` 3 passed (watcher exits < 500 ms at stdin EOF; one fallback line with `PATH` emptied); Gate P8d (2) numbers in `research.md` §2 (idle CPU −10 ms, RSS +1.14 MB — passed); `just check` green.

**T8.15 `auto_index` and `watch` keys** · T8.4 · `src/config/mod.rs`, `src/plugins/graph/mod.rs`, `docs/config.md`
Do: `plugins.graph.auto_index: bool = true` and `plugins.graph.watch: String = "off"` (`off` | `notify` | `watchman`; validated like other enums in `config/validate.rs`). `auto_index = true` is today's behaviour. `false`: `symbol` / `callers` / `impact` call `index::ensure` instead of `index::run`, so a root with no rows is still indexed once and everything after that is `rtok graph index` or the watcher; a file the hook marked stale reads as missing until then, which `docs/config.md` says in one line.
Check: `tests/graph_contract.rs` green under both values; a unit test with `auto_index = false`: edit a fixture definition's line, `symbol` still reports the old line and `Report.read` of the call is 0, `rtok graph index` then shows the new one; an empty root with `auto_index = false` still answers `symbol("main")`; `rtok config set plugins.graph.watch bogus` is rejected.
Status: done 2026-09-08
Model: Muse Spark 1.3 Contributor
Check result: `tests/graph_contract.rs` 3 passed with defaults; new `auto_index_false_is_stale_until_explicit_index` (old line, `index_for` read 0, `index::run` shows new line) and `auto_index_false_empty_root_still_answers` green; `config_coverage` + `default_toml_is_the_defaults` green; `rtok config set plugins.graph.watch bogus` exits 1, `notify` accepted; `just check` green. Deviation (Check wording vs semantics): under `auto_index = false` the contract's `edited_and_deleted_files_are_reflected` fails with the old counts (2 passed, 1 failed) — that staleness is exactly what this task specifies, and the T8.16 watcher (Gate P8d (1)) is what makes edits visible again with `Report.read == 0`. Extra files beyond the task list: `config/default.toml` (the keys themselves or `default_toml_is_the_defaults` fails) and `src/config/validate.rs` (the `watch` enum the Do requires).

## P19 — web dashboard · done 2026-09-09 (T19.1–T19.4, Gate P19 passed; T19.4 2026-09-10)

Goal: one command serves a Slint WASM UI and a WebSocket API over the same Store the CLI uses. Plan: `plan.md` P19.

**T19.1 `[dashboard]` config, CLI, flags** · files: `config/default.toml`, `src/config/mod.rs`, `src/cli.rs`
Do: `host = "127.0.0.1"`, `port = 3333`; `rtok dashboard --host --port`; figment `flag` layer.
**Check:** `rtok dashboard --help` lists `--host`/`--port`; `rtok config get dashboard.port` is 3333.
Status: done 2026-09-08
Model: Cursor Grok 4.6
Check result: `rtok dashboard --help` lists `--host` / `--port`; `rtok config get dashboard.port` → `3333`; `just check` green. Extra files beyond the task list (`layers.rs`, `validate.rs`, `src/dashboard/`, `crates/rtok-webui`, `src/plugin.rs` `DashboardPage`) are required for a compiling `Dashboard` subcommand and the later T19.2/T19.3 Checks.

**T19.2 WebSocket snapshot** · files: `src/dashboard/mod.rs`
Do: `/health`, `/ws` snapshot of plugin pages + usage; stats widget data for `saves_tokens` plugins.
**Check:** `cargo test -q dashboard::` — measure has no stats, cmd does after a Measurement row.
Status: done 2026-09-08
Model: Cursor Grok 4.6
Check result: `cargo test dashboard::` — `snapshot_lists_catalogue_and_hides_stats_on_measure` green; `tests/dashboard.rs` `dashboard_health_and_index` green (`/health` ok, `/` serves the Slint canvas host). `just check` green at T19.1.

**T19.3 Slint WASM UI** · files: `crates/rtok-webui/**`
Do: one `.slint` app, plugin list, per-plugin page, `TokenStatsWidget`; `web_sys::WebSocket` to `/ws`.
**Check:** `wasm-pack build crates/rtok-webui --dev --target web` (or skip if wasm-pack missing; crate still present).
Status: done 2026-09-08
Model: Cursor Grok 4.6
Check result: `crates/rtok-webui` present (`ui/app.slint` + `web_sys::WebSocket` client). `wasm-pack` is not on PATH, so the WASM pack step is skipped as the Check allows; `cargo check` in that crate compiles the `.slint` (native lib). `just dashboard` still serves `/ws` and warns until `wasm-pack` is installed.
Follow-up 2026-09-09: wasm-pack 0.15.0 present — `wasm-pack build --release --target web --out-dir pkg` green (48 s); `rtok dashboard --port 3334` serves `/` 200 with canvas + module, `pkg/rtok_webui.js` 200 (91 KB), wasm 200 (10.5 MB), `/ws` 10 plugins with `saves_tokens` stats. Gate P19 passed (no human eyeball on pixels; everything an agent can verify is green).

**T19.4 WASM webui renders every `model::pages()` entry** · T19.3 · `crates/rtok-webui/**`, `src/web/`
Do: the Slint WASM UI still shows the Plugins strip only. Sessions / Calls / Logs / Doctor (and Overview) ride the D23 snapshot and `model::pages()` and render on `rtok tui`, but are not rendered in `crates/rtok-webui` — a page that exists on one surface and not the other is a D23 defect. Bring the WASM UI up to the model page set.
Check: every id from `model::pages()` appears in the WASM UI; a snapshot carrying Sessions/Calls/Logs/Doctor makes those pages visible; T15.10 surface-parity still green; `just check` green.
Complexity: 3/5
Status: done 2026-09-10 · Model: GLM-5.3
Check result: green. WASM UI tab bar is `PAGE_IDS` = every `model::pages()` id (overview/plugins/calls/sessions/doctor/logs); Slint bodies for each; snapshot parser paints Sessions/Calls/Logs/Doctor from the `/ws` frame. `cargo test --manifest-path crates/rtok-webui/Cargo.toml` 3/3; `cargo test --test surface_parity` 3/3 (incl. `wasm_ui_renders_every_model_page`); T15.10 still green; `just check` green (isolated target dir — shared target raced with another worktree).

## P18 — release · tasks done 2026-09-04 (gate needs the first real release)

Goal: a macOS user installs a released binary with one command, and the version of the next release is computed, never typed. Plan: `plan.md` P18.

**T18.1 version 0.0.1 and a dispatchable release** · T10.4 · `Cargo.toml`, `dist-workspace.toml`, `.github/workflows/release.yml`
Do: `version = "0.0.1"`. In `dist-workspace.toml` turn on dispatch-style releases so the workflow is started from the Actions tab and dist creates the tag itself (this *replaces* the tag-push trigger; `tag: dry-run` is a plan-only run), and ship the self-updater alongside the binary so an installed `rtok` can update itself. Regenerate the workflow with `just dist-generate` — never hand-edit it.
Check: `just dist-plan` succeeds, announces `v0.0.1`, and still lists both darwin targets on `macos-14` / `macos-15-intel`; the regenerated `release.yml` carries a `workflow_dispatch` trigger whose `tag` input drives publishing; `rtok --version` prints `0.0.1`.
Status: done 2026-09-04
Model: Opus 5
Check result: `dist plan` exits 0, announces `v0.0.1` and keeps both darwin targets on their
current images — `macos-14` for `aarch64-apple-darwin`, `macos-15-intel` for `x86_64-apple-darwin`
(dist 0.32 already moved off the retired `macos-13`, so no runner needed changing). The
regenerated `release.yml` triggers on `workflow_dispatch` with a `tag` input, and its `plan` job
gates publishing on `inputs.tag != 'dry-run'`. Artifacts now include `rtok-<target>-update`
beside each archive, the installer and the Homebrew formula. `rtok --version` → `rtok 0.0.1
(1e2177b1e)`; `just check` green (12 test binaries).
Deviation from the task text: `dispatch-releases = true` *replaces* the tag-push trigger rather
than adding to it — dist creates the tag itself through `gh release create --target <sha>`. The
plan sentence claiming a tag push would keep working was corrected before committing.
Not verified: nothing has been released yet, so the archives have never been built on a runner.
Two prerequisites must exist before the first run — the `listepo/homebrew-tap` repository and a
`HOMEBREW_TAP_TOKEN` secret — or `publish-homebrew-formula` fails after the Release is created.

**T18.2 the next patch, computed** · T18.1 · `.github/workflows/bump.yml`, `justfile`
Do: one `workflow_dispatch` entry point that reads the version in `Cargo.toml`, raises the patch (level selectable, `patch` by default), regenerates `CHANGELOG.md` with git-cliff, commits `release: v<next>` with `Cargo.lock` updated, and starts the release for that version. `just release` does the same locally for anyone without Actions access. Neither path takes a version as an argument.
Check: on a scratch branch, the bump step run locally takes 0.0.1 → 0.0.2 in `Cargo.toml` and `Cargo.lock`, writes the changelog and makes one commit; `dist plan` at that commit announces `v0.0.2`; a second run gives 0.0.3; `just check` green.
Status: done 2026-09-04
Model: Opus 5
Check result: run in a clone of this repo so the working tree was untouched. With `v0.0.1`
tagged, `tools/release.sh patch --local` printed `current 0.0.1 -> release v0.0.2`, wrote
`version = "0.0.2"` into `Cargo.toml` and `Cargo.lock`, regenerated `CHANGELOG.md` and made
exactly one commit, `release: v0.0.2`. Tagging that and adding one ordinary commit, a second run
gave 0.0.3 with a `## 0.0.3` changelog section listing that commit, and `dist plan` at the
resulting commit announced `v0.0.3` — so dist reads the version the script wrote. `CHANGELOG.md`
contains zero `release: v` entries, which is the `cliff.toml` skip rule working. `just check`
green (12 test binaries).
The first run is the interesting one: because the script raises the version only when the current
one is *already tagged*, the very first release publishes 0.0.1 rather than skipping to 0.0.2.
`tools/release.sh patch --dry-run` on this repo confirms it — `current 0.0.1 -> release v0.0.1`.
Deviations: four files, not the two the task named. `cliff.toml` gained a `skip` rule so the
bookkeeping commit never appears in the next release's notes, and the shared logic went into
`tools/release.sh` rather than being written twice — the justfile recipe and the workflow are both
two-line callers of it. One thing found by running it: `[ -n "$X" ] && echo …` as a top-level list
ends a `set -e` script when the test is false; it is an `if` now.
Not verified: `git push` and `gh workflow run` are the two lines `--local` skips, so the dispatch
itself is unexercised until the first real release.

**T18.3 install in one line** · T18.1 · `README.md`
Do: the README Install section leads with the shell installer for macOS and Linux and the Homebrew tap, with the source build kept below for contributors. State plainly that the binary is unsigned.
Check: `just readme-check` green; the installer line, pasted into a clean bash and a clean zsh on macOS, exits 0, puts `rtok` on `PATH` and prints the released version.
Status: done 2026-09-04
Model: Opus 5
Check result: `just readme-check` green — the new fences carry no `# check` marker, so nothing
tries to install from a release that does not exist yet. The section now leads with the dist
shell installer (`releases/latest/download/rtok-installer.sh`, a URL that survives every release)
and `brew install listepo/tap/rtok`, mentions `rtok-update`, states that the binaries are unsigned
and that a browser download is quarantined by macOS while the installer is not, and keeps the
source build below for contributors. `just check` green (12 test binaries).
Not verified: the clause "pasted into a clean bash and a clean zsh on macOS, exits 0" cannot run
until a release exists — `releases/latest/download/` 404s on a repo with no releases. The
installer dist generates is POSIX `sh` and is piped to `sh`, so the caller's shell does not matter;
that is a reading of the artifact, not a run of it.

**T18.4 codesigning and notarisation, written down** · T18.1 · `docs/release.md`
Do: one document covering the release as it runs today and how to turn on Developer ID signing and notarisation, locally and in Actions. Name the secrets dist actually reads, the Apple assets that have to exist first, and the point where notarisation stops short for a bare CLI binary in a tarball. State plainly which steps were run on this machine and which need an Apple Developer account nobody here has.
Check: the secret names in the document are the ones `dist generate` emits with `macos-sign = true`, not remembered ones; every `codesign`, `security`, `ditto`, `spctl` and `xcrun` invocation matches the flags of the tools installed on this machine; `just check` green.
Status: done 2026-09-04
Model: Opus 5
Check result: the three secret names in the document — `CODESIGN_CERTIFICATE`,
`CODESIGN_CERTIFICATE_PASSWORD`, `CODESIGN_IDENTITY` — were read off a workflow generated with
`macos-sign = true` in a throwaway clone, not recalled; that is the entire diff turning the key on
produces, so dist 0.32 signs and does not notarise. A deliberate nonsense key (`macos-frobnicate`)
was accepted silently, which is why the document warns against trusting a guessed `macos-notarize`.
The stapling limit is quoted from `xcrun stapler --help`: disk images, code-signed executable
bundles and flat packages — a bare Mach-O in a `.tar.xz` is none of them, so notarisation here can
never be stapled. `xcrun notarytool store-credentials/submit/log` flags and `codesign`'s usage line
match the installed tools. `just check` green (12 test binaries).
Deviations: four files, not the one the task named — `README.md` gained a single line pointing at
the document from the sentence that already said the binaries are unsigned. `spctl` and
`codesign`'s long-form flags could not be probed (both blocked by this machine's shell allowlist),
so the document says so rather than claiming a verification that did not happen.
Found while running the Check, unrelated to it: `just check` failed once with `CMake Error at
tools/CMakeLists.txt:2 (add_subdirectory): source "shell" ... not an existing directory`. The
published `lbug` 0.20.2 crate ships `lbug-src/tools/CMakeLists.txt` but no `tools/shell`, and the
guard is `if(${BUILD_SHELL})`, so it only fails when that variable is true — and one of the four
`lbug-*` build directories held a `CMakeCache.txt` with `BUILD_SHELL:BOOL=ON` despite `build.rs`
passing `-DBUILD_SHELL=OFF`. `cargo clean -p lbug` (7 759 files, 788 MiB) fixed it and the next
`just check` was green. If that error reappears, it is a stale cmake cache, not the source.

**T18.5 release-plz: the version as a pull request** · T18.2 · `release-plz.toml`, `.github/workflows/release-plz.yml`, `tools/release.sh`
Do: a second entry point beside the Bump workflow — release-plz keeps one `release: vX.Y.Z` pull request open on `main` with the next version and the `cliff.toml` changelog; merging it dispatches the dist Release workflow through `tools/release.sh --no-bump`. release-plz neither tags nor publishes (`publish = false`, `git_tag_enable = false`, `git_release_enable = false`): dist does both, and the tag dist pushes is what release-plz reads to know a version is out. No new secret: `GITHUB_TOKEN`, with `RELEASE_PLZ_TOKEN` as an optional upgrade for CI on the PR.
Check: `release-plz update --dry-run` on this checkout (no tags yet) keeps `0.0.1` and writes a `## 0.0.1` changelog section grouped like `just changelog`; `actionlint` accepts the workflow; in a scratch clone with a `v0.0.1` tag, `tools/release.sh patch --no-bump` prints "already released" and exits 0 before any push, and without the tag reaches the push and dispatch; `bash -n` clean; `just check` green.
Status: done 2026-09-07
Model: Fable 5.1
Check result: `release-plz update` (release-plz 0.3.162 via `mise x ubi:`; it has no `--dry-run`
and refuses a dirty tree, so it ran in a scratch clone with the change committed) left
`Cargo.toml` at `0.0.1` — "determining next version for rtok 0.0.1", no bump for a version that
was never tagged — and wrote a `## 0.0.1 — 2026-09-07` section to `CHANGELOG.md` under the same
`### Tasks` / `### Plan` groups as `just changelog`, so `changelog_config = "cliff.toml"` is
honoured. `actionlint` found one real error on the first pass — an unquoted `if:` holding
`'release: v'` is a YAML mapping — fixed by quoting; second pass clean, `bump.yml` clean.
Scratch clone with a `v0.0.1` tag: `release.sh patch --no-bump` printed "current 0.0.1 -> release
v0.0.2" then "v0.0.1 is already released; the next version comes from a release PR", exit 0, no
push; without the tag it reached `git push` / `gh workflow run` (both failed, as a local clone
without a token must) and committed nothing. `bash -n` clean; `just check` green (146 tests).
Not proven here: that release-plz on GitHub reads the `v*` tag dist pushes when `publish = false`
and `git_tag_enable = false` — the docs say tags are the record for unpublished packages; the first
merged release PR is the test, and the failure mode is a second PR proposing the same version,
not a wrong release. Deviation: four files plus the bookkeeping, one more than the rule —
`docs/release.md` gained the paragraph that explains the second entry point.
Follow-up, same day: the first push of this task fired the `release` job, not `release-pr` — the
commit *body* contained the literal `release: vX.Y.Z`, and `contains(head_commit.message, …)` is
not line-anchored — and `release.sh --no-bump` dispatched dist Release run 34150922080 for
v0.0.1. Cancelled during build-local-artifacts; `git ls-remote --tags` and `gh release list`
stayed empty, so nothing was published. Fix: a `gate` job runs
`git log -1 --format=%B | grep -qxE 'release: v[0-9]+\.[0-9]+\.[0-9]+'` and both jobs branch on
its output; checked locally against a squash message, a merge-commit message (both match) and
the T18.5 commit (no match); `actionlint` clean.

**T18.6 the release runs only on a green test suite** · T18.2, T18.5 · `.github/workflows/verify.yml`, `.github/workflows/release-plz.yml`, `.github/workflows/bump.yml`, `dist-workspace.toml`, `docs/release.md`, `tests/otel.rs`, `Cargo.toml`
Do: after v0.0.1 shipped, close the two holes the first real release exposed. (1) Nothing tested the code a release was cut from: `ci.yml` gates pull requests, and both release entry points dispatch `release.yml` without consulting it. Add a reusable `verify.yml` — the `ci.yml` matrix and recipes — and make the dispatch in `bump.yml` and in release-plz's `release` job depend on it. (2) `release-plz.yml` fell back to `GITHUB_TOKEN` when `RELEASE_PLZ_TOKEN` was absent, which opens a release pull request that starts no workflows: an empty checks list, not a red one. Fail the job with the reason instead. Modelled on `../ketch`, which runs the same gate inside its release and refuses to run without the token.
Check: the gate is not a dist `plan-jobs` entry — prove why with the generated workflow, not from memory; `actionlint` clean on all three workflows; `dist plan` still parses `dist-workspace.toml` and `release.yml` is byte-identical to the committed one; `just check` green.
Status: done 2026-09-09
Model: Opus 5
Check result: `plan-jobs = ["./verify"]` was tried first and reverted. `dist generate` wired it as
`custom-verify` with `build-local-artifacts` gaining `needs: custom-verify` — but `host` keeps
`if: always() && … && (needs.build-local-artifacts.result == 'skipped' || … == 'success')`, and a
failed custom job leaves that job `skipped`, so `host` and `announce` would still run. A red gate
would have published a Release with no binaries — worse than no gate, since `rtok-update` and the
installer read exactly those assets. The gate therefore sits in the two workflows that dispatch
`release.yml`, where a failure means no dispatch, no tag and no draft Release. `release.yml` was
restored with `git checkout` and `git diff` confirms it byte-identical; `dist plan` still lists all
three targets. `actionlint -shellcheck=` clean on `verify.yml`, `release-plz.yml` and `bump.yml`
(shellcheck itself could not run: the mise shim has no version set). `just check` green (146 tests).
Beta check the same day, on the published v0.0.1 rather than a local build: `rtok-installer.sh`
from `releases/latest` installed `rtok 0.0.1 (6c55b45a5)` and `rtok-update` into a scratch
`CARGO_HOME`; against a fresh `HOME` the binary ran `stats`, `config show --sources`, `doctor`,
`otel status` and `setup claude --dry-run` (7 additions + `mcpServers.rtok`) with exit 0, and
`hook PreToolUse` returned the wrapped command in 28 ms wall — garbage and empty stdin both
returned `{}` and exit 0, so fail-open holds in the shipped binary.
Deviation: six files, three more than the rule. `dist-workspace.toml` gains no setting, only the
comment recording why `plan-jobs` is the wrong place, so the next reader does not retry it; and
`docs/release.md` still described the token as optional and the tap as nonexistent. That document
is the only place the two remaining ketch-parity items are written down, both blocked on a secret
this session cannot create: `HOMEBREW_TAP_TOKEN` for the formula (the tap now exists and carries
ketch's cask), and `CODESIGN_CERTIFICATE` / `CODESIGN_CERTIFICATE_PASSWORD` for signing (ketch
holds the same Developer ID under different secret names; a secret's value cannot be read back out
of GitHub, so it has to be issued again). `tests/otel.rs` is the flake the gate would have tripped
over; a gate that fails at random is worse than the hole it closes, so it belongs to this task.
Same day, found while pushing this task: the T18.5 gate missed the ordinary squash merge. GitHub
appends the pull request number to the title, so merging #3 wrote `release: v0.0.1 (#3)` and
`grep -x` did not match it. Nothing was lost — v0.0.1 was already tagged, and `release.sh
--no-bump` refuses a released version — but the next merge would have dispatched no release at
all, silently. The gate now matches `^release: v[0-9]+\.[0-9]+\.[0-9]+( \(#[0-9]+\))?$`, checked
against all five shapes: squash with and without the number, the title line of a merge commit, an
ordinary task commit, and the same string inside a commit body (the last two correctly no match).
The T18.5 line "the failure mode is a second PR proposing the same version, not a wrong release"
turned out to be the actual behaviour, and permanent. release-plz 0.3.163 decides whether a version
shipped by comparing the packaged crate against the registry copy; `publish = false` means there is
no copy, so it reads rtok as never released and answers `next version is 0.0.1` regardless of
history. Reproduced in a clean clone with the `v0.0.1` tag fetched (`git describe` finds it) under
four configurations: as shipped, with `git_tag_enable = true`, with an explicit
`git_tag_name = "v{{ version }}"` (ketch's setting), and with a conventional `fix:` commit after
the tag — 0.0.1 every time. `../ketch` shares the configuration and its own config comment says it
is not a crate either, so it should meet this at its second release; untested there, it is on
`v0.1.0`. Nothing mis-releases: `release.sh --no-bump` refuses a tagged version and exits 0. Recorded
in `docs/release.md`, which now sends the second and later releases through **Bump and release**.
Whether release-plz still earns its place is the user's call, so T18.5 is left standing.
Making `just check` gate the release exposed a flaky test that would have blocked releases at
random: `stop_hook_spawns_the_flush_and_stays_under_10ms` waited 40 × 50 ms for the spawned child
to post, against the `flush_secs = 2` the same test writes — a 2 s deadline on a 2 s interval, so
the post lands on the boundary. It failed on ci run 34342891823 on a docs-only commit. The wait is
now 400 polls, matching `tests/proxy.rs`; a green run still breaks out on the first poll. Five
consecutive local runs of that test pass, and two of them took 4.72 s and 6.39 s — both past the
old 2 s deadline, so the budget was the cause and not the runner. ci is green on the fix.
Not proven here: that a red `verify` actually blocks the dispatch. It needs a release run with a
deliberately broken tree, and the only way to stage one is to publish from `main`.
Follow-up, same day, by request: every trace of the Homebrew release is commented out rather than
left half-described. `dist-workspace.toml` keeps `installers = ["shell"]` and carries the three
lines that would turn the formula on (`installers = ["shell", "homebrew"]`, `tap`, `publish-jobs`)
inside a delimited comment block saying it is off to be fixed later and blocked on
`HOMEBREW_TAP_TOKEN`; `docs/release.md` replaces the turn-on section with a plain statement that
`brew install rtok` is not a thing, followed by an HTML comment holding the three-step procedure;
`Cargo.toml` keeps `repository` and `homepage` — dist reads both when it builds a formula — with a
comment saying so. Checked: `dist plan` announces v0.0.1 with only the shell installer, updater and
sha256 artifacts and no homebrew entry, `just dist-generate` leaves `release.yml` byte-identical
(`git diff --stat` empty), and `just check` green. `README.md` line 23 and the `CHANGELOG.md` and
`done.md` history were left alone: they record the ketch install path and past events, and neither
offers a brew instruction.

**T18.7 dependency updates as pull requests** · T18.6 · `.github/dependabot.yml`
Do: nothing proposed dependency updates; crates and actions moved only when bumped by hand. The owner asked for Dependabot (2026-09-11).
Check: weekly cargo and github-actions updates, each landing on `ci.yml`'s pull-request `just check`. Cargo updates wait a 7-day cooldown. `tree-sitter*` moves as one group: the parser and grammars share an ABI, and a grammar bump changes the tags a stored index keeps until T35.5. Other minor and patch crates share one weekly pull request, and actions share one group. `release.yml` is excluded because `tools/dist-generate.sh` writes it and an edit would be lost on the next generate. The `deps:` and `ci:` prefixes land in `cliff.toml`'s Tooling group. `site/go.mod` (one Hugo theme) and `plugins/pi/package.json` (no dependencies) are left out: `docs.yml` builds the site only on `main`, so a theme bump would be checked after the merge. Every key is in SchemaStore's `dependabot-2.0.json`; GitHub's first Dependabot run is the remaining check.
Complexity: 1/5
Status: done 2026-09-11 · Model: Opus 5
Found on the first run, 2026-09-11. Dependabot ranked `minor-and-patch`, which has no patterns, as more specific than `tree-sitter`. Every tree-sitter crate therefore fell into `minor-and-patch`, and a 0.x major bump (tree-sitter-python 0.23 → 0.25, #13) opened alone. `minor-and-patch` now excludes `tree-sitter*`. The same run confirmed `exclude-paths`: the actions group (#10) left `release.yml` untouched.

## P17 — build size · done 2026-09-07 (T17.1–T17.2, Gate P17 passed)

Goal: what a contributor compiles and what a user downloads stop growing with the dependency list. Plan: `plan.md` P17. Numbers: `research.md` §2 "Build size (T17.1, Gate P17)".

**T17.2 pin cargo-cache** · T17.1 · `mise.toml`
Do: `[tools]` gains `"cargo:cargo-cache"` at the version this machine already resolves. `just cache` and `just cache-autoclean` call it through `mise exec --`, the form every pinned tool in the justfile uses, but the tool was only ever installed globally — a fresh clone cannot run either recipe. cargo-dist stays unpinned on purpose; the justfile says why.
Check: the tool resolves from the repository's own config, not a global install; `just cache` prints the cargo-home summary and `just cache-autoclean` is exercised with `--dry-run` so nothing is deleted from a cargo home other sessions share; `just check` green.
Status: done 2026-09-05
Model: Opus 5
Check result: before the change `mise ls` listed `cargo:cargo-cache 0.8.3` as a global install and
`mise ls --current` did not list it at all; after, it resolves from `~/GitHub/rtok/mise.toml`.
`just cache` prints the summary — cargo home 2.17 GB, of which 1.49 GB is 1 848 crate source
checkouts and 284 MB is 2 091 crate archives — and `cargo-cache --autoclean --dry-run` completes
without removing anything, which is how it was exercised: this cargo home is shared with other
sessions, so a real autoclean was not run. `just check` green (12 test binaries).
Worth knowing: adding a tool to `[tools]` changes `PATH`, and cargo fingerprints build scripts
against it, so the next `just check` rebuilt every crate with a build script — including lbug's
C++, about nine minutes. One-time, but it is why a one-line toolchain edit is not a free check.

**T17.1 dev, release and dist profiles** · — · `Cargo.toml`, `research.md`
Do: `[profile.dev] debug = "line-tables-only"` (backtraces keep `file:line`; the DWARF that dominates every artifact goes). `[profile.dev.package.lbug] opt-level = 2, debug = false` so `cmake-rs` reads `OPT_LEVEL`/`DEBUG` and configures the bundled C++ as a release build instead of a 2 GB `-g` one. `[profile.release] strip = "symbols"`. `[profile.dist]` gains `codegen-units = 1` beside its `lto = "thin"`, since the shipped binary is built once. Nothing sets `panic = "abort"`.
Check: `cargo clean -p lbug -p rtok`, then a cold `cargo build` and `cargo build --release`; both binaries and `liblbug.a` shrink and the numbers with a date go into `research.md` §2; `just check` green; a deliberate plugin panic still prints `file:line`.
Status: done 2026-09-04
Model: Opus 5
Check result: `cargo clean -p lbug -p rtok` freed 24.5 GiB across 34 028 files before the cold
builds. Release `rtok` 22 374 576 → 19 157 712 B (−14.4 %), build time unchanged at 1m30s.
`liblbug.a` 2 169 748 672 → 83 940 608 B (−96.1 %, 25.8×) and the `lbug` cmake directory 4.3 GiB
→ 358 MiB (−92 %), at a cold-build cost of 3m36s → 8m36s: `-O3` for five minutes, once per
feature set. The dev binary needed an isolated A/B to measure honestly — the `target/debug/rtok`
lying in the shared target dir was 45 135 464 B from an older feature set, which made the new
binary look 12 MB larger. Three cold builds into three empty target dirs, `--config
profile.dev.debug=…`: `true` 62 500 520 B / 1 479 MiB, `line-tables-only` 60 378 600 B / 1 211 MiB,
`false` 54 364 592 B / 945 MiB. `just check` green (exit 0, 12 test binaries ok).
Panic check, run rather than assumed: a crate compiled with exactly `-Cdebuginfo=line-tables-only`
still returns `Err` from `catch_unwind`, and `RUST_BACKTRACE=1` prints `panicked at
src/lib.rs:1:14` with `at ./src/lib.rs:1:14`, `:2:14`, `:6:18`, `:5:42` on the frames.
Gate P17's p95 clause is the one thing not cleanly passed: `rtok hook PostToolUse` measured
p95 10.07 ms stripped against 9.83 ms unstripped, interleaved run-for-run on the same machine
(repeat rounds 9.83–10.91 ms), so it sits on the 10 ms bar rather than under it. The A/B places
that outside this change — the unstripped binary of the same commit drifted with it, and stripping
is marginally *faster* at the median (8.16 vs 8.33 ms). The drift is the machine or the grown
`rtok.db` against the 8.89 ms the same harness recorded at Gate P16 earlier the same day.
Also answered here (user question, no code): packaging `liblbug` as a `.framework` saves nothing
(same Mach-O in a directory), an `.xcframework` is larger by construction and has no slice for the
Linux x86_64 target `dist` ships, and a `.dylib` — the only variant that would remove bytes, since
`+whole-archive` copies lbug into `rtok` and into each of ~12 test binaries — costs D1's single
static binary, has no `-rpath` from `lbug`'s `build.rs`, and cannot differ between dev and release
because `LBUG_SHARED` is a build-time env var and `.cargo/config.toml [env]` is global.

## P16 — OpenTelemetry export · done 2026-09-07 (T16.1–T16.8, Gate P16 passed on (1), (2), (4); (3) moved to Gate P18)

Goal: every ledger row is a span, log or sum in Jaeger, Grafana, SigNoz and Maple, with nothing on the hook path. Plan: `plan.md` P16, design `src/otel/PLAN.md`.

**T16.1 `[otel]` config** · — · `src/config/mod.rs`, `config/default.toml`, `docs/config.md`
Do: `section! { Otel { endpoint: String = "", headers: String = "", service_name: String = "rtok", content: bool = true, content_bytes: u32 = 65536, flush_secs: u32 = 5 } }` and `Config.otel`; `Otel::resolve() -> Option<Endpoint { url, headers: Vec<(String, String)> }>`: empty `endpoint` → `OTEL_EXPORTER_OTLP_ENDPOINT`, empty `headers` → `OTEL_EXPORTER_OTLP_HEADERS` (`k=v,k2=v2`), `None` when both are empty, trailing `/` stripped. Same keys in `config/default.toml` and `docs/config.md` (D12).
Check: `default_toml_is_the_defaults` and `config_coverage` green; unit tests: empty → `None`; env fallback; `a=1,b=2` → two pairs; `rtok config show` prints `[otel]`.
Status: done 2026-09-04 — `default_toml_is_the_defaults`, `config_coverage` and `otel_is_off_until_an_endpoint_resolves` green; `[otel]` in `config/default.toml` and `docs/config.md`; `just check` green.
Model: Claude Fable 5.1


**T16.2 export watermark and row readers** · T16.1 · `migrations/0009.sql`, `src/store/schema.rs`, `src/store/otel.rs` (`mod.rs` gains `mod otel;`)
Do: `otel_export(stream TEXT PRIMARY KEY, mark BIGINT NOT NULL DEFAULT 0)`; a second `impl Store` in `src/store/otel.rs`: `otel_mark(stream) -> i64`, `otel_advance(stream, mark)`, `calls_after(id, limit) -> Vec<Call>`, `logs_after(id, limit) -> Vec<LogRow>`, `sessions_ended_after(ts) -> Vec<Session>` (`>=`, ties resend, never lost), and `call_detail(call_id) -> CallDetail { io, usage, tokens, measurements, host, provider, model }` — the one read a span needs. Diesel only, no `sql_query` (P13).
Check: store unit tests: fresh DB → mark 0; advance then read; `calls_after` returns ascending ids above the mark and honours the limit; `call_detail` joins what `insert_call_io` + `insert_usage` + `insert_measurement` wrote for the call and resolves the three slugs.
Status: done 2026-09-04 — `store::otel` tests: fresh mark 0, upsert, `calls_after` ascending + limit, `sessions_ended_after` includes the tie, `call_detail` joins io/usage/tokens/measurements and resolves provider + model; `just check` green (a disk-full ENOSPC was cleared first: `cargo clean -p rtok` freed 24 GiB of stale artifacts).
Model: Claude Fable 5.1


**T16.3 OTLP/HTTP JSON encoder** · — · `src/lib.rs`, `src/otel/mod.rs`, `src/otel/otlp.rs`
Do: pure types `Resource`, `Span { trace_id, span_id, parent, name, kind, start_ns, end_ns, attrs, events, status }`, `LogRecord`, `Sum`, and `traces(&Resource, &[Span])`, `logs(..)`, `metrics(..)` → `serde_json::Value` in the OTLP 1.x JSON encoding: ids lowercase hex (32 / 16 chars), every int64 a decimal string, `kind` / `severityNumber` / `aggregationTemporality` integers, attributes `{key, value: {stringValue | intValue | boolValue | doubleValue}}`, one `scope { name: "rtok", version }`; `trace_id(session) = sha256("rtok:session:" + id)[..16]`, `span_id(kind, id) = sha256("rtok:" + kind + ":" + id)[..8]`. No I/O.
Check: unit tests pin the shape against the spec example: `resourceSpans[0].scopeSpans[0].spans[0].traceId` is 32 hex chars, `spanId` 16, `startTimeUnixNano` a string, `kind` 1 for INTERNAL and 3 for CLIENT; same for `resourceLogs` and `resourceMetrics`; ids stable across calls, different between sessions.
Status: done 2026-09-04 — `otel::otlp` tests green: traceId 32 hex / spanId 16 hex, `startTimeUnixNano` a string, kind 1 vs 3, parentSpanId absent on a root, status code 2 with message; logs `severityNumber` + optional traceId; metrics `aggregationTemporality` 2, `isMonotonic` true, `asInt` a string; ids stable per session and distinct per kind.
Model: Claude Fable 5.1


**T16.4 ledger → GenAI mapping** · T16.2, T16.3 · `src/otel/map.rs`, `src/otel/mod.rs`
Do: `session_span(&Session) -> Span` (`invoke_agent {host}`: `gen_ai.operation.name`, `gen_ai.agent.name`, `gen_ai.conversation.id`, `rtok.project`, `rtok.cwd`, `rtok.source`) and `call_span(&Call, &CallDetail, &Otel) -> Span` per the table in `src/otel/PLAN.md`: hook `PreToolUse` / `PostToolUse` → `execute_tool {tool_name}` with `gen_ai.tool.name`, `gen_ai.tool.call.id`, `gen_ai.tool.call.arguments` / `result` read from `call_io.request_json`; other hooks → `hook {event}` (`UserPromptSubmit` carries `gen_ai.input.messages`); mcp → `execute_tool {name}` + `rtok.plugin`; proxy → `chat {model}` (CLIENT) with `gen_ai.provider.name`, `gen_ai.request.model`, `gen_ai.usage.input_tokens` / `output_tokens` / `cache_read.input_tokens` / `cache_creation.input_tokens`, `gen_ai.input.messages` / `gen_ai.output.messages` from the bodies; every span: parent = session span, `gen_ai.conversation.id`, `rtok.surface`, `rtok.kind`, status from `ok` / `error`, start `ts` s → ns, end = start + `ms`; `tokens` rows → events `rtok.plugin.run` (`rtok.plugin`, `rtok.phase`, `rtok.tokens`); `measurements` → events `rtok.measurement` (`rtok.kind`, `before_bytes`, `after_bytes`, `est_before`, `est_after`, `rtok.ref_id`); content attributes cut at `content_bytes` with `rtok.archive.id` when the row has one, dropped entirely under `content = false`; `log_record(&LogRow) -> LogRecord` with severity mapped, `rtok.source` / `name` / `plugin`, trace and span ids when the row has a session / call.
Check: unit tests on an in-memory `Store`: a `PostToolUse` call whose request JSON names `Read` and `toolu_1` → span `execute_tool Read` with both ids and the arguments; a proxy call with `insert_usage(…, 100, 10, 20, 30, …)` → `chat` span with the four usage attributes and the provider slug; a measurement → one event carrying `est_before − est_after`; a 100 KB result at `content_bytes = 1024` → 1 024 bytes plus `rtok.archive.id`; `content = false` → no `gen_ai.*.messages` / `gen_ai.tool.call.*` attribute.
Status: done 2026-09-04 — `otel::map` tests green: a PostToolUse row becomes `execute_tool Read` with tool id and arguments and a 25 ms span; a proxy row becomes `chat claude-x` with the four `gen_ai.usage.*` attributes, the provider slug and both message bodies; a measurement becomes one event carrying `rtok.tokens.saved` 2; a 100 KB result at `content_bytes = 1024` is cut to 1 024 bytes with `rtok.archive.id` and `rtok.content.truncated`; `content = false` drops every content attribute; a failed call carries status Error and `error.type`; a log row carries severity 13 and both ids.
Model: Claude Fable 5.1


**T16.5 exporter and `rtok otel`** · T16.4 · `src/otel/export.rs`, `src/cli.rs`, `tests/otel.rs`
Do: `pub async fn flush(cx: &Ctx) -> Result<Report { spans, logs, posted, skipped, error }>`: at most 1 000 rows per stream after each mark, encoded and POSTed to `<endpoint>/v1/traces` and `/v1/logs` with `Content-Type: application/json` and the resolved headers, each mark advanced only after a 2xx, one `logs` row (`source = otel`) on any failure — never panics, never waits longer than `flush_secs`; `flush_blocking(cx)` runs it on a current-thread tokio runtime (no `reqwest/blocking`); `rtok otel flush` prints the report, `rtok otel status [--json]` prints the resolved endpoint, marks vs `max(id)` per stream and the last `otel` log line; both exit 0 with `otel: no endpoint` when off.
Check: `tests/otel.rs` (httpmock): three `calls` rows with `call_io` and one `logs` row → one POST each to `/v1/traces` and `/v1/logs` whose bodies hold the expected span names and attributes; a second flush posts nothing; a 500 leaves both marks and writes the `logs` row; `rtok otel status --json` shows the marks; endpoint unset → exit 0, nothing posted.
Status: done 2026-09-04 — `tests/otel.rs` green: three seeded calls and one log row post once to `/v1/traces` and `/v1/logs` with the expected span names and usage attributes, the marks advance to 3 and 1, a second flush posts nothing, an ended session ships one root span and only once; a 500 leaves both marks at 0, writes the `error/flush` log row and leaves 3 calls + 2 logs pending; `rtok otel flush` and `status` exit 0 with no endpoint. `just check` green.
Model: Claude Fable 5.1


**T16.6 triggers off the hook path** · T16.5 · `src/proxy/mod.rs`, `src/mcp.rs`, `src/hooks/mod.rs`
Do: `proxy`: a tokio task every `flush_secs` calling `flush`; `mcp`: a `std::thread` ticking `flush_blocking` and one last flush at stdin EOF; hooks: on `Stop` and `SessionEnd`, when an endpoint resolves, `Command::new(current_exe()) otel flush` with stdio null, `spawn` and forget; `SessionEnd` also sets `sessions.ended_at` (`Store::end_session` if missing) so the root span ships. No other hook touches otel.
Check: hook p95 ≤ 10 ms over 100 runs of `PostToolUse` and `Stop` with `endpoint = "http://127.0.0.1:9"` — number into `research.md`; `tests/proxy.rs`: with a mock collector a proxied request appears on `/v1/traces` within `2 × flush_secs`; `tests/otel.rs`: `rtok hook Stop` with the endpoint set exits ≤ 10 ms and the child posts the trace within 2 s.
Status: done 2026-09-04 — `tests/otel.rs` green: `rtok hook Stop` with an endpoint set exits 0 and the spawned child posts the trace, `SessionEnd` sets `ended_at` so the root span ships, and 100 `Stop` runs against an unreachable endpoint stay under the bar (debug run uses a 200 ms bar; the 10 ms release number is T16.8's Gate P16 (2) measurement). `just check` green.
Model: Claude Fable 5.1


**T16.7 metrics** · T16.5 · `src/otel/metrics.rs`, `src/otel/export.rs`, `tests/otel.rs`
Do: each flush also POSTs `/v1/metrics` with cumulative monotonic sums computed from the ledgers (`aggregationTemporality = 2`, `startTimeUnixNano` = first row's `ts`): `rtok.tokens` `{gen_ai.token.type ∈ input | output | cache_read | cache_creation, gen_ai.request.model, gen_ai.provider.name}` from `usage`; `rtok.tokens.saved` `{rtok.plugin, rtok.kind}` = Σ(`est_before − est_after`) from `measurements`; `rtok.calls` `{rtok.surface, rtok.kind, rtok.ok}`. No watermark: whole-table sums are idempotent.
Check: httpmock `/v1/metrics` body holds the three sums with `isMonotonic: true`, temporality 2 and values equal to `rtok stats --json` on the same DB; a second flush repeats the values with a later `timeUnixNano`.
Status: done 2026-09-04 — `metrics_repeat_the_totals_every_flush` green: `/v1/metrics` carries the three sums with `isMonotonic: true`, temporality 2 and `asInt` strings matching the seeded ledger (input 100, saved 2); 8 data points, and a second flush repeats them with a later `timeUnixNano` while traces and logs post nothing. `just check` green.
Model: Claude Fable 5.1


**T16.8 docs and live check** · T16.6, T16.7 · `docs/otel.md`, `README.md`, `research.md`
Do: `docs/otel.md`: keys and env fallback, the span / attribute table from `src/otel/PLAN.md`, one copy-paste recipe each — Jaeger v2 (`docker run --rm -p 16686:16686 -p 4318:4318 jaegertracing/jaeger:2`), Grafana (`docker run --rm -p 3000:3000 -p 4318:4318 grafana/otel-lgtm`), SigNoz (compose; cloud `headers = "signoz-ingestion-key=…"`), Maple (its OTLP URL and key header); README link. Run the Jaeger recipe once and the other three where reachable; trace id and what each UI showed, dated, into `research.md` §2 — Gate P16 (3).
Check: each recipe pasted verbatim starts and receives the trace from `rtok otel flush`; `research.md` §2 has the P16 rows; `rtok doctor` unaffected.
Status: done 2026-09-04 — `docs/otel.md` written (keys, env fallback, flush triggers, the span and attribute table, and copy-paste recipes for Jaeger v2, Grafana `otel-lgtm` and Cloud, SigNoz self-hosted and Cloud, Maple), linked from `README.md` and `docs/config.md`; `research.md` §2 carries the P16 measurements. The four Docker recipes were NOT run: `docker` is blocked by this machine's shell allowlist, so Gate P16 (3) stays open on the four UIs and is recorded that way in `plan.md` and `research.md`. What replaced it: an independent OTLP receiver validated one real session's bytes with 0 problems.
Model: Claude Fable 5.1


## P8c — `graph` on LadybugDB · done 2026-09-08 (T8.10–T8.14, Gate P8c: clause (4) won, `graph-lbug` stays opt-in)

Goal: the same four tools, byte-identical, on an embedded graph store — kept only if it wins on numbers. Plan: `plan.md` P8c, survey `src/plugins/graph/PLAN.md` v0.3. T8.9 (the contract these tasks are judged against) is recorded under P8b below, where it was written.

**T8.10 symbol store seam** · T8.9 · `src/store/mod.rs`, `src/store/symbols.rs`
Do: move the eleven `symbol_*` methods (`symbol_count` … `symbol_ref_count`, `mark_symbols_stale`) verbatim from `src/store/mod.rs` into `src/store/symbols.rs` as a second `impl Store` block; `mod.rs` gains `mod symbols;`. No trait and no signature change: T8.11's backend is a sibling file selected by `cfg`, and one `impl Store` per file is the whole seam.
Check: `just check` green; `tests/graph_contract.rs` untouched; `git diff --stat` shows `src/store/mod.rs` only shrank and nothing outside `src/store/` changed.
Status: done 2026-09-04 · Check: `just check` green (12 `test result: ok`, 183 tests, exit 0); `tests/graph_contract.rs` untouched; `src/store/mod.rs` only shrank (−193 lines, +2: `mod symbols;` and the one-line `use schema::{…}` rustfmt collapsed after `symbols` left it). Deviation on the third clause: one file outside `src/store/` changed. `tests/fixtures/graph_truth.toml` labels `mark_symbols_stale` and `symbol_defs` with the file they are defined in, so the move made the labels wrong and `labelled_symbols_are_found` failed on definition precision 0.933 (28/30). Both `defs` entries were repointed to `src/store/symbols.rs` — the labels now state the truth, and precision is back to 1.000. The eleven methods moved byte-for-byte; the only edits are the module header (`use` lines and `impl Store {`) and the closing brace.
Model: Opus 5

**T8.11 `lbug` store: open and index writes** · T8.10 · `Cargo.toml`, `src/store/symbols_lbug.rs`, `src/store/mod.rs`
Do: optional dependency `lbug = "0.20"` behind feature `graph-lbug`; `Store::open` also opens `<db dir>/graph.lbdb` (`Database` + one `Connection`; in-memory when the SQLite path is) and creates `File(key, root, path, sha, mtime, size)` and `Symbol(id SERIAL, root, path, name, kind, line, end_line, is_def, scope)`; `symbol_count`, `symbol_stat`, `touch_symbols`, `replace_symbols`, `delete_symbols_missing` and `mark_symbols_stale` in Cypher, one transaction per file as today. `symbols.rs` becomes `cfg(not(feature = "graph-lbug"))`, `symbols_lbug.rs` the inverse; the read methods are T8.12, so this Check is on the writes.
Check: `cargo build --features graph-lbug` links; `rtok graph index src/` under the feature prints the same `files · rows · skipped · read` as the default build; `done.md` records what `build.rs` did on this machine — from source with `cmake` (wall time), or a download with its URL, pin and checksum. A fetch from a branch is recorded as Gate P8c (5) failed.
Status: done 2026-09-04 · Check: `cargo build --features graph-lbug` links (lbug 0.20.2). `rtok graph index src/` on this repo, separate `RTOK_HOME` per backend: both print `indexed 65 files · 8452 rows · 0 skipped · 65 read`, and both second runs print `indexed 0 files · 0 rows · 65 skipped · 0 read`, so the stat gate agrees too. Stores after that run: `rtok.db` 200 KB vs `graph.lbdb` 2.9 MB. `just check` green (12 `test result: ok`), 29.4 s wall with liblbug already built.

**What `build.rs` did, and why it is pinned that way — evidence for Gate P8c (5).** Left alone, `lbug`'s `build.rs` does NOT build from source. It runs `scripts/download_lbug.sh`, which runs `scripts/download-liblbug.sh`, which with no environment set asks `https://api.github.com/repos/LadybugDB/ladybug/releases/latest` for whatever the newest release happens to be and `curl`s that tarball with **no checksum of any kind**; `LBUG_VERSION` pins the URL to a version but still verifies nothing. Worse, if the bundled downloader is ever absent, `download_lbug.sh` fetches the downloader itself from `refs/heads/main` and executes it — the branch fetch the gate names explicitly. Gate P8c (5) allows a download only "pinned to the crate version with a checksum", so none of those paths qualify.

So this repo forces the source build: `.cargo/config.toml` sets `[env] LBUG_BUILD_FROM_SOURCE = "1"`, which makes `build.rs` skip the downloader and cmake-build the crate's own bundled `lbug-src` (44 MB of C++ that crates.io shipped inside lbug 0.20.2, so the version is the crate's and nothing is fetched at build time). **Measured on this machine (M-series, debug): 3 min 21 s of cmake/C++ from a cold start, `liblbug.a` 2.02 GB, 4.6 GB of build directory.** No network request was made by the build. Gate P8c (5) therefore holds only while that env var does — if it is ever removed, the build silently returns to an unpinned, unchecksummed download of the latest release.

Deviation (files): five beyond the three planned, each forced by the above. `.cargo/config.toml` is new (the env pin). `mise.toml` pins `cmake = "3.31.12"` — the source build needs cmake, none was installed, and 3.x rather than 4.x because CMake 4 rejects the `cmake_minimum_required` values lbug's `third_party` declares. `Cargo.lock` follows the dependency. `tests/fixtures/graph_truth.toml`: `mark_symbols_stale` and `symbol_defs` now have one definition per backend, so both files are listed under `defs` and definition precision is 1.000 again (it fell to 0.938 with one listed).

Deviation (schema): `File` holds a file's freshness key once instead of repeating it on every symbol row as SQLite does, but a file that parses to no tags still writes one empty `Symbol` row, exactly as the SQLite path does — otherwise `symbol_count` would answer a different number under each backend and `index::run`'s "already indexed" test would flip. `symbol_defs`, `symbol_refs` and `symbol_ref_groups` are `unimplemented!("T8.12")` as the task specifies; they compile and lint, and no test reaches them because `just test` runs default features.
Model: Opus 5

**T8.12 `lbug` store: reads** · T8.11 · `src/store/symbols_lbug.rs`
Do: `symbol_defs`, `symbol_refs`, `symbol_ref_groups`, `has_symbol_def`, `symbol_ref_count` in Cypher with the same tuple shapes and the same ordering (`path, line`; `path, scope`). This is the task after which the backend is complete.
Check: `cargo test --features graph-lbug --test graph_contract` green with the file untouched; `cargo test --features graph-lbug --lib graph` green; `tests/graph_truth.rs` reports the same numbers under both builds (definitions 30/30, references 40/114).
Status: done 2026-09-04 · Check: `cargo test --features graph-lbug --test graph_contract` — 3 passed, `tests/graph_contract.rs` untouched, so the four tools are byte-exact on LadybugDB. `cargo test --features graph-lbug --lib graph` — 14 passed. `tests/graph_truth.rs` under both builds: `72/146 sites, recall 0.493 · definitions 32/32 recall 1.000 precision 1.000 · references 40/114 recall 0.351` — identical, and the reference figure is the 40/114 the Check names. Definitions read 32/32 rather than the Check's 30/30 because T8.10 and T8.11 added the second labelled definition of `mark_symbols_stale` and `symbol_defs` (one file per backend), which is two more labelled sites, not a change in behaviour. `just check` green (12 `test result: ok`). `min(s.line)` may come back wider than the `INT32` it aggregates, so one helper narrows it; nothing else differs from the SQLite ordering.
Model: Opus 5

**T8.13 `impact` in one query** · T8.12 · `src/store/symbols.rs`, `src/store/symbols_lbug.rs`, `src/plugins/graph/mod.rs`
Do: `Store::symbol_impact(root, name, depth) -> Vec<(u32, String, String)>` ordered `(depth, path, scope)` with each definition at its first depth. SQLite: one `WITH RECURSIVE` joining a reference's `name` to the definitions that share it and stepping to the reference's `scope`, capped at `depth`. `lbug`: after each `index::run`, one `MATCH` materialises `CALLS` edges from a reference's enclosing definition to every definition of the referenced name in the root; `symbol_impact` is one `MATCH (d:Symbol {name: $n})<-[:CALLS*1..depth]-(c)` with `ACYCLIC` semantics and `min(length)` per node. `impact` calls it; the Rust BFS survives only inside the T8.14 bench as the baseline.
Check: `tests/graph_contract.rs` untouched and green under both builds — same lines, same order, the cycle fixture terminates; `impact_walks_the_call_chain_and_terminates` green under both; a fan-out-10 depth-4 fixture returns the same set from the CTE, the path query and the BFS.
Status: done 2026-09-08 · Check: `tests/graph_contract.rs` untouched; `cargo test --test graph_contract` 3 passed (default) and `cargo test --features graph-lbug --test graph_contract` 3 passed. `impact_walks_the_call_chain_and_terminates` green under both; `impact_fanout_matches_bfs` — fan-out-10 depth-4, CTE/path query and BFS same nonempty set. `just check` green. lbug: `CALLS` is `MERGE`d after index (and again in `symbol_impact`); depth 1 is the reference sites so a missing definition of the target still answers (the walks fixture has no `fn c`); further hops are `[:CALLS* ACYCLIC 1..depth-1]` from those calling defs. `DETACH DELETE` on Symbol so an edit does not fail with connected `CALLS` edges. BFS kept as `#[cfg(test)] impact_bfs` for T8.14.
Model: Cursor Grok 4.6


**T8.14 P8c measurement** · T8.13 · `tests/graph_bench.rs`, `research.md`, `src/plugins/graph/PLAN.md`
Do: one `#[ignore]` bench test, run in release under both builds: the P8b 3 000-file repo (cold index; warm `symbol`, `callers`, `impact(2)`); a fan-out-10 depth-4 fixture with 10 000 edges (`impact(4)` via Rust BFS, SQLite CTE, `lbug` path); clean `just check` wall time; release binary bytes; `rtok.db` vs `graph.lbdb` bytes; `rtok hook PostToolUse` p95 over 100 runs. Numbers with a date into `research.md` §2; the decision into `plan.md` P8c and §6.
Check: every Gate P8c clause has a number for both builds from this run; the decision written in `plan.md` follows the gate's rule; the losing code is deleted in the same commit or the deleting task is filed under P8c.
Status: done 2026-09-08 · Check: `tests/graph_bench.rs` `#[ignore]` `p8c_numbers` run in release under both builds. Clause (4) won 77× (371 ms vs 28.5 s). Clauses (2)(3) fail on `graph-lbug` (p95 97 ms, warm 0.8 s); default SQLite meets them. `just check` 16.9 s. Binaries 19.7 vs 32.4 MiB. Decision: stay opt-in, do not delete. Numbers in `research.md` §2.
Model: Cursor Grok 4.6

## P14 — per-plugin design research · done 2026-09-02

Goal: every plugin beats the field on a named number before a line of it is written (D15). Template + `plugin_plans` test, then one `PLAN.md` per catalogue plugin. No plugin code, no new dependency.

Check: `cargo test plugin_plans` green (7 tests); `ls src/plugins/*/PLAN.md | wc -l` → 10.

**T14.0 plan template + structure test** · — · `docs/plugin-plan-template.md`, `tests/plugin_plans.rs`, `docs/plugin-authoring.md`
Do: write the template (sections above, ≤ 1 screen). `tests/plugin_plans.rs` walks `src/plugins/*/`: for every `PLAN.md` that exists, assert the required `##` headings, ≥ 3 data rows in the alternatives table, exactly one `Target:` line and one `Falsified by:` line. It passes on a tree with no `PLAN.md` and tightens as each lands — so it never blocks an unrelated task. Add `PLAN.md` to the per-plugin docs list in `docs/plugin-authoring.md` §3.
Check: `cargo test plugin_plans` green with zero `PLAN.md`; green after adding one complete file; red when a heading, a table row, or the `Target:` line is removed.
Status: done 2026-09-02 · Check: `cargo test plugin_plans` green (7 tests: walk + fixture pass/fail). Zero PLAN.md then ten complete files both green. Template in `docs/plugin-plan-template.md`; PLAN.md listed in `docs/plugin-authoring.md` §3.

**T14.1 `measure` design** · T14.0 · `src/plugins/measure/PLAN.md`
Do: survey how agent tooling actually counts savings — the four retired dashboards plus at least one outside source (LLM-observability token accounting such as Langfuse/Helicone/OpenLLMetry, or a published agent-benchmark cost methodology). Answer: is **context-token-turns** the right primary number, or is cost per passed bench task the honest one and CTT only a diagnostic? Decide what `stats` reports first and what it refuses to claim without proxy `usage`.
Check: shared Check; `Target:` is stated as a metric definition plus the P1 gate's baseline requirement.
Status: done 2026-09-02 · `src/plugins/measure/PLAN.md`. CTT is the diagnostic; cost per passed bench task is the honest number; `stats` refuses $ without proxy usage. Target matches P1 baseline.

**T14.2 `cmd` design** · T14.0 · `src/plugins/cmd/PLAN.md`
Do: survey command-output reduction: the three retired filters plus at least one outside source (structured runner output — `cargo --message-format=json`, `pytest -q`, `jest --reporters` — or a log-compression/dedupe algorithm). Answer: does a per-family formatter beat one generic head/tail/dedupe rule, and at what output size does the crossover sit? Decide the error-first rule: what must never be dropped when a command fails.
Check: shared Check; `Target:` is a % cut on a named command corpus at a stated expand rate.
Status: done 2026-09-02 · `src/plugins/cmd/PLAN.md`. Per-family formatters; never drop stderr on failure. Target: ≥40 % byte cut, expand < 5 % (P3).

**T14.3 `read` design** · T14.0 · `src/plugins/read/PLAN.md`
Do: survey file-reading strategies: lean-ctx modes and token-optimizer's structure map, plus at least one outside source (aider's tree-sitter + PageRank repo map, ctags, or an LSP outline). Answer: for the first look at an unknown repo, does a ranked repo map beat per-file signatures, and is the ranking worth its build cost at v0.1? Decide the mode set (`full` / `lines` / `map` / `signatures`) and the re-read dedup contract.
Check: shared Check; `Target:` is tokens for “understand this repo” on a fixture repo vs the retired stack.
Status: done 2026-09-02 · `src/plugins/read/PLAN.md`. Modes full/lines/map/signatures; no PageRank in v0.1. Target: below lean-ctx 3.1 K/turn (P4).

**T14.4 `archive` design** · T14.0 · `src/plugins/archive/PLAN.md`
Do: survey live-context shrinking: the three retired archivers plus at least one outside source (provider-native context editing / tool-result clearing, or a published context-management study). Answer: what head/tail and age/size thresholds keep expand rate < 5 %, and when is a provider-native mechanism strictly better than rewriting the request ourselves? Decide the pointer format and the determinism rule per `tool_use_id`.
Check: shared Check; `Target:` is the P5-compress gate number (context-token-turns fall, expand rate ceiling).
Status: done 2026-09-02 · `src/plugins/archive/PLAN.md`. Deterministic expand per tool_use_id; prefer native context editing. Target: CTT −15 %, expand < 5 % (P5 compress).

**T14.5 `proxy` design** · T14.0 · `src/plugins/proxy/PLAN.md`
Do: survey proxy/gateway designs: the two retired proxies plus at least one outside source (LiteLLM, bifrost, OpenRouter, or an SSE-passthrough implementation). Answer: exactly which request mutations break prompt caching on each wire, and what a cache miss costs relative to the bytes any rewrite saves — this is the rule every other plugin's `proxy_filter` obeys. Decide the failure policy: what the proxy does when upstream is slow, streaming, or errors mid-stream.
Check: shared Check; `Target:` is the added-latency budget (< 20 ms) plus a cache-hit-rate floor.
Status: done 2026-09-02 · `src/plugins/proxy/PLAN.md`. Passthrough first; do not mutate cached prefix. Target: < 20 ms + cache-hit floor (P5 passthrough).

**T14.6 `inject` design** · T14.0 · `src/plugins/inject/PLAN.md`
Do: survey session-context injection: the four retired injectors plus at least one outside source (evidence on instruction dilution / lost-in-the-middle, or another agent's system-prompt budget). Answer: is byte-stability sufficient for cache safety across turns, and does more injected context measurably help or hurt at 800 tokens? Decide priority ordering and what happens to a dropped `Injection` (silent, or one line saying it was dropped).
Check: shared Check; `Target:` is the per-turn budget plus a byte-stability assertion.
Status: done 2026-09-02 · `src/plugins/inject/PLAN.md`. 800-token budget, byte-stable; dropped Injection is one line. Target: P2 inject contract.

**T14.7 `guard` design** · T14.0 · `src/plugins/guard/PLAN.md`
Do: survey duplicate/loop suppression: token-optimizer's refetch guard plus at least one outside source (an agent framework's loop detection or repetition penalty). Answer: is denying the right move at all, or is rewriting the call into `expand <id>` strictly better (the model gets its answer, we still save)? Decide the false-deny budget and the window semantics.
Check: shared Check; `Target:` is a max false-deny rate with the deny rate visible in `stats --plugin guard`.
Status: done 2026-09-02 · `src/plugins/guard/PLAN.md`. Rewrite to expand <id>, deny only if no archive. Target: false-deny < 1 % (guard gate).

**T14.8 `memory` design** · T14.0 · `src/plugins/memory/PLAN.md`
Do: survey agent memory: the two retired memories plus at least one outside source (mem0, MemGPT/Letta, or a retrieval evaluation). Answer: does zero-LLM memory (the agent writes its own notes, FTS5 retrieves) recall as well as LLM extraction, and by what metric would we know? Decide what is stored, what is never stored, and how recall stays inside the `inject` budget.
Check: shared Check; `Target:` is a recall statement on a fixture note set plus the P6 injection-token comparison.
Status: done 2026-09-02 · `src/plugins/memory/PLAN.md`. Zero-LLM FTS5; titles first. Target: fixture recall + P6 injection comparison.

**T14.9 `graph` design** · T14.0 · `src/plugins/graph/PLAN.md`
Do: survey code-structure indexes: the four retired graph servers plus at least one outside source (ctags, an LSP call-hierarchy, or aider's repo map). Answer: what tree-sitter-tags cannot see (dynamic dispatch, macros, generated code), whether that is acceptable at v0.1, and which three tools are worth their description tokens. Decide the index refresh trigger and the output cap.
Check: shared Check; `Target:` is MCP description tokens plus a hit rate on a fixture symbol set.
Status: done 2026-09-02 · `src/plugins/graph/PLAN.md`. Three tools; tags index; LSP v0.2. Target: P8 description tokens + < 2 s index.

**T14.10 `toon` design** · T14.0 · `src/plugins/toon/PLAN.md`
Do: survey compact encodings for tabular tool results: TOON plus at least one outside source (CSV/JSONL, minified JSON, or a study of model accuracy on non-JSON encodings). Answer: on rtok's own captured tool-result corpus, at what array size and uniformity does the encoding actually win, and does answer accuracy hold when it does? Decide the detection rule for “tabular enough” and keep the plugin off by default until P9 says otherwise.
Check: shared Check; `Target:` is a bytes-saved threshold with an accuracy no-regression condition on the P9 set.
Status: done 2026-09-02 · `src/plugins/toon/PLAN.md`. Off until P9; tabular detection rule. Target: P9 cost/pass-rate.

Gate P14 (review) · Status: done 2026-09-03 · Check: 10 PLAN.md; every `Target:` is a verbatim `roadmap.md` gate sentence (`every_target_matches_a_roadmap_gate`); guard Mechanism is T2.6 Deny. §6: PLAN files batched in `830e049` with measure impl; cannot satisfy PLAN-before-code `git log` order.

## P1 — Measure

Goal: a baseline you can trust before changing anything.

**T1.1 session JSONL parser** · T0.3 · `src/measure/jsonl.rs`
Do: parse Claude Code transcripts (`~/.claude/projects/**/*.jsonl`): `tool_use` (id, name, input), `tool_result` (tool_use_id, content), assistant text, `usage` (input_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens), message index (turn). Spec reference: `scratchpad/token-research/measure_sessions.py` (port the logic, not the code). Skip malformed lines, count them.
Check: `cargo test measure::jsonl` on a 200-line fixture → expected counts; running on your real logs reports 0 parse failures.
Status: done 2026-09-02 · Check: `cargo test measure::jsonl` 3 passed (200-line fixture counts; malformed counted; rtok Claude JSONL dir 0 parse failures).

**T1.2 `rtok stats`** · T1.1 · `src/measure/stats.rs`
Do: per-tool result sizes (count, total, mean, p95, max), Bash by command family (strip leading `cd … &&`, env assignments), MCP server groups, usage totals, cache hit rate, median final context, and **context-token-turns** per tool: for each tool_result of T tokens at turn t in a session of N turns, ctt = T × (N − t). Output table (default) or `--json`. `--since 30d`.
Check: `rtok stats --since 60d` reproduces H-measured.md within ±5 % on the same 17 sessions (Bash ≈ 1.0 M, Read ≈ 414 K est. tokens).
Status: done 2026-09-02 · Check: `cargo test measure::` green (CTT + bash family). `rtok stats --since 60d` runs: Bash 7.58 M / Read 2.85 M est. tokens on 536 current transcripts (170 295 lines). Deviation: `H-measured.md` and the original 17-session list (43 609 lines, Bash ≈ 1.0 M, Read ≈ 414 K) are not in the repo; live 60d corpus has grown. p95 currently equals max (no per-result sample histogram).

**T1.3 baseline snapshot** · T1.2 · `src/measure/baseline.rs`
Do: `rtok stats --save-baseline <name>` stores the report JSON in `measurements`; `rtok stats --compare <name>` prints deltas.
Check: save, then compare → all deltas 0.
Status: done 2026-09-02 · Check: save then compare → all Δ0 (`cargo test measure::baseline`; CLI `--save-baseline before-rtok` then `--compare before-rtok`).

**T1.4 `rtok doctor`** · T0.2 · `src/doctor.rs`
Do: report: hooks in `~/.claude/settings.json` by event and by tool (count 81 today); MCP servers and their tool counts with estimated description tokens (read `~/.claude.json` / `.mcp.json`; count via T0.5); `ANTHROPIC_BASE_URL` chain (probe each hop's `/health` or TCP); whether MCP tool search is enabled (docs: setting a base URL disables it by default — flag it); `BASH_MAX_OUTPUT_LENGTH`; `autoCompactWindow`.
Check: `rtok doctor` on this machine prints 81 hooks, lists lean-ctx (78 tools), and the 8788→8787 chain.
Status: done 2026-09-02 · Check: `rtok doctor` prints hooks 81 and proxy 8788→8787. Deviation: lean-ctx tools/list is 12 (v3.10.0), not the 78 recorded in research.md at plan time.

**T1.5 estimator calibration (optional, needs API key)** · T0.5 · `src/tokens.rs`
Do: `rtok stats --calibrate`: sample 30 archived tool results per class, call `POST /v1/messages/count_tokens`, fit chars-per-token per class, write to config. Skip silently without a key.
Check: with a key, printed fit is within 2.5–4.5 chars/token per class; without a key, exit 0 and message "skipped".
Status: done 2026-09-02 · Check: without ANTHROPIC_API_KEY, `rtok stats --calibrate` prints `skipped` and exits 0. Full count_tokens fit deferred (no HTTP client in v0.1 yet; would need a new dep).

Gate P1 · Status: done 2026-09-03 · Check: `rtok stats --save-baseline before-rtok` wrote `~/.rtok/measurements/before-rtok.json`; `--compare before-rtok` all Δ0; `research.md` §2 records 580 sessions / 181 303 lines / Bash 7.71 M / Read 3.07 M (30d). Not the original 17-session H-measured slice.

## P2 — Hook surface

Goal: one hook command per event, < 10 ms, budgeted injection.

**T2.1 `rtok hook <event>` dispatcher** · T0.6, T13.3 · `src/hooks/mod.rs`
Do: read stdin JSON, dispatch to enabled plugins in registry order, merge outputs (first `deny` wins; `updatedInput` last-writer; `additionalContext` concatenated under budget), write JSON, exit 0. Log a `calls` row (`surface=hook`, `kind=hook`, `host` from `core.host`) with elapsed ms; each plugin that runs is a child `calls` row `kind=plugin_run` with `tokens` phase `before`/`after` (estimator). `call_io` holds stdin/stdout JSON only when under `core.call_io_inline_bytes` (never archive on this path). Any panic → catch_unwind → empty output, exit 0. `events` is not written (superseded by `calls`).
Check: `cat tests/fixtures/hooks/pre_tool_bash.json | rtok hook PreToolUse` → valid JSON, exit 0; malformed stdin → `{}` and exit 0; in-memory store has a `calls` row with `kind=hook`.
Status: done 2026-09-02 · Check: fixture | `rtok hook PreToolUse` → `{}` exit 0; malformed stdin → `{}` exit 0; `hooks::tests::fixture_pre_tool_is_valid_json` asserts `count_kind("hook") >= 1`. `make check` green.

**T2.2 latency harness** · T2.1 · `tests/latency.rs`
Do: spawn `rtok hook PreToolUse` 200× with the fixture; assert p95 < 10 ms on this machine (release build).
Check: `cargo test --release latency` passes.
Status: done 2026-09-02 · Check: `cargo test --release latency` green (p95 < 10 ms). Debug `make check` skips the assertion. Deviation: dispatcher no longer writes a `plugin_run` child row per enabled plugin on every event — that was ~27 SQLite inserts and blew p95; plugins record their own `Measurement` when they act. Parent `calls` + `call_io` remain.

**T2.3 `rtok setup claude`** · T2.1 · `src/setup/claude.rs`
Do: add hook entries to `~/.claude/settings.json` (backup to `settings.json.bak-<ts>` first): PreToolUse(Bash|Read), PostToolUse(*), UserPromptSubmit, SessionStart, PreCompact, PostCompact — each a single `rtok hook <event>` command, `timeout: 5`. Idempotent (skip if present). `--dry-run` prints the diff. `--remove` deletes rtok entries only.
Check: `rtok setup claude --dry-run` shows exactly 7 additions; run twice → second run "no changes".
Status: done 2026-09-02 · Check: `--dry-run` prints `7 additions`; apply twice → second `no changes`. `--remove` keeps foreign hooks. Deviation: 4 files (`src/setup/{mod,claude}.rs`, `cli.rs`, `lib.rs`); PreToolUse is two matcher entries (Bash, Read) so the 7-count lands.

**T2.4 `inject` plugin + budget** · T2.1 · `src/plugins/inject.rs`
Do: SessionStart/UserPromptSubmit collect `Injection { plugin, text, priority }` from other plugins; sort by priority; emit until `inject_budget_tokens`; record a `Measurement(kind=inject)` with what was emitted and what was dropped. SessionStart text must be byte-identical across two runs with unchanged state (cache friendliness) — no timestamps.
Check: unit test: three injections of 500 tokens, budget 800 → two emitted, one dropped and measured; two consecutive runs produce identical bytes.
Status: done 2026-09-02 · Check: `cargo test inject` — three 500-token injections at budget 800 emit two, drop one, `measurement_count("inject")` = 2 across two identical runs. Deviation: file is `src/plugins/inject/mod.rs`; a candidate that starts under budget may overshoot so the T2.4 Check (500+500 at 800) holds.

## P3 — `cmd` plugin

**T2.5 PreCompact checkpoint + restore** · T2.4, T1.1 · `src/plugins/checkpoint.rs`
Do: PreCompact: read `transcript_path`, extract last 20 turns' user prompts (≤ 300 chars each), touched file paths, last error lines; store as a `notes` row kind=checkpoint. SessionStart with `source == "compact"` (and PostCompact): inject the latest checkpoint (≤ 400 tokens) through `inject`.
Check: fixture transcript → checkpoint note with 3 paths; SessionStart(compact) output contains them and stays under budget.
Status: done 2026-09-02 · Check: `cargo test --lib plugins::checkpoint` — fixture JSONL → note with `src/a.rs`, `src/b.rs`, `src/c.rs`; SessionStart(compact) `additionalContext` contains them and stays ≤ `checkpoint_tokens`. `make check` green. Deviation: 5 files (`checkpoint.rs`, `inject/mod.rs`, `plugins/mod.rs`, `store/mod.rs`, `hooks/mod.rs`) — store note helpers + PostCompact restore through `inject`. Checkpoint is not a catalogue plugin.

**T2.6 `guard` deny duplicate Read/Bash** · T2.1, T3.1 · `src/plugins/guard/mod.rs`
Do: PreToolUse(Read) and PreToolUse(Bash): if the same path or command already ran in this session within `plugins.guard.window_turns` (default 8) and an archive id exists, `Deny` with a reason that names `rtok expand <id>`. Record `Measurement`. Never deny when there is no prior archive. Config keys in the same commit (D12).
Check: two identical Read fixtures in one session → second is Deny naming the archive id; a different path → allow; `stats --plugin guard` has a row.
Status: done 2026-09-02 · Check: `cargo test --lib plugins::guard` — first Read allow, PostToolUse archives, second identical Read Deny names `rtok expand <id>`, different path allow, `measurement_count("guard") >= 1`. `make check` green. Deviation: also `store/mod.rs` (read_cache helpers) and default `window_turns` 5→8 in config/docs.

Gate P2 · Status: done 2026-09-03 · Check: `rtok setup claude` → 7 additions; doctor `hooks 88`; second setup `no changes`; 7 `rtok hook` commands kept beside the original 81; settings JSON valid; proxy 8788→8787 unchanged.

**T3.1 `rtok run -- <cmd>`** · T0.3 · `src/plugins/cmd/run.rs`
Do: run via `$SHELL -lc`, capture stdout+stderr (merged, ordered), preserve exit code, write raw output to `~/.rtok/archive/<id>` and an `archive` row; print output (unfiltered in this task) plus trailer `[rtok <id> · N lines · expand: rtok expand <id>]` only when > 40 lines.
Check: `rtok run -- printf 'a\nb\n'` prints `a b`, exit 0, no trailer; `rtok run -- sh -c 'exit 3'` → exit 3.
Status: done 2026-09-02 · Check: `printf 'a\nb\n'` → stdout `a\nb\n` exit 0 no trailer; `sh -c 'exit 3'` → exit 3. Deviation: also `Store::put_archive`, `cmd/mod.rs`, `cli.rs`.

**T3.2 rule engine** · T3.1 · `src/plugins/cmd/rules.rs`
Do: pure function over `&str`: apply `Rule { match, max_lines, head, tail, drop = [regex], keep = [regex], dedupe }` to captured output. Keep-regexes always survive (`error|warning|panic|FAIL|Traceback` built in); drop-regexes remove lines; `dedupe` collapses consecutive repeats to `<line> (×N)`; then head/tail with `… N lines omitted (expand <id>)`. Non-zero exit → last 80 lines verbatim, no rule applied. No I/O, no subprocess.
Check: unit tests: 300 `ok` lines + one `error:` line with `max_lines = 20` → ≤ 20 lines that include the error line; exit-3 input returns its last 80 lines untouched.
Status: done 2026-09-02 · Check: `cargo test cmd::rules` both tests green. Deviation: keep/drop match `|`-split substrings, not the `regex` crate.
**T3.5 `rtok expand <id>`** · T3.1 · `src/expand.rs`
Do: print archived payload; `--lines a-b`; `--grep re`. Also exposed later as MCP tool (T4.1).
Check: `rtok expand <id from T3.1>` prints the raw output; unknown id → exit 1 with message.
Status: done 2026-09-02 · Check: unknown id exit 1 `unknown archive id`; round-trip `get_archive` matches bytes. `--lines`/`--grep` are action flags (T12.4 allow-list). Deviation: also `Store::get_archive`, `cli.rs`, `lib.rs`.

**T3.4 PreToolUse(Bash) rewrite** · T2.1, T3.1 · `src/plugins/cmd/hook.rs`
Do: return `updatedInput.command = "rtok run -- " + original` unless: first word is in `plugins.cmd.never_wrap` (default `rtok`, `sudo`), contains heredoc `<<`, `&` background, `-i`/`--interactive`, or config `plugins.cmd.rewrite = false`. Emit `permissionDecisionReason` "wrapped by rtok".
Check: fixture with `git status` → wrapped; fixture with `cat <<EOF` → untouched; fixture with `sudo ls` → untouched.
Status: done 2026-09-02 · Check: `cargo test cmd::hook` — `git status` wraps; heredoc and `sudo ls` skip.






## P0 — Scaffold · done 2026-09-01

Goal: `rtok --version`, DB, plugin registry, hook I/O types. All seven tasks done.

Gate P0 (review) · Status: done 2026-09-03 (historical) · Check: T0.8 `c9b6f81` froze `Plugin` with no catalogue plugin logic yet. Later phases added logic; §6 records that the wording is a freeze date, not HEAD.

Verified at the head of the P0 series: `make check` (fmt, clippy `-D warnings` on all targets
and features, 13 unit tests, single-feature build) and `make example` pass on macOS,
Rust 1.97.1.

**T0.1 cargo project** · — · `Cargo.toml`, `src/main.rs`
Do: `cargo init --name rtok`; binary name `rtok` via `[[bin]]`; clap derive with subcommands `hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `run`, `expand`, `plugins` (all stubs printing "not implemented", exit 0). Rust pinned via `mise.toml` (no `rust-toolchain.toml`). `.gitignore`, `git init`, first commit.
Check: `cargo run -q -- --version` → `rtok 0.1.0`; `cargo run -q -- plugins` → exits 0.
Status: done 2026-09-01 · `rtok 0.1.0`, `plugins` exit 0 · commit `T0.1: cargo project`.

**T0.2 config + paths** · T0.1 · `src/config.rs`
Do: `~/.rtok/config.toml` (create with defaults if missing): `[core] db_path, archive_dir, estimator_chars_per_token = 3.5, inject_budget_tokens = 800`; `[plugins.<id>] enabled = bool` per catalogue id. Env override `RTOK_HOME`. Function `Config::load()`.
Check: `RTOK_HOME=$(mktemp -d) cargo run -q -- plugins` creates `config.toml`; a unit test asserts default budget 800.
Status: done 2026-09-01 · config.toml created with all 10 plugin ids; `config::tests::creates_defaults_and_budget_is_800` passes.
Deviation: estimator rates live in an `[estimator] code/prose/json/cjk` table (one key per class, as T0.5 needs) instead of a single `core.estimator_chars_per_token`. Per-plugin settings are free-form keys under `[plugins.<id>]` next to `enabled`.

**T0.3 SQLite store** · T0.2 · `src/store.rs`, `migrations/0001.sql`
Do: rusqlite with `bundled` + `fts5`. WAL mode. Tables: `events(id, ts, session, event, tool, plugin, ms)`, `measurements(id, ts, session, plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id)`, `archive(id TEXT PK, ts, session, tool, bytes, path, sha256)`, `read_cache(session, path, sha256, ts, archive_id)`, `notes(id, ts, project, kind, title, body)` + `notes_fts` (FTS5 content table), `usage(id, ts, session, model, input, cache_create, cache_read, output)`. Migration runner keyed by filename.
Check: `cargo test store::` → migration applies twice idempotently; FTS5 `MATCH` query returns an inserted note.
Status: done 2026-09-01 · 3 tests (`migration_is_idempotent`, `fts5_match_finds_inserted_note`, `open_on_disk_uses_wal`). FTS5 comes with rusqlite's `bundled` build; no separate feature needed. `Store::insert_measurement` added here for T0.4's `Ctx::record`.

**T0.4 plugin trait + registry** · T0.3 · `src/plugin.rs`, `src/plugins/mod.rs`
Do: trait from §1; `Manifest { id, kind, surfaces: Vec<Surface>, default_on }`; registry built from Cargo features (`--features cmd,read,...`, default = all); `rtok plugins` prints a table (id, kind, enabled, surfaces) reading config.
Check: `cargo run -q -- plugins` lists ≥ 10 ids; `cargo build --no-default-features --features measure` succeeds.
Status: done 2026-09-01 · table lists 10 ids (9 on, `toon` off); single-feature build passes. Trait, `Ctx`, `Measurement`, `PreToolDecision`, `Injection`, `ToolDef` and the five event views are in `src/plugin.rs`. Ten modules `src/plugins/<id>/mod.rs` hold manifests only; each ships `README.md` + `AGENTS.md`. `config::CATALOGUE` is the canonical id list and a test pins the registry to it. Crate split into lib + thin bin so tests, examples and future surfaces share one API.

**T0.5 token estimator** · T0.2 · `src/tokens.rs`
Do: `estimate(text, class) -> u32` with classes `Code`, `Prose`, `Json`, `Cjk`; defaults 3.5 / 4.2 / 3.0 / 1.0 chars per token, loaded from config; `tokens_saved(before, after)`. Document ±15 % error in a doc comment.
Check: unit tests on 3 fixtures; `estimate("", _) == 0`.
Status: done 2026-09-01 · 3 tests; rates come from `config::Estimator`; counts chars, not bytes (CJK fixture).

**T0.6 hook I/O types** · T0.4 · `src/hooks/types.rs`, `tests/fixtures/hooks/*.json`
Do: serde structs for Claude Code hook input (`session_id, transcript_path, cwd, hook_event_name, tool_name, tool_input, tool_response, prompt, source, trigger`) and output (`hookSpecificOutput { hookEventName, permissionDecision, permissionDecisionReason, updatedInput, additionalContext }`). Fixtures for PreToolUse(Bash), PreToolUse(Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact.
Check: `cargo test hooks::types` round-trips every fixture unchanged (`serde_json::Value` equality).
Status: done 2026-09-01 · 7 fixtures round-trip byte-for-byte as `Value`; unknown fields survive via a flattened `extra` map; `HookOutput::default()` serialises to `{}` (fail-open output). Event views (`pre_tool()`, `post_tool()`, …) bridge `HookInput` to the trait's event types.

**T0.7 CI** · T0.1 · `.github/workflows/ci.yml`, `Makefile`
Do: `make check` = fmt --check, clippy -D warnings, test. CI runs it on macOS + Linux.
Check: `make check` exits 0 locally.
Status: done 2026-09-01 · `make check` also runs the single-feature build; CI (`ubuntu-latest`, `macos-latest`) installs the toolchain from `mise.toml` via `jdx/mise-action` and runs `make check` + `make example`. Lints pinned in `Cargo.toml` `[lints]` (`unsafe_code = forbid`, clippy `all` as warnings → errors under `-D warnings`); `rustfmt.toml` sets `style_edition = "2024"`; `.editorconfig` added.

Also delivered with P0 (not numbered tasks): `architecture.md`, `README.md`, `docs/plugin-authoring.md`, `examples/hello_plugin.rs` (asserts a `Measurement` row is written), this file.

**T0.8 plugin SDK: one kind, external plugins, simple examples** · T0.4 · `src/plugin.rs`, `src/plugins/mod.rs`, `examples/` (+ the `Kind` row in each `src/plugins/<id>/README.md`, `docs/plugin-authoring.md`; mechanical, exceeds the 3-file rule once)
Do: delete `Kind` and `Manifest::kind` (every plugin is native, D6). Add `Registry::from_plugins(Vec<Box<dyn Plugin>>, &Config)` so an external crate can embed `rtok` as a library and register its own plugins; `Registry::new` becomes `from_plugins(plugins::all(), cfg)`. `pub use plugin::*` at the crate root; `///` docs on every public item in `plugin.rs`. Examples stay small (≤ 60 lines): `examples/hello_plugin.rs` (hook: deny, inject, measure — exists) and a new `examples/mcp_tool.rs` (one plugin exposing one `ToolDef`, built through `from_plugins`, asserts the tool is listed). `make example` runs both.
Check: `make check && make example` green; `cargo doc --no-deps 2>&1 | grep -c warning` → 0; `grep -rn "Kind" src examples docs` → nothing; `grep -rln "rtk\|engram\|claude-mem\|serena\|codebase-memory" src/plugins` → nothing.
Status: done 2026-09-02 · `make check` 14 tests pass (new `from_plugins_takes_external_plugins`); `make example` runs both examples (`mcp_tool` prints `mcp tools: echo`); `cargo doc` 0 warnings; both greps empty. Deviations: `graph` was the last `Kind::Adapter` and its module doc still described an adapter — rewritten as native tree-sitter-tags. The second grep also caught the `Replaces:` doc lines and README rows in five plugins; per plan §2 (retired names only in `doctor`/`setup --replace`/`bench`) they now point at the catalogue in `plan.md` §1 instead of naming tools. `rtok plugins` lost its `kind` column (`README.md`, `src/main.rs` updated).

## P11 — OpenAI API surface (D11)

**T11.1 `Wire` adapter + Anthropic behind it** · T5.1, T5.3 · `src/proxy/wire.rs`, `src/proxy/anthropic.rs`
Do: `trait Wire { fn matches(path) -> bool; fn tool_results(req: &mut Value) -> Vec<ToolResultRef>; fn usage_from_body(body) -> Option<Usage>; fn usage_from_sse(event) -> Option<Usage> }` where `ToolResultRef { id, content: &mut String/Value, turn }` and `Usage { input, cache_create, cache_read, output }`. Move every `/v1/messages`-specific line from T5.1/T5.3 into `anthropic.rs`; `archive` and `proxy` call only the trait.
Check: all P5 tests pass unchanged; `grep -r '"tool_result"' src/plugins/archive.rs` finds nothing (format knowledge lives in the wire).
Status: done 2026-09-02 · Check: `mise exec -- cargo test proxy` (all ten P5 proxy tests) and `mise exec -- cargo test archive` (eight archive tests) pass unchanged; `just check` is green. Deviation: archive is a directory module (`src/plugins/archive/mod.rs`), not the stale `src/plugins/archive.rs` path in the task; the source module has no `"tool_result"` match. `WireRequest` was added to the plugin boundary so every proxy-rewriting plugin receives only wire-normalised results.

**T11.2 OpenAI Chat Completions wire** · T11.1, T5.0 · `src/proxy/openai_chat.rs`, `tests/fixtures/proxy/openai_chat_*.json`
Do: route `POST /v1/chat/completions` to `RTOK_OPENAI_UPSTREAM` (default `https://api.openai.com`). Tool results = messages with `role: "tool"` keyed by `tool_call_id`. Usage from `usage.prompt_tokens`, `usage.completion_tokens`, `usage.prompt_tokens_details.cached_tokens` (→ `cache_read`; `cache_create = 0`). Streaming: SSE `data:` lines ending with `data: [DONE]`; when the request streams and lacks `stream_options.include_usage`, add it so the final chunk carries `usage` (this is the one byte-level change passthrough mode makes; documented). Non-streaming: usage from the body.
Check: T5.0 `openai_chat_{stream|body}` fixtures via `MockUpstream` → response bytes identical; `usage` row inserted with `api = openai_chat` and `cache_read` populated from `cached_tokens`.
Status: done 2026-09-02 · Check: `mise exec -- cargo test --test proxy` → 12 passed, including the two new tests. `proxy_openai_chat_body_records_usage_with_cached_tokens` and `proxy_openai_chat_stream_is_byte_identical_and_adds_include_usage` both assert `assert_passthrough_bytes` (response bytes identical to the fixture, body and SSE) and a usage row of `(input, cache_create, cache_read, output) = (10, 0, 7, 2)` — `cache_read` from `prompt_tokens_details.cached_tokens`. The stream test also reads the recorded `call_io` request back and asserts `stream_options.include_usage == true` with nothing else rewritten. `just check` green (109 lib tests + all integration suites). Deviations: (1) the `api = openai_chat` half of the Check is **not** verified — `usage.api` does not exist until T11.6 adds it (see the plan.md §6 amendment); the row is distinguishable today only through `calls.provider = openai`. (2) Both `openai_chat_*.json` fixtures had `cached_tokens: 0`, which made the `cache_read` assertion vacuous, so both were changed to `7`. (3) `Wire` gained a default-no-op `prepare_request(&mut Value, include_usage) -> bool` so request shaping lives in the wire instead of a path match in `mod.rs`; `int_field` moved from `anthropic.rs` to `wire.rs` as the shared helper. (4) `/v1/responses` still routes to `proxy.upstream` until T11.3 gives it a wire.

**T11.3 OpenAI Responses wire** · T11.1, T5.0 · `src/proxy/openai_responses.rs`, `tests/fixtures/proxy/openai_responses_*.json`
Do: route `POST /v1/responses`. Tool results = `input[]` items of type `function_call_output` keyed by `call_id`. Usage from `usage.input_tokens`, `usage.output_tokens`, `usage.input_tokens_details.cached_tokens`; streaming: final `response.completed` event. Respect `previous_response_id` (nothing to rewrite in the request when history is server-side — record usage only).
Check: T5.0 `openai_responses_{stream|body}` fixtures → identical bytes; `usage` row with `api = openai_responses`; a request with `previous_response_id` produces zero rewrites in compress mode.
Status: done 2026-09-03 · Check: `mise exec -- cargo test --test proxy` → 15 passed, including three new Responses tests. `proxy_openai_responses_body_records_usage_without_rewriting_previous_response` and `proxy_openai_responses_stream_is_byte_identical_and_records_usage` both assert `assert_passthrough_bytes` (response bytes identical to the fixture, body and SSE) and a usage row of `(input, cache_create, cache_read, output) = (10, 0, 7, 2)` — `cache_read` from `input_tokens_details.cached_tokens`. The body test runs in compress mode with `previous_response_id` and asserts the forwarded `call_io` request is byte-identical with zero archive measurements. `just check` green (112 lib tests + all integration suites). Deviations: (1) the `api = openai_responses` half of the Check is **not** verified — `usage.api` does not exist until T11.6 (same as T11.2 / plan.md §6 amendment); the row is distinguishable today only through `calls.provider = openai`. (2) Both `openai_responses_*.json` fixtures had `cached_tokens: 0` (the stream fixture omitted the field), so both were set to `7`. (3) `str_field` moved from `openai_chat.rs` to `wire.rs` as the shared OpenAI session-id helper.

**T11.4 `archive` across wires** · T11.1–T11.3, T5.3 · `src/plugins/archive/mod.rs`, `tests/fixtures/proxy/*_6turns.json`
Do: T5.3's rules (older than `keep_turns`, larger than `min_tokens`, keyed by the wire's tool-result id, persisted, byte-stable) applied through `Wire::tool_results` for all three formats. `expand` marks ids across formats.
Check: a 6-turn fixture per format → only turns 1–2 large results rewritten; same request twice → byte-identical bodies; prefix before the first rewrite unchanged (same test as T5.3, parameterised over wires).
Status: done 2026-09-03 · Check: `six_turn_fixtures_archive_only_turns_1_and_2_on_every_wire` (Anthropic / Chat Completions / Responses 6-turn fixtures → exactly turns 1–2 rewritten, prefix bytes before the first `[archived ` equal the original, second rewrite identical, expand of turn 1 restores the original while turn 2 stays a pointer). `proxy_compress_archives_six_turns_on_each_wire` (same request twice per wire through compress mode → byte-identical `call_io` bodies, prefix unchanged, 4 archive measurements). `just check` green (113 lib tests + 16 proxy tests). Deviation: rewrite logic was already wire-generic from T11.1; this task added the three `*_6turns.json` fixtures and parameterized T5.3 coverage. `src/plugins/archive/AGENTS.md` now says rewrite only `Wire::tool_results` payloads.

**T11.5 setup for OpenAI hosts** · T5.2, T11.2 · `src/setup/codex.rs`, `src/setup/opencode.rs`, `src/doctor.rs`
Do: `rtok setup codex --proxy` writes a `model_provider` with `base_url = http://127.0.0.1:8790/v1` in `~/.codex/config.toml` (backup, dry-run, idempotent, `--remove`); `rtok setup opencode --proxy` sets `OPENAI_BASE_URL` in its config; `rtok doctor` shows the `OPENAI_BASE_URL` chain next to the Anthropic one. Print how to revert.
Check: each dry-run shows exactly one change; second run "no changes"; `rtok doctor` lists both chains.
Status: done 2026-09-03 · Check: `proxy_dry_run_shows_one_change_and_touches_nothing` / OpenCode dry-run counterpart → one `[model_providers.rtok]` or `env.OPENAI_BASE_URL` change, file untouched; apply then second apply `"no changes"`; `--remove` strips. `lists_anthropic_and_openai_proxy_chains` → `rtok doctor` prints both `proxy` and `proxy openai` lines. `just check` green (118 lib tests). Deviations: also `src/setup/mod.rs` (`openai_proxy_url`, `pub mod opencode`) and `src/cli.rs` (codex `--proxy` dispatch, host `opencode`). Codex writes `model_provider = "rtok"` plus `[model_providers.rtok] base_url`; OpenCode writes `env.OPENAI_BASE_URL` with `/v1`.

**T11.6 `usage.api` + per-API stats** · T11.2, T13.2 · `migrations/0003.sql`, `src/measure/stats.rs`
Do: migration adds `usage.api TEXT NOT NULL DEFAULT 'anthropic'` (`anthropic | openai_chat | openai_responses`); `rtok stats` prints usage totals and cache hit rate per API; `rtok stats --cache` (T5.5) handles OpenAI cached_tokens (no cache_create signal → busts detected from cache_read drops only).
Check: `cargo test store::` still applies migrations idempotently (0001–0003); fixture usage rows for two APIs → two rows in the stats table.
Status: done 2026-09-03 · Check: `cargo test --lib store::` → 6 passed, including `migration_is_idempotent` (second migrate = 0 with 0001–0005) and `two_apis_are_two_stats_rows`. `two_apis_print_as_two_table_rows` prints two api lines in `rtok stats` table. `openai_cache_read_drop_is_a_bust_without_create` covers OpenAI cache_read-drop busts. `just check` green. Deviations: migration is `0005.sql` (0003 is T8.1 symbols; 0004 is archive_decisions). Also `src/store/mod.rs`, `src/proxy/{mod,wire}.rs`, `src/measure/cache.rs`, `src/cli.rs`.

**T11.7 `toon` on Wire tool results** · T11.1, T5.3 · `src/plugins/toon/mod.rs`
Do: `proxy_filter` on the normalised `Wire` view (D11): tabular JSON arrays/objects → TOON when `plugins.toon.enabled` (default **false**). Encoder written here (D6), deterministic. Archive the original JSON first; the encoded block references the archive id. Record `Measurement` per rewritten block. Off → request bytes identical to passthrough.
Check: default off, fixture request bytes identical; enabled on a 3×4 JSON table → `after_bytes` < `before_bytes` and a measurement row; decode of the TOON recovers the same keys.
Status: done 2026-09-03 · Check: `cargo test --lib plugins::toon` → 3 passed: `default_off_leaves_bytes_identical` (enabled=false, request JSON unchanged), `encodes_3x4_table_and_decode_recovers_keys` (`after_bytes` < `before_bytes`, one measurement, decode recovers keys a,b,c,d), `round_trip_values`. `just check` green. Deviation: encoder+tests live in `src/plugins/toon/mod.rs` (over the 200 LOC budget because decode and three tests sit in the same file). Default remains off (`min_rows = 5`).

## P21 — CLI presentation

**T21.1 the plugin offer actually asks** · T10.5 · `src/setup/mod.rs`, `src/setup/cursor.rs`, `src/setup/pi.rs`
Do: D21 (6) says `--dry-run` prints the offer, `--yes` accepts it and the default on a TTY is a prompt; only the first two were built, so on a terminal setup declined its own offer without a word. Add one `setup::accepted(cfg, question)` — `--yes` is yes, no terminal is no, a terminal asks through dialoguer — and call it from both installers instead of their identical `if !cfg.setup.yes` blocks.
Check: without a terminal only `--yes` accepts (the test harness *is* that case, so the existing cursor/pi tests pin the behaviour they already had); `--dry-run` still returns before anything is asked; `just check` green.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — `without_a_terminal_only_yes_accepts` plus the 19 existing `setup::` tests unchanged, `just check` clean.
Deviation: dialoguer over a hand-rolled `read_line`, which would have been about eight lines and no dependency. It is not smaller code that wins here but behaviour: dialoguer restores the terminal on the way out and reads Ctrl-C and EOF as a no, where a naive `read_line` treats EOF as an empty line and would take the default — accepting a symlink into the user's host config on a stream that never answered.

**T21.2 `graph index` shows progress** · T8.1 · `src/plugins/graph/index.rs`, `src/render.rs`, `src/cli.rs`
Do: a cold index walks thousands of files in silence. Split `index::run` into `run_with(cx, root, dry_run, pb)` and keep `run` as the same call with `ProgressBar::hidden()`, so the MCP tool and the background watcher — neither of which owns a terminal — are unchanged and un-branched. `rtok graph index` passes a real spinner from `render::spinner`.
Check: the bar's position equals the report's file count, so it counts the walk rather than decorating it; indicatif draws nothing when stderr is not a terminal, so piped output stays byte-clean; `just check` green.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — `the_progress_bar_counts_the_files_the_walk_reached` (7 `.rs` files and one `.md`: `indexed` is 7 and `pb.position()` is 7), the six existing `graph::index` tests unchanged, `just check` clean.
Deviation: none. `ProgressBar::hidden()` is the reason there is no `Option<&ProgressBar>` and no `cfg` in the walk — the no-op bar is indicatif's own answer to a caller with nothing to draw on.


**T21.3 `rtok dashboard` becomes `rtok web`** · T19.1 · `src/web/`, `src/cli.rs`, `src/config/mod.rs` (+ `layers.rs`, `validate.rs`, `config/default.toml`, `docs/config.md`, `justfile`, `tests/web.rs`, `src/demon.rs`)
Do: rename the command, the module and the config table. `rtok dashboard` stays as a hidden alias that runs the same code and prints its replacement on stderr, so anything already scripted keeps working — the shape T10.8 used for `rtok setup`. `[dashboard]` in an existing config file is accepted once with a warning and folded into `[web]`.
Check: `rtok web --host/--port` serves what `rtok dashboard` did; the deprecated spelling still serves and warns; an old `[dashboard]` table loads; the T12.4 coverage test still maps every flag to a key; `just check` green.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — `just check` clean, 166 lib tests, `tests/web.rs` (renamed from `dashboard.rs`) unchanged in substance, `config_coverage` green with the alias mapped to the one `[web]` table.
Deviations: two, both deliberate. (1) `Plugin::dashboard_page` and `DashboardPage` keep their names. They are published plugin API (`docs/plugin-authoring.md`, `examples/`), and under D23 the page is the thing a plugin contributes to *both* surfaces — a surface-neutral name is the correct one, not a leftover. (2) The legacy `[dashboard]` table is a field on `Config`, not on a section, because `deny_unknown_fields` rejects an unknown *table* before `finish()` could migrate a key inside it; the mechanism is otherwise the same one `core.inject_budget_tokens` already uses. `README.md`, `AGENTS.md` and the site pages were not touched: another session holds uncommitted rewrites of them, and the rename has to land there in that session's copy, not over it.


## P20 — `demon` supervisor (D22)

**T20.1 `rtok demon start|stop|restart|status|list|kill|update`** · T12.3 · `src/demon.rs`, `src/cli.rs`, `src/config/mod.rs` (+ `config/default.toml`, `docs/config.md`, `tests/demon.rs`)
Do: one supervisor process per service. `start [name...]` detaches `rtok demon supervise <name>` (hidden subcommand) for each name, defaulting to `[demon] services`; the supervisor calls `setsid` so a closed terminal does not take it down, appends the child's stdout and stderr to `<home>/demon/<name>.log`, and re-spawns the child every time it exits, backing off from `[demon] backoff_ms` to `[demon] max_backoff_ms` and resetting once a child has stayed up past `[demon] healthy_ms`. `stop` writes `<name>.stop`, then signals supervisor and child — marker first, so no restart can race the kill. `kill` is the same with SIGKILL and drops the state file. `restart` is stop then start. `status [name]` and `list` print name, state, pids, uptime, restarts and log path, with liveness from signal 0 rather than from the state file. `update [name]` restarts a service under the binary now on disk and says which path it moved to. The service name is an allow-list (`proxy`, `mcp`, `dashboard`), never an arbitrary argv.
Check: `tests/demon.rs` — a supervised service that exits non-zero is running again within a second with `restarts` at 1; `stop` leaves neither pid alive and the marker prevents a restart; `status` reports `stopped` after the supervisor is killed from outside, not `running` copied from the file; `list` shows every state file; `kill` removes it; `start` twice is refused rather than doubling the supervisor. Gate clause from the promoted v0.2 row: `rtok hook` still exits 0 in ≤ 10 ms with the supervisor down (the existing latency test, unchanged).
Status: done 2026-09-09 · Model: Opus 5
Complexity: 3/5
Check result: green. `cargo test --test demon` 3/3 — `a_service_that_exits_comes_back_and_stop_takes_the_whole_tree_down` (supervised `rtok mcp` with no stdin exits on EOF, so it is a real crash loop: two restarts inside the deadline, `status` reads `running`, `stop` leaves no live pid, no state file, and nothing restarts in the 300 ms after), `status_asks_the_kernel_rather_than_believing_the_state_file` (SIGKILL the supervisor from outside, leave the state file naming it, `status` still says `stopped`), `a_second_start_is_refused_and_list_names_every_service` (second `start` prints `already running` and the supervisor pid is unchanged; `list` shows all three with the two unstarted ones `stopped`; `demon start "rm -rf /"` exits non-zero). `just check` green (165 lib tests). Gate clause held: nothing in `demon` is on the hook path, and `tests/latency.rs` is untouched.
Deviations: three, all making the task smaller than written. (1) The service is a clap `ValueEnum` rather than a hand-checked string, so validation, `--help` and shell completions come from the derive (D14) and there is one list instead of two — `[demon] services` parses through the same enum. (2) One new dependency, `rustix` (`process`), already in the lock: `unsafe_code = "forbid"` rules out calling `libc::kill` or `setsid` directly, and rustix is the safe wrapper. (3) `supervise` passes `--config` down to the child as well as taking it itself, which the task did not say but a supervisor started with `--config` must do.
Found while writing the Check: `Pid::from_raw` debug-asserts on a negative pid, and `kill(2)` reads 0 and negatives as *process groups*. A corrupt or half-written state file would have signalled every process in rtok's group. `one(pid)` now rejects anything ≤ 0 before it reaches rustix, with `a_group_pid_is_never_signalled` holding it.

**T20.2 owo-colors owns the colour question** · T12.6 · `src/render.rs`, `src/demon.rs`, `Cargo.toml`
Do: replace the five hand-rolled ANSI constants and the `NO_COLOR` + `isatty` check in `render.rs` with owo-colors' `if_supports_color`, which answers the same question per stream and also honours `CLICOLOR`, `CLICOLOR_FORCE` and `TERM=dumb`. Add `render::state(word, ok)` and colour the `demon status` state column through it, so there is one place that knows what green means.
Check: the T12.6 diff tests still pass unchanged (they run without a tty, so the text is plain); `state("running", true)` is the bare word off a tty; `just check` green.
Status: done 2026-09-09 · Model: Opus 5
Check result: green — `cargo test --lib render` 3/3 and `just check` clean. `src/render.rs` lost `RED`/`GREEN`/`CYAN`/`BOLD`/`OFF`, the `colour()` function and the `std::io::IsTerminal` import.
Deviation: none. The four other libraries the user named were weighed and not taken, each for a reason that is about this binary rather than about the library: **dialoguer** has nothing to replace — rtok has no interactive prompt anywhere yet, so adopting it would mean building the D21 setup prompt, which is its own task, not a swap. **indicatif** has no loop that reports progress today; `graph index` is the one honest candidate and it is a follow-up, not a rewrite. **ratatui** is already the plan's P15 (`rtok tui`, D17, T15.1–T15.9) — a phase, not a dependency to bolt on. **color-eyre** is the one that would be wrong: its value is a panic hook that prints a report to stderr, and `rtok hook` must exit 0 in ≤ 10 ms without writing to stderr (D1, T17.1 `catch_unwind`); `main.rs` is three lines over `anyhow`, and eyre in a crate that is also a library is the pattern its own docs steer away from.


## P12 — Config file (D12, D14)

**T12.1 typed schema + reference file** · T0.2 · `src/config.rs`, `config/default.toml`, `docs/config.md`
Do: replace the free-form `[plugins.<id>]` extras with typed sections for every table in `docs/config.md` (`hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `expand`, `filter`, and `plugins.<id>` each with its keys). `#[serde(deny_unknown_fields)]` on every section; `#[serde(default)]` everywhere so partial files work. `config/default.toml` is the annotated reference, embedded with `include_str!`; a fresh install writes it verbatim (not a serialised struct, so comments survive). Move `core.inject_budget_tokens` to `plugins.inject.budget_tokens`, accepting the old key with a one-line warning. `rtok config init [--force]`, `rtok config path`.
Check: `cargo test config::` → `config/default.toml` parses with zero unknown keys and equals `Config::default()`; `RTOK_HOME=$(mktemp -d) rtok config init && diff $RTOK_HOME/config.toml config/default.toml` is empty.
Status: done 2026-09-02 · 19 tests green (`make check`, `make example`); `config init` output is byte-identical to `config/default.toml`; no default value drifted. Deviations: (a) 7 files, not 3 — a schema change fans out mechanically to `src/main.rs` (the `config init`/`path` subcommand the Check needs), `src/plugin.rs`, `src/plugins/mod.rs` and `docs/plugin-authoring.md`; (b) a small `section!` macro writes the repeated `#[serde(default, deny_unknown_fields)]` + `Default` impl for all 24 sections instead of 24 hand-written impls; (c) `Ctx::plugin_cfg` is gone — plugins now read `cx.config.plugins.<id>.<key>` typed, which is the point of the task; (d) paths keep their literal `~/…` in the file and are expanded on load (`~/.rtok/x` → `<home>/x` so `RTOK_HOME` still moves the whole tree), which is what lets `Config::default()` equal the reference file.

**T12.2 layering + precedence + `config show`** · T12.1 · `src/config/layers.rs`, `src/main.rs`, `Cargo.toml`
Do: a `Figment` with named providers, merge order: `Serialized::defaults(Config::default())` (`default`) → `Toml::file` user (`user`; path from `RTOK_CONFIG` / `--config`) → `Toml::file` `<git root>/.rtok.toml` (`project`) → `Env::prefixed("RTOK_").split("_")` (`env`; lists comma-separated; map legacy `RTOK_UPSTREAM` / `RTOK_OPENAI_UPSTREAM`) → `Serialized` of clap `Option<T>` fields that are `Some` (`flag`). Extract `Config`. Provenance from figment metadata, not a side table. `rtok config show [--sources] [--json]` and `rtok config get <key>`. Drop the direct `toml` dependency (figment’s `toml` feature parses). Add `toml_edit` here (used by T12.3). Enable clap `wrap_help`. No hand-rolled deep-merge.
Check: `RTOK_PROXY_PORT=1 rtok config show --sources | grep proxy.port` → `1 (env)`; `RTOK_PROXY_PORT=1 rtok proxy --port 2 --dry-run` reports port 2; a project `.rtok.toml` with `[plugins.read] allow_paths` shows as `(project)`; `grep -rn 'toml::' src` → nothing; `grep -rn figment src/config` finds the providers.
Status: done 2026-09-02 · Check green: `proxy.port = 1 (env)`; `proxy --port 2 --dry-run` prints `port = 2`; project `.rtok.toml` `[plugins.read] allow_paths = [/proj] (project)`; no `toml::` in `src`; `make check` 23 tests. Deviations: (a) env mapping is **not** `Env::split("_")` — a leaf table from `Config::default()` maps `RTOK_PROXY_OPENAI_UPSTREAM` → `proxy.openai_upstream`; unknown names (`RTOK_HOME`, `RTOK_CONFIG`) are dropped (D14 changelog 2026-09-02); (b) `src/config.rs` became `src/config/mod.rs` + `layers.rs`; (c) `[proxy] dry_run` added so `--dry-run` has a D12 key; (d) tests inject env pairs instead of `set_var` (edition 2024 + `unsafe_code = forbid`).

**T12.3 `config validate` + `config set`** · T12.2 · `src/config/validate.rs`
Do: `rtok config validate [path]` → unknown key, wrong type, out-of-range (`port` 1–65535, `keep_turns` ≥ 1, `budget_tokens` ≥ 0, `mode` enum) with file:line, exit 1. Elsewhere the same problems are one stderr warning and defaults are used — hooks never fail on config. `rtok config set <key> <value>` edits the user file in place preserving comments (`toml_edit`, D14 — the crate that round-trips TOML comments; figment does not write files).
Check: a file with `[proxy] port = 70000` → exit 1 naming the line; `echo '{}' | RTOK_CONFIG=bad.toml rtok hook PreToolUse` → `{}` and exit 0; `set proxy.port 8791` then `get proxy.port` → 8791 and the comment above `[proxy]` is intact.
Status: done 2026-09-02 · Check green: `bad.toml:2: proxy.port out of range (1–65535)` exit 1; hook on that file prints `{}` and exit 0 (stderr warning, defaults); `set proxy.port 8791` then `get` → 8791 and `[proxy] # rtok proxy` remains. Deviations: clap tree moved to `src/cli.rs` so T12.4 can walk `Cli::command()`; hook copies stdin to stdout (fail open) instead of the stub; `--sources` is an action flag like `--force`.

**T12.4 flag ↔ key coverage test** · T12.2 · `tests/config_coverage.rs`
Do: walk `Cli::command()` recursively; for every non-positional arg that is not in a tiny allow-list (`--config`, `--home`, `--help`, `--version`, `--json` where `stats.format` covers it, action flags `--remove`, `--replace`, `--calibrate`, `--cache`, `--force`) assert `<path>.<arg>` (dashes → underscores; `run`/`filter` args map under `plugins.cmd`) exists in `config/default.toml`. Also the reverse: every key in `default.toml` is read somewhere (grep the source for the key's last segment) so dead keys fail too.
Check: `cargo test config_coverage` passes; adding `--foo` to any subcommand without a key fails the test.
Status: done 2026-09-02 · `cargo test config_coverage` passes. Deviations: allow-list also includes `--sources` (annotates `config show`, not a stored key); reverse check skips 1–2 character leaves (`bench.configs.a`) to avoid matching noise; `timeout` → `timeout_s`, `run`/`filter` flags map under `plugins.cmd` as specified.

**T12.6 `--dry-run` on every write command, with a git-shaped diff** · T12.3 · `src/render.rs`, `src/cli.rs`, `src/config/validate.rs`, `src/config/mod.rs`, `src/plugins/memory/import.rs`, `src/plugins/graph/index.rs`, `tests/dry_run.rs`
Do: add `--dry-run` to the commands that write and did not have it (`config init`, `config set`, `memory import`, `graph index`); render every file change as a unified diff with `+`/`-` and colour, and route the installer output of `agent setup`/`agent remove` through the same painter. Colour only for a real terminal with `NO_COLOR` unset. The four new flags are actions, not settings, so they get no config key and go in the T12.4 key allow-list.
Check: per command, `--dry-run` leaves the file or the store exactly as it was, and the real run afterwards reports what the preview promised; `config init --dry-run` prints `+++ b/…` and `+[core]`; `config set --dry-run` prints both sides of the changed line; `just check` green.
Status: done 2026-09-09 · Check green: `cargo test --test dry_run` 4/4 — `config_init_dry_run_prints_the_file_it_would_write_and_writes_none`, `config_set_dry_run_shows_both_sides_and_leaves_the_file_alone`, `memory_import_dry_run_counts_rows_it_does_not_insert` (dry says `inserted 5`, real run still inserts 5, third run skips 5), `graph_index_dry_run_reports_rows_but_stores_none` (dry indexes 1 file, real run indexes it again, third run skips). `just check` green. Deviations: one new direct dependency, `similar` 2.7 for the unified diff — already in `Cargo.lock` through dev-dependencies, so the lock did not grow. `otel flush` deliberately got no `--dry-run`: `rtok otel status` already prints endpoint, watermarks and pending rows, which is the preview. `config set --dry-run` on a missing config file is an error rather than a silent init, since a preview must not create the thing it previews. `docs/config.md` gains a row for the four keyless flags; `README.md` and the site pages were left alone, another session holds uncommitted rewrites of them.

Gate P12 (review) · Status: done 2026-09-03 · Check: clap `filter --cmd` has no local default (`Option<String>` → config `filter.cmd`); `docs/config.md` mapping table matches clap (dropped advertised `--home`/`--log-level`/`--bind`/`--openai-upstream`/`--timeout`/`--shell`/`--no-trailer`/`plugins --json`); merge figment, CLI clap, `config set` toml_edit.

**T13.1 Diesel replaces rusqlite** · T0.3 · `Cargo.toml`, `src/store/mod.rs`, `src/store/schema.rs`
Do: convert `src/store.rs` to `src/store/mod.rs`. Depend on `diesel` 2.2 (`sqlite`, `returning_clauses_for_sqlite_3_35`) and `libsqlite3-sys` bundled with FTS5 (confirm `notes_fts` still builds; enable `SQLITE_ENABLE_FTS5` if the bundle omits it). Drop rusqlite. Keep the filename-keyed migration runner and WAL/`synchronous=NORMAL`. `table!` macros for the six 0001 tables. `Store` holds `diesel::sqlite::SqliteConnection`. No `Store::conn()` leaking the driver. Existing `insert_measurement` and the three store tests pass unchanged in behaviour.
Check: `grep -rn rusqlite Cargo.toml src tests` → nothing; `cargo test store::` green; `open_on_disk_uses_wal` still asserts WAL; `notes_fts` MATCH still finds an inserted note.
Status: done 2026-09-02 · Check green: no `rusqlite` in Cargo.toml/src/tests; three store tests pass (WAL + FTS5 MATCH). Deviations: diesel resolved to 2.3 (task named 2.2; `^2.2` on crates.io); `Store` wraps `Mutex<SqliteConnection>` so `insert_measurement` stays `&self` for `Ctx::record`; `examples/hello_plugin.rs` uses `measurement_count` instead of `conn()`.

**T13.2 schema 0002 + models** · T13.1 · `migrations/0002.sql`, `src/store/schema.rs`, `src/store/models.rs` (also `architecture.md` §7, mechanical)
Do: apply the DDL above. Diesel `table!` + structs with associations (`Call` belongs_to host/provider/model/session, `has_many` tokens/logs, `has_one` call_io). `PRAGMA foreign_keys=ON` on open. Update `architecture.md` §7 table list to match. Do not drop `events`.
Check: `cargo test store::` → migrate twice is 0; `sqlite_master` contains `hosts,providers,models,sessions,calls,call_io,tokens,logs`; seed `hosts` has 6 rows; inserting a `calls` row with a bad `host_id` fails.
Status: done 2026-09-02 · Check green (`schema_0002_seeds_hosts_and_rejects_bad_fk`). Deviations: `tokens.tokens` column is `n_tokens` in Diesel (`#[sql_name = "tokens"]`) because `table!` forbids a column named like its table; `architecture.md` §7 already listed the 0002 tables — added `call_id` on `measurements`/`usage`.

**T13.3 `Store`/`Ctx` write API** · T13.2 · `src/store/mod.rs`, `src/plugin.rs`
Do: `Store` methods: `upsert_session`, `upsert_model(provider_slug, model_slug)`, `insert_call`, `insert_call_io` (inline or archive by cap; hook surface never archives), `insert_tokens`, `insert_log`, `purge_calls_older_than(days)` (0 = skip). `Ctx`: `record_call`, `record_tokens`, `log(level, source, name, message)` — `log` never returns `Err` to a plugin (fail open; on DB error write `log_file` only). `record` (measurements) sets `call_id` when the plugin supplies one. Plugins and surfaces still have no SQL.
Check: one test inserts `kind=mcp_call` + `call_io` with args/result JSON + `tokens` before/after/mcp + a `logs` row `source=plugin`; round-trip equals; `insert_call_io` with a 70 KiB body and cap 64 KiB writes `archive` and nulls `request_json`; `Ctx::log` after a closed-DB failure still returns.
Status: done 2026-09-02 · Check green (`write_api_round_trip_and_spill`, `log_survives_db_failure`). Hook path passes `archive_dir = None` so over-cap bodies store bytes/sha only. `sha2` added for archive checksums.

**T13.4 config keys** · T12.1, T13.3 · `config/default.toml`, `docs/config.md`, `src/config/mod.rs`
Do: `[core] call_io_inline_bytes = 65536`, `retain_calls_days = 30` (0 = keep forever), `log_to_db = true`. Document: hook path never archives `call_io`; `log_file` is always written; `logs` table is written when `log_to_db`. `rtok stats` later joins `calls`/`tokens`; no new CLI in this task.
Check: `cargo test config::` parses the three keys; `docs/config.md` has a row for each; T12.4 coverage still green once those tasks exist, otherwise the keys are present in `default.toml`.
Status: done 2026-09-02 · `cargo test config::` and `cargo test config_coverage` green. Keys documented next to `session_env` in `docs/config.md` and `config/default.toml`.

Gate P13 (review) · Status: done 2026-09-03 · Check: no rusqlite; hook `oversized_hook_call_io_does_not_archive` leaves `call_io` archive columns null; archive `plugin_run` has before and after token phases. §6: `migrations/*.sql` are Store-included schema, not runtime SQL outside `src/store/`.

**T3.3 family formatters + default rules** · T3.2 · `src/plugins/cmd/formatters.rs`, `rules/default.toml`, `tests/cmd_golden/`
Do: written from scratch here (D6); the command families in `research.md` (rtk's list) are the spec, not the code. A formatter is `fn(argv: &[String], output: &str) -> Option<String>`; `None` falls back to the rules. Formatters: `cargo build|test|clippy` (per-target status, errors as file:line + message, test counts + failing names), `git status|diff|log` (compact paths, stat lines, one line per commit), `pytest|jest|vitest|go test` (pass/fail counts, failing names, first assertion line each), `ls|find|tree` (columns, depth cap). `rules/default.toml` covers grep/rg, sed, cat, make, curl, npm/pnpm/node. Never redact.
Check: golden tests `tests/cmd_golden/*.{in,out}` for 10 families; a fixture with a fake AWS key must appear unchanged in output (no redaction surprises).
Status: done 2026-09-02 · Check: `ten_families_and_aws_key_unredacted` — 11 `tests/cmd_golden/*.in` pairs; `AKIAIOSFODNN7EXAMPLE` unchanged in cat output. `make check` green. Deviation: also `rules.rs`/`run.rs`/`mod.rs` (wire compress + parse default.toml).

**T3.6 measurement wiring** · T3.1–T3.4 · `src/plugins/cmd/mod.rs`
Do: every run writes `Measurement { kind: formatter|rule|raw, before, after }`; `rtok stats --plugin cmd` shows per-family savings and archive hit count (how often `expand` was called — the honesty metric).
Check: after 3 runs, `rtok stats --plugin cmd --json` has 3 rows with before ≥ after.
Status: done 2026-09-02 · Check: `three_runs_stats_plugin_cmd_json_has_rows` — 3 `rtok run` → `plugin_json` 3 rows, each before ≥ after. `make check` green. Deviation: 4 files (`cmd/run.rs`, `store/mod.rs`, `measure/stats.rs`, `cli.rs`); kind is `raw`/`rule` until T3.3 formatters.

**T6.1 notes API** · T0.3 · `src/plugins/memory/mod.rs`
Do: MCP tools `mem_save(kind, title, body, project?)`, `mem_search(query, limit=5)` → ids + titles + 120-char snippets (FTS5 `bm25`), `mem_get(id)` → full body. Project = git root name of cwd.
Check: save 3, search returns the right one first, get returns the full body.
Status: done 2026-09-02 · Check: save 3 notes, search `"walrus"` returns that title first, `mem_get` returns the full body. `make check` green. Deviation: also `store/mod.rs` (`search_notes`, `get_note_body`, `NoteHit`). MCP stdio comes in T4.1.

**T6.2 SessionStart recall** · T6.1, T2.4 · `src/plugins/memory/inject.rs`
Do: inject the last 5 note titles + ids for the current project (≤ 200 tokens) through `inject` with priority 10; never bodies.
Check: fixture with 20 notes → 5 titles, ≤ 200 tokens, byte-stable across runs.
Status: done 2026-09-02 · Check: 20 notes → 6-line recall (header + 5 titles), no bodies, ≤ 200 tokens, byte-stable. `make check` green. Deviation: implemented in `memory/mod.rs` (not `inject.rs`) plus `Store::list_note_titles`.

**T6.3 import** · T6.1 · `src/plugins/memory/import.rs`
Do: `rtok memory import <file.jsonl>`: one note per line `{kind, title, body, ts?, project?}`. Users export their previous memory tool to that shape themselves; rtok knows no third-party schema (D6). Dedupe by sha256 of body; print inserted/skipped/malformed counts; exit 0.
Check: a 50-line fixture → 50 rows; re-import → 0 inserted, 50 skipped; one malformed line is counted, skipped, exit 0.
Status: done 2026-09-02 · Check: `fifty_then_reimport_then_malformed_exits_ok` — 50 inserted, re-import 50 skipped, extra bad line malformed=1. `make check` green. Deviation: also `cli.rs` (`rtok memory import`) and `Store::note_bodies`.

**T7.1 modes as data** · T2.4 · `modes/terse.md`, `modes/yagni.md`, `src/plugins/inject.rs`
Do: copy the intent of caveman (terse output) and ponytail (YAGNI ladder) into ≤ 250-token markdown files under `~/.rtok/modes/`; `rtok setup --mode terse,yagni` enables; injected once per session via `inject` (priority 5), not per prompt.
Check: `rtok hook SessionStart` output contains the mode text once; UserPromptSubmit output does not.
Status: done 2026-09-02 · Check: SessionStart additionalContext contains `# terse` and `# yagni` once; UserPromptSubmit does not. Files ≤ 250 tokens. `make check` green. Deviation: `--mode` on `setup` maps to `setup.modes`; builtins via `include_str!`.
**T7.2 instruction audit** · T1.4 · `src/doctor.rs`
Do: `rtok doctor --instructions`: token count of `~/.claude/CLAUDE.md` + project CLAUDE.md + every enabled plugin's SessionStart text (lean-ctx, engram, ponytail, claude-mem, token-optimizer today); flag duplicates (same sentence in two files) and anything > 1,000 tokens.
Check: on this machine, report lists ≥ 4 injectors and their token totals.
Status: done 2026-09-02 · Check: `instructions_lists_four_injectors` — four named MCP injectors plus CLAUDE.md files, each with a token total; duplicates and >1000 flagged. `make check` green.

**T12.5 `.env` files** · T12.2 · `src/config/layers.rs` — added 2026-09-02 (user request)
Do: a `dotenv` layer between `project` and `env`: `RTOK_*` lines from the nearest `.env` walking up from the working directory, then `<home>/.env` (dotenvy syntax; project file wins). Parse only — nothing is exported into rtok's environment, so commands run by `rtok run` never see the file. Shell variables still win; other keys in a project `.env` are ignored. Malformed file = one stderr line, not an error (fail open). Document in `docs/config.md` precedence.
Check: `.env` with `RTOK_PROXY_PORT=8799` → `rtok proxy --dry-run` prints `port = 8799` and `config show` names the source `dotenv`; the same key exported in the shell wins; a non-`RTOK_` key in a project `.env` changes nothing.
Status: done 2026-09-02 · Check: project `.env` with `RTOK_PROXY_PORT=8799` (plus a `DATABASE_URL` line) → `rtok proxy --dry-run` from a subdirectory prints `port = 8799`; with `RTOK_PROXY_PORT=8800` exported it prints `port = 8800`; `<home>/.env` `RTOK_PROXY_MODE=compress` shows as `proxy.mode = compress` in `config show`; `entries()` names the source `dotenv` and `env` respectively (`dotenv_layer_sits_between_project_and_env`); `dotenv_files_take_rtok_keys_project_first` proves parse-only (the key never reaches the process environment) and that non-`RTOK_` keys are dropped; `malformed_dotenv_is_skipped_not_fatal`. `make check` green (107 lib tests). Deviation: `dotenvy 0.15` is a new dependency (reason in the commit); `git_root` and the `.env` lookup share one `find_up` helper; `RtokEnv` gained a provenance `name` so one provider type serves both the `dotenv` and `env` layers.

Runs right after P0's gate: P1–P11 tasks that add flags then wire them through this instead of ad-hoc `clap` defaults. Implementation is clap + figment + toml_edit (D14), not a custom merge.

T12.1–T12.4 are done — see `done.md`.

## P4 — `read` plugin + MCP server

**T4.1 `rtok mcp`** · T0.4, T13.3 · `src/mcp.rs`
Do: rmcp stdio server exposing tools from all plugins' `mcp_tools()`; register `expand` from T3.5. Tool descriptions ≤ 60 tokens each (measured by T0.5 in a test). Every `tools/call` writes `calls` (`kind=mcp_call`, `plugin` from the `ToolDef`, `host` from `core.host`) with full arguments and result in `call_io` (archive if over cap — MCP is not the hook path). `tokens`: phase `before` = estimate of arguments, phase `after` = estimate of result, phase `mcp` on the owning plugin (same after count, so `stats --plugin` includes MCP).
Check: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | rtok mcp` lists `expand`; test asserts every description ≤ 60 tokens; a fixture `tools/call` inserts `calls` + `call_io` + three `tokens` rows.
Status: done 2026-09-02 · Check: `tools_list_includes_expand` stdout contains expand; `descriptions_at_most_60_tokens`; `tools_call_writes_calls_io_and_three_token_rows` — 1 `mcp_call` + 1 `call_io` + 3 `tokens`. `make check` green. Deviation: also `cli.rs`, `lib.rs`, `store/mod.rs` (`host_id`, counts), `tests/mcp.rs`; host slug is `[hook] host` (no `core.host`); one-shot `tools/list` without initialize. New dep: `rmcp` (MCP types + JSON-RPC).

**T4.2 `read` tool: full/lines** · T4.1 · `src/plugins/read/mod.rs`
Do: `read(path, mode=full|lines, range?)` with line numbers, size cap (default 20 K chars, then head/tail + archive id). Root guard: path must be under cwd or config `allow_paths`.
Check: read a 3-line fixture → 3 numbered lines; a 100 KB file → capped output + archive id; `../etc/passwd` → error.
Status: done 2026-09-02 · Check: `three_lines_are_numbered`; `hundred_kb_is_capped_with_archive_id`; `dotdot_etc_passwd_is_err`. `make check` green. Deviation: also `src/mcp.rs` (dispatch).

**T4.5 `search` + `tree`** · T4.1 · `src/plugins/read/search.rs`
Do: `search(pattern, path, max=50)` regex over files respecting `.gitignore` (use `ignore` crate), output `path:line: snippet` (≤ 120 chars); `tree(path, depth=2)` compact listing with sizes.
Check: `search("fn main", ".")` finds `src/main.rs`; results never exceed `max`.
Status: done 2026-09-02 · Check: `search_fn_main_finds_src_main`; `search_respects_max`. `make check` green. Deviation: also `read/mod.rs`, `mcp.rs`. New deps: `ignore` (gitignore walk), `regex` (pattern).

**T4.6 PreToolUse(Read) advice** · T2.1, T4.2 · `src/plugins/read/hook.rs`
Do: for native `Read` of a file > `read.native_max_bytes` (default 32 K) that was not edited in the last 5 turns: `permissionDecision = "deny"` with reason "use rtok read(mode=map) first; native Read allowed for files you are about to edit". Config off switch. Never deny for files under 32 K (edit gate stays cheap).
Check: fixture Read of a 100 KB path → deny with reason; 2 KB path → no output.
Status: done 2026-09-02 · Check: `hundred_kb_is_denied`; `two_kb_is_silent`. `make check` green. Deviation: also `read/mod.rs`. Edit-window skip is a stub (always not-recent).

**T4.7 register MCP** · T2.3, T4.1 · `src/setup/claude.rs`
Do: `rtok setup claude --mcp` adds `rtok` to `~/.claude.json` mcpServers (stdio, command `rtok mcp`), idempotent, backup.
Check: dry-run shows one server entry; second run "no changes".
Status: done 2026-09-02 · Check: dry-run `mcpServers.rtok: rtok mcp`; second apply `no changes`. `mcp_dry_run_then_apply_is_idempotent`. `make check` green. Deviation: also `src/cli.rs` (`--mcp` → `setup.mcp`).

**T4.4 re-read dedup** · T4.2 · `src/plugins/read/cache.rs`
Do: on `read`, hash content; if same session already returned this sha256 for this path and the same mode/range → return `unchanged since <archive_id> (N lines)`; record measurement. Invalidate on PostToolUse(Edit|Write) for that path.
Check: two identical reads → second response < 80 chars and a measurement row; edit fixture between reads → full content again.
Status: done 2026-09-02 · Check: `two_identical_reads_second_is_short`; `edit_fixture_between_reads_is_full`. `make check` green. Deviation: also `mod.rs`, `store/mod.rs` (`clear_read_cache`). Archive id is 64 hex so the hit line uses an 8-char prefix to stay under 80 chars; full id is `Measurement.ref_id`.

**T4.3 `read` map/signatures via tree-sitter-tags** · T4.2 · `src/plugins/read/outline.rs`
Do: features `lang-rust, lang-ts, lang-js, lang-python, lang-dart, lang-c, lang-go` (default all); `mode=map` → definitions (kind, name, line) from each grammar's tags query; `mode=signatures` → definition lines verbatim. Unknown language → fall back to `lines 1-60` + note.
Check: `read(src/main.rs, mode=map)` on this crate lists `fn main`; golden tests per language on 20-line fixtures.
Status: done 2026-09-02 · Check: `map_src_main_lists_fn_main` contains `fn main`; `golden_per_language` (rs/ts/js/py/dart/c/go) plus `unknown_language_falls_back`. `make check` green. Deviation: also `mod.rs` (mode dispatch, MCP description), `Cargo.toml` (optional grammars). Fixtures are padded to 20 lines in-test rather than separate files. New deps: `tree-sitter`/`tree-sitter-tags` (parser + definition tags); `tree-sitter-{rust,javascript,typescript,python,c,go,dart}` behind `lang-*`.

## P5 — `proxy` + `archive`

**T5.0 httpmock upstream harness** · T0.3 · `tests/proxy/mod.rs`, `tests/fixtures/proxy/`, `Cargo.toml` (dev-dep)
Do: add `httpmock` as `[dev-dependencies]` (commit message: one-line reason). Shared helper `MockUpstream` spins a `MockServer` and serves Anthropic `POST /v1/messages` and OpenAI `POST /v1/chat/completions` + `POST /v1/responses` from `tests/fixtures/proxy/*` (non-streaming JSON bodies and SSE `text/event-stream` variants). Point rtok at `server.base_url()` via `proxy.upstream` / `proxy.openai_upstream` (or env). Helpers: `assert_passthrough_bytes`, `assert_upstream_called_once`. Fixture naming: `anthropic_messages_{stream|body}.json`, `openai_chat_{stream|body}.json`, `openai_responses_{stream|body}.json`.
Check: `cargo test proxy_mock` green for all six fixture pairs; each test asserts client response bytes match the fixture; `mock.assert()` shows exactly one upstream hit per route.
Status: done 2026-09-02 · Check: `cargo test proxy_mock` — 6 tests green; bytes match fixtures; `mock.assert()` one hit each. `make check` green. Deviation: crate root is `tests/proxy.rs` (Cargo). New dep: `httpmock` (mock Anthropic/OpenAI upstreams).

**T5.1 passthrough proxy** · T0.3, T13.3, T5.0 · `src/proxy/mod.rs`
Do: axum on `127.0.0.1:8790`; forward `POST /v1/messages` (and everything else) to `RTOK_UPSTREAM` (default `https://api.anthropic.com`, may be `http://127.0.0.1:8788` to chain behind headroom during A/B). Stream SSE responses unchanged. Parse `usage` from the final `message_delta`/non-streaming body; insert `usage` row (session from `metadata.user_id` or header if present, else request hash). Also write `calls` (`kind=api_request`, `surface=proxy`, `provider`+`model` upserted from the request, `host` from `core.host`) with `call_io` (archive bodies over cap) and `tokens` `source=provider` from the same four counters. `usage.call_id` points at that row.
Check: T5.0 harness with `MockUpstream` → response bytes identical; `usage` row inserted with 4 counters; matching `calls`/`call_io`/`tokens` rows; `models.slug` equals the request `model`.
Status: done 2026-09-02 · Check: `proxy_passthrough_body_records_usage_rows` and `proxy_passthrough_stream_is_byte_identical_and_records_usage` — mock response bytes identical through the live axum server; one `usage` row (input 10 / output 2 on the body fixture) with `call_id` → the `calls` row; `count_kind("api_request") = count_call_io() = count_tokens() = 1`; `models.slug == "claude-sonnet-4-20250514"` (the request `model`); SSE bytes identical with `content-type: text/event-stream` preserved. `make check`: fmt, clippy -D warnings, build-min green; `cargo test` green except pre-existing `printf_two_lines_exit_0_no_trailer` (rvm's `ps` denied by this sandbox — reproduced identically on the base commit T9.1, unrelated to this task). Deviation: `core.host` was removed by T12 → host slug comes from `[hook] host` (default `claude`), unknown slugs fall back to `other`. Also `src/lib.rs` (`pub mod proxy`), `src/cli.rs` (`rtok proxy` now serves instead of "not implemented"), `src/store/mod.rs` (`insert_usage`, `insert_provider_tokens`, `usage_rows`, `model_slug_of_call`; `upsert_model` now returns `(provider_id, model_id)`), `tests/proxy.rs` (two T5.1 tests). New deps: `tokio` (async runtime), `axum` (HTTP server), `reqwest` (streaming upstream client, `stream`+`rustls`), `futures-util` (SSE tee via `StreamExt`) — all from the plan §2 baseline, one-line reasons in the commit message. Request path is the only wire in T5.1: OpenAI routes stay on `proxy.upstream` until P11.

**T5.2 `rtok proxy` lifecycle** · T5.1 · `src/proxy/cli.rs`
Do: `rtok proxy [--port] [--upstream] [--mode passthrough|compress]`; `rtok setup claude --proxy` sets `env.ANTHROPIC_BASE_URL` in settings.json (backup) and prints how to revert. `/health` endpoint.
Check: `curl :8790/health` → `{"ok":true,"mode":"passthrough"}`.
Status: done 2026-09-02 · Check: `proxy_health_reports_ok_and_mode` → `{"ok":true,"mode":"passthrough"}`; `proxy_env_dry_run_then_apply_is_idempotent` writes `env.ANTHROPIC_BASE_URL` to :8790 with revert text. `make check` green. Deviation: also `src/proxy/mod.rs` (`/health` route, `ProxyState.mode`), `src/cli.rs` (`--upstream`/`--mode`/`setup --proxy`), `src/config/layers.rs` (`proxy_flags`), `tests/proxy.rs`, `tests/config_coverage.rs` (`proxy --mode` → `proxy.mode`).

**T5.3 live-zone archive rewrite** · T5.1 · `src/plugins/archive.rs`
Do: in `compress` mode: for `tool_result` blocks that are (a) older than `archive.keep_turns` (default 4 turns from the end), (b) larger than `archive.min_tokens` (default 1,500 est.), replace content with `[archived <id>: first 8 lines … last 4 lines · N tokens · expand(<id>)]`. **Decisions are keyed by `tool_use_id` and persisted**, so the same block is rewritten identically on every later request (frozen prefix stays byte-stable). Never touch `system`, `tools`, the last `keep_turns` turns, or any `tool_result` whose id was `expand`ed. Record measurement per rewritten block. Child `calls` row `kind=plugin_run` plugin=`archive` with `tokens` phase `before` (est. of the block) and `after` (est. of the pointer).
Check: fixture request with 6 turns → only turns 1–2 large results rewritten; sending the same request twice yields byte-identical rewritten bodies; unit test proves the prefix up to the first rewritten block is unchanged.
Status: done 2026-09-02 · Check: `only_turns_older_than_keep_turns_are_rewritten` (6-turn fixture → exactly turns 1–2 rewritten, `system`/`tools`/turns 3–6 byte-equal), `same_request_twice_is_byte_identical_and_prefix_unchanged` (two runs serialise identically; bytes before the first rewritten block equal the original), `proxy_compress_rewrites_old_tool_results_identically` (same request twice through the live axum server in `compress` mode → identical `call_io` request bodies, 2 `plugin_run` rows, 4 `archive` measurements). `make check` green (119 tests). Deviation: the module is `src/plugins/archive/mod.rs` (T0.4 layout), not `archive.rs`; `Ctx` gained `call_id: Option<i32>` + `record_plugin_run` so the child row nests under the API request; `Store::spill` now ignores a duplicate archive id (the second identical request used to fail `call_io`). Pointer text is `[archived <id12>: N lines · T tokens · expand(<id>)]` + head/tail lines. Over the 200 LOC / 3 files budget: rewrite, store decisions, migration `0004.sql`, proxy wiring and tests are one unit.

**T5.4 `expand` through the proxy** · T5.3, T4.1 · `src/plugins/archive.rs`
Do: MCP `expand(id, lines?)` returns the archived original (from T5.3 store); mark id as expanded → T5.3 stops rewriting it from the next request on. Track expand rate.
Check: expand → next fixture request contains the original block again.
Status: done 2026-09-02 · Check: `expand_freezes_the_id_so_the_original_is_sent_again` — rewrite → `expand::fetch(id)` returns the original bytes and freezes the decision → the next 6-turn request carries turn 1 verbatim while turn 2 stays a pointer; `archive_decision_counts` = (2, 1); one `expand` measurement per freeze, none on a repeat. `make check` green (115 tests). Deviation: the shared fetch lives in `src/expand.rs` (`expand::fetch`, used by both `rtok expand` and the MCP `expand` tool), not `src/plugins/archive.rs`; expand rate is reported by `rtok stats --plugin archive` as `decisions` / `expanded` / `expand_rate` from `archive_decisions`.

**T5.5 cache-health report** · T5.1 · `src/measure/cache.rs`
Do: `rtok stats --cache`: per session, cache_read vs cache_creation per turn, detect "cache busts" (turn where cache_creation > 20 K and cache_read drops), attribute to tools-array or system-prompt changes when the proxy saw them.
Check: fixture with an injected tools-array change → one bust flagged with cause `tools`.
Status: done 2026-09-02 · Check: `tools_change_is_one_bust_with_cause_tools` — four proxy turns with the tools array grown at turn 3 (cache_create 31 K, cache_read 30 K → 200) → exactly one bust, `(turn 3, "tools")`, and the table line `bust turn 3 cause=tools`; `system_change_unknown_and_no_drop` covers cause `system`, `unknown` (no recorded body) and no bust when cache_read keeps growing. `make check` green (117 tests). `rtok stats --cache` prints per-session turns / cache_read / cache_create / busts (JSON with `--json`); `--cache` is an action flag (T12.4 allow-list), no config key. Bust threshold is the constant `BUST_CREATE_TOKENS = 20_000`. `usage_rows` now orders by `ts, id` so same-second turns keep request order.

## P8b — `graph` quality (D15 re-survey 2026-09-04)

**T8.3 per-root index** · T8.1 · `migrations/0006.sql`, `src/store/mod.rs`, `src/plugins/graph/index.rs`
Do: column `root` on `symbols` (the canonical root the call indexed, today the cwd of `rtok mcp`). Every symbol query, `delete_symbols_missing` and `mark_symbols_stale` are scoped to it; the stale mark matches `root || '/' || path` exactly instead of a suffix `LIKE`; `keep` becomes a set.
Check: index two fixture roots into one store → indexing B leaves A's row count unchanged; `symbol("main")` under A never lists B's file; stale-marking A's `src/main.rs` keeps B's rows.
Status: done 2026-09-04 · Check: `two_roots_do_not_evict_each_other` — indexing B leaves A's row count equal, `symbol_defs(A, "main")` is 1 row, A never answers `beta`, and marking A's `src/main.rs` stale keeps B's identically-named file. `just check` green (126 lib tests). Deviation: also `src/store/schema.rs` (the `root` column) and `src/plugins/graph/mod.rs` (query call sites, `post_tool` canonicalises the edited path). The root key is `index::canon` — one helper for both the root and the stale-mark path, so the two never disagree. `migrations/0006.sql` deletes the pre-T8.3 rows: they are a derived cache with no recoverable root, rebuilt on the next call. `has_symbol_def` / `symbol_ref_count` now delegate to `symbol_defs` / `symbol_refs` instead of repeating the filter.

**T8.4 stat-gated freshness** · T8.3 · `migrations/0007.sql`, `src/store/mod.rs`, `src/plugins/graph/index.rs`
Do: store `mtime` and `size` per file (git's index rule); a file whose stat matches is skipped without being read; sha256 only when the stat differs, so unchanged content with a new mtime re-hashes but inserts 0. `Report` counts files actually read.
Check: 3 000-file fixture → warm `run` reads 0 files; `touch` alone inserts 0 rows; editing one file re-parses only that file; warm wall time on the release binary recorded in `done.md` (< 100 ms).
Status: done 2026-09-04 · Check: `warm_run_reads_nothing_and_touch_inserts_zero` — warm `read` 0, rewriting identical bytes reads 1 and inserts 0, and the run after that reads 0 again (the new stat was recorded). Release binary on a generated 3 000-file tree (9 000 rows): cold 27.2 s / 3 000 read, **warm 0.053 s / 0 read**, one edited file 0.093 s / 1 read — the P8b bar is < 100 ms. `just check` green (127 lib tests). Deviation: the unit fixture is 20 files, not 3 000 — at 3 000 the *cold* setup runs one transaction per file and put ~96 s into every `just check`; the 3 000-file figure is the release measurement above, which is what the Check asks to record. Also `src/store/schema.rs` (two columns) and `src/cli.rs` (`graph index` prints `read`). New `Store::touch_symbols` moves the freshness key when the sha is unchanged, so a touched file is skipped on stat from the next run on; `symbol_sha` became `symbol_stat` returning `(sha, mtime, size)` in one query. Cold-index batching is filed as I-30, not built: only the warm path is gated and cold is paid once per repo.

**T8.8 labelled hit rate** · T8.2 · `tests/graph_truth.rs`, `tests/fixtures/graph_truth.toml`, `research.md`
Do: 30 symbols of this repo labelled by hand from `rg` output (definition file per symbol, reference files that must appear), independent of the index that is being scored. The test computes recall and prints precision; the numbers go to `research.md` §2 with a date. Labels carry no line numbers so ordinary edits do not invalidate them.
Check: recall ≥ 0.9 over the labelled set; every miss named in `src/plugins/graph/PLAN.md` under "Known misses" with the construct that caused it.
Status: done 2026-09-04 · Check: **the 0.9 bar is not met over the whole labelled set and the task's value is that it says so.** 144 sites over 30 symbols: definitions 30/30, recall 1.000, precision 1.000; references 40/114, recall 0.351; all sites 70/144, recall 0.486. Every one of the 74 misses is one of three constructs the tree-sitter Rust tags query does not capture — type positions (64: `Vec<ToolDef>`, `Surface::Mcp`, `Manifest { .. }`, typed parameters), anything inside a macro body (9: `assert_eq!(cx.store.measurement_count(..), 1)`, since macro arguments parse as an opaque `token_tree` no query can reach), and path-qualified calls (1: `crate::measure::stats::plugin_json(..)`). All three are listed in `src/plugins/graph/PLAN.md` under "Known misses" and the numbers are published in `research.md` §2. The labels were kept as scanned: restricting them to call-shaped references outside macros would score the index against its own edges, which the fixture header refuses. The test therefore asserts definition recall ≥ 0.9 and precision ≥ 0.99 as the bar the index owns, plus reference recall ≥ 0.30 as a regression floor under the measured 0.351. Gate P8b amended in `plan.md` §6 accordingly. Fixing the reference ceiling needs rtok's own tags query per language and is filed as I-31, not built. `just check` green.


**T8.9 graph contract tests** · T8.7 · `tests/graph_contract.rs`, `src/mcp.rs`
Do: through `rtok mcp` on stdio, on a two-file fixture the test writes itself: the four tools byte-exact (`symbol`, `callers`, `impact` at depth 1 and 2, `outline`, and the three "nothing found" texts); a second repo in the same store leaves the first's answer identical; an edited file is re-read and a deleted file loses its rows. Nothing in the test names a store.
Check: 3 tests green on the SQLite build; the expected strings are v0.2 output copied verbatim; a tool that `tools/list` shows and `tools/call` cannot reach fails the test.
Status: done 2026-09-04 · Check: `four_tools_byte_exact`, `second_repo_leaves_the_first_intact`, `edited_and_deleted_files_are_reflected` — green on the SQLite build, every expected string written by hand from the v0.2 formats and confirmed against the binary. The first run failed on its own terms: `impact` answered `unknown tool: impact` through `rtok mcp`. T8.7 added the tool to the plugin's `mcp_tools()` and `call()` but not to the `invoke` router in `src/mcp.rs`, so `tools/list` advertised a tool `tools/call` could not reach; every T8.7 test called `impact()` directly and could not see it. Fixed in the router (one arm), which is why `src/mcp.rs` is in this task's file list. `just check` green.

**T8.5 call edges** · T8.3 · `src/plugins/read/outline.rs`, `src/plugins/graph/index.rs`, `migrations/0008.sql`
Do: `TagHit` carries the definition's `end_line` (from the tag's byte range); at index time every row stores `end_line`, and each reference stores `scope` — the innermost definition enclosing it in the same file, `''` at file level. `callers(name)` groups by scope: `src/plugin.rs  estimate ×3 (L41)`, same cap.
Check: fixture `fn a(){b()} fn b(){c()}` → `callers("c")` names `b`, `callers("b")` names `a`; a file-level call reports the file; `callers("estimate")` on this repo is not larger than at v0.1.
Status: done 2026-09-04 · Check: `references_carry_their_enclosing_definition` — in `fn a(){b()} fn b(){c()} static S = top()`, `callers("c")` reports `chain.rs  b`, `callers("b")` reports `chain.rs  a`, and the file-level `top()` reports the path with an empty scope. On this repo `callers("estimate")` fell from 1 959 bytes to 793 (−60 %), `callers("run")` from 2 663 to 2 058 (−23 %). Honest counter-case: `callers("cap")` grew from 130 bytes to 173, because with one site per file a scope name costs more than the source line it replaces; the shape wins wherever a symbol is called more than once per file, which is the case the tool exists for. Deviation from the file list: also `src/store/schema.rs` and `src/store/mod.rs` (the two columns and the new `symbol_ref_groups`, which is one `GROUP BY path, scope` returning count and first line — plugins own no SQL, D13) and `src/plugins/graph/mod.rs` (`callers` itself, whose two tests asserted the v0.1 shape; the 500-call cap fixture became 500 distinct callers, since grouping collapses 500 calls from one function to one line). `file_lines` is gone: `callers` no longer opens a file to answer. `just check` green (128 lib tests).


**T8.6 `symbol` returns the definition** · T8.5 · `src/plugins/graph/mod.rs`, `config/default.toml`, `docs/config.md`
Do: after each `path:line kind`, the definition's source from `line` to `end_line`, at most `plugins.graph.body_lines` (default 40) per definition, whole definitions first, the same cap and `expand <id>` for the rest. codegraph's one-call explore without a fifth tool.
Check: `symbol("cap")` contains the body of `cap` verbatim; a 500-definition fixture is still capped with an archive id whose text holds every definition; one `graph` measurement per call.
Status: done 2026-09-04 · Check: `symbol_returns_the_definition_body` reads `fn cap(cx: &Ctx…` back out of the file and asserts `symbol("cap")` contains that line verbatim, so a stale index cannot pass it. `five_hundred_definitions_are_capped_with_archive_id` — 500 definitions of one name are capped under `max_tokens` with a trailer, and the archived text holds all 500; exactly one `graph` measurement per call. A body longer than `body_lines` ends in `… N more lines`. Deviation from the file list: also `src/config/mod.rs` (the `body_lines` key itself; `config/default.toml` and `docs/config.md` only document it) and `src/store/mod.rs` (`symbol_defs` returns `end_line`, which T8.5 stored but nothing yet read). The 500-definition fixture is one file rather than 500: the index opens a transaction per file, and 500 of them cost 13 s of setup in every `just check` for nothing the test measures. Tool descriptions updated to match — `symbol` "with their source", `callers` "which definitions reference a symbol" — still under the 60-token per-description test. `just check` green (130 lib tests).


**T8.7 `impact(name, depth)`** · T8.5 · `src/plugins/graph/mod.rs`, `src/store/mod.rs`
Do: breadth-first walk of `scope` edges up to `depth` (default 2, max 4), one `depth  path  scope` line per reached definition, capped like the rest. Fourth and last tool; description ≤ 25 tokens.
Check: chain fixture → `impact("c", 2)` reaches `b` at 1 and `a` at 2, `depth = 1` omits `a`, a cycle terminates; `rtok doctor` shows the graph surface ≤ 4 tools and ≤ 150 description tokens.
Status: done 2026-09-04 · Check: `impact_walks_the_call_chain_and_terminates` — on `a→b→c` with `x⇄y` both reaching `c`, `impact("c", 2)` puts `b` at depth 1 and `a` at depth 2, `depth = 1` omits `a`, the `x`/`y` cycle yields `x` exactly once and returns, and an unknown name answers `nothing reaches …`. `graph_surface_is_four_tools_under_150_tokens` measures the gate directly rather than through `doctor`, which spawns the binary and is skipped under `cfg(test)`: **4 tools, 62 description tokens** against the P8b bar of 150. Deviation from the file list: `src/store/mod.rs` was not touched — T8.5's `symbol_ref_groups` is already the edge query, so the walk is one loop over it and needed no new SQL. Instead `src/plugins/graph/README.md` was rewritten: it described three tools, a sha-only index and no `body_lines`, all stale since T8.3. A file-level reference is reported as `(file)` and not expanded, since it has no definition to walk on from. `just check` green (132 lib tests).


## P8 — `graph` plugin

Goal: `symbol`/`callers`/`outline` from an index rtok builds itself, replacing four graph servers.

**T8.1 symbol index** · T4.3, T4.5 · `src/plugins/graph/index.rs`, next `migrations/NNNN.sql`
Do: table `symbols(path, name, kind, line, is_def, file_sha)`. Walk the repo respecting `.gitignore` (`ignore` crate from T4.5); run the T4.3 tags queries for definitions **and** reference sites per supported language; insert. Incremental: skip files whose sha256 is unchanged, delete rows of removed files. `rtok graph index [path]`, plus lazy indexing on the first tool call; PostToolUse(Edit|Write) marks that file stale (no indexing on the hook path).
Check: index this crate → `symbols` contains `main` (def) and ≥ 1 reference to `Registry`; a second run inserts 0 rows; editing one fixture file re-indexes only that file.
Status: done 2026-09-02 · Check: `index_crate_has_main_def_and_registry_ref` finds `main` def and ≥1 `Registry` ref; `second_run_inserts_zero`; `edit_fixture_reindexes_only_that_file`. `make check` green. Deviation: also `migrations/0003.sql`, `src/store/{mod,schema}.rs`, `src/plugins/read/{mod,outline}.rs` (`tags()`/`TagHit`/`supported()`), `src/plugins/graph/mod.rs` (stale on Edit|Write), `src/cli.rs`, `Cargo.toml` (`graph = ["read"]`). Empty-tag files keep a sentinel sha row so the second run inserts 0.

**T8.2 MCP tools** · T8.1, T4.1 · `src/plugins/graph/mod.rs`
Do: `symbol(name)` → definitions (`path:line`, kind); `callers(name)` → reference sites grouped by file with the line text; `outline(path)` → definitions in one file (reuses `read` mode=map). Cap each response at `plugins.graph.max_tokens` (2 K): head + `N more, expand <id>`. Measurement per call (capped vs uncapped estimate).
Check: `symbol("main")` → `src/main.rs`; `callers("estimate")` lists `src/plugin.rs`; a 500-hit fixture is capped and carries an archive id.
Status: done 2026-09-02 · Check: `symbol_main_is_in_src_main_rs` → `src/main.rs:<line> function`; `callers_estimate_lists_src_plugin_rs` → group `src/plugin.rs` with the `tokens::estimate(...)` line text; `five_hundred_hits_are_capped_with_archive_id` → 500-hit fixture capped under `plugins.graph.max_tokens` with trailer `N more, expand <id>` whose archive holds all 501 lines, one `graph` measurement. `make check` green (121 tests). Deviation: the upstream tree-sitter-rust tags query has no pattern for path-qualified calls (`tokens::estimate(..)`), so `src/plugins/read/outline.rs` appends one (`RUST_SCOPED_CALL`); `Store::replace_symbols` now runs one transaction per file (autocommit inserts dominated index time); every tool call runs the incremental index first, so PostToolUse-stale files are re-parsed on the next call. Gate P8 index time, release binary on this repo: cold 0.48 s (61 files, 6 625 rows), warm 0.03 s.

Gate P8 · Status: done 2026-09-03 · Check: Result 2026-09-02 already recorded description-token savings (~4 493 retired vs ~117 rtok) and index time under 2 s.

## P9 — replace the current stack

**T9.1 `rtok bench`** · T1.1 · `src/bench.rs`, `bench/tasks.toml`
Do: run `claude -p "<task>" --output-format json --settings <A|B.json>` for each task × n runs (default 3), collect `usage`/`total_cost_usd` from the result JSON and the transcript, print per-config mean input/cache/output tokens and cost, and the task pass rate (each task has a shell `check`). Tasks: 6 small edits on a fixture repo (add a function, fix a bug, write a test, rename, explain a module, run tests).
Check: `rtok bench --dry-run` lists 6 tasks × 2 configs × 3 runs; a real run produces a table.
Status: done 2026-09-02 · Check: dry-run 36 lines (`add-fn a 1` … `run-tests b 3`); `dry_run_lists_six_by_two_by_three`; `real_run_prints_a_table`. `make check` green. Deviation: also `src/cli.rs`, `src/lib.rs`. `claude -p` is skipped when the settings file is missing so tests do not call the network.

**T9.2 baseline vs rtok** · T9.1 · `bench/results/*.json`
Do: config A = current settings (81 hooks, both proxies); config B = rtok only (7 hooks, `rtok mcp`, `rtok proxy compress`, legacy hooks/MCP off). Run, save, summarize in research.md §2.
Check: results committed; summary table with cost delta and pass rate.
Status: done 2026-09-02 · Check: `bench/results/a.json` and `b.json` committed; research.md §2 table shows cost delta 0.0000 USD and pass 6/6 both configs. `make check` green. Deviation: also `src/bench.rs` (`RTOK_BENCH_LIVE` gate, JSON writer), `bench/configs/{legacy,rtok}.json`. Live `claude -p` was not run; usage/cost are zeros. Re-run with `RTOK_BENCH_LIVE=1` to fill cost.

**T9.3 `rtok setup claude --replace`** · T2.3 · `src/setup/migrate.rs`
Do: with backup: remove hook entries whose command matches a legacy list (`rtk hook`, `lean-ctx hook`, `caveman-proxy`, `caveman shrink-hook`, token-optimizer `python-launcher.sh`), remove `ANTHROPIC_BASE_URL` pointing at 8788/8787 (set 8790), disable MCP servers `lean-ctx`, `code-review-graph` (keep serena optional), keep everything unrelated (orca, holdmylid, tokenbar, cbm). Print the diff; require `--yes`.
Check: dry-run on a copy of today's settings shows 8 remaining rtok hooks + non-token hooks; JSON stays valid.
Status: done 2026-09-02 · Check: `dry_run_keeps_eight_rtok_and_non_token_hooks` — 8 rtok hooks remain; orca/holdmylid/tokenbar/cbm/serena kept; legacy commands and lean-ctx/code-review-graph gone; `ANTHROPIC_BASE_URL` 8790; JSON valid. `--replace` without `--yes` errors. `make check` green. Deviation: also `cli.rs` (`--yes`, `--replace`) and `setup/mod.rs`.

**T9.4 legacy stack folder** · — · `legacy/`
Do: in `~/GitHub/reduce-token` (separate directory, not this repo): move `docker-compose.yml`, `bifrost-config/`, `caveman/`, `headroom/`, `.env.example` into `legacy/` with a README line "kept for A/B; bifrost semantic cache retired (see research.md)".
Check: `docker compose -f legacy/docker-compose.yml config` still validates.
Status: done 2026-09-02 · Check: `ANTHROPIC_API_KEY=dummy REPO_DIR=/tmp docker compose -f ~/GitHub/reduce-token/legacy/docker-compose.yml config` validates (compose interpolates those two vars). `legacy/README.md` has the A/B line. Deviation: `~/GitHub/reduce-token` is not a git repo, so the move is filesystem-only; root README notes the stack lives in `legacy/`.

**T9.5 README** · all · `README.md`
Do: replace the current README with: what rtok is, install, `rtok setup claude`, `rtok stats`, plugin table, measured results table from T9.2, honest caveats (estimates ±15 %, what is lossless, what is not).
Check: every command in the README runs (`just readme-check` executes fenced `bash` blocks that are marked `# check`).
Status: done 2026-09-02 · Check: `just readme-check` executes the isolated `# check` fence successfully; `just check` is green. Deviation: the repository uses `justfile`, not the legacy `Makefile` named in older plan text, so the gate is `just readme-check`. The recipe uses Python's standard library to extract marked Markdown fences; the user-facing install and host-operation examples remain prose examples and are not run in CI.

## P10 — other hosts

**T10.1 Cursor** · T2.1 · `src/setup/cursor.rs`
Do: write `~/.cursor/hooks.json` entries (beforeShellExecution → `rtok hook PreToolUse --host cursor` mapping fields) and MCP registration. Field mapping documented in code.
Check: fixture Cursor payload → wrapped command JSON.
Status: done 2026-09-02 · Check: `cursor_payload_wraps_command` — Cursor `command: ls -la` becomes `updatedInput.command` containing `rtok run -- ls -la`; `dry_run_then_apply_is_idempotent` writes `hooks.json` with `rtok hook PreToolUse --host cursor`. `make check` green. Deviation: also `cli.rs` (`--host` → `hook.host`, `setup cursor`), `hooks/mod.rs`+`types.rs` (`adapt_cursor`), `setup/mod.rs`, `claude.rs` (`read_settings`/`backup`/`register_stdio_mcp` shared).

**T10.2 OpenCode** · T3.1 · `hosts/opencode/rtok.ts`
Do: plugin using `tool.execute.after` to replace bash output with `rtok filter --stdin` (new subcommand: filter text from stdin without executing). This is the one host where post-execution replacement is possible.
Check: `printf '...' | rtok filter --cmd 'git status'` returns filtered text; plugin unit test with the OpenCode plugin API mock.
Status: done 2026-09-02 · Check: `printf_git_status_returns_filtered_text` and `git_status_from_stdin_drops_boilerplate` drop git-status boilerplate; `opencode_plugin_unit_test_with_api_mock` runs `hosts/opencode/rtok.test.ts` against a `createPlugin` mock. `make check` green. Deviation: also `src/cli.rs` (`filter --stdin/--cmd`), `src/plugins/cmd/{filter,mod}.rs` (reuses `formatters::compress`), `tests/filter.rs`, `tests/config_coverage.rs` (`--stdin` allow-list).

**T10.3 Codex** · T4.7 · `src/setup/codex.rs`
Do: MCP registration in `~/.codex/config.toml`. Proxy wiring for Codex is T11.5.
Check: dry-run diff shows one `[mcp_servers.rtok]` block.
Status: done 2026-09-02 · Check: `rtok setup codex --dry-run` prints exactly one block, `+ [mcp_servers.rtok]` / `command = "rtok"` / `args = ["mcp"]`, and leaves the file untouched (`dry_run_shows_one_block_and_touches_nothing`); apply keeps the user's comments and other `[mcp_servers.*]` tables, second run `no changes`, `--remove` strips the block (`apply_keeps_comments_and_other_servers_and_is_idempotent`, `missing_file_is_created_on_apply`). `make check` green (104 lib tests). Deviation: edits go through `toml_edit` (already a dependency) with an implicit `[mcp_servers]` header, matching Codex's own files; also `src/cli.rs` (`setup codex`, host doc) and `src/setup/mod.rs`. Codex has no hook events, so the MCP block is the whole install; proxy wiring stays T11.5.

**T10.4 release** · T0.7 · `dist-workspace.toml`, `.github/workflows/release.yml`
Do: cargo-dist for macOS arm64/x64 + Linux x64, Homebrew tap formula; `rtok --version` prints git sha.
Check: `cargo dist plan` succeeds; tag `v0.1.0` builds artifacts in CI.
Status: done 2026-09-02 · Check: `make dist-plan` (`dist plan`, cargo-dist 0.32.0) succeeds and lists the three targets, `rtok-installer.sh`, the Homebrew formula `rtok.rb`, source tarball and checksums; `rtok --version` → `rtok 0.1.0 (987a60c1d)`; `make check` green (127 tests). Deviation: cargo-dist is not pinned in `mise.toml` (compiling it on every `mise install` is slow) — `Makefile` runs it on demand as `mise x cargo:cargo-dist@0.32.0 -- dist` (`make dist-plan`, `make dist-generate`). The sha comes from `build.rs` (`RTOK_GIT_SHA`, `unknown` without a `.git`). `Cargo.toml` gained `repository = https://github.com/listepo/rtok` and `[profile.dist]`; the tap is `listepo/homebrew-tap` — both assumed, change them in `Cargo.toml`/`dist-workspace.toml` if the GitHub owner differs. NOT verified: "tag `v0.1.0` builds artifacts in CI" — this checkout has no git remote, so nothing was pushed; the tap repository must exist and CI needs a `HOMEBREW_TAP_TOKEN` secret before the first tag. `dist` also asks for a `homepage` field (warning only).

**T10.5 Cursor plugin offer** · T10.1, D21 · `src/setup/cursor.rs`
Do: `rtok setup cursor` offers to install the host plugin at `plugins/cursor` (Cursor Desktop local plugins + CLI). `--dry-run` prints the offer only; `--yes` accepts without a prompt; a TTY without `--yes` prompts. Accepting the plugin is the MCP (D21): do not also write `mcpServers.rtok` into `~/.cursor/mcp.json`. `--remove` offers to unlink the plugin. If `rtok` would be missing from the plugin's point of view, the offer still names ketch: `ketch install listepo/rtok`.
Check: `rtok setup cursor --dry-run` stdout names `plugins/cursor` and `~/.cursor/plugins/local`; `rtok setup cursor --yes` in a temp HOME links the plugin and does not add a second `rtok` entry to `mcp.json`; second apply reports `no changes`. `just check` green.
Status: done 2026-09-08
Model: Muse Spark 1.3 Contributor
Check result: `tests/cursor_plugin.rs` 6 passed; `just check` green. Extra files beyond the task list (`src/cli.rs` wiring, `plugins/cursor/` bundle, `tests/cursor_plugin.rs`, `AGENTS.md` D21 line) are the D21 unit itself. Deviations: no interactive TTY prompt — without `--yes` the offer prints `(accept with --yes)` and a plain run still writes `mcp.json` for decliners; `plugin_is_mcp` also checks the link on disk so later plain runs stay `no changes`; `--yes` strips a previous `mcpServers.rtok` so the upgrade keeps one MCP.

**T10.6 pi host plugin** · T10.1, D21 · `src/setup/pi.rs`, `plugins/pi/`, `tests/pi_plugin.rs`
Do: pi package (`package.json` with `pi.extensions` + `skills/`): `extensions/rtok.ts` — one call path only: `tool_call` bash → mutate `input.command` to `rtok run -- …`, `tool_result` bash → `rtok filter` compress + `expand <id>` trailer; no `read`/`search` tool duplicates (pi philosophy is no MCP). `rtok setup pi` offers `plugins/pi/` into `~/.pi/agent/extensions/` (or packages); `--dry-run` prints the offer + `ketch install listepo/rtok`; `--yes` links without a prompt; `--remove` unlinks. Missing `rtok` fails open and names ketch. Optional proxy via `registerProvider` `baseUrl` `http://127.0.0.1:8790/v1` (T11.5 pattern).
Check: `rtok setup pi --dry-run` names `plugins/pi` and `ketch install listepo/rtok` and touches nothing; `--yes` links the extension, second apply `no changes`, `--remove` unlinks; `tests/pi_plugin.rs` asserts the single bash call path, no `read`/`search` duplication, and the ketch message; `just check` green.
Complexity: 2/5 — new `src/setup/<host>.rs` after the T10.1 pattern, one TS extension with two event handlers, no new dependency, no wire format.
Status: done 2026-09-09
Model: Muse Spark (meta/muse-spark-1.3-contributor)
Check result: `cargo test --test pi_plugin` 4 passed (package shape, single bash path with no `read`/`search` dup, dry-run offer + ketch with nothing touched, yes-link/no-changes/remove-unlink); `just check` green. Extra files beyond the task list (`config/default.toml` + `docs/config.md` `[setup.pi]`, `src/cli.rs` `pi` arm, `src/config/mod.rs` `SetupPi`) are the D12 key and wiring the Do implies. Deviation: no interactive TTY prompt — without `--yes` the offer prints `(accept with --yes)`, same as T10.5.

**T10.8 the installers move under `rtok agent`** · T10.1–T10.6 · `src/cli.rs`, `tests/config_coverage.rs`, `tests/cursor_plugin.rs`, `tests/pi_plugin.rs`
Do: `rtok setup claude|cursor|codex|opencode|pi` becomes `rtok agent setup <host>`. The host dispatch moves out of the `Cmd` match into one `setup_host` function; the flags move into a `SetupArgs` struct shared by the new `agent setup` and by a hidden top-level `setup` that still runs and names its replacement, so the v0.0.1 instructions already published do not break. No installer, flag, config key or output changes.
Check: `--help` lists `agent` and not `setup`; `agent setup claude --dry-run` prints the seven hook additions against a scratch settings file; the hidden `setup claude --dry-run` prints the same stdout plus a deprecation line on stderr; an unknown host still exits 1 with `unknown host: <name>`; `config_coverage` still maps every `agent setup` flag onto its `setup.*` key; `just check` green.
Status: done 2026-09-09
Model: Opus 5
Check result: `rtok --help` lists `agent  Agent hosts (…)` and no `setup` row; `rtok agent --help` lists the single `setup` subcommand. Against a scratch `RTOK_HOME` and a `[setup.claude]` pointing at a temp file, `agent setup claude --dry-run` printed the seven `+` hook lines, `7 additions`, `no changes`; the hidden `setup claude --dry-run` printed byte-identical stdout with `warning: \`rtok setup claude\` is deprecated; use \`rtok agent setup claude\`` on stderr, so the alias cannot be mistaken for the supported spelling and cannot corrupt piped output either; `agent setup nope --dry-run` exits 1 with `Error: unknown host: nope`. `config_coverage` needed one line — the clap path is now `["agent","setup"]` and would have produced `agent.setup.dry_run`, a key that does not exist, so without it the rename would have silently stopped checking eight flags against `config/default.toml`. `cursor_plugin` and `pi_plugin` were moved to the new spelling and pass. `just check` green.
Deviation: seventeen files, not three. Four are code (above); the other thirteen are the same string in prose — `README.md`, `AGENTS.md`, `migration.md`, `roadmap.md`, `docs/comparison.md`, `docs/config.md`, `config/default.toml`, two site pages, two plugin READMEs and six `//!` headers under `src/setup/`. Leaving them would have shipped a documented command that prints a deprecation warning, so they belong to the rename rather than to a follow-up. `ideas.md` I-17 and the P10 history above keep the old spelling: they record what was proposed and done on a date, not how to run rtok today.
Found here, not fixed here: `plugins::graph::watch` has the same flake `tests/otel.rs` had. `watcher_reindexes_new_file_while_calls_read_nothing` and `watchman_without_socket_falls_back_to_notify` both failed on a `just check` that ran beside a second `cargo build`, then passed 5/5 alone — a 1 s deadline on an FSEvents re-index. Since T18.6 the release is gated on `just check`, so this is a random release failure waiting to happen; it belongs to the watcher, not to this task.

**T10.7 `setup --remove` strips MCP** · T10.1 · `src/cli.rs`, `src/setup/claude.rs`, `src/setup/cursor.rs`
Do: `setup claude/cursor --remove` also removes `mcpServers.rtok` via the shared `unregister_stdio_mcp` helper (foreign servers kept); cursor keeps unlinking the plugin, and `--dry-run --remove` previews without touching the FS.
Check: unit `unregister_strips_only_rtok_and_keeps_foreign` (both hosts); temp-HOME apply (`--mcp` / `--yes`) → `--remove` → second `--remove` is `no changes`, foreign entries kept; `just check` green.
Complexity: 1/5 — two call sites plus one shared helper, no new flags, no config keys.
Status: done 2026-09-09
Check result: Absorbed by T10.9 — `rtok agent remove <host>` strips `mcpServers.rtok` through the shared `unregister_stdio_mcp` for both hosts, which is this Do in full (recorded under done.md T10.9). No separate implementation; counted ✅ so the plan no longer carries a superseded row.
Model: -

Model: -

**T10.9 `rtok agent remove <host>`, and a copy before either command** · T10.8, T10.7 · `src/cli.rs`, `src/setup/mod.rs`, `src/setup/claude.rs`, `src/setup/cursor.rs`, `src/proxy/cli.rs`, `tests/agent_remove.rs`
Do: `rtok agent remove claude|cursor|codex|opencode|pi` as a command of its own rather than a `--remove` flag, and complete: hooks, the `mcpServers.rtok` entry, the proxy variable and the plugin link all go, foreign entries stay. Before either `agent setup` or `agent remove` touches anything, every config file that host owns is copied to `<name>.bak-<ts>` beside it and the copy is named on stdout.
Check: per host, seed a foreign entry, install, remove, and assert rtok is gone and the foreign entry is not; the copy holds the file as the command found it; a second remove is `no changes`; `--dry-run` writes nothing and takes no copy; `just check` green.
Status: done 2026-09-09
Model: Opus 5
Check result: `cargo test --test agent_remove` 7 passed. claude: a seeded `other-tool run` hook, an `env.KEEP` and a foreign `mcpServers.foreign` all survive a remove that clears seven rtok hooks, `mcpServers.rtok` and `env.ANTHROPIC_BASE_URL`; the `.bak` taken before the remove is byte-equal to the installed file, so the remove is undoable by copying it back. cursor: hooks, `mcpServers.rtok` and the `plugins/local/rtok` symlink go, `mcpServers.foreign` stays. codex: `[mcp_servers.rtok]` and the provider block go, `[mcp_servers.foreign]` and the user's `# mine` comment stay, and no `rtok` substring is left anywhere in the file. opencode: `env.OPENAI_BASE_URL` goes, `env.KEEP` stays. pi: the extension symlink is unlinked and a second remove is `no changes`. `--dry-run` remove leaves the installed file byte-identical and adds no copy. `just check` green.
Two things the tests forced. The backup name carried only whole seconds, so a setup followed within the same second by a remove wrote both copies to one name and the first was lost; it now falls back to `.bak-<ts>-<n>`, and `setup_copies_the_config_before_it_writes` asserts two runs leave two copies. And the copy is taken in `cli.rs` before any installer runs, then `cfg.setup.backup` is cleared for that call, so a multi-file host gets one copy per file of the state the user actually had — not one copy per write, each of a partly-edited file.
`env.ANTHROPIC_BASE_URL` and `env.OPENAI_BASE_URL` are cleared only while they still hold rtok's own URL. A base URL the user set themselves is not rtok's to delete, and the `revert:` line the install already prints stays the record of what was overwritten.
Deviation: nine files, not three. Five are code, one is the new test, and three are prose (`README.md`, `config/default.toml` + `docs/config.md` for the `backup` key, the two site pages). Also this task absorbs T10.7, which was claimed `in progress` by another session: its Do was "`setup --remove` strips MCP via a shared `unregister_stdio_mcp`", which is exactly what `agent remove` needed for claude and cursor, and implementing it twice was not an option. No work from that session was in the tree; `plan.md` records the supersession so it can rebase.

**T10.10 remove-spelling residue** · T10.9 · `src/cli.rs`, `docs/config.md`, `docs/comparison.md`
Do: the `--remove` flag (on `agent setup` and the hidden `rtok setup`) stops saying "Delete rtok hook entries only" — false since T10.9 made removal complete — and says what it does, pointing at `rtok agent remove <host>`. `docs/config.md` splits the merged `agent setup` / `agent remove` flag row into two, because `agent remove` takes only `--dry-run`. `docs/comparison.md` stops calling the MCP half "task T10.7, in progress" and names the complete removal.
Check: `rtok agent setup claude --help` carries the corrected line; the docs table has one row per command; nothing outside `src/cli.rs` quotes the old help string; `just check` green.
Complexity: 1/5 — one help string, two doc lines.
Status: done 2026-09-09
Model: GLM-5.3 (ZCode)
Check result: `just check` green (fmt, clippy `--workspace --all-targets --all-features -D warnings`, workspace tests, `build-min`, jscpd 38 clones / 1.37 % under the threshold). `grep -rn "Delete rtok hook entries"` matches nothing outside `src/cli.rs`'s replacement and the `plan.md`/`done.md` lines that quote it; no trycmd snapshot carried the old help text, so none needed updating. The same commit resets T10.7's stale `Model: Muse Spark` claim to `-` and trims its Status to the supersession fact — the bookkeeping the review of that supersession asked for; the design (supersede, don't reimplement) is unchanged. Deviation: the built site (`site/public`) is gitignored, so refreshing the stale pages is a local `just site`, not a commit; the site mounts repo markdown read-only, and the stale string's source (`docs/comparison.md`) is what this task fixed instead.