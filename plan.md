# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T48.8 | todo | P2 | 3 | 0% | |
| T49.2 | todo | P2 | 4 | 0% | |
| T50.1 | todo | P2 | 3 | 0% | |
| T50.3 | todo | P3 | 3 | 0% | |
| T51.1 | in progress | P3 | 5 | 5% | OpenCode / Muse Spark 1.3 |
| T52.2 | todo | P3 | 3 | 0% | |
| T52.3 | in progress | P3 | 4 | 10% | OpenCode / Muse Spark 1.3 |
| T53.1 | in progress | P3 | 3 | 10% | OpenCode / Muse Spark 1.3 |
| T53.3 | in progress | P3 | 3 | 0% | OpenCode / Muse Spark 1.3 |
| T53.4 | todo | P3 | 2 | 0% | |
| T55.8 | todo | P2 | 2 | 0% | |
| T55.9 | todo | P3 | 2 | 0% | |
| T55.10 | todo | P3 | 1 | 0% | |
| T55.11 | todo | P2 | 3 | 0% | |
| T55.12 | todo | P2 | 2 | 0% | |
| T55.13 | todo | P3 | 1 | 0% | |
| T55.14 | todo | P3 | 1 | 0% | |
| T55.15 | todo | P3 | 2 | 0% | |
| T55.16 | todo | P3 | 1 | 0% | |
| T56.1 | done | P2 | 2 | 100% | |
| T56.2 | in progress | P2 | 3 | 95% | |
| T56.3 | in progress | P2 | 3 | 85% | |
| T56.4 | done | P3 | 2 | 100% | |
| T56.5 | in progress | P2 | 2 | 80% | |
| T57.1 | todo | P3 | 3 | 0% | |

### T48.8. VS Code Copilot Chat host

From I-17. GitHub Copilot Chat in VS Code reads MCP servers from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in the VS Code profile dir, and agent mode may run hooks; T46.4 covered only the Copilot CLI and the desktop app.
Done when the VS Code user dir per OS (Code, Code - Insiders) is resolved, `rtok agents install vscode` registers `servers.rtok`, hooks are added only if VS Code documents a hook file the T46.3 Copilot mapping can serve, remove keeps foreign servers, and the host joins the e2e matrix, config and docs.

### T49.2. Ingest Codex, OpenCode and Cursor session logs

From I-03. `measure` reads Claude Code JSONL only; the other hosts reach the `usage` table only when they go through `rtok proxy`, so their sessions without the proxy are invisible to `stats`, the TUI and the dashboard.
Done when each host's local session store (Codex `~/.codex/sessions/*.jsonl`, OpenCode `opencode.db`, Cursor where it exposes token counts) is read by one reader per host behind the existing `measure` ingest, rows carry the host slug, re-ingest is idempotent, and each reader has a fixture test. A host without token counts is documented as unsupported, not estimated.

### T50.1. More `cmd` filter families

From I-05. `rules/default.toml` covers grep, rg, sed, cat, make, curl, npm, pnpm, node on top of the built-in cargo/git/test/ls rules; python, pytest, pip, go, docker, kubectl, gh and friends pass through unfiltered.
Done when the families are chosen by `rtok discover`-style counts from real transcripts (the evidence goes into `research.md`), each new rule has a fixture with before/after bytes and keeps failures and the `expand <id>` trailer, and `Measurement` rows show the saving per family.

### T50.3. Extra `read` modes

From I-07. `read` has full, lines, map and signatures. The measured Read tail (38–68 K char files) may still be served whole when only imports or code without comments are needed.
Done when a measurement on those files shows which extra mode (imports-only, comments-stripped, or none) saves tokens without losing the answer; each added mode goes through tree-sitter where a grammar exists, falls back to `full`, keeps the read cap and dedup, and has a test per language. If no mode wins, the card closes with the numbers.

### T51.1. Compress JSON and code inside the live zone

From I-09. `archive` rewrites only old `tool_result`s; huge JSON dumps and `data:` blobs in other live-zone fields stay whole every turn.
Done when a proxy-side pass shrinks such payloads losslessly (archived, `expand <id>`), only for content that is byte-stable across turns so the prompt cache holds, with a byte-stability test over a six-turn fixture on both wires and a `Measurement` row. Off by default until a bench shows cost per passed task does not rise.

Execution plan (OpenCode / Muse Spark 1.3; scope from I-09: NON-`tool_result` content — nested JSON dumps + `data:` blobs in user content blocks; plain live-tail text/code stays (model is working with it; last-2-turns rule); Responses/Gemini deferred with default-empty, card's "both wires" = Anthropic + Chat):
1. SDK `wire.rs` (additive, defaulted): `BlobRef { content, turn }` (no provider id — keyed by content hash) + `ToolResults::live_blobs(req)` (empty default) + `WireRequest::live_blobs()` passthrough.
2. `Anthropic`/`OpenAiChat` `impl ToolResults`: override `live_blobs` — user blocks only, SKIP `tool_result` blocks (archive owns them), yield text/image-document base64 + big-JSON text strings with turn counting mirrored from `tool_results`.
3. `archive/mod.rs`: `rewrite_blobs()` — `[plugins.archive] live_blobs = false` gate (default off: zero behavior change); eligible turn >= 2 (proxy invariant, not keep_turns); candidate = `data:`/base64 or JSON-parseable text over `min_tokens`; key `blob:{sha256}`, `pointer()` reuse with kind `live_blob`, expanded-skip, shared `record_run` tail with `rewrite()`.
4. Config key in mod.rs + default.toml + docs/config.md + config-show snapshot (env free, no flag).
5. `tests/proxy.rs`: six-turn inline requests (stable JSON blob in user turns) on Anthropic + Chat, compress mode — two identical POSTs byte-identical upstream; eligible turns pointered, turns 0-1 untouched, archive rows 0, `live_blob` Measurements present, `rtok expand <id>` recovers the original.
6. Verify: fmt, clippy, targeted nextest, build-min, jscpd. ~10 files — deviation noted.

### T52.2. More grammars and compressed index payloads

From I-16. Tags cover Rust, TS, JS, Python, Dart, C and Go. Java, Kotlin, Swift, C#, Ruby and PHP repos get no `symbol`/`outline`, and large indexes store plain text.
Done when each added grammar is an optional feature (dependency reasons in the commit, creator approval for new crates) with a fixture test, and index payload compression is added only if a large repo's `rtok.db` size is measured before and after.

### T52.3. Ranked repo map at SessionStart

From I-28 (aider repo map). The most-referenced definitions could orient the model at session start.
Done when a P7-style A/B shows the map lowers cost per passed task; the map is ranked by reference count from `symbols`, fits a share of the D5 budget alongside `memory`, is byte-stable across turns, and is off by default until that A/B passes.

Execution plan (OpenCode / Muse Spark 1.3): `bench` shells to `claude -p` (LLM-gated), so no A/B pass is obtainable in-task → implement OFF BY DEFAULT, record the outcome (stays off). One key `plugins.graph.map_tokens = 0` (0 = off; nonzero = token cap, the D5-budget share next to `memory.recall_tokens`); no indexing on the hook path (map reads existing rows only, empty index → no injection). `Store::symbol_top_refs(root, limit)`: names with ref counts + one def site, ORDER BY refs DESC, name ASC (byte-stable). `Graph::session_start` offers priority-1 `repo map` lines trimmed to the cap. Files: `src/plugins/graph/mod.rs`, `src/store/symbols.rs`, `src/config/mod.rs` + `config/default.toml` + `docs/config.md` (D12). Tests: ranked order, byte-stability, cap trim, off-by-default (no injection at 0), SessionStart hook e2e on/off. Measure map tokens on this repo for the record. Verify in isolation (main red on concurrent WIP).

### T53.1. Coaching nudges under an A/B

From I-18. Short nudges ("do not re-read", "use expand") may cut waste, but they are re-read every turn and dilute instructions.
Done when an opt-in `inject` nudge set exists as data (D7), stays inside the D5 budget and byte-stable, and a P7-style A/B on the bench shows it does not raise cost per passed task; without that result it stays off.

Execution plan (T53.1, OpenCode / Muse Spark 1.3):
1. `modes/nudges.md` (new, data per D7): re-read/expand/outline-first/search-before-Grep nudges, ≤250 tokens like terse/yagni.
2. `src/plugins/inject/mod.rs`: `NUDGES` const + `builtin("nudges")` arm (same resolution as terse/yagni; opt-in via modes list, default off); test: ≤250 tok, SessionStart-once + byte-stable, absent from UserPromptSubmit.
3. Evidence: hook SessionStart bytes on/off (measured), dry `rtok bench` both ways (pass parity; zeros without RTOK_BENCH_LIVE), recorded in `research.md`; live cost gate stays open → default off. No live bench (needs API spend + approval — not run).
4. Verify in isolated worktree: fmt, clippy `-D warnings`, nextest (inject, hook e2e).

### T53.3. Hook start without Security.framework

From I-32. On macOS the one binary links Security.framework and CoreFoundation for reqwest's platform verifier, costing about 1.3–1.5 ms of dyld time per hook spawn, as much as the hook's own work.
Done when the creator picks the trade-off (webpki roots with `use_preconfigured_tls` and dead-stripped dylibs, versus a second tiny hook binary), the choice is recorded as a decision, and the hook p95 before/after is measured and stored in `research.md`. Corporate CA support must be documented either way.

Execution plan (OpenCode / Muse Spark 1.3; decision as given: webpki + `use_preconfigured_tls`, single binary): verified in reqwest 0.13.4 source that feature removal cannot work — `rustls_platform_verifier::Verifier::new` is referenced ungated under `__rustls`, and 0.13 has no webpki-roots feature, so `__rustls`-only does not compile; the `rustls` feature stays and the verifier becomes never-called-but-linked (otool decides the fact). 1) `Cargo.toml` + `rustls 0.23` + `webpki-roots 1` + `rustls-pemfile 2` (one-line reason in commit), `toolchain.md` + workspace `rust.md` rows; 2) `src/tls.rs`: `preconfigured()` builds `rustls::ClientConfig` (aws-lc-rs provider) from Mozilla roots, extended with `SSL_CERT_FILE` PEM bundle when set (fail closed with context — curl parity); unit tests (roots non-empty, bundle loads, missing/empty errors); 3) `src/proxy/mod.rs` + `src/otel/export.rs`: both client builders add `.use_preconfigured_tls(...)` (one shared helper, no duplication); 4) `research.md` T53.3 section: decision record + otool before/after + release hook p95 before/after via `tests/latency.rs` serialized + binary size; 5) `docs/config.md` [proxy] corporate-CA paragraph. Verify in isolation worktree (HEAD + own files): fmt, clippy `-D warnings`, nextest (tls/proxy/otel scope); release latency + otool before/after. Commit own hunks only.

### T53.4. `just otel-check` against real backends

From I-33. OTel export is gated by mock collectors; the Jaeger 2.11 and Grafana `otel-lgtm` recipes in `docs/otel.md` were checked by hand once.
Done when `just otel-check` starts both containers on shifted ports, flushes a copy of a fixture ledger, and asserts through their APIs: Jaeger has `execute_tool` spans for `service=rtok`, Tempo answers the trace id, Prometheus has `rtok_calls_total`; it skips with a clear message when Docker is missing, and it stays out of `just check`.

### T55.8. Guard `read:` keys survive a mutating Bash

From review 2026-09-17 (code read, no fix). `src/plugins/guard/mod.rs` `post_tool` clears only the `bash\t…` prefix on a non-keyed `Bash`/`Edit`/`Write`; the guard's own `read:{path}` keys are cleared only by `plugins::read::cache::invalidate` on `Edit`/`Write`. So `Read foo.rs` → `Bash cargo fmt` (or `sed -i`, `git checkout`) → `Read foo.rs` within `window_turns` is denied with the stale archive; the same happens on `Edit` when the `read` plugin is disabled. `Store::clear_read_cache` deletes `path` or `path\t…`, so `read:{path}` cannot be cleared by prefix today.
Done when the guard owns invalidation of its own keys: a mutating `Bash` drops every guard `read` key (key scheme `read\t{path}` or a prefix clear), `Edit`/`Write` drop that path's key without depending on the `read` plugin, with unit tests `bash_mutation_allows_the_next_read` and `edit_with_read_plugin_off_allows_the_next_read`, and the existing `edit_clears_guard_read_so_the_next_read_is_allowed` still passes.

### T55.9. Guard Bash key is cwd-blind

From review 2026-09-17 (code read, no fix). `norm_cmd` strips every leading `cd … &&`, so `cat x` and `cd docs && cat x` share one key and the second is denied as a duplicate of the first (`bash_repeat_behind_cd_prefix_denies` pins this as intended). Relative paths and `git status` differ per directory, so the deny returns the wrong archive. `strip_cd_and` also cuts at the first `&&` even inside quotes (`cd 'a && b' && ls`).
Done when the key keeps the effective `cd` target (normalized spacing, quotes handled by one helper shared with `measure::stats` if the shapes match), `cd a && ls` ≠ `ls` ≠ `cd b && ls`, `cd a && cd a && ls` = `cd a && ls`, a quoted path with `&&` inside is not split, and the existing test is rewritten to assert those cases.

### T55.10. One `cmd_stem`

From review 2026-09-17 (code read, no fix). The "basename, split on `/` and `\`, drop `.exe` case-insensitively" helper exists three times: `plugins::cmd::formatters::cmd_stem`, `measure::stats::bash_family` (inline), `agents::is_rtok_bin` (inline). `bash_family` cannot call the `cmd` one because `measure` builds without the `cmd` feature (`just build-min`).
Done when one `pub(crate) fn cmd_stem` lives in a feature-free module (e.g. `src/util.rs` or `src/agents/mod.rs`), the other two call it, behavior is unchanged, and `cmd_stem_strips_windows_path_and_exe` plus the `bash_family_*` and `is_rtok_bin` tests still pass under `just check` and `just build-min`.

### T55.11. `expand` never freezes the owning session's pointer

From review 2026-09-17, second pass (reproduced). `store::mark_expanded` (T45.3) updates `archive_decisions … WHERE session = ? AND archive_id = ?`, but every `expand` caller runs under a session that owns no decisions: CLI `rtok expand <id>` opens `Runtime::open(cfg, "expand")` (`src/expand.rs:122`) and MCP `expand` opens `mcp-<pid>` (`src/mcp.rs:110`), while the decisions belong to the proxy session (`metadata.user_id` / `x-rtok-session` / body sha). So `mark_expanded` always touches 0 rows: the pointer is rewritten again on every later request (the doc claim in `src/expand.rs:8-11` — "the owning plugin sends the original from the next request on" — is false end-to-end), `live_zone_pointer` never matches so every expand Measurement is attributed to `archive` (never `toon`), and `archive_decision_counts`'s expanded count stays 0 — the expand-rate honesty metric reads 0 % forever. Repro (scratch test, run and deleted 2026-09-17): put a decision under session `proxy-sess`, call `expand::fetch` through a `Runtime` with session `expand`, then `archive_decision("proxy-sess", …).expanded` is still `false`.
Done when expanding an id freezes the decisions that actually point at it — either `mark_expanded` drops the session filter (one `expand` freezes every session following that pointer; cross-session blast radius stated in the card) or expand learns the owning session — with tests `cli_expand_freezes_the_owning_sessions_pointer` (decisions rows flipped) and `expand_measurement_attributes_toon_pointers` (`live_zone_pointer` without a session), and `tests/proxy.rs` extended: after `rtok expand <id>`, the next forwarded request carries the original block, not the pointer.

### T55.12. Windows `wrap_quote` corrupts apostrophes under POSIX host shells

From review 2026-09-17, second pass (code read — not reproducible on macOS). `run::wrap_quote` (`src/plugins/cmd/run.rs:18-24`) emits PowerShell `''` escaping on Windows, and `cmd/hook.rs:45` rewrites the Bash command to `rtok run -- 'echo it''s fine'`. Claude Code on Windows executes the Bash tool through Git Bash (POSIX sh), where `'echo it''s fine'` concatenates to the single argv `echo its fine`: the apostrophe is silently dropped and the command the model asked for is not the command that runs (fail-open violation, no error anywhere). T55.4 weighed PowerShell and cmd.exe but not the POSIX host.
Done when a Windows rewrite containing `'` cannot reach a POSIX shell unchanged-but-wrong — the minimal fix mirrors the heredoc skip: on `cfg!(windows)`, `skip_wrap` also returns true for any command containing an apostrophe (nothing is wrapped, output stays whole; compression loss is the safe direction) — with pure tests `windows_apostrophe_commands_stay_unwrapped` and a parse-simulation `ps_quoting_does_not_round_trip_under_sh` proving the current form is lossy, plus the existing `wrap_keeps_apostrophe_host_safe` updated to the new contract.

### T55.13. Copilot `Read` events use `path`, guard and read-advice match `file_path` only

From review 2026-09-17, second pass (reproduced). `hooks::types::adapt_copilot` maps `view`/`read_file` to tool name `Read` but leaves the Copilot input key `path` (the fixture in `types.rs` shows `toolArgs: {"path": "a.rs"}`). `guard::cache_key` (`src/plugins/guard/mod.rs:120`) and `read::hook::pre_tool` (`src/plugins/read/hook.rs:13`) read `tool_input["file_path"]` only, so on the Copilot host the re-read deny and the large-file advice never fire; `cache::invalidate` already accepts both keys, so the codebase is inconsistent. Repro: `post_tool` + `pre_tool` with `{"path": p}` returns no deny where the `{"file_path": p}` twin denies.
Done when one shared helper (or the two lookups) accepts `file_path` **or** `path` in `guard::cache_key` and `read::hook::pre_tool`, with tests `copilot_path_key_dedups_like_file_path` (guard deny fires for `path`) and `copilot_path_key_gets_the_read_advice` (deny over `native_max_bytes`), and the existing `file_path` tests unchanged.

### T55.14. Semantic-cache key drops tool_result content

From review 2026-09-17, second pass (reproduced). `semantic_cache::messages_text`/`content_text` (`src/proxy/semantic_cache.rs:318-347`) keep only `text` fields, so a `tool_result` block contributes nothing to `CachePrompt` — neither to the direct hash nor to `scope_hash`. Two requests that differ only in tool-result text hash identically: repro shows `canonical_hash` byte-equal for `content: "tests passed: 200 ok"` vs `content: "tests FAILED: 1 broken"`, both `eligible` under default config (one user message, no tools, not streaming). Under today's defaults (`max_messages = 1`, direct tier) the collision needs a same-shape retry with a changed tool result; the moment an operator relaxes `max_messages` (the config comment itself cites bifrost's 3), any two agent runs sharing a prompt but not tool outputs serve each other's cached answer. I-23's "a hit can be a wrong answer" does not cover a key that ignores the request's own data.
Done when `content_text` folds tool_result block text into the prompt (tool_use ids + result text; images contribute their sha256), with tests `tool_result_text_joins_the_cache_key` (the two bodies above hash differently), `tool_use_ids_join_the_cache_key`, and the existing `only_near_identical_prompts_clear_the_threshold` family unchanged.

### T55.15. `live_blobs` rewrites image/document payloads into invalid blocks

From review 2026-09-17, second pass (reproduced, flag off by default). `Anthropic::live_blobs` (`src/proxy/anthropic.rs:96-105`) yields `source.data` of `image`/`document` blocks and `OpenAiChat::live_blobs` yields `image_url.url` (`src/proxy/openai_chat.rs:65-70`); `archive::rewrite_blob` then overwrites that field with pointer text. Repro: after `rewrite_blobs`, `source.data` reads `[archived 7da78e9924c6: 1 lines · 3200 tokens · expand(7da…)]` — not base64, so the moment `[plugins.archive] live_blobs = true` is switched on, every request carrying an old image is rejected by the API (400), which is not fail-open. Text blocks carrying `data:` URIs are the case T51.1 actually wants.
Done when binary-bearing fields are never rewritten in place: `live_blobs` yields text blocks (and `data:` URIs inside text) only, or the rewrite replaces the whole block with a `text` pointer block; tests `image_source_data_is_never_rewritten` and `openai_image_url_is_never_rewritten` assert the fields stay byte-identical through `rewrite_blobs`, and the existing `live_blobs_*` suite still passes.

### T55.16. Guard deny loads the whole archive on the PreToolUse hot path

From review 2026-09-17, second pass (code read). `guard::pre_tool` (`src/plugins/guard/mod.rs:45-50`) calls `cx.get_archive(&id)` and estimates tokens over the full body just to fill the denial Measurement — on the ≤ 10 ms hook path, for an archive that can be megabytes (`cmd` archives raw stdout). The `archive` row already stores `bytes`; retrievability needs an existence check, not the body.
Done when the deny path touches only metadata (row `bytes` + file existence; the estimate derived from size, or the Measurement reduced to what a size-based estimate supports), with the existing guard tests green and a new `deny_does_not_read_the_archive_body` (poison/absent body file still denies or fails open without reading megabytes — assert via a huge archive and the latency harness or by construction).

### T56.1. Test VFS helper and convention

**All tests must prefer a virtual filesystem** over host `TempDir` / raw `std::fs` as the primary approach. Goal: unit tests run against an in-memory FS so they do not depend on real disk layout, and Windows/macOS path quirks (case fold, spaced profiles) can be simulated. `src/testutil.rs` ships `Vfs` (path → bytes) with `write` / `read` / `read_str` / `len` / `exists` / `paths` / `paths_under`.
**Done** — D29 recorded, AGENTS.md notes the rule, Vfs covers the API above, and read (`search_max_bytes_gate_uses_vfs_sizes`), cmd (`Settings::from_vfs` rules tests), and graph (`file_uri_encodes_spaces`) unit tests use it with no host temp dir.

### T56.2. Migrate read/search unit tests to VFS

Hottest filesystem tests first: `display_rel` / search size-gate logic should use `Vfs` or pure `Path` values. WalkBuilder-backed integration may stay on disk until a walk adapter exists (T56.4).
**In progress** — pure `display_rel` + Vfs size-gate/regex hits + Vfs line-numbering/range twins landed. WalkBuilder e2e disk fixtures **kept** with Vfs rewrite twins: `search_and_tree_skip_git_dir_from_vfs`, `tree_paths_stay_relative_from_vfs`, `search_paths_stay_relative_from_vfs`, `search_skips_files_over_search_max_bytes_from_vfs` (`.git` skip + tree rows helpers). T56.4 walk adapter landed (dir metadata / list + read). T56.5 `ReadFs` + `read_with`/`resolve_with`: disk e2e **kept**; Vfs twins for three-lines / range / caps / symlink escape. Remaining: polish any leftover disk-only read paths if still useful.

### T56.3. Migrate cmd/setup path tests to VFS

Quoting tests are already pure strings; setup/agent install tests that write hook files should use `Vfs` (or a directory trait) where practical.
**In progress** — disk `Settings::load` twins restored (`user_rules_*`, `drop_ins_*`, `a_broken_drop_in_*`); Vfs twins kept (`*_from_vfs`) plus extras. `issues_in_from_vfs` + spaced-path twin added (disk `issues_in_*` kept). Agent setup hook writers: disk e2e **kept**; Claude/Kimi Vfs rewrite twins for insert/strip/idempotent/foreign/wrong-shape/spaced profile. Remaining: more hosts (cursor/codex/…) Vfs twins if needed; MCP register still disk (SDK write path).

### T56.4. Optional walk/VFS adapter for search/tree

If search/tree keep needing real walks, introduce a narrow trait (metadata + read bytes + list dir) with a `Vfs` impl so oversized-file and relative-path tests run without host disk.
Done when search/tree unit tests for the size-cap and relative-path cases can run against `Vfs`, or the card closes with a measured reason to keep WalkBuilder-on-disk.
**Done** — `plugins::read::walk::{WalkFs, walk, search_hits, tree_rows}` + `Vfs::{list_dir, meta}` (dirs inferred). Production `search`/`tree` keep host `WalkBuilder` (gitignore); disk e2e kept. Size-cap / relative-path / skip-`.git` twins run on the adapter. `HostFs: WalkFs` stub landed under test (T56.5); prod swap of WalkBuilder is T56.5 follow-up, not required here.


### T56.5. ReadFs trait + optional HostFs walk swap

Post-T56.4 leftover: drive full `read()` resolve/content (line numbering, caps, symlink) through a narrow FS trait on `testutil::Vfs` without deleting disk e2e; optionally share `WalkFs` with production search/tree later.
**In progress** — `plugins::read::fs::{ReadFs, HostFs}` + `read_with` / `resolve_with`; production `read`/`resolve` use `HostFs`. Vfs gains optional symlinks. Disk tests kept; Vfs twins for three-lines / range / caps / symlink. `HostFs: WalkFs` stub + smoke test (cfg test). **Follow-up (not this PR):** swap production `search`/`tree` from `ignore::WalkBuilder` to `WalkFs`+`HostFs` only if gitignore parity is measured and the swap stays small — do not rewrite for its own sake.


### T57.1. Flag-aware `guard` read-only classes

From I-38 (promoted 2026-09-17). `guard::read_only` decides which Bash calls get a dedup key from a fixed stem list (`ls cat head tail grep rg find tree wc` plus `git status|log|diff|show|branch`). It ignores flags, redirections and pipes, so it errs both ways:
- **Writers keyed as read-only** (correctness): `find . -name x -delete`, `cat a > b`, `grep x > out`, `ls | xargs rm`, `tail -f log` are keyed, so they never reach the "mutating Bash clears every `bash` key" arm; a following repeat of `ls` or `cat b` is denied with a stale archive (fail-open violation, same family as T55.8).
- **Repeats never keyed** (missed savings): `sed -n 1,40p f`, `jq . f`, `awk '{print $1}' f`, `git rev-parse HEAD`, `cargo metadata`, `wc -l` under a pipe.
Done when:
1. Evidence first: stem and flag counts over real transcripts (`[stats] transcripts_dir`, the `measure::stats::collect` path `doctor` already uses) for Bash calls that repeat inside `window_turns`, recorded in `research.md` with the date and command; stems are added or removed only with a count behind them.
2. `read_only` becomes flag-aware: a command is keyed only if its first stem is read-only **and** it has no writer marker — `>` / `>>` redirection, `| tee`, a pipe into a non-read-only stem, `find … -delete` / `-exec`, `sed -i` / `--in-place`, `tail -f`. Any command with a writer marker takes the mutating path and clears the `bash` keys. New read-only stems come from step 1 (expected: `sed` without `-i`, `jq`, `awk`, `git rev-parse`, `cargo metadata`). Parsing stays first-word + marker scan; no shell grammar (`cmd/AGENTS.md`).
3. Unit tests in `src/plugins/guard/mod.rs`: `sed -n` keyed and `sed -i` mutating; `find -delete` mutating; `cat a > b` mutating; `tail -f` never keyed; `cat a | grep b` keyed; `ls | xargs rm` mutating; and the false-deny Check: `ls` → `find . -delete` → `ls` is allowed.
4. `guard` deny Measurements (`kind = guard`) on the hook e2e fixture before and after, so the change in deny count is a measured row, not a claim. Off-by-default is not needed: the change only removes wrong denies and adds keyed repeats that already carry a retrievable archive.
Depends on T55.8 and T55.9 (guard key ownership and cwd) landing first, so the tests do not pin two behaviors at once.

## Reference

Historical phase notes (P0–P39) live in `done.md`. Companion evidence: `research.md`, `architecture.md`. Per-plugin plan: `roadmap.md`. Unapproved propositions: `ideas.md`.

Claim a `todo` row before work: set Status to `in progress` and Agent to `Provider / model`. Before stopping unfinished work, set Status to `todo` and clear Agent. When the Check passes, move the task entirely to `done.md` (Do/Check + Check result) and drop it from this table, its card, and `todo.md`.

### Decisions (read before any task)

| # | Decision | Why (evidence in `research.md`) |
| --- | --- | --- |
| D1 | **Rust, one static binary, in-tree plugins behind one trait + Cargo features.** No WASM, no subprocess plugins, no daemon **on the hook path in v0.1** (`rtok demon` supervises the long-running surfaces only, D22). A later WASM plugin host is P32 (landed); this repo still does not wrap third-party tools (D6). | Hooks run on every tool call; Rust cold start vs Python hooks. |
| D2 | **One binary, three surfaces:** `rtok hook` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`). | PostToolUse hooks cannot modify tool results. Only PreToolUse rewrite, MCP tool replacement, or a proxy can shrink what the model sees. |
| D3 | **Measurement first.** Nothing ships until `rtok stats` reads real usage from session logs and the proxy. Every plugin logs before/after. Metric = *context-token-turns* (tokens × turns they stay in context), plus output tokens. | Vendor claims vs measured savings; nobody in the stack measures end to end. |
| D4 | **Lossless by default.** Every compression keeps the original retrievable via `rtok expand <id>` / MCP `expand`. Lossy only where the source is regenerable (re-run the command). | Trust is the product. |
| D5 | **Injection budget.** All SessionStart/UserPromptSubmit injections go through one plugin with a per-turn token cap (default 800) and a byte-stable prefix. | Injections are re-read (cached) every turn. |
| D6 | **Every plugin is native, written from scratch in this repo. No third-party plugins.** A plugin never spawns, links, imports, or reads the data of another tool. Third parties extend rtok from outside through the public plugin API (`rtok-plugin-sdk`, `Registry::from_plugins`, `docs/plugin-authoring.md`), never through this repo. | One code path per method is what `Measurement` can attribute. |
| D7 | **Prompt "modes" (terse, YAGNI) are data files, not code.** | Measured effect must be A/B tested, not assumed. |
| D8 | **One SQLite file** (`~/.rtok/rtok.db`, WAL). Schema is D13. Raw archived payloads on disk under `~/.rtok/archive/` (D4); the DB holds indexes and inline JSON under a size cap. | Field tools converge on SQLite (+FTS5). |
| D9 | **Agents are provider-agnostic.** Route by job: low-cost for mechanical work; mid-tier for coding; high-performance for research only after the user confirms. Host names are products, not the implementer. | User constraint: cost-aware routing, any provider. |
| D10 | **Retire, don't stack.** Phase 9 replaces the legacy hook pile with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities across bash/read/memory/graph tools. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages and OpenAI Chat Completions / Responses are `Wire` adapters behind one proxy. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. | One proxy, two parsers is cheaper than two proxies. |
| D12 | **One config file holds every setting; every CLI flag is a config key.** Precedence: defaults < user file < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env < flags. `rtok config show --sources` tells where each value came from. | Hooks, long-running servers, and benches need one precedence rule. |
| D13 | **Core persists through a sync ORM on bundled SQLite.** Diesel (`sqlite` + bundled `libsqlite3-sys` with FTS5). Plugins never write SQL; `Store` is the only DB owner. Hook path: metadata always, body only if under the inline cap — never archive, never fail the hook (D1). | Diesel is sync, so the ≤ 10 ms hook path stays blocking and fail-open. |
| D14 | **CLI is clap 4 (derive); config layers are figment; TOML writes are toml_edit.** Env is `RTOK_<SECTION>_<KEY>` looked up in a leaf table from `Config::default()`. | Clap owns the subcommand tree; Figment tracks per-key provenance. |
| D15 | **Every plugin is designed against alternatives before it is built.** Each catalogue plugin has `src/plugins/<id>/PLAN.md`. | From-scratch only pays if the design beats what it retires. |
| D16 | **Git is `main` only.** Agents commit on `main`. Do not create feature branches. One task is still one commit (`<task-id>: <title>`). | `main` is the single line of work. |
| D18 | **The graph index lives in SQLite with the ledgers (D8).** LadybugDB and Grafeo were gated, frozen, then removed (P39). No live `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` feature flags. SQL for symbols lives only in `src/store/symbols.rs`. D6 holds: no spawned graph tool. | Both graph-store candidates were priced and deleted per the gate. Survey archive: `src/plugins/graph/PLAN.md`. |
| D19 | **Observability is a projection of the ledgers, never a second recorder.** OpenTelemetry export reads existing rows and posts OTLP/HTTP JSON. Nothing runs on the hook path. | Delivery is at-least-once behind a per-stream watermark. |
| D20 | **Local web UI is an operator surface, not a catalogue plugin.** `rtok web` serves axum + a Slint WASM UI (`crates/rtok-webui`). Slint is not linked into the hook binary. | Linking Slint into `rtok hook` would fail the size/latency gate. |
| D21 | **Every new plugin is plugin and MCP as one unit, a singleton, with one call path per capability.** Host plugins load in that host's desktop app and its CLI. Missing `rtok`: fail open and print that it must be installed with ketch. | Duplicate MCP processes and duplicate call paths break D18 and measurement. |
| D22 | **`rtok demon` supervises rtok's own long-running surfaces**, not a catalogue plugin. Allow-list names (`proxy`, `mcp`, `dashboard`). Nothing in it is on the hook path. | The proxy is the wire hop; when it dies every host silently loses it. |
| D23 | **`rtok tui` and `rtok web` are two renderings of one operator model.** A page that exists on one surface and not the other is a defect. | Two independently built surfaces drift. |
| D24 | **`rtok report` renders; it never computes a number of its own.** It reads the D23 operator model. Recommendations are rules over those rows, never an LLM call. | A saving that is not a `Measurement` row does not exist (D3). |
| D25 | **The plugin contract is `rtok-plugin-sdk`.** Required methods are explicit; event methods keep no-op defaults. `rtok` is the only dispatcher. | One contract, no in-tree shortcut. |
| D26 | **One log with two readers:** a rotating text file and the `logs` table OTel exports. Bounded (`max_bytes` 1 MiB, `files` 5). | An unbounded log on a long-running proxy is a disk-full bug. |
| D27 | **Anything a command prints, or the store keeps, is a page on `rtok web` and `rtok tui`.** Writing commands stay CLI-only. | The two surfaces plus CLI must not disagree about what a session is. |
| D28 | **The agent-host contract is `rtok-agent-sdk`.** Installers go through it; host-specific code stays in `src/setup/<host>.rs`. | One write cycle, one plugin-offer body. |
| D29 | **Unit tests prefer a virtual filesystem (`testutil::Vfs`) over host TempDir/std::fs.** Pure path/content/size logic must not require real disk; Windows/macOS quirks are simulated in Vfs. Migrate hottest suites first (read/search/cmd/setup) as T56.x — not a big-bang rewrite of e2e. | Hermetic tests; reproducible CI; path-case and spaced-path bugs (T55) need a simulated FS. |

### Architecture

```
                    ┌──────────────── rtok (one binary) ────────────────┐
 Claude Code ──hook─┤ hook <event>  → EventBus → plugins (Pre/Post/...) │
 Cursor/OpenCode ───┤ mcp           → tools: read search tree run expand │
                    │                 mem_save mem_search symbol callers │
 ANTHROPIC_BASE_URL ┤ proxy         → usage capture, live-zone rewrite   │──▶ api.anthropic.com
                    │ stats | bench | doctor | setup | run | expand      │
                    └──────────┬─────────────────────┬──────────────────┘
                         ~/.rtok/rtok.db        ~/.rtok/archive/
```

`Ctx` gives every plugin: the DB handle, the token estimator, the archive store, config, session id. `Measurement { plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id }` is the only way savings enter the DB.

Plugin catalogue (v0.1). Every plugin is native Rust written from scratch here (D6).

| id | spec (replaces; evidence in research.md) | surface | mechanism |
| --- | --- | --- | --- |
| `measure` | rtk gain, headroom savings, lean-ctx gain | `stats`, `bench`, proxy | session JSONL ingest + proxy `usage`; context-token-turns |
| `cmd` | rtk hook, lean-ctx ctx_shell | PreToolUse(Bash) → `rtok run` | archive raw output; per-family formatters + TOML rules; pointer trailer |
| `read` | lean-ctx ctx_read/search/tree | MCP `read`,`search`,`tree` + PreToolUse(Read) | modes via tree-sitter-tags; re-read dedup |
| `archive` | token-optimizer archive_result, headroom CCR | proxy live zone + `expand` | replace old large `tool_result` with pointer + head/tail |
| `proxy` | headroom proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` | passthrough + SSE; usage capture; never touches system, tools, or last 2 turns |
| `inject` | caveman shrink-hook, ponytail/caveman modes | SessionStart, UserPromptSubmit | budgeted, byte-stable prefix; modes as markdown |
| `guard` | token-optimizer refetch_guard | PreToolUse | identical read/command within N turns → deny |
| `memory` | engram, claude-mem | MCP `mem_save/search/get` | agent-written notes, SQLite FTS5 |
| `graph` | codebase-memory-mcp, serena, codegraph | MCP `symbol`,`callers`,`outline` | tree-sitter-tags index in SQLite; optional LSP is T30.2 |
| `toon` (off by default) | caveman toon, TOON | proxy/MCP | tabular JSON → TOON |

### Working agreement

- Graph backends: do not claim tasks that reintroduce `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` / `graph-grafeo` / cmake-for-liblbug. Symbol index is SQLite only.
- One task = one commit on `main` only. Never skip the Check.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message.
- Don't duplicate code or logic: find the existing helper and reuse it, or extract one shared helper at the responsible layer.
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run` green before every Check.
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.
- Every new CLI flag gets a key in `config/default.toml` and a row in `docs/config.md` in the same commit (D12).
- No plugin shells out to, links, imports from, or reads the data of a third-party tool (D6).
- Every new plugin obeys D21.
---

## Review 2026-09-17 — bug hunt (post #37–#47)

Scope: `origin/main` after cross-platform agent fixes #37–#47. Local WIP from other agents was stashed (`preserve-other-agents-wip-before-docs-review-bugs-plan`) and not reviewed. No code fixes in this pass — findings tracked as T55.x.

### Blockers

None for the macOS/Linux happy path on current main. Windows correctness gaps below are P1.

### Should fix

1. ~~**T55.1**~~ — done in #49 (`display_rel` case-insensitive strip).
2. ~~**T55.2**~~ — done in #49 (`never_wrap` case-insensitive stem).
3. ~~**T55.3**~~ — done in #49 (`file_uri` percent-encoding).
4. ~~**T55.4**~~ — done in #49 (host-shell-safe wrap / cmd quoting).
5. ~~**T55.5**~~ — done in #49 (`search_max_bytes`).
6. ~~**T55.6**~~ — done in #49 (`call` in mcp.cmd).

### Nits

1. ~~**T55.7**~~ — done (`skip_word`; quoted `cd` paths bucket by family).
2. **`expand::parse_range` when start > line count** — empty slice quietly; optional clamp.
3. **`guard::strip_wrap`** — updated in #49 for PowerShell `''`.
4. **T55.8 / T55.9 / T55.10** — filed from the T55.7 code read: guard `read:` keys survive a mutating Bash, guard Bash key cwd-blind, three copies of `cmd_stem`.

### Residual still open (called out before)

- T55.1–T55.6 closed by #49.
- PATH / single-quote parsers — rtok: T55.7 closed; larger residual in ketch.
- Guard false denies — T55.8 (P2), T55.9; flag-aware read-only classes promoted from I-38 as T57.1.
- Test VFS migration — D29 / T56.x (`Vfs` helper in #49).
- T48.3 still todo: Cursor plugin mcp.json still bare `rtok mcp`.

### Second pass (2026-09-17, later the same day)

Scope: hook dispatcher/types, guard, cmd (hook/run/rules/formatters), read (mod/cache/hook/search), archive, proxy (mod/wire/anthropic/openai_chat/semantic_cache), expand, store (archive/decisions/read_cache paths), mcp session handling. Four findings reproduced with a scratch integration test (written, run, deleted — `cargo nextest run --test zz_review_repro` → 4 failed exactly as predicted); two filed from code read. No code fixes in this pass — findings tracked as T55.11–T55.16, propositions as I-39/I-40.

- **T55.11 (P2)** — `expand` never freezes the owning session's pointer: every expand caller runs under session `expand` / `mcp-<pid>`, decisions belong to the proxy session; expand rate stays 0, toon attribution dead. Reproduced.
- **T55.12 (P2)** — Windows `wrap_quote` `''` quoting is wrong under Git Bash (Claude Code's Windows shell): apostrophes silently dropped from the rewritten command. Code read.
- **T55.13 (P3)** — Copilot `Read` inputs use `path`; `guard::cache_key` and `read::hook` match `file_path` only → dedup and advice never fire on that host. Reproduced.
- **T55.14 (P3)** — semantic-cache key ignores tool_result text; two requests differing only in tool results hash identically and are both eligible under defaults. Reproduced.
- **T55.15 (P3)** — `live_blobs` overwrites image/document `source.data` / `image_url.url` with pointer text → invalid request when the flag is on. Reproduced.
- **T55.16 (P3)** — guard deny reads the full archived body (and estimates over it) on the ≤ 10 ms PreToolUse path. Code read.

### Out of scope this pass

Concurrent agent WIP on local `main` (stashed as `preserve-other-agents-wip-before-docs-review-bugs-plan`). Half-finished T48–T53 cards on that WIP were not judged as shipped bugs.


---

## Note 2026-09-17 — testing library candidates

Shared catalog: [`listepo/rust.md`](../../rust.md) → *Testing candidates*.
Catalog only — no blanket `Cargo.toml` adds (Working agreement: justify each dep).

Fits for rtok (1–3):

1. `vfs` (crates.io) — evaluate against in-house T56 `src/testutil.rs` `Vfs`
   before adopting; mandate stays "prefer VFS over host TempDir".
2. `mockall` — trait mocks for provider / plugin host seams when hand fakes
   get noisy (`httpmock` stays for HTTP).
3. `tokio-test` — async unit helpers beyond `#[tokio::test]` where time/task
   control matters.

Already covered: `assert_cmd`, `divan`, `httpmock`, `insta`, `rstest`,
`trycmd`, `similar`. Skip `test-case` / `expect-test` / `mockito` duplicates;
`testcontainers` / `bolero`/`honggfuzz` only if a measured e2e/fuzz gap appears.
