# rtok — completed tasks

### T104. Migration and `schema.rs` drift guard

`MIGRATIONS` is a hand-kept list, and `src/store/schema.rs` `table!` macros are hand-kept too. A unit test: every `migrations/*.sql` file is in `MIGRATIONS`, in filename order, and after all migrations each `table!` column set equals `PRAGMA table_info`.

Check: deleting a line from `MIGRATIONS` or a column from `schema.rs` fails the test; `just test` green.

Do (2026-09-21): two unit tests in `src/store/mod.rs`. `migrations_list_matches_the_directory` reads `migrations/*.sql`, sorts the names and compares them with `MIGRATIONS` in order. `schema_rs_matches_the_migrated_tables` parses every `diesel::table!` in `schema.rs` (`include_str!`, `#[sql_name]` resolved to the SQL name) and compares each column set with `pragma_table_info` on a fully migrated in-memory store. Its first run found real drift: `0017.sql` (T69.2) added `notes.uses` and `notes.last_used`, which `schema.rs` never listed — both added (`Integer`, `Nullable<BigInt>`). Every `notes` query selects columns by name, so nothing else changed.

Check result (2026-09-21): mutation-checked — deleting the `0018.sql` line from `MIGRATIONS` fails `migrations_list_matches_the_directory` ("MIGRATIONS drifted"); deleting `notes.last_used` from `schema.rs` fails `schema_rs_matches_the_migrated_tables` ("schema.rs `notes` vs the migrated table"). Both green on the real tree; `just check` green.

### T103. Unit tests for untested store queries

No test calls `Store::memory_recall_totals` (`src/store/mod.rs`) or `call_io_archives`. Add unit tests on an in-memory store: empty store, one row, many sessions, rows outside the window.

Check: both functions covered by `src/store` unit tests; `just test` green.

Do (2026-09-21): two unit tests in `src/store/mod.rs` on `Store::open_in_memory`. `memory_recall_totals_sums_recalls_in_the_window_only`: empty store `(0, 0, 0)`; one row; three sessions summed while a `memory`/`save` row and a `read`/`recall` row stay out; one row backdated a day falls outside a one-hour window, and a window starting in the future is empty. `call_io_archives_names_only_spilled_bodies`: no `call_io` row → `(None, None)`; inline bodies → `(None, None)`; an over-cap request → its sha, which names a file in the archive dir, response `None`; both over cap → both ids; over cap with no archive dir (the hook path) → `(None, None)`.

Check result (2026-09-21): both tests pass; `just check` green with the branch.

### T92. `rtok agents install omp` — oh my pi: the shared pi extension plus native MCP

Creator request 2026-09-21: a host plugin for oh my pi CLI + desktop. oh my pi (https://github.com/can1357/oh-my-pi, binary `omp`) is a fork of pi with no desktop app — a TUI plus Zed ACP, which runs the same binary and config — so the host has one CLI variant. Its extension loader accepts `package.json` `omp.extensions` **or legacy `pi.extensions`**, treats symlinked directories as discovery targets, scans `~/.omp/agent/extensions` (not `~/.pi/agent/extensions`), and delivers the events `plugins/pi/extensions/rtok.ts` already subscribes to (`tool_call`, `tool_result`, `context`, `session_start`, `session_compact`); the extension imports nothing from upstream pi. So `plugins/pi` is reused as is — no `plugins/omp/` tree. Unlike pi, omp has native MCP (`~/.omp/agent/mcp.json`, `mcpServers.{command,args,env}`). Evidence: `docs/extension-loading.md`, `docs/extensions.md`, `docs/mcp-config.md` in that repo (fetched 2026-09-21).

Creator decision 2026-09-21: extension + native MCP. The extension owns the bash call path, context and compaction; tools come from `rtok mcp` registered in `mcp.json`; `registerTool` stays off under omp — one call path per capability (D21).

Plan:
1. `src/agents/omp/mod.rs` + `README.md` (`## Docs`: extensions, extension loading, hooks, MCP config, skills, marketplace): `HostPlugin { src_rel: "plugins/pi", host: "oh my pi", dest: <extensions_path>/rtok }` and `rtok_agent_sdk::register_mcp` on `mcp_path`; `support`: `plugin` → `Flag("--yes")`, `mcp` → yes, `hooks` → `No` (omp hooks are in-process TS modules; the extension owns that path), `proxy` → `No` (`models.yml` is not edited by setup, the pi rule). `[setup.omp] extensions_path = "~/.omp/agent/extensions"`, `mcp_path = "~/.omp/agent/mcp.json"` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md` (a named profile is a path override). Registered in `HOSTS` and `host()`.
2. `plugins/pi/README.md` gains the omp section and links; `tests/pi_plugin.rs` asserts the manifest still declares `pi.extensions` (the key omp's loader falls back to).
3. Unit tests: offer names `plugins/pi` and the ketch line; `--yes` links and writes `mcpServers.rtok`, second apply `NO_CHANGES`, remove takes back exactly ours and leaves foreign servers; `docs/agents.md` blessed; `tests/trycmd/agents-list*.toml` re-blessed.
Verified 2026-09-21 on omp 18.1.14 (probe extension in a scratch `PI_CODING_AGENT_DIR`, nothing written to `~/.omp`): (a) a **symlinked** directory whose `package.json` declares only legacy `pi.extensions` is discovered and its factory runs — the exact mechanism `plugins/pi` uses; (b) host signal: omp injects its SDK as `pi.pi` (an object with `getAgentDir()` and `VERSION`; `process.title` is `omp`), upstream pi's `ExtensionAPI` has no `pi` member — so `registerPiTools` returns early when `pi.pi` is an object, because that host has native MCP; without it a machine with pi (`[setup.pi] tools = true`) and omp would get every tool twice under omp (D21); (c) by source (`src/capability/mcp.ts` `key: server => server.name`, `src/capability/index.ts` first-wins dedupe in provider-priority order, native config highest): a native `rtok` entry and one imported from a Claude Code / Cursor config collapse into **one** server. Not verified: a real model turn whose bash call goes through `rtok run` — the only model key in this environment has no credit (`credit_balance_exhausted`); the creator runs one `omp -p` turn after install.
Also found by reading omp's source (`src/session/agent-session.ts` `#beforeToolCall`, `src/extensibility/extensions/wrapper.ts`): omp applies a revised input only when the handler **returns** `{ input }`; upstream pi documents mutating `event.input` in place. The bash rewrite reaches omp today only because omp hands `bash` handlers the live args object — an undocumented alias.

Split (each ≤200 LOC / ≤3 files):
- T92.1 (done, see `done.md`) — `plugins/pi/extensions/rtok.ts`: the bash rewrite mutates `event.input` in place **and** returns `{ input: event.input }` (pi ignores the extra field; omp's documented path); `registerPiTools` returns early when `pi.pi` is an object (omp — native MCP owns the tools). `plugins/pi/tests/rtok.test.ts`: two cases pin both. Check: `pi_plugin` green (it runs the Node test file).
- T92.2 (done) — host `src/agents/omp/` (`mod.rs` + `README.md`), `HOSTS` / `host()` in `src/agents/mod.rs`, `[setup.omp]` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`, unit tests from Plan step 3.
- T92.3 (done) — `docs/agents.md` bless, `tests/trycmd/*` re-bless, `plugins/pi/README.md` omp section.

Check: the unit tests above; `rtok agents list` shows `omp`; `agents_doc`, `host_docs`, `config_coverage`, `pi_plugin` green; `just check`.

Do (2026-09-21): T92.2 — `src/agents/omp/` (`mod.rs` + `README.md` with the module table and `## Docs` linking omp's extensions, extension-loading, hooks, MCP config, skills and marketplace pages). One CLI variant (`omp`). `apply` is `HostPlugin { src_rel: "plugins/pi", host: "oh my pi" }` linked to `<extensions_path>/rtok` plus `mcpServers.rtok = {command, args}` through `rtok_agent_sdk::register_server` (omp's documented shape has no `type`, the Kimi call); remove unlinks and drops only our server. `installed()` reports `plugin` only when `PLUGIN.ours` (T75) and `mcp` from `mcp.json`. `support`: plugin `--yes`, mcp yes, hooks and proxy no. Registered in `HOSTS` (after `pi`) and `host()`; `[setup.omp] extensions_path`, `mcp_path` in `config/default.toml`, `src/config/mod.rs`, `docs/config.md`. T92.3 — `docs/agents.md` blessed, `tests/trycmd/{config-init,config-show,doctor,report-md}` re-blessed (the `windsurf` block `TRYCMD=overwrite` folded into `...` was put back by hand), `plugins/pi/README.md` gains an oh my pi section, and `tests/pi_plugin.rs` says why `pi.extensions` must stay.

Check result (2026-09-21): unit tests `agents::omp::tests` (dry-run offer names `plugins/pi` and ketch, writes nothing; `--yes` links and registers with no `type`, second apply `NO_CHANGES` twice, remove keeps a foreign server; a foreign dest directory is not `installed`) pass; `agents_doc`, `host_docs`, `config_coverage`, `pi_plugin`, `cli_trycmd` green. On this machine (omp 18.1.14 at `~/.bun/bin/omp`) `rtok agents list` shows `CLI: oh my pi` and `rtok agents install omp --dry-run` offers `plugins/pi → ~/.omp/agent/extensions/rtok` and `mcpServers.rtok: rtok mcp`, writing nothing. `just check` green: 1109 passed, 4 skipped. Still open, as the card says: one real `omp -p` turn whose bash call goes through `rtok run` (no model credit here).

### T111. TS plugin tests on vitest, with snapshots

The host plugins' TypeScript tests (`plugins/opencode/rtok.test.ts`, `plugins/pi/tests/*.test.ts`) run on `node:test` + `node:assert`. Move them to vitest (creator's request) and pin structured outputs as snapshots where a hand-written deep-equal only restates the value.

Do: vitest 5.0.1 as a mise `npm:` tool like oxlint/jscpd (no `package.json`), `globals: true` and `cacheDir: target/vitest` in `vitest.config.mjs` so tests need no `vitest` import; rewrite the three files to `test`/`expect`/`vi`; inline snapshots for the registered tool names, the context replacement array and the compaction/transform outputs; `tests/filter.rs` and `tests/pi_plugin.rs` run `vitest run <file>` instead of `node --test`; `toolchain.md` and the `mise.toml` comment.

Check: `vitest run` green; `cargo nextest run --test filter --test pi_plugin` green; `just js` green.

Check result (2026-09-21): `vitest run` 3 files, 42 tests green; `cargo nextest run --test filter --test pi_plugin` 7/7; `just js` green. Snapshots: the opencode PreCompact/SessionStart hook payloads, the pi `context` replacement array, the pi `mcp --call` line per registered tool. `tests/common::vitest` is the one runner both wrappers use (Windows: `vitest.cmd`).

### T110. oxlint and oxfmt for the JS/TS files

Asked for by the creator. The six TypeScript files (`plugins/opencode/*.ts`, `plugins/pi/**/*.ts`, `tests/node/fake-rtok.ts`) had no linter or formatter; their line widths and quoting differed file to file. mise pins `npm:oxlint` 1.83.0 and `npm:oxfmt` 0.68.0 (both released 2026-09-14, oxc-project — maintained). `just js` runs `oxlint --deny-warnings` and `oxfmt --check` over `git ls-files '*.ts' '*.tsx' '*.js' '*.mjs' '*.cjs'` and is part of `just check`, so CI's `check` job enforces it; `just js-fmt` rewrites. JSON is deliberately outside the file list: oxfmt would reformat the plugins' manifests (`hooks.json`, `package.json`), which tests compare byte for byte. Defaults, no config file.

The one lint hit was real: `tests/node/fake-rtok.ts` used a ternary as a statement for its side effect (`no-unused-expressions`) — now `if/else`. Formatting diff is cosmetic only (100-column wrap, quote normalisation).

Check: `just js` exits 0 (0 warnings, 6 files formatted); the plugin behaviour tests that drive these files (`filter::opencode_plugin_unit_test_with_api_mock`, `pi_plugin::*`, `opencode_plugin::*`) pass after the reformat; `just check` green.

### T108. Guard tests for the Windows CI job

Asked for by the creator on the T82/T93 PR. `tests/windows_ci.rs`, no new dependency (`regex`, `ignore` are already in `Cargo.toml`):

1. `rtok_exe_reserves_an_8_mib_main_thread_stack` (`cfg(windows)`) reads `SizeOfStackReserve` from the PE optional header of `CARGO_BIN_EXE_rtok` and requires ≥ 8 MiB — dropping the T93 line from `build.rs` fails one named test instead of ~100 e2e crashes.
2. `byte_compared_files_are_lf_in_the_working_tree` — no CR byte in `tests/trycmd/` or `skills/`; on a Windows checkout this is what `.gitattributes` (T82) guarantees.
3. `windows_exclusion_list_names_only_existing_tests` — every `binary(x)` in the `cfg(windows)` `default-filter` is a `tests/x.rs`, every test name is a `fn name(` somewhere in `src/`, `tests/` or `crates/`, so a rename cannot leave a line that skips nothing and T83's list cannot look non-empty by accident.

Check result (2026-09-21): macOS — 2 passed (the PE test is Windows-only); mutation: renaming `test(=cli)` to `test(=cli_renamed_away)` in `.config/nextest.toml` makes test 3 fail with `stale exclusions: ["test cli_renamed_away"]`. `rustfmt --check` and `clippy --test windows_ci -D warnings` clean. The Windows half is proved by the PR's `windows` job.

### T109. `.editorconfig` matches the repository

The file dated from T0.7: a `[Makefile]` section for a repo without one, and whitespace rules applied to byte-exact fixtures — `tests/trycmd/{config-show,man}.stdout`, `{doctor,report-md}.toml` and `help-subcommands.trycmd` carry trailing spaces, 18 `tests/cmd_golden/*.out` and the 3 proxy JSON fixtures end without a newline, so an editor honouring the old file broke a golden on first save. Now: 2-space indent also for ts/js/sh/html/css/gotmpl (their measured majority), whitespace/newline/indent rules unset for `tests/{trycmd,cmd_golden,fixtures}/**` and `**/snapshots/**`, everything unset for the vendored fonts (CRLF `OFL.txt`) and `*.svg`; `end_of_line = lf` stays in step with `.gitattributes`.

Check: survey script over `git ls-files` (indent histogram per extension, trailing-whitespace and no-final-newline lists) — every file the old rules would have rewritten falls under an unset section.

### T82. Windows back in CI: green job with a named exclusion list

The advisory `windows` job in `.github/workflows/ci.yml` has been commented out since `224b215` (2026-09-17) because `cargo nextest` hung for hours. The hang itself is already handled: with `slow-timeout = { period = "60s", terminate-after = 3 }` (`552ad01`) the last Windows run (35244082778) finished in ~10 min — 833 run, 808 passed, 22 failed, 3 timed out. A job that is red on every push hides regressions, so the job comes back green: the known Windows failures are skipped by name and everything else must pass.

Plan:
1. `.config/nextest.toml`: one `[[profile.default.overrides]]` with `platform = 'cfg(windows)'` and a `default-filter` that excludes exactly the 25 tests that failed in run 35244082778, grouped by binary with the run id in the comment. One list, one place; `just test` on a Windows box skips the same set. Linux/macOS selection is unchanged (verify: `cargo nextest list` count is identical before and after).
2. `.github/workflows/ci.yml`: restore the `windows` job (`windows-latest`, `timeout-minutes: 30`, same `cargo nextest run --workspace` command). It stays `continue-on-error` and outside `revert-on-failure`'s `needs` until T83 empties the list — a Windows-only break must not auto-revert main yet.
3. `.gitattributes`: `* text=auto eol=lf` (the one CRLF blob, the webui font licence, is `-text`) — Windows runners check out with `core.autocrlf=true` and `tests/skill.rs` compares bytes.
4. Run the job on a scratch branch via `workflow_dispatch`; add tests that fail on current main (the suite grew 833 → 1 072 since the last Windows run) to the list until the job is green.

Check: a `ci` run on the final commit shows the `windows` job green with the skipped count equal to the list length; `just check` green locally; `cargo nextest list` on macOS selects the same tests as before the change.

Extra tests (creator request 2026-09-21): a test that every test named in the `cfg(windows)` `default-filter` still exists in `cargo nextest list --workspace` (a renamed or deleted test must not leave a stale exclusion that silently skips nothing); on macOS/Linux the filter selects zero tests out.

Check result (2026-09-21): ci run 35578041497 (`workflow_dispatch` on scratch branch `t82-windows-ci`, tree = origin/main + this change + T93) — `windows` success in 8 min, `1042 tests run: 1042 passed, 33 skipped` (29 by the platform filter + 4 `#[ignore]`); `check (ubuntu-latest)` and `check (macos-latest)` success. The list grew 25 → 29 on the way: run 35573995011 exposed T93 (104 stack overflows, fixed, not excluded) and 2 CRLF failures (fixed by `.gitattributes`, not excluded); run 35576438155 left 4 real failures, now listed and handed to T83. macOS `cargo nextest list` before/after the override: identical (1078 = 1078); with the platform flipped to `cfg(unix)` the filter removes exactly the listed tests. `just check` green locally.

### T93. `rtok` overflows the main-thread stack on Windows

Found by T82's first run on current main (ci run 35573995011, `windows-latest`, debug build): 104 of 117 new failures are the spawned `rtok.exe` dying with `thread 'main' has overflowed its stack`, exit `0xC00000FD` (-1073741571) — on `report`, `stats`, `logs`, `config init`, `plugins`, `info`, `mcp`, `agents install`, `hook PreToolUse` and `hook PostToolUse`. The hook crash breaks "fail open" on that platform. Cause: Windows reserves 1 MiB for the main thread where Linux and macOS give 8 MiB, and `cli::run()` is one function whose ~85-arm `match` keeps every arm's locals in a single debug frame. The shipped `x86_64-pc-windows-msvc` release build was not tested; its frames are smaller, but nothing guarantees the margin.

Plan:
1. `build.rs`: for a Windows target emit `cargo:rustc-link-arg-bins` with an 8 MiB stack reserve (`/STACK:8388608` on msvc, `-Wl,--stack,8388608` on gnu) — parity with the Unix default. Reserve is address space, not committed memory: no cost on the ≤ 10 ms hook path, no new dependency, and it holds for `cargo install` and `dist` builds alike (a `.cargo/config.toml` rustflag would not).
2. Verify on the T82 scratch branch: the `windows` job no longer shows `overflowed its stack` / `-1073741571` anywhere in its log.
3. Splitting `cli::run()` into per-command functions is the structural fix and stays out of scope (≤ 200 LOC rule); note it in `ideas.md` only if the 8 MiB reserve proves insufficient.

Check: `windows` job log of a `ci` run on the change has 0 occurrences of `overflowed its stack`; `just check` green on macOS (the script is a no-op off Windows).

Extra tests (creator request 2026-09-21): an integration test that runs in the `windows` job: `rtok hook PreToolUse` and `rtok hook PostToolUse` on garbage stdin exit 0 with `{}` — the fail-open rule asserted on the platform that broke it.

Check result (2026-09-21): ci run 35573995011 (before) — 117 failed, 104 of them `overflowed its stack` / `-1073741571`. ci run 35576438155 (after, same tree plus `build.rs` and `.gitattributes`) — 4 failed, 0 occurrences of either string in the `windows` job log. Local `rustfmt --check build.rs` and `cargo clippy -p rtok --bins -- -D warnings` clean; the branch is a no-op off Windows. The shipped release build was not tested before or after — the reserve applies to it as well.

### T92.1. The shared pi extension runs correctly under oh my pi

Part of T92 (`rtok agents install omp`). Verified 2026-09-21 on omp 18.1.14, in a scratch `PI_CODING_AGENT_DIR` (nothing written to `~/.omp`): a symlinked directory whose `package.json` declares only legacy `pi.extensions` is discovered and its factory runs; the real `plugins/pi/extensions/rtok.ts`, loaded through a recording wrapper, subscribes to `tool_call`, `tool_result`, `context`, `session_before_compact`, `session_compact`, `session_start` without error — each has an `on()` overload in omp's `ExtensionAPI`. Two gaps found in omp's source:

1. omp applies a revised tool input only when a `tool_call` handler **returns** `{ input }` (`src/session/agent-session.ts` `#beforeToolCall`, `src/extensibility/extensions/wrapper.ts`); upstream pi documents mutating `event.input` in place. The rewrite reached omp only through an undocumented alias of the live args object.
2. `registerPiTools` read `setup.pi.tools` with no idea of the host, so a machine with pi (`tools = true`) and omp would get every rtok tool twice under omp — `registerTool` plus omp's native MCP — against D21.

Do (2026-09-21): `plugins/pi/extensions/rtok.ts` — the bash rewrite still mutates `event.input` (pi) and now also returns `{ input: event.input }` (omp; pi ignores the field); `registerPiTools` returns early when `pi.pi` is an object — omp injects its SDK there (`getAgentDir`, `VERSION`), pi's `ExtensionAPI` has no such member. `plugins/pi/tests/rtok.test.ts` — `load()` takes extra API members; new cases: the rewrite is returned as `input`, and with `pi.pi` present no tool registers even when `setup.pi.tools` is true; the three guard-allow cases now assert the returned rewrite instead of `undefined`.

Check: `pi_plugin` green (it runs the Node test files).

Check result (2026-09-21): `rtok.test.ts` 23/23 and `load.test.ts` pass under `node --test`; `cargo nextest run --test pi_plugin` 5 passed; `cargo fmt --check` exit 0. Full workspace run: 1081 passed, 5 failed — all five (`agents::kilo::tests::*`, `readme_tables_match_support`, `agents_doc`, `agents_install setup_twice…`, `cli_trycmd`) come from another session's uncommitted `kilo` host (`src/agents/kilo/` untracked, `"kilo"` added to `HOSTS`), none touch `plugins/pi`. Not verified: a real model turn whose bash call runs through `rtok run` under omp — the only model key here has no credit (`credit_balance_exhausted`).

### T90. Antigravity plugin tree (`plugins/antigravity/`)

Creator request 2026-09-21: a host plugin for Google Antigravity CLI + desktop, like Claude's and Cursor's. Antigravity's plugin format is a directory: manifest `plugin.json` (only `name` is required, `^[a-zA-Z0-9-_]+$`), optional `mcp_config.json`, `hooks.json`, `skills/`, `agents/`, `rules/`. Global plugins live in `~/.gemini/config/plugins/` and are read by all three surfaces — Antigravity CLI (`agy`), Antigravity 2.0 and Antigravity IDE — so one tree is D21's "plugin and MCP as one unit" for CLI and desktop. Evidence: https://antigravity.google/docs/plugins/, https://antigravity.google/docs/mcp/ (fetched 2026-09-21).

Creator decisions 2026-09-21: the plugin is the only install path (no direct edit of `~/.gemini/config/mcp_config.json`), and it ships **no hooks** — per https://antigravity.google/docs/hooks/ `PreToolUse` answers only `decision` (`allow` / `deny` / `ask` / …) with a `reason` and cannot rewrite tool input, and `PostToolUse` answers `{}` and cannot add context, so neither the `rtok run` rewrite nor a context note is expressible. The bash path is the hub skill plus the MCP tools. A `deny`-with-reason redirect was left out (it costs a model turn per command and has no `Measurement` row).

Do (2026-09-21): `plugins/antigravity/plugin.json` (`name: "rtok"`, description), `plugins/antigravity/mcp_config.json` (`mcpServers.rtok` → `{command: "rtok", args: ["mcp"]}` directly — I-37: launcher scripts never run, the ketch hint lives in the README), `plugins/antigravity/README.md` (install by hand with `agy plugin install <path>` or by placing the folder at `~/.gemini/config/plugins/rtok`, files, why there are no hooks, `## Docs` with five links — plugins, MCP, hooks, skills, CLI install — each fetched on 2026-09-21). No `hooks.json` and no copy of the skill: T91 installs the hub skill. New `tests/antigravity_plugin.rs` (3 tests): the manifest name is `rtok` and matches the documented pattern, `mcp_config.json` is exactly the one `rtok` server, and the tree holds no `hooks.json`.

Check: `host_docs` and the new manifest test green; `just check`.

Check result (2026-09-21): `--test antigravity_plugin` 3 passed, `--test host_docs` 2 passed. The `just check` steps, run one by one: `fmt-check` exit 0, `lint` green, `cargo nextest run --workspace --no-fail-fast` 1083 passed / 4 skipped, `build-min` and `dup` green. An earlier `just check` in the same shared checkout stopped at `fmt-check` on another session's in-progress `src/hooks/types.rs` and once failed `cli_trycmd` while that session was re-blessing `tests/trycmd/`; neither repeated once those edits settled, and nothing under `src/` or `tests/trycmd/` names `antigravity`. Not verified live: `agy` and the Antigravity desktop apps are not installed on this machine, so the plugin has not been loaded by a real host — T91 carries that check.

### T80. `demon status` names the proxy endpoint (bind:port)

Creator 2026-09-21. `rtok demon status` said whether a service was running but never *where*: the proxy row carried no host/port, so answering "is the proxy up and on what address?" meant `rtok proxy --dry-run` or reading config by hand.

Do (2026-09-21): `demon::Row` gains `endpoint: Option<String>` — for `proxy`, the effective `[proxy] bind:port` (so `127.0.0.1:8790` by default), `None` for the stdio surfaces (`mcp`, `web`'s row stays `-`). The value is config, not state, so a stopped proxy still names the address it would listen on; the doc comment on the field records the caveat that it is read from the status process's config, which can differ from a supervisor started with `--config`. `table()` renders it as an `endpoint` column after `state`; `--json` and the web model page (`Model::demon` returns the same rows, D27) pick the field up through the existing `Serialize`.

Check: unit `the_proxy_row_names_its_configured_endpoint_stopped_or_not` (row + table carry the configured address while stopped; mcp stays `None`); integration `tests/demon.rs::status_names_the_proxy_endpoint_running_or_stopped` (custom `[proxy] port = 8123` shows stopped, in `--json` with mcp `null`, and — after `demon start proxy` — the same address while a `TcpStream` actually reaches it); trycmd `demon-status` / `demon-json` refreshed. `just check`.

Check result (2026-09-21): full `just check` green in the worktree — 1061 passed, 3 skipped, exit 0; demon units 9/9 (incl. the new one), `--test demon` 6/6, trycmd `cli` green over the refreshed `demon-status` / `demon-json` goldens.

### T81. Ship the WASM bundle with the release archive

Do (2026-09-21): T80 taught an installed `rtok web` to explain a missing bundle; this puts the bundle in the archive. `Cargo.toml`'s `[package.metadata.dist] include` gains `crates/rtok-webui/pkg/`, so every archive carries `pkg/` beside the binary exactly as it already carries `plugins/` and `skills/` — and `pkg/` beside `current_exe()` is the first candidate `pkg_dir` tries. The open question (which targets pay the ~4.2 MB) was answered by the tool, not by taste: `include` is package-local with no per-target form, so it is every archive or none. `.github/build-setup.yml` — the hook dist injects into `build-local-artifacts` — installs wasm-pack via `taiki-e/install-action` (Linux/macOS/Windows) and runs the build before `dist build`; `.github/workflows/release.yml` was regenerated with `just dist-generate` (9 added lines, nothing hand-edited). The build itself moved into `tools/webui-bundle.sh` so `just web` and CI cannot drift: `--require` turns every skip into a failure (CI), `--compress` writes the `.br`/`.gz` `rtok web` negotiates (`just web` only — the archive serves loopback and does not need them). The script also refuses a bundle over the T60.7 gate, which is what a silent loss of wasm-opt would produce: `wasm-pack` runs wasm-opt itself when binaryen is reachable (measured 4,392,425 B; the explicit `-Oz` on top gives 4,232,904 B), and without it the bundle is ~10.5 MB — not something to discover after a release.

Blocker found and fixed first (commit `cba11ac`, separate): `crates/rtok-webui` did not compile for wasm at all. `ui.on_expand_archive` passed a `slint::SharedString` into `serde_json::json!`, which has no `Serialize` for it. `just check` stayed green because the crate is excluded from the workspace and nothing compiled it — the same gap produced an earlier "fix webui WASM build" commit (`0731efd`). New `just webui-check` (`cargo check --manifest-path crates/rtok-webui/Cargo.toml --target wasm32-unknown-unknown`) now runs in `ci.yml` beside `just check`, so the next wasm-only break fails on the push that causes it.

Check: build the archive dist would publish, run the `rtok web` inside it, and `GET /pkg/rtok_webui.js` is 200 with the same bytes as the archive's copy. `just check` green.

Check result (2026-09-21): `dist build --artifacts=local --target aarch64-apple-darwin` produced `rtok-aarch64-apple-darwin.tar.xz` (8,942,672 B) whose root holds `rtok`, `plugins`, `skills`, `pkg`. With the repo's own `crates/rtok-webui/pkg` moved aside — so the source-tree fallback could not answer — the extracted binary served `/pkg/rtok_webui.js` 200 (101,376 B, byte-identical to the archive copy via `cmp`) and `/pkg/rtok_webui_bg.wasm` 200 (4,232,904 B). `just check` green: 1078 passed, 4 skipped. New `tests/release_bundle.rs` (6 tests) holds the packaging contract as files, not as a hope: the `include` entry, the executable-adjacent candidate in `src/web/mod.rs`, the `--require` build step, `release.yml` being in step with `build-setup.yml` (it is generated — a forgotten `just dist-generate` is otherwise invisible), and the gate number matching `tests/web_wasm.rs`. Mutation-checked: dropping the `include` entry and the `--require` flag failed exactly those two tests and nothing else. This task touched more files than the usual ≤3 — packaging spans the manifest, the build script, two workflows, the justfile and the docs.

### T111. Embed the WASM bundle in the binary

Creator 2026-09-21. A ketch install keeps only the binary, so T81's `pkg/` beside it never reaches the user and `rtok web` prints the T80 "bundle is not on this machine" line. The release build must carry the UI inside the executable.

Do (2026-09-21): `build.rs` sets `cfg(rtok_web_embed)` when `crates/rtok-webui/pkg/{rtok_webui.js,rtok_webui_bg.wasm}` exist (with `rerun-if-changed` on both, so a new `just web-bundle` re-embeds), and panics under `RTOK_WEB_EMBED=require` when they do not. `src/web/mod.rs` gains `Pkg { Dir, Embedded, Missing }` and `resolve_pkg`: a bundle on disk still wins (`RTOK_WEB_PKG`, beside the binary, `share/rtok/pkg`), then the `include_bytes!` copy served with `application/wasm` / `text/javascript`, then the T80 503. The source-tree fallback is dropped in embedded builds — it held the same bytes and would have hidden the embedded path from every test. `.github/build-setup.yml` exports `RTOK_WEB_EMBED=require` after `tools/webui-bundle.sh --require`; `release.yml` carries the same line (the step is copied verbatim by `just dist-generate`). The archive `include` of `pkg/` is gone — a second 4.2 MB copy beside a binary that already has it. New dev-dependency `tokio-tungstenite` 0.29 (creator-approved; already in the lock via axum `ws`) drives a real WebSocket in the new `tests/web_e2e.rs`.

Check: `tests/web_e2e.rs` spawns the real `rtok web` (temp `HOME`/`RTOK_HOME`, no `RTOK_WEB_PKG`): `/health` ok, `/` references `./pkg/rtok_webui.js`, `/pkg/rtok_webui.js` 200 JavaScript, `/pkg/rtok_webui_bg.wasm` 200 `application/wasm` byte-identical to the bundle; over `/ws` the first frame is a snapshot, an unknown `expand` and a non-plugin `set` answer `message` frames, and `plugins.cmd.enabled=false` lands in the next snapshot and in `config.toml`. `tests/web.rs::web_serves_the_embedded_bundle` covers `Pkg::Embedded` in-process (404 for other names). `tests/release_bundle.rs` now holds the embed guard in `build.rs`, both workflows and the absent `include`.

Check result (2026-09-21): with `crates/rtok-webui/pkg` moved aside, `RTOK_WEB_EMBED=require cargo check` fails naming the missing files and a plain `cargo check` succeeds. `just check` green: 1087 passed, 4 skipped.

### T80. `rtok web` from an installed binary 404s the whole UI

Do (2026-09-21): `src/web/mod.rs` resolved the Slint bundle as `env!("CARGO_MANIFEST_DIR")/crates/rtok-webui/pkg` — baked at compile time, so the v0.3.2 ketch binary looked for CI's `/Users/runner/work/rtok/rtok/crates/rtok-webui/pkg`. That directory exists on no user machine, `pkg.is_dir()` was false, `/pkg` was never mounted, and the dashboard answered `/` 200 with a blank canvas while `GET /pkg/rtok_webui.js` 404'd, saying nothing about why. `pkg_dir` now resolves at run time and returns an `Option`: `RTOK_WEB_PKG`, then `pkg/` beside the executable (where a release archive unpacks), then `share/rtok/pkg` beside and one level above `bin/`, then the source tree as the dev fallback. `app` splits into `app_with_pkg(state, Option<PathBuf>)` so both surfaces are testable without touching the process environment (same reason as T78's handed-in gate). With no bundle, `/pkg/{*path}` answers 503 with the paths tried and how to build one, and `serve` prints that same text once at startup — one string, two places.

Check: `tests/web.rs` gains two cases — a temp `pkg/` passed to `app_with_pkg` serves its file 200, and `None` answers `/pkg/rtok_webui.js` with 503 whose body names `RTOK_WEB_PKG` while `/health` stays 200. `just check` green.

Check result (2026-09-21): `--test web` 7 passed, 0 failed; `just check` green (1072 passed, 4 skipped). Measured before the change: installed `rtok web --port 3333` → `/` 200, `/pkg/rtok_webui.js` 404 0 B; `./target/debug/rtok web --port 3399` → the same file 200, 101 196 B. After: `target/debug/rtok` copied to a scratch directory with a marker `pkg/rtok_webui.js` beside it served that marker (200), so the executable-adjacent candidate wins over the source tree. The bundle still does not ship with a release — T81 covers that, and until it lands an installed `rtok web` explains itself instead of serving the UI.

### T77. One descriptor for the host-plugin offer

Do (2026-09-21): Cursor, OpenCode and pi each repeated the same four items around `rtok_agent_sdk::PluginLink` — a `PLUGIN_SRC_REL` const, a `plugin_dest(cfg)`, a private `link(cfg)` respelling every `PluginLink` field, and an `offer_plugin(cfg, remove)` that only forwarded — plus `link(cfg).ours()` in `installed()` (T75). Nothing but the four values differed. New `src/agents/plugin.rs`: `HostPlugin { src_rel, host, label, dest: fn(&Config) -> PathBuf }` with `path` / `linked` / `ours` / `offer`, one per thing a host asks of a link. Each host now declares one `static PLUGIN` and keeps only what is genuinely its own — Cursor's `offer_plugin` still wraps `PLUGIN.offer` for the D21 singleton rule (a linked plugin *is* the MCP, so `mcpServers.rtok` is cleared). `PluginLink` keeps owning backup, the `--yes` question and the remove rules (D28); this only stops spelling them three times. `src/` net **−78 / +57** lines.

Check: `just check` green, with the three host plugin integration tests (`cursor_plugin`, `opencode_plugin`, `pi_plugin`) unchanged — they are the behaviour contract, so an untouched pass is the proof the extraction is behaviour-free.

Check result (2026-09-21): `just check` green — `just test` 1070 passed, 4 skipped, and the three plugin test files were not edited. Two units added in `plugin.rs`: distinct `src_rel` per declared host, and `path` resolving from the handed config rather than the real home.

### T78. Host installers against real copies of the machine's own agent configs

Do (2026-09-21): Every agents test wrote a synthetic config — a couple of keys, all of them ours — so nothing exercised what install actually meets: a 25 KB `settings.json`, a `hooks.json` already holding other tools' hooks, an `mcp.json` with a dozen foreign servers. `tests/common/agents.rs` gained `real_config(rel)` (the invoking user's own file, `None` when absent **or** `CI` is set), `real_config_from(ci, home, rel)` — the gate with its inputs handed in, because `unsafe` is denied in this crate so a test cannot set `CI` — plus `seed_real` and `skip`. New `tests/agents_real_config.rs` copies each host's real config into a throwaway `HOME` (the original is never opened for writing), runs `agents install <host> --yes`, installs again, then `agents remove`, and at each stage holds every **foreign** entry — anything not naming rtok — to its seeded value: objects by key, arrays by containment (rtok appends, so foreign indices shift), falling back to trimmed-line containment for TOML. Local-only by construction: no such file, or a CI runner, and the test prints a skip line instead of failing.

Check: on this machine the tests run and pass against the real `.cursor/hooks.json`, `.cursor/mcp.json`, `.claude/settings.json`, `.codex/config.toml`, `.config/opencode/opencode.json`, `.kimi-code/config.toml`, `.zcode/cli/config.json`, `Library/Application Support/Code/User/settings.json`, `.codeium/windsurf/mcp_config.json`; `ci_hides_what_this_machine_really_has` proves both sides of the gate; `just check` green.

Check result (2026-09-21): `--test agents_real_config` 9 passed, 1 ignored; `just check` green (1070 passed, 4 skipped — the fourth skip is the new ignore below). The suite found a real bug on its first run: `agents install zed` aborts on a Zed-written `settings.json` (JSONC) with `trailing comma at line 44 column 3`, because `read_json` is strict `serde_json`. Not fixed here — out of this card's scope; filed as T79 with the reproduction kept as the `#[ignore]`d `zed_keeps_the_real_settings_json`. VS Code shares the risk in principle; this machine's file is strict JSON, so its test passes and proves nothing either way.

### T74. Make the two load-sensitive gate tests deterministic

Do (2026-09-21): The card's assumed mechanism ("key-injection → frame-assert waits with no internal deadline") does not exist — both tests are synchronous. The real mechanism, found by timing and `sample`:
- `space_toggles_the_selected_plugin_through_config_set` was **not hermetic**: `Config::load_from` expands `~` against the real `$HOME` (`expand` → `env_user_home`), so `stats.transcripts_dir` stayed `~/.claude/projects` and every `model::snapshot` in the test parsed the developer's real session JSONL — ≈30 s CPU per snapshot, ≈150 s for the test standalone, >180 s under suite load (the observed nextest kill). New shared helper `tui::app::tests::hermetic` points the doctor paths **and `stats.transcripts_dir`** at the temp home; used by `config()`, `cursor_on_plugin`, the toggle test, `fresh_store`, `logged`, and the unreadable-store test. All 36 tui tests: 0.18 s total (toggle alone was 150 s).
- `otel::hooks_stay_fast_with_an_unreachable_endpoint` now re-measures both interleaved configs to a deadline (60 s debug / 120 s release) instead of deciding on one sample set: a load blip lands on both sides of a retry, a real regression still fails every attempt.

The same leak sat in `tests/web.rs` (`serve`, `snapshot_error_when_store_path_is_a_directory`) — both now hermetic too; `ws_set_accepts_plugin_enabled` dropped from >120 s to 87 s (its remaining cost is the snapshot-TTL wait, green before and after).

Follow-up filed as I-87: the same transcripts parse makes production `rtok tui`/`rtok web` freeze ~36 s per 30 s doctor-TTL window on a heavy history (out of T74 scope; plan change proposed, not implemented).

Check: both tests green repeatedly; tui suite sub-second; `just check`.

Check result (2026-09-21): full `just check` green in the isolation worktree — 1060 passed, 3 skipped, EXIT=0; tui:: 36/36 in 0.183 s, otel latency test 1/1.

### T75. `agents uninstall` leaves the host marked installed (green check stuck)

Do (2026-09-21): Reproduced the class end-to-end (all 12 hosts install→uninstall in isolated HOMEs): the plain flow is clean; the stuck shape is the **plugin module**. `PluginLink::run(remove)` unlinks symlinks and wipes copies carrying `.rtok-owned`, but a directory a host *materialized* from our symlink (same bytes, no marker) was refused as foreign — while `installed()` counted *any* metadata at the dest, so the green check stayed on exactly as reported ("uninstall did not take effect on disk" + checkmark stuck). Fix, both sides of the card's Plan: (1) `PluginLink::ours()` — a dest is rtok's when it is a link/file, or a dir holding `OWNED_MARKER`, **or a byte-complete copy of our own tree** (`tree_copies`: every `src` file present with the same bytes, extras allowed); `run(remove)` wipes exactly that. (2) cursor/opencode/pi `installed()` now read `ours()` — the mark the UI renders can never outlive an uninstall: what remove takes back is what reads installed, and what it leaves foreign reads not installed.

Check: uninstall a previously installed host; `rtok agents info --json` marks off; files gone. Regression: `agent_remove::uninstall_clears_the_installed_marks_over_a_materialized_plugin_copy` (install --yes → replace the link with a marker-less copy → uninstall → 0 installed marks, copy gone). SDK units: `remove_wipes_a_materialized_copy_of_our_tree`, `a_directory_that_differs_by_a_byte_is_not_ours`.

Check result (2026-09-21): full `just check` green in the isolation worktree — 1060 passed, 3 skipped, EXIT=0; SDK 19/19, `--test agent_remove` 14/14, `agents::` lib 104/104; the 12-host isolated matrix shows marks on after install and 0 after uninstall everywhere.

### T76. Offer to restart the host after `agents install` / `uninstall`

Do: After successful `rtok agents install|uninstall <host>` config writes, ask whether to restart that host. Yes → stop then start; No → leave alone. Config `[setup].restart_prompt_timeout_seconds` defaults to `0` (wait forever); positive → silence = No. Interactive TTY shows a left in-place spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` ~80ms, ASCII `-\|/` fallback). Skip under `--dry-run` and non-TTY stdin.

Check: yes/no/timeout-silence→no / `0` no auto-no / spinner clear / dry-run skip; `just check`.

Check result (2026-09-21): `just check` green — 1054 passed, 3 skipped. Desktop apps (e.g. Cursor/Claude `.app`) get quit+reopen on macOS; CLI-only hosts stop binaries and tell the user to relaunch manually.

## T75 — `rtok mcp` / `rtok proxy` died at session start on a contended store

Creator 2026-09-20. Both surfaces run the retention purge at session start (`run_retention`, default `retain_calls_days = 30`), and the purge's deferred read-then-write transaction came back "database is locked" when another rtok process held the store's write lock — instantly (a deferred snapshot upgrade returns SQLITE_BUSY without running the busy handler) or after the steady 1 s. The `?` took the whole process down: an MCP client spawning `rtok mcp` saw the server exit before `initialize`, logged `Error: database is locked` / "Server disconnected", and its ~1 s-later retry succeeded once the winner committed. Observed 2026-09-19/20 in the client log (three incidents, deaths at 22–171 ms — too fast for the 1 s busy wait); the binary was v0.3.1, which already had every earlier mitigation (open retry loop, `busy_timeout = 1000`, the 30 s migration window), so the purge was the remaining unguarded startup write.

Done when: the purge takes the writer lock up front under the same maintenance contract as `migrate` (BEGIN EXCLUSIVE inside `PRAGMA busy_timeout = 30000`, restored to the hook's 1 s bound afterwards whatever happened inside), a final purge failure no longer kills `mcp`/`proxy` (one stderr line; WAL reads keep every tool serving; the next start retries), and regression tests hold a competing `BEGIN IMMEDIATE` writer on the store file at both the store and the binary level.

**Result (2026-09-20).** `purge_calls_older_than` runs `exclusive_transaction` under the 30 s window (`src/store/mod.rs`); `mcp::run` and `proxy::serve` log-and-continue on a retention error. Red→green: `store::tests::purge_waits_out_a_concurrent_writer` failed pre-fix in 39 ms with `database is locked` and passes post-fix (waits out a 1.2 s writer, purges the seeded old call); `tests/mcp.rs::mcp_serves_while_another_process_holds_the_store_writer` failed pre-fix with the binary's own `Error: database is locked` under a 2.5 s writer and passes post-fix — initialize + tools/list answered, exit 0. `diesel` added to dev-dependencies (same crate/features as the main dependency) so the e2e test can hold the competing writer. Also refreshed the two trycmd snapshots the v0.3.1 release commit (`f07088f`) left at `0.3.0` — `just check` was deterministically red on `main` before any of this task's edits.

**Check:** both regression tests above red pre-fix / green post-fix; full `just check` green in the isolated worktree `_worktrees/rtok-mcp-db-locked` (branch `fix/mcp-database-locked`, off `2f01e57`).

---

## T57.1 — Flag-aware `guard` read-only classes

From I-38 (promoted 2026-09-17). `guard::read_only` decided which Bash calls got a dedup key from a fixed stem list (`ls cat head tail grep rg find tree wc` plus `git status|log|diff|show|branch`). It ignored flags, redirections and pipes, so it keyed writers (`find -delete`, `cat a > b`, `ls | xargs rm`, `tail -f`) and never keyed repeats of `sed -n`, `jq`, `awk`, `git rev-parse`, `cargo metadata`.
Done when: (1) stem/flag counts over real transcripts in `research.md` with date and command; stems added only with a count; (2) keyed only if every `|` segment's first stem is read-only and no writer marker (`>`/`>>`, `| tee`, pipe into a non-read-only stem, `find -delete`/`-exec`, `sed -i`/`--in-place`, `tail -f`); parsing is first-word + marker scan, no shell grammar; (3) unit tests in `src/plugins/guard/mod.rs` including the false-deny Check `ls` → `find . -delete` → `ls` allowed; (4) `kind = guard` Measurements on the hook e2e fixture before and after.

**Result (2026-09-18).** Evidence: `research.md` §2 `guard` read-only stems. Added `sed` (2,417 Bash, 2,238 `-n`), `jq` (20), `awk` (359), `git rev-parse` (8), `cargo metadata` (9). Writer markers take the mutating path and clear `bash` keys. Hook e2e fixture Measurements `kind = guard`: before 9 rows / 52 `before_bytes`, after 8 / 32 — wrong writer denies gone, new keyed repeats present. Tests: `flag_aware_read_only_keys`, `find_delete_allows_the_next_ls`. One file, +113/−3.

---

## T72 — `agents info`, `_backup` folder, `uninstall`, faster `list`

Creator 2026-09-19. `rtok agents info <host>` prints the same blocks as `agents list` for that host (`--json` too). `agents list` probes hosts in parallel (`std::thread::scope`, no new crate). Install and uninstall copy configs into a sibling `_backup/` directory and skip when any file in that folder already has the same bytes, name ignored. `agents remove` is now `agents uninstall` (`remove` stays a clap alias). A stderr spinner (`render::loader`) runs for list/info/install/uninstall; indicatif stays silent off-TTY.

**Result (2026-09-19).** `visit_hosts` in `src/agents/mod.rs` walks hosts in parallel; `list` / `info` / `model::agents_listed` share it. CLI: `AgentCmd::Info`, `Uninstall` with visible alias `remove`. SDK `backup` writes `_backup/<name>.bak-<ts>` and skips when any regular file in that folder is byte-equal. `render::loader` on stderr for list/info/install/uninstall (silent off-TTY).

**Check:** `info` of one host has that host's block and not another's; unknown host refused; identical bytes in `_backup/` under any name skip a new copy; `cargo clippy -p rtok -p rtok-agent-sdk --all-targets -- -D warnings` green; agents unit tests, `agents_install`, `agent_remove`, `cli_trycmd`, `surface_parity`, `host_docs` pass. One unrelated flaky `store::tests::concurrent_opens_of_a_fresh_store_all_migrate` in full nextest.

---

## T69.6 — `rtok memory sync`: a managed block in `CLAUDE.md` / `AGENTS.md`

## T59.5 — Byte-stable `tools[]` description rewrite in the proxy

From I-45 (Portkey / LiteLLM "tool description compression + allowlist", 18–28 % claimed, unverified). Redundant on Claude Code with Tool Search deferral (`doctor` flags `mcp_tool_search_disabled`); a host without deferral pays every schema on every turn at cache-read price.

Done when:
1. Evidence: `doctor` already prices descriptions per server; a `stats` row shows description tokens × turns per session for a host without deferral, recorded in `research.md`. Below 3 % of session input, the card closes with the number.
2. Proxy option `proxy.tools_rewrite = { max_description_tokens = N, allow = [..], deny = [..] }`, off by default: descriptions truncated at a sentence boundary to N tokens (the tokenizer `measure` uses), tools outside `allow` or inside `deny` dropped from `tools[]`; the rewrite is deterministic so the cached prefix changes once per session, and `input_schema` is never touched.
3. `Measurement { plugin = "proxy", kind = "tools_rewrite" }` per request with before/after description bytes; wire tests for Anthropic and OpenAI Chat request shapes; a tool the model then calls that was dropped by `deny` is forwarded unchanged (the proxy never blocks a call).

**Result (2026-09-18).** Isolated worktree `.worktrees/T59.5` from `t61.2`. Evidence: `rtok doctor` MCP surface 8,951 description tokens across 11 servers; `mcp_tool_search likely disabled` (`ANTHROPIC_BASE_URL` set). Transcripts `~/.claude/projects/**/*.jsonl` mtime ≥ 30 d, unique `message.id` (same rule as `measure::jsonl`): 936 sessions, 40,402 API turns, session input 5.834 B → **6.2 %** of session input. Above the 3 % gate, so the rewrite shipped **off by default**.

`[proxy.tools_rewrite]` (`enabled = false`, `max_description_tokens = 60`, empty `allow` = keep all not in `deny`). Descriptions truncate at a sentence boundary with `tokens::estimate` / `Class::Prose`; `input_schema` / `parameters` are never written. A `deny`d name leaves `tools[]` but a later `tool_use` / `tool_calls` entry is forwarded. `Measurement { plugin = "proxy", kind = "tools_rewrite" }` records description bytes. Tests: unit (Anthropic + OpenAI Chat) and `tests/proxy.rs` httpmock; T61.2 skill-archive proxy test still passes.


## T61.2 — Archive skill bodies outside the live zone

From the graymatter gap review (`research.md` §14). graymatter's `context-sync` projects the highest-weight facts into a marker-fenced block in `CLAUDE.md` / `AGENTS.md` within an explicit token budget, detects hand edits inside the block, backs the file up and never writes outside the markers. Every host reads those files natively — including the hosts whose `support()` row has no SessionStart injection (`docs/agents.md`) — and the block sits in the cached prefix at the same price as a hook injection. Risk: on a host where hook recall is on, the same titles are paid twice (the T59.7 overlap class).
Done when:
1. `rtok memory sync [--file CLAUDE.md|AGENTS.md] [--budget N] [--dry-run] [--remove]` writes pinned notes first, then remaining live notes by id desc (T69.2 closed without ranking), as `id title` lines between `<!-- rtok:memory -->` / `<!-- /rtok:memory -->`, ≤ `[plugins.memory] sync_tokens` (default 300), byte-stable for an unchanged store (no timestamps); creates the block at the end of the file when absent; backs the file up through `rtok_agent_sdk::backup` (one helper, no copy); never changes a byte outside the markers; `--remove` deletes the block and nothing else.
2. Hand-edit guard: the sha256 of the last written block is kept in the store; a block whose bytes differ is refused with a message and exit 1 unless `--force`; `--dry-run` prints the unified diff.
3. Not automatic: no hook writes a file (fail-open rule). `doctor` (the T59.7 list) prints the overlap when a synced block exists and hook recall is on for the host; `sync` prints the same line.
4. `Vfs` tests: create, update, hand-edit refusal, `--remove`, outside bytes identical, budget trim; a trycmd golden; `docs/config.md` row; the memory page.

**Result.** `rtok memory sync` writes a managed marker block. Default `sync_tokens = 300`. SessionStart still only injects recall — it does not write files. T69.2 order is pinned first, then remaining live notes by id desc.

**Check:** `plugins::memory::sync` Vfs tests, doctor overlap, `config_coverage`, `surface_parity`, `cli_trycmd` (memory-sync --help). `just check` fmt-check/lint stay red on pre-existing HEAD rustfmt/clippy outside this task.

---

## T69.3 — Memory recall bench: planted, drifted, superseded facts

From the graymatter gap review (`research.md` §14). graymatter publishes a no-LLM benchmark (`go run ./benchmarks/token_count`, keyword embedder): tokens per session against full-history injection at 1 / 10 / 30 / 100 sessions, a fact planted 96 sessions ago retrieved 83 % of the time, superseded facts returned 0 % (its numbers, not re-measured). rtok's `memory` has no recall-quality number at all — `graph` has one (T8.8, 30 hand-labelled symbols) — and Gate P6 ("revert if recall is worse") has nothing to compare against. D3.
Done when:
1. `tests/memory_bench.rs` (`cargo test --test memory_bench -- --nocapture`, the `mode_bench` shape) builds an in-memory store from a seeded generator: N sessions (1, 10, 30, 100) × K notes of realistic length, 20 target facts planted at known session offsets, 5 of them revised later (T69.1); no network, no LLM.
2. Reported per configuration — FTS5 default; `half_life_days = 30` (T69.2); `embed.enabled` hybrid (P29): hit rate of the target in `mem_search` top-`search_limit` for a query built from the fact's own words; superseded facts returned (the test asserts 0 after T69.1); SessionStart recall bytes per session against the "full injection" baseline (every live body of the project) — rtok's own version of graymatter's table.
3. Numbers land in `research.md` §14 with the command and date and on the memory docs page; `README.md` / `docs/comparison.md` cite that row and never graymatter's. The gate for T69.2's default is written from this run.
4. The generator and the expected hit rates are checked in; a change that lowers the hit rate on any row fails the test.

**Result (2026-09-18).** Commits `b029b12`, `9af0c34`, `0cd5c27`. `tests/memory_bench.rs` seeded generator (N=1/10/30/100 × 6 filler notes, 20 planted facts, 5 `mem_revise`). FTS5 and P29 hybrid both 20/20; superseded returned 0. SessionStart recall 95–100 bytes vs 6 331 / 39 566 / 113 240 / 371 866 bytes full live-body injection. `half_life_days = 30` is N/A: T69.2 shipped no scorer. T69.2's default stays off (FTS5 already 20/20 at N=100). Numbers in `research.md` §14; cited from the memory page, `README.md`, `docs/comparison.md`. Never graymatter's 83 %.

**Check:** `cargo test --test memory_bench -- --nocapture` pass (1/1)

---

## T53.3 — Hook start without Security.framework

From I-32. On macOS the one binary links Security.framework and CoreFoundation for reqwest's platform verifier, costing about 1.3–1.5 ms of dyld time per hook spawn, as much as the hook's own work.
Done when the creator picks the trade-off (webpki roots with `use_preconfigured_tls` and dead-stripped dylibs, versus a second tiny hook binary), the choice is recorded as a decision, and the hook p95 before/after is measured and stored in `research.md`. Corporate CA support must be documented either way.

**Result (2026-09-18).** Decision D30: webpki + `use_preconfigured_tls`, one binary (second hook binary rejected). Implementation already in `d899760` (`src/tls.rs`, proxy/otel `.use_preconfigured_tls`, rustls 0.23.43, webpki-roots 1.0.9, rustls-pemfile 2.2.0). This branch recorded `otool -L` (Security.framework still linked before and after), hook p95, and `SSL_CERT_FILE` docs. Release `rtok` 25,124,800 → 25,562,032 bytes; dylib set unchanged. Sequential n=200 spawn-to-exit on this machine (1-min load 50): PreToolUse p95 79.71 → 80.82 ms, PostToolUse 66.47 → 92.60 ms — no dyld win while the frameworks stay linked. Corporate CAs: `docs/config.md` (TLS and corporate CAs). Commit `9be22b6`.

**Check:** `cargo test --lib tls` 3 passed; nextest `--test proxy --test otel` 40 passed, 1 skipped.

---

## T69.2 — Recall ranking: recency decay and use counts, off by default

From the graymatter gap review (`research.md` §14). graymatter ranks recall by vector + keyword + recency with a deterministic 30-day half-life and per-signal "receipts"; facts fade without access and are never hard-deleted. rtok: SessionStart recall is the newest `recall_titles` (5) ids of the project; `mem_search` is bare BM25 (`search_notes`) or RRF over BM25 + hash-embed when `embed.enabled` — a note used in every session for a month drops out of recall the moment five newer notes exist, and a stale note ranks as high as a fresh one.
Done when:
1. Evidence first: `rtok memory status` (T69.4) on this machine — notes per project and how many projects hold more than `recall_titles` live notes — recorded in `research.md` §14. If no project does, the order never matters and the card closes with the number.
2. A migration adds `notes.uses INTEGER NOT NULL DEFAULT 0` and `notes.last_used INTEGER NULL`; `mem_get` and every `mem_search` hit bump them; inclusion in a SessionStart recall does not (the hook path writes nothing per note, D13).
3. `[plugins.memory] half_life_days = 0` — 0 keeps today's id-desc order with byte-identical output; N > 0 scores `ln(1 + uses) × 0.5^(age_days / N)`, ties by id desc. Recall and search share one scoring function; search re-ranks the top `3 × limit` BM25 / RRF hits so FTS5 still does the retrieval. `NoteHit` gains `score` and `age_days` (graymatter's receipts), so the MCP result shows why a hit ranked. Decay ranks, never prunes (D4).
4. Tests: a fixture of 20 notes where a 60-day-old note used 10× outranks a fresh unused one only when `half_life_days > 0`; `half_life_days = 0` reproduces the T6.2 recall bytes exactly; scoring is deterministic under a frozen clock.
5. The default stays 0 until T69.3 shows a higher hit rate on the 100-session run without more recall bytes; the card records the numbers either way. `docs/config.md` row in the same commit (D12).

**Result (2026-09-18).** Installed `rtok 0.1.1` has no `memory status`. T69.4's `memory_note_aggs` query (`kind NOT LIKE 'checkpoint%'`) on `~/.rtok/rtok.db` returned **0 live notes** and **0 projects**. `recall_titles = 5`. No project holds more than five live notes, so ranking order never matters: no migration, no scorer, no `half_life_days` key. The store still has 25 `checkpoint` rows (project `rtok`, title `compact`); T69.4 excludes them. Evidence: `research.md` §14.1.

**Check:** docs-only close; no ranking code.

---

## T71.4 — Measure the per-skill listing overhead through the proxy

From `research.md` §10.6 (open question). The docs say "~100 tokens per skill"; the measured description here averages 194 chars ≈ 49 tokens, so the framing per listed skill (name, path, wrapper text) is unknown, and T61.3 / T63.1 total "description bytes ≈ tokens per request" without it.
Done when one Claude Code request captured through `rtok proxy` on this machine (a `call_io` row under the inline cap, or the request body dumped behind `[proxy] dump_request_dir` — off by default, one key with its `docs/config.md` row, added only if no existing row holds the body) is measured: bytes of the skills block, bytes per listed skill beyond its description, count of listed skills; recorded in `research.md` §10.6 with date and command; T61.3's total and T63.1's header use the measured per-skill constant (one named const in `doctor`, dated) instead of the docs figure; the card closes with the number alone if an existing capture already answers it.

**Result (2026-09-18).** Commit `45c4531`. `src/measure/skills_listing.rs` measures the skills block from a captured proxy request (`tests/fixtures/proxy/skills_listing_request.json`); per-skill framing bytes and count recorded in `research.md` §10.6; `tests/skill_listing.rs` pins the numbers.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T71.3 — rtok's own skill, installed with the host plugin

From I-52. Nothing tells a model that `expand <id>`, `read` modes, `mem_search` or `symbol` exist unless the human writes it into `CLAUDE.md`; a skill is the host-native way.
**Creator confirmed (2026-09-18): one hub skill per host, not one per surface.**
Done when:
1. `skills/rtok/SKILL.md` in the repo: description ≤ 120 chars, body ≤ 2 KB (a `tests/skill.rs` check on both), `disable-model-invocation` unset, body = when to use `expand`, `read` modes, `search` / `tree`, the memory and graph tools, each one line pointing at its `docs/` page — no second copy of the docs.
2. `rtok agents install <host>` copies it into the host's documented skill root for every host whose format is in `research.md` §10.1 (Claude Code `~/.claude/skills/rtok/`; others per that table), `remove` deletes only that directory, both idempotent and byte-stable; hosts without a skill format are untouched. Through `rtok-agent-sdk` (D28), one write cycle with the plugin offer.
3. `doctor`'s skill section (T61.3) lists it like any other skill; its description bytes on this machine go into `research.md` §10.2.
4. `Vfs` tests: install, re-install (no change), remove (foreign skills kept); `tests/host_docs.rs` covers the skill-root doc link per host; `docs/agents.md` host table re-blessed if `support()` changes.

**Result (2026-09-18).** Branch `t71.3`. One hub `skills/rtok/SKILL.md` (description 112 chars, body 780 B); `SkillCopy` in rtok-agent-sdk copies it during `rtok agents install <host>` for claude, cursor, codex, opencode, copilot and skips hosts without a §10.1 skill format. `Vfs` tests cover install / reinstall / remove; doctor lists the hub like any other user skill; description bytes recorded in `research.md` §10.2. `support()` unchanged — no `docs/agents.md` re-bless.

**Check:** `cargo test --lib agents::skill`, `skills_audit_lists_the_rtok`, `-p rtok-agent-sdk skill_`, `--test skill --test host_docs` pass. Full `just check` is blocked on pre-existing HEAD fmt/clippy in unrelated files.

---

## T70.7 — Cursor: `inject` has no path in, and the host table says it does

From `research.md` §15.3. `agents::reaches` marks a plugin reachable when the host supports the plugin's declared surface, so `inject` (Surface::Hook) is listed as reached on Cursor — but the installer writes only `beforeShellExecution` and `afterShellExecution`, and neither carries a session start or a user prompt, so the injection budget (D5) never runs there. Either the path or the claim is wrong, and today the generated table in `docs/agents.md` overstates what an install does.
Done when:
1. Step 1: verify against https://cursor.com/docs/agent/hooks which events carry session start and prompt submission and what their output schema accepts (the scan of 2026-09-18 reports `sessionStart` with `additional_context` and `beforeSubmitPrompt`); record the verified schema in the card.
2. If the events exist: `plugins/cursor/hooks/hooks.json` and `src/agents/cursor/mod.rs` register them onto `rtok hook SessionStart` / `rtok hook UserPromptSubmit --host cursor`, the injection is byte-stable and inside the existing budget, and a hook e2e asserts the same bytes Claude Code gets for the same store.
3. If they do not exist: `inject` stops being claimed on Cursor — the surface claim is narrowed where `reaches` computes it, not patched in the markdown — and the card records the doc line that says so.
4. Either way `RTOK_BLESS=1 mise exec -- cargo test --test agents_doc` re-blesses the host table, `src/agents/cursor/README.md` explains the outcome, and the same audit is run for every other host whose table claims a plugin no registered event can carry (one line per host in the card).

**Result (2026-09-18).** Commit `45c4531`. Cursor `sessionStart` and `beforeSubmitPrompt` hooks registered in `plugins/cursor/hooks/hooks.json`; `src/agents/cursor/mod.rs` dispatches to `rtok hook SessionStart` / `UserPromptSubmit`; host table re-blessed.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T69.5 — `remember:` prompts save a note; per-turn recall stays behind an A/B

From the graymatter gap review (`research.md` §14). graymatter's `UserPromptSubmit` hook does two things: a prompt starting with `remember: <text>` is saved instantly, and every turn injects the top-3 facts recalled for the prompt. rtok's `UserPromptSubmit` carries nothing from `memory`: saving a note costs a `mem_save` tool round trip (an API turn plus the output tokens of the call) even when the human typed the fact; per-turn recall would cost ~30–60 tokens per turn, unmeasured — and graymatter's own table compares against full-history injection, not against no injection.
Done when:
1. The SDK gains `Plugin::user_prompt(&self, ev: &UserPrompt, cx: &Ctx) -> Option<Injection>` with a no-op default (semver-minor; `inject` keeps the budget and the byte-stability test for modes), dispatched from `rtok hook UserPromptSubmit`.
2. `memory` handles it: a prompt whose first line matches `^remember:\s*(.+)` saves kind `user`, title = the first 80 chars of the rest, body = the rest (through the in-place `mem_save` path, so a repeat is a no-op), project from the hook cwd, and answers with one line `saved note <id>`; any other prompt → no output; the hook stays ≤ 10 ms and fails open (store error → nothing). The prompt is stored only as that note, never logged. Hook e2e: with and without the prefix; a second identical save returns the same id.
3. `[plugins.memory] prompt_recall = 0` (0 = off; N = titles per turn): when N > 0 the hook runs the T69.2 ranking on the prompt's words and offers N `id title` lines at priority 11 inside the D5 budget; a turn whose top-N equals the previous turn's (per-session sha in the store) emits nothing; `Measurement { plugin = "memory", kind = "prompt_recall" }`. Stays 0 by default until a `rtok bench` A/B (T53.1 shape) shows cost per passed task does not rise; the card records the dry result.
4. `docs/config.md` rows, the memory page, and `docs/agents.md` re-blessed if the host table changes.

**Result (2026-09-18).** Commit `45c4531`. `Plugin::user_prompt` dispatched from `UserPromptSubmit`; `memory` plugin saves `remember:` notes in-place and supports `prompt_recall` config (default 0).

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T69.4 — `rtok memory status` and the Memory page rows

From the graymatter gap review (`research.md` §14). graymatter's `status` and its 4-tab `tui` show facts stored, memory cost (KB), recall counts, health and weight distribution. rtok's Memory dashboard page (`web::model::config_fields`) shows two config keys and nothing from the store: how many notes exist, per project and kind, their bytes, pinned / retired, recalls in the window and the bytes they injected. T69.2 step 1 needs these numbers, and D27 says anything the store keeps is a page.
Done when:
1. SessionStart recall records `Measurement { plugin = "memory", kind = "recall" }` (before = bytes of the live bodies the injected titles stand for, after = injected bytes, `ref_id` = session); `mem_search` / `mem_get` counts come from the `calls` rows `rtok mcp` already writes (`mcp::record`).
2. `rtok memory status [--project <name>] [--since 30d] [--json]` prints: notes live / pinned / retired, per project, per kind, body bytes, oldest / newest ts, recalls in the window with injected vs stood-for bytes, `mem_search` / `mem_get` calls; `--json` is the same `web::model` type (T60.1 rule: one serde path).
3. The Memory page on `web` and `tui` renders those rows through one model accessor (D23; `tests/surface_parity.rs`); a TUI `TestBackend` snapshot and a `tests/web.rs` case on an in-memory store with three notes; a trycmd golden for `status` on the fixture store (T60.2 style).
4. Docs: memory page and the CLI table in `README.md`.

**Result (2026-09-18).** Commit `45c4531`. `rtok memory status` CLI with `--json`; `src/plugins/memory/status.rs`; Memory page rows on web/tui; `tests/memory_status.rs` and trycmd golden.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.10 — `[plugins.graph]` exclude, include and extension map

From the codegraph / graphify review. codegraph's `codegraph.json` has `exclude` (gitignore-style), `include` (force gitignored source back in), `deprioritize` and `extensions` (`.tpl → php`); graphify has the same for its walker. rtok's walker is `ignore::WalkBuilder` with `.gitignore` only and a fixed extension → grammar table, so a vendored tree cannot be dropped, a gitignored generated source cannot be indexed, and projects with custom extensions get no rows.
Done when `[plugins.graph]` gains `exclude = []`, `include = []` (both gitignore syntax, applied through `WalkBuilder` overrides — no hand-written matcher) and `extensions = {}` (`ext = "grammar"`, unknown grammar names rejected by `config validate`); the watcher applies the same three (`relevant()` / `absorb_event` share the matcher with the walker); `deprioritize` is not added (rtok ranks by reference count, T52.3); rows in `docs/config.md` and the config-show golden; a unit test on a `Vfs` tree with an excluded dir, an included gitignored file and a mapped extension; `rtok graph index` reports how many files each list changed.

**Result (2026-09-18).** Commit `45c4531`. `[plugins.graph]` exclude/include/extensions in config; `src/plugins/graph/walk.rs` shared matcher for walker and watcher; validate rejects unknown grammars.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.8 — Markdown outline shared by `outline`, `read map` and the skill digest

From the codegraph / graphify review. graphify indexes Markdown headings as nodes beside code. rtok's `outline` and `read` mode `map` return nothing for `.md`, and T62.1's execution plan writes a first heading outliner inside `guard/skill.rs` — a second one would be a duplicate the day `outline` gains it.
Done when `plugins::read::outline` has one Markdown mode (`#` headings with their level and first non-empty body line, fenced code blocks skipped, line numbers as for code), `read` mode `map` and `outline` on `.md` / `.mdx` use it, T62.1's digest calls the same function (moved there if it landed first), no `tree-sitter-md` dependency (headings are a line scan), test on a three-heading fixture with a heading inside a fence, and the `read` docs page lists `.md` under `map`.

**Result (2026-09-18).** Commit `45c4531`. Shared Markdown outline in `src/plugins/read/outline.rs`; `guard/skill.rs` digest reuses it; `.md`/`.mdx` supported in `map` and `outline`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.7 — Mark ambiguous references

From the codegraph / graphify review. graphify tags every edge `EXTRACTED` / `INFERRED` / `AMBIGUOUS`; codegraph marks heuristic bridges `provenance: heuristic`. rtok's tags backend resolves by name: `callers(new)` on a repo with twelve `new` definitions merges them all and the model cannot tell.
Done when `callers`, `impact` and T68.1 append ` ?` to a reference line whose name has more than one definition in the root (`symbol_defs` count > 1, one query per distinct name, cached per call), the answer's first line says `N names ambiguous (?): narrow with path or kind, or backend = "lsp"` when any is, the LSP backend never marks (its resolution is exact), unfiltered contract strings for names with one definition stay byte-identical, unit test with two `new` definitions and one `alpha`.

**Result (2026-09-18).** Commit `45c4531`. Ambiguous names marked with ` ?` on `callers`, `impact`, and `explore`; banner line when any name has multiple definitions.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.4 — `impact` with a target: call paths between two symbols

From the codegraph / graphify review. graphify `path A B` and codegraph's "call paths between them" answer "how does A reach B"; rtok's `impact` walks outward from one symbol and prints every reachable definition, so the model reads the whole fan-out to find one chain.
Done when `impact` takes optional `to` (MCP field, CLI `--to`) and prints only the chains from `name` that reach `to` within `depth`, one line per chain `a → b → c` in BFS order, `no path from a to b within depth N` when none, the same `impact_bfs` walk with its parent map kept (no second traversal); the LSP backend applies the same filter on its `callHierarchy` result; description still ≤ 60 tokens; unit test on the `impact` fixture (one chain found, one absent, depth too small).

**Result (2026-09-18).** Commit `45c4531`. `impact` gains optional `to` / `--to`; prints only chains reaching the target within depth via `symbol_paths`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.3 — Staleness line on every graph answer

From the codegraph / graphify review. codegraph prepends a banner naming files edited during the watcher's debounce window and tells the model to read those directly; graphify re-extracts on post-commit / post-checkout. rtok marks a file stale in `post_tool` and the watcher settles after `QUIET`, but a `symbol` call inside that window, or any call with `auto_index = false`, answers from old rows and says nothing.
Done when every graph tool answer with `auto_index = false`, or with the watcher on and a non-empty pending set, starts with `stale: N files pending (a.rs, b.rs, …)` (up to 5 names, sorted, byte-stable for the same pending set) followed by the answer; `Store::symbol_pending(root)` counts rows carrying the T8.3 stale mark; the watcher publishes its in-flight pending set through `Ctx` (one shared set behind a mutex, read-only from the tools); `rtok graph status` (CLI, `--json` per T60.1) prints root, rows, files, pending, watcher mode and last index time; a test edits a file with `auto_index = false`, sees the line, runs `graph index`, sees it gone. Hook p95 unchanged (the line is built on the MCP path only).

**Result (2026-09-18).** Commit `45c4531`. Staleness banner on graph answers; `rtok graph status` CLI; `src/plugins/graph/status.rs`; watcher pending set via `Ctx`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T68.2 — `symbol` lists what a definition calls

From the codegraph / graphify review. codegraph exposes `callees`; graphify's `explain` shows a node's outgoing edges. rtok stores the edge already (T8.5: every reference row carries `scope` = its enclosing definition) and shows only the incoming side (`callers`).
Done when `symbol` output ends each definition with one `calls: a, b, c (+N)` line — distinct referenced names whose `scope` is that definition, ordered by first line, capped at `body_lines / 2` names — from one `symbol_callees(root, name)` query on the existing rows (no schema change, no new tool: the surface budget is spent on T68.1); the LSP backend derives the same line from `documentSymbol` plus references inside the span or prints nothing; the unfiltered `symbol` contract strings are re-blessed once in the same commit; unit test on the `impact` fixture asserts the chain reads forward as `callers` reads it backward.

**Result (2026-09-18).** Commit `45c4531`. `symbol` prints `calls:` line per definition via `Store::symbol_callees`; contract strings re-blessed.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T67.2 — `expand --context N` around grep hits

From I-54 (`research.md` §12). After T67.1 the model needs two calls to see the lines around a hit (grep, then `--lines`); in rtok's metric every extra call is a turn that re-reads the whole prompt, so one call that returns hit ± N lines is cheaper than two smaller ones.
Done when `expand` takes `context` (CLI `--context N`, MCP `expand.context`, default 0 = today's output) and, with `grep`, prints each hit with N numbered lines before and after it, overlapping windows merged, windows separated by `--`, still under `expand.max_lines`; without `grep` the flag is ignored; per call like `--grep` (no config key; the `config_coverage` allow-list and the `docs/config.md` row name it); a test with two hits whose windows overlap and one at the file edge; the README example gains the one-call form.

**Result (2026-09-18).** Commit `13a6608`. `expand --context N` merges overlapping windows around grep hits; MCP `expand.context` field; README example updated.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T65.4 — Never cut a stack trace

From `research.md` §11. sqz's safe mode passes stack traces and secrets through whole. rtok keeps single lines matching `BUILTIN_KEEP` (`error`, `panic`, `traceback`) but the head/tail cut in `rules::apply` drops the frames under them, which is the part the model needs; secrets are deliberately not redacted (`ten_families_and_aws_key_unredacted`) and stay so.
Done when `rules::apply` detects a trace block — Python `Traceback (most recent call last):` to the next non-indented line, Rust `thread '…' panicked at` plus a following `stack backtrace:` block, JS `Error:` with `    at ` frames, Go `goroutine N [` frames, Java `Exception in thread` with `\tat` frames — and keeps the whole block in the output regardless of `head`/`tail`, only the block's own length counting against `max_lines`; a fixture per language shows the frames survive a 40-line cap; the trailer still names the archive id.

**Result (2026-09-18).** Commit `1d09962`. Trace-block detection in `rules::apply` keeps full Python/Rust/JS/Go/Java stack traces under head/tail caps; golden fixtures per language.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T64.2 — `cmd` dedupe across non-adjacent lines with normalised keys

From `research.md` §11. `rules::dedupe` folds consecutive identical lines to `line (×N)`; logs repeat the same line with a different timestamp, pid or request id and never fold, and a line that repeats after one other line never folds either.
Done when `dedupe = "normalized"` (the current behaviour stays `dedupe = true`) keys a line with timestamps, hex ids, pids and durations replaced by placeholders, folds every later match into the first occurrence as `line (×N, also lines k, l, …)` keeping the first verbatim, and a fixture of 3,000 log lines (nginx access log, a `cargo test` run with 200 identical warnings, `kubectl logs`) shows the bytes saved against `dedupe = true`; unit tests for the key normaliser (no false merge of two different error codes). Ordering of the kept lines is unchanged so the head/tail cut still works.

**Result (2026-09-18).** Commit `45c4531`. `dedupe = "normalized"` mode with timestamp/pid/id placeholders; non-adjacent duplicate folding with line references.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T60.9 — Web theme toggle

`app.slint` has a dark-mode icon (`crates/rtok-webui/ui/app.slint:288`, survey 2026-09-17) but no toggle and no `prefers-color-scheme` read; the UI is dark-only.
Done when the web UI follows `prefers-color-scheme` on load, the icon toggles it, the choice persists in `localStorage`, every colour comes from one palette struct (no literals in components), and the Slint e2e test flips the theme.

**Result (2026-09-18).** Commit `45c4531`. Theme toggle in `app.slint` with `prefers-color-scheme` on load, `localStorage` persistence, and e2e coverage in `crates/rtok-webui/tests/e2e.rs`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T60.7 — WASM bundle size gate

`crates/rtok-webui/pkg/rtok_webui_bg.wasm` is 10,560,601 bytes (`ls -l`, 2026-09-17), served uncompressed from `rtok web`; no release profile, `lto` or `wasm-opt` is set for the crate.
Done when the webui release profile sets `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `just web` runs `wasm-opt -Oz` when it is on PATH (fail open to the unoptimised file otherwise, with a line), `rtok web` serves the file with `Content-Encoding` negotiation for a pre-compressed `.wasm.br`/`.wasm.gz` when present, a test asserts the served size is under a number set from the measured result of this task, and the before/after bytes go into `research.md` with the date and command.

**Result (2026-09-18).** Commit `45c4531`. Release profile in `crates/rtok-webui/Cargo.toml`; `just web` runs `wasm-opt -Oz`; `tests/web_wasm.rs` gates at 4,500,000 B; measured 4,130,017 B in `research.md`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T60.6 — Error and connection states on `web` and `tui`

A store that fails to open renders an empty web page, and a dropped WebSocket leaves the last frame on screen with no hint (survey 2026-09-17, `src/web/mod.rs`, `crates/rtok-webui`); the TUI shows a doctor failure string but no store error line.
Done when the snapshot carries an `error: Option<String>` the model fills when the store or doctor fails, both surfaces render it as a banner instead of empty pages, the web client reconnects with capped backoff and shows "reconnecting" until the next frame, and `tests/web.rs` plus a TUI `TestBackend` test cover the unreadable-store case (a `Vfs`-style fixture: point `db_path` at a directory).

**Result (2026-09-18).** Commit `45c4531`. `error` field on web snapshot; error banner and WebSocket reconnect in `rtok-webui`; TUI store-error line; `tests/web.rs` unreadable-store case.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T59.8 — Token-sink ranking in `report`

From I-48 (caveman `learn`, context-budget plugin). `stats` has per-family and per-tool rows and `report` renders the D24 rules; what is missing is "which ten paths and commands cost the most, and which rtok switch would have shortened each".
Done when `report` gains one rule that prints the top-10 sinks by bytes over the session window (file path for Read/read, first stem for Bash/cmd, server/tool for MCP), each with the switch that applies (`read.default_mode = map`, a `[stem]` rule, `--wrap`, or "none: already shortened"), sourced from `Measurement` rows only, with a fixture test and a line on the report docs page.

**Result (2026-09-18).** Commit `45c4531`. Top-10 token-sink rule in `src/report/advice.rs`; fixture test; `docs/report.md` updated.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T59.3 — Batch the cold `graph` index in one transaction per N files

From I-30 (codebase-memory-mcp: Linux kernel in 3 min). Measured 2026-09-04: 3 000 files cold 27.2 s, warm 0.053 s; the cold path is paid once per repo, so it was parked.
Done when the cold index writes symbols and edges in one Diesel transaction per 200 files instead of per file, the T8.4 cold bench on the same fixture is re-run and recorded in `research.md` next to the old number, the warm path and the ≤ 10 ms hook stay untouched, and the change is reverted if the cold time does not drop by a third.

**Result (2026-09-18).** Commit `45c4531`. Cold graph index batches Diesel transactions per 200 files in `src/plugins/graph/index.rs`.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T59.1 — Per-stem interactive table for `skip_wrap`

From I-39. `skip_wrap` treats any `-i` / `--interactive` token as interactive, so `ffmpeg -i in.mp4`, `curl -i`, `ssh -i key` are never wrapped: their output is neither archived nor filtered, and for `ffmpeg` and `curl` that is most of the family's bytes. A wrong "non-interactive" verdict wraps a prompt-waiting command and hangs the tool call, so the table is per stem, not per flag.
Done when:
1. Evidence: count of unwrapped Bash calls by stem and bytes (`stats` over transcripts, the T57.1 path) in `research.md`; stems whose `-i` is a real REPL flag (`python`, `node`, `psql`, `sqlite3`, `irb`, `bash`, `sh`, `zsh`, `docker exec/run`, `kubectl exec`) stay interactive.
2. `skip_wrap` consults the stem first: `-i` means interactive only for the REPL stems above and `--interactive` anywhere; every other stem is wrapped. Table lives next to `never_wrap` and is overridable in config.
3. Unit tests: `ffmpeg -i x` wrapped, `ssh -i key host` wrapped, `python -i` skipped, `docker run -i` skipped, `--interactive` always skipped; hook e2e: `curl -i` produces a `Measurement`.

**Result (2026-09-18).** Commit `45c4531`. Per-stem interactive table in `src/plugins/cmd/hook.rs`; `-i` interactive only for REPL stems; unit and hook e2e tests.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T53.4 — `just otel-check` against real backends

From I-33. OTel export is gated by mock collectors; the Jaeger 2.11 and Grafana `otel-lgtm` recipes in `docs/otel.md` were checked by hand once.
Done when `just otel-check` starts both containers on shifted ports, flushes a copy of a fixture ledger, and asserts through their APIs: Jaeger has `execute_tool` spans for `service=rtok`, Tempo answers the trace id, Prometheus has `rtok_calls_total`; it skips with a clear message when Docker CLI / Colima is missing, and it stays out of `just check`.

**Result (2026-09-18).** Commit `45c4531`. `tools/otel-check.sh` and `just otel-check` recipe; `tests/otel.rs` smoke test; skips cleanly without Docker.

**Check:** cargo test relevant areas pass (700/702 lib; skill_listing, memory_status, web_wasm pass)

---

## T65.2 — `cmd` JSON output compaction

From `research.md` §11. sqz strips nulls and flattens arrays in JSON output; rtok cuts `gh … --json`, `aws`, `kubectl -o json` and `curl` bodies by line position, which keeps the opening of the document and loses the keys the model asked for. `toon` (off) is the wire-side encoder and does not run in the hook path.
Step 1 (gate): `stats` share of Bash result bytes whose body parses as JSON, 30 d, this machine, into `research.md` §11.
Done when output that parses as JSON is rewritten before the line cut: null / empty-string / empty-container fields dropped, arrays beyond `json_items` (default 20) elements shown as `… +K more`, object keys kept, strings longer than `json_string` (default 200) cut with their length, one line per top-level key; lossless via the archived raw body and the trailer; a fixture per source (`gh pr list --json`, `aws ec2 describe-instances`, `kubectl get pods -o json`) records the bytes against the default rule; a body that does not parse is untouched.

**Result (2026-09-18).** Isolated worktree on `t65.2` from `t64.1`. Gate: 4 / 200 non-empty `cmd` measurements, 5 776 / 2 792 960 B = **0.21 %** JSON (`~/.rtok/rtok.db`, 30 d, this machine). Rewrite still shipped. `rules::apply` compacts a body that parses as a JSON object or array after grouping and before the head/tail cut: null / empty-string / empty-container fields dropped, arrays beyond `json_items` (default 20) as `… +K more`, strings longer than `json_string` (default 200) cut with their length, one line per top-level key. Unparseable bodies untouched. Table formatters stand down when the body is JSON. Raw archive + trailer unchanged.

Fixtures vs the default rule (raw / line-cut / compact B, est tokens saved vs raw = bytes/4): `gh pr list --json` 10275 / 908 / 4530 (1436); `aws ec2 describe-instances` 19516 / 519 / 5890 (3406); `kubectl get pods -o json` 26271 / 353 / 8297 (4493). Compact is larger than the line cut and keeps the keys the cut drops.

Deviation: `src/plugins/cmd/rules.rs` is 221 insertions (over the 200 LOC guide) because parse/strict/default wiring and the compact helpers live in one file.

Check: `cargo test --lib -- plugins::cmd::` 64 passed, including JSON compact, unparseable untouched, goldens, and kubectl table still `formatter`.

---

## T50.3 — Extra `read` modes

From I-07. `read` has full, lines, map and signatures. The measured Read tail (38–68 K char files) may still be served whole when only imports or code without comments are needed.
Done when a measurement on those files shows which extra mode (imports-only, comments-stripped, or none) saves tokens without losing the answer; each added mode goes through tree-sitter where a grammar exists, falls back to `full`, keeps the read cap and dedup, and has a test per language. If no mode wins, the card closes with the numbers.

**Result (2026-09-18):** On 11 Rust files in this repo's 38–68 K char class (534 894 B), comments-stripped saves **18.9 %** (100 897 B, ~25 K est. tokens) and keeps function/type bodies → MCP `read` `mode=stripped` via existing tree-sitter grammars (Rust, TS, JS, Python, Dart, C, Go); unknown language or parse fail → `full`; read cap, T65.1 content-hash and T58.1 delta unchanged; `Measurement { kind = "stripped" }` when it shrinks; one test per language. imports-only is 0.1–2.1 % of each file and drops those bodies → not added. `app.slint` (38 064 B, no grammar) stays `full`. Numbers in `research.md` §2.

## T65.1 — Content-hash dedup of tool output within a session

From `research.md` §11 (sqz, 2026-09-18). sqz's flagship: content seen before in the session comes back as a 13-token `§ref:HASH§` instead of the text. rtok's `guard` dedups by input key (`guard::cache_key`: same tool, same normalised input), so `cat a` followed by `head -1000 a`, or the same `cargo test` failure printed twice, is paid twice.
Step 1 (gate): `stats` gains a `repeat` column — share of tool_result bytes whose SHA-256 (`sha2` is already a dependency, T13.3) equals an earlier result in the same session — measured over 30 d on this machine into `research.md` §11. Proceeds only above 1 % of result bytes; otherwise the card leaves for `ideas.md` with the number.
Done when `cmd::run` and the `read` plugin hash the raw output before archiving, a hit in the same session returns `[rtok <id> · identical to a result N turns ago · expand: rtok expand <id>]` instead of the body (`Measurement { kind = "dedup" }`, before = body bytes), a miss archives as today, the lookup is one indexed query on the archive table (≤ 10 ms, fail open), and a test replays two different commands with identical output.

**Result (2026-09-18, `rtok stats --since 30d`, 924 sessions):** 6,649 later tool_results whose SHA-256 equalled an earlier result in the same session; 1.83 MB of 95.25 MB result bytes (**1.9 %**) — above the 1 % gate. Landed: `RepeatRow` in `stats`; `cmd::run` and MCP `read` hash raw output before `put_archive`; a same-session `archive.id` hit returns `[rtok <id> · identical to a result N turns ago · expand: rtok expand <id>]` (`Measurement { kind = "dedup" }`, before = body bytes); miss archives as today; lookup is one PK query (`id` = sha256 and `session`); fail open on error or when the pointer would be longer than the body; a test replays `printf` and `sh -c printf` with identical output.

## T58.1 — `read` delta since last read

From the competitive gap review (`research.md` §9.3, §9.4 item 2; idea I-41; precedent: lean-ctx `diff` read mode, token-optimizer-mcp delta reads). Read is 15 % of tool-result tokens on the measured workload and the top single results are Reads. The sha256 dedup already answers an unchanged re-read with one line; a re-read of a file that changed since (typically after an Edit) still returns the whole file. The previous read's archive id is already stored, so a unified diff against it is the lossless short form.
Done when:
1. Evidence first: over real transcripts (`measure::stats::collect`) count Read calls of a path already read in the same session with an Edit/Write to that path in between, and their bytes; record in `research.md` §2 with date and command. Below 3 % of Read bytes → close the card with the number and no code.
2. MCP `read` (and the PreToolUse advice for native Read) answers such a re-read with a unified diff against the archived previous content plus that archive id; full content when the diff is not below `read.delta_max_ratio` (default 0.6) of the file or the previous archive is gone. Lossless: `expand <id>` of the new result returns the full file.
3. `Measurement` rows `plugin = read`, `kind = delta`, before = full bytes, after = diff bytes. Vfs unit tests: unchanged → existing "unchanged since" line; small change → hunks; large change → full; missing archive → full; CRLF preserved.
4. Byte-stable for the same file state; `read.delta = true` by default (safe because of the full fallback), documented in the read plugin's docs page with the measured row from step 1.
5. Parity with lean-ctx: its `diff` mode is opt-in per call and its unchanged re-read costs ~13 tokens (own README). rtok's delta is automatic (no mode to remember) and also reachable as `mode = "diff"` for the edit → verify flow; the unchanged-re-read line is measured on the same fixture and stays ≤ 13 tokens or the card says why.

**Result (2026-09-18, `rtok stats --since 90d`, 959 sessions):** 593 native Read calls of a path already read in-session with Edit/Write/MultiEdit in between; 1.79 MB of 24.61 MB Read result bytes (**7.3 %**) — above the 3 % gate. Landed: `ReadDeltaRow` in `stats`; MCP `read` returns a unified diff (`similar` via `render::unified_diff`) against the archived previous raw file plus `previous <id>` / `expand <id>` of the full file; full fallback when the diff is not below `read.delta_max_ratio` (default 0.6) or the archive is gone; `read.delta = true` by default; PreToolUse advice on a cached large file after Edit points at `mode=diff`; Vfs tests cover unchanged / small / large / missing archive / CRLF; unchanged re-read ≤ 13 estimated tokens on the fixture.

## T52.2 — More grammars and compressed index payloads

From I-16. Tags cover Rust, TS, JS, Python, Dart, C and Go. Java, Kotlin, Swift, C#, Ruby and PHP repos get no `symbol`/`outline`, and large indexes store plain text.
Done when each added grammar is an optional feature (dependency reasons in the commit, creator approval for new crates) with a fixture test, and index payload compression is added only if a large repo's `rtok.db` size is measured before and after.

**Result (2026-09-18).** All six languages added as optional `lang-*` features on the `read` bundle (tree-sitter stays 0.25). Crates: `tree-sitter-java` 0.23.5, `tree-sitter-kotlin-ng` 1.1.0 (fwcd `tree-sitter-kotlin` 0.3.8 needs tree-sitter <0.23 — skipped), `tree-sitter-swift` 0.7.3 (its `LOCALS_QUERY` uses `@local.definition.import`, rejected by tree-sitter-tags 0.25 — tags only), `tree-sitter-c-sharp` 0.23.5 (`TAGS_QUERY` is `cfg(with_tags_query)`, so the tags string lives in rtok), `tree-sitter-ruby` 0.23.1, `tree-sitter-php` 0.24.2. `golden_per_language` covers each; extractor `INDEX_VERSION` 3. T8.8 `graph_truth` labelled_symbols_are_found: definition recall 1.000, reference recall 0.305 (floor 0.30). This-repo debug index 172 files / 35 231 rows, `rtok.db` 14 077 952 bytes; gzip of concatenated TEXT columns 197 682 bytes is a one-stream dictionary win, not a per-row one — live payload compression skipped (`research.md`). Workspace `rust.md` + `toolchain.md` updated with the crates.

## T52.3 — Ranked repo map at SessionStart

From I-28 (aider repo map). The most-referenced definitions could orient the model at session start.
Done when a P7-style A/B shows the map lowers cost per passed task; the map is ranked by reference count from `symbols`, fits a share of the D5 budget alongside `memory`, is byte-stable across turns, and is off by default until that A/B passes.

**Result (2026-09-18).** Live A/B was not run (`bench` shells to `claude -p`; no API spend). Default stays off: `plugins.graph.map_tokens = 0` (nonzero is the D5-budget share next to `memory.recall_tokens`). `Store::symbol_top_refs` ranks names by ref count with one def site (`ORDER BY refs DESC, name ASC`); import rows are not refs. `Graph::session_start` offers priority-1 `repo map` lines trimmed to the cap and does not index on the hook path (empty index → no injection). This-repo debug index (`RTOK_HOME=$(mktemp -d) rtok graph index <worktree>`): 172 files, 35 196 rows, 2 494 named defs; untrimmed map 28 894 prose tokens; `map_tokens = 200` keeps 23 lines / 195 tokens. Check: `top_refs_rank_by_count_then_name`, `top_refs_picks_first_def_site`, `repo_map_off_by_default_and_empty_index`, `repo_map_ranked_byte_stable_and_trimmed`, `graph_session_start_map_off_by_default_and_on_when_capped`.

## T68.6 — Import edges in the index

From the codegraph / graphify review. Both tools store `imports` edges (codegraph resolves them to source files; graphify's `module_source`); rtok's rows are definitions and reference sites only, so a file that imports a module without calling a uniquely named symbol has no edge, and T68.5 cannot reach it. T52.5 already appends rtok's own tags queries to the grammar's, so this is query data plus one row kind.
Done when the extra queries capture `use` / `import` / `require` / `from … import` for Rust, TS/JS, Python, Go and Dart as rows of kind `import` whose `name` is the last path segment, `scope` empty, `is_def = false`; `symbol_imports(root, path)` lists a file's imports and `symbol_importers(root, module)` the files importing a module; `outline` prints an `imports:` line first; `impact_bfs` follows an import row to the file's definitions at cost 1 (one extra step in the same query, argument `follow_imports` default true); T8.8 recall on the 30-symbol set unchanged (imports never count as references); index time on this repo before / after in `research.md` with the command; no migration (kind is a string) — the extractor fingerprint bump re-indexes.

**Result (2026-09-18).** Extra tags queries (appended like T52.5) emit `kind = import` rows: Rust `use`, JS/TS `import`/`require`, Python `import` / `from … import`, Go `import`, Dart `import`. `name` is the last path segment; `scope` empty; `is_def = false`. `symbol_imports` / `symbol_importers` list them; `outline` (read map) prints `imports:` first. `symbol_refs` / `callers` skip imports. `symbol_impact` and `impact_bfs` (`follow_imports` default true) take one extra hop from an import to that file's definitions, so T68.5 `affected` reaches import-only tests. Extractor fingerprint includes the new queries (`INDEX_VERSION` 2). This-repo release index: 172 files, 33 312 → 35 017 rows, 0.270 s → 0.325 s (`RTOK_HOME=$(mktemp -d) rtok graph index <repo>`). T8.8 reference recall 0.305 (floor 0.30).

## T68.5 — `affected`: which tests a change touches

From the codegraph / graphify review. codegraph `affected` traces a diff to the test files it
reaches so the agent runs those instead of the suite; rtok had `impact(name)` and
`is_test_path`, and no path from "these files changed" to "run these tests", so `cargo test` /
`pytest` output — the largest Bash family in `research.md` §2 — was paid for the whole suite.
Done when `rtok graph affected [--since <ref> | --staged]` (CLI, `--json`) takes changed files
from `git diff --name-only` (no libgit — `cmd` already shells out to git), their definitions
from `symbol_defs`, `impact_bfs` to `depth` (default 3), and prints the reachable definitions
whose file passes `is_test_path` as `test file ← via symbol` grouped by file, with the command
to run them per language (`cargo test <name>`, `pytest path::name`, `go test -run`, `vitest
path`); MCP `impact` accepts `path` alone (no `name`) with the same semantics; an empty result
says `no indexed test reaches the change; run the suite`; `Measurement { kind = "affected" }`
is written only when a transcript or T68.9 shows the subset actually ran (before = the suite's
last measured bytes, after = the subset's), never on the print alone; test on a fixture repo
with two tests, one reaching the change.

**Result (2026-09-18).** `affected_from_paths` indexes the root, collects definition names in
each changed file (`outline::tags` then `symbol_defs` to confirm), walks `impact_bfs` to depth
3, and keeps `(path, scope)` hits whose path passes `is_test_path`. CLI
`rtok graph affected [--since <ref> | --staged] [--json]` shells out to
`git -C <root> diff --name-only --relative -z` (`--cached` when `--staged`); a git failure
fail-opens to the empty message. MCP `impact` with `path` and no `name` uses the same walk
on that file (D21: one tool). Print is `file ← via symbol` plus the language command; JSON is
`{"tests":[{"file","symbol","command"}]}`. No `cap` / Measurement on print. Empty:
`no indexed test reaches the change; run the suite`. Import edges (T68.6) are not followed.

## T68.9 — With / without bench for the graph tools

From the codegraph / graphify review. codegraph's number is the only measured one in the pair: median of 4 runs, 7 repos, Claude Opus 4.8 answering architecture questions with and without the graph — tool calls, wall time, tokens, cost — and it also reports the cost (80 % more retrieval context resident at session end). rtok's `docs/comparison.md` §5 still says no end-to-end win is demonstrated, and Gate P8b's task-set clause was never closable in code.
Done when `rtok bench --suite graph` runs N fixed questions (≥ 10, three repos including this one, in `bench/graph.toml`) through the existing `claude -p` harness twice — rtok MCP on, rtok MCP off (native Read / Grep only) — and reports per question and in total: tool calls, tokens in / out / cache-read, wall time, cost via `stats --price`, resident context at the last turn, pass / fail against an expected-answer regex; `--dry-run` prints the schedule without spend; the live run needs the creator's go (API spend) and its result goes into `research.md` and `docs/comparison.md` §4 / §5 with the date and command; the vendor's 88 % / 62 % numbers are quoted there only next to rtok's own.

Check: `rtok bench --suite graph --runs 1 --dry-run` prints 24 lines `{id} {repo} {mcp|native} {n}` covering 12 questions × 3 repos (this tree, `bench/repos/mini-rs`, `bench/repos/mini-py`) × two arms. Offline table headers are `id repo arm tools in out cache wall_ms cost resident pass` plus `TOTAL` rows; cost uses `stats --price` (`row_cost`) when live. Unit tests `graph_dry_run_lists_each_question_on_both_arms` and `graph_offline_table_names_the_metrics`; trycmd `tests/trycmd/bench-graph-dry-run.toml`.

**Live API clause remains open.** `RTOK_BENCH_LIVE` was not set; no `claude -p` spend. Dated live numbers are not in `research.md` / `docs/comparison.md`; those files record the suite and the dry-run command only.

## T63.1 — Skills page on `tui` and `web`

Asked 2026-09-18. Nothing on the operator surfaces shows what the skills cost: which of the 66 listed skills (`research.md` §10.2, this machine) were ever invoked, which never, how many bytes each body is, and how much of the input a session carried as skill bodies. `rtok stats` gains the numbers in T61.1 and `doctor` the audit in T61.3; this task renders both on the same page.
Done when `web::model::pages()` gains `("skills", "skills")` and the TUI gets the same page (D23: one `model` accessor, two renderings, `tests/surface_parity.rs` asserts the page exists on both): one row per skill the host lists — name, source (user / project / plugin), description chars, body bytes, invocations in the window, bytes resident (T61.1's column), last invoked — sorted by resident bytes, never-invoked rows marked; a header line with totals (skills listed, description bytes ≈ tokens per request, resident bytes in the window, share of input tokens); TUI `↑/↓` + `n` toggling never-invoked-only, web the same as a checkbox; empty state when the store has no skill rows yet ("run T61.1's `rtok stats` first" is not acceptable — the listing half from T61.3 renders even with zero invocations). Gated on T61.1 and T61.3 landing; tests: a `TestBackend` snapshot with three skills (one never invoked) and a Slint e2e case for the filter.

**Result (2026-09-18).** `model::skills_from` joins T61.3 `SkillsAudit` listing to T61.1 `stats::SkillRow` resident/count (no UI crate file walk). Snapshot `skills` rides `pages()` `("skills", "skills")`. Header uses desc bytes/4 (`research.md` §10.2 ≈ 49 tok vs docs "~100"). Empty listing is "no skills listed"; zero invocations still show the host list. TUI `n` / web checkbox filter never-invoked. Tests: `skills_from_joins_listing_and_resident_without_stats_rows`, `skills_tab_lists_three_rows_and_n_hides_invoked`, `skills_never_only_checkbox_hides_invoked`, `skills_page_exists_on_both_surfaces`.

## T60.4 — Archive `expand` on `tui` and `web`

Lossless by default means every trailer id is retrievable, but only `rtok expand <id>` retrieves it; the Calls detail on both surfaces prints `ref_id` as text (survey 2026-09-17).
Done when a Calls row with an archive id opens the payload in a scrollable pane — `e` on the TUI, a button on the web — through `expand::fetch` with `--lines`/`--grep` parity (a `/` filter on the TUI, a filter box on the web); the web path is one inbound WebSocket request `{"expand": id}` answered with the payload, capped by `[expand] max_lines` like the CLI; fetching a live-zone pointer freezes it exactly as the CLI does (same function, no second path); tests: TUI `TestBackend` on a fixture store, `tests/web.rs` request/response, and `surface_parity` lists the page on both.

**Result (2026-09-18).** Accessor `model::expand_payload` calls `expand::fetch` then `render_lines` (`[expand] max_lines`). Snapshot `ref_ids` maps call id → archive id. TUI `e` opens a scrollable pane; `/` filters via `filter_lines`. Web inbound `{"expand": id}` answers `{type: expand, text}` without touching the T60.5 `set` allow-list; the Calls page has an expand button and filter box. Tests: `expand_payload_caps_greps_and_freezes_like_cli`, `e_opens_the_archive_pane_and_slash_filters_it`, `ws_expand_returns_payload_and_unknown_id`, `expand_payload_exists_on_both_surfaces`.

## T60.3 — Per-session drill-down on `tui` and `web`

`SessionTotals` carries `project`, `api`, `started_at`, `last_activity`, `ended_at` (survey 2026-09-17, `src/web/model.rs`) and neither surface shows them; the Sessions page is a list on both, so "what did this session cost and which calls made it" needs the CLI.
Done when Enter on a Sessions row (TUI) and a click (web) open a detail pane with those fields, the API row, and the session's calls filtered from the same snapshot; both surfaces read the same `model` accessor (D23: one model, two renderings), `tests/surface_parity.rs` asserts the detail exists on both, and a TUI `TestBackend` test plus a Slint e2e case cover the selection.

**Result (2026-09-18).** Accessor `model::session_detail(snapshot, id)` returns `(&SessionTotals, Vec<&CallRow>)` from the same snapshot. TUI Enter toggles a pane with project, api, started/last/ended, the API usage row, and those calls; web click selects the same fields. Tests: `session_detail_filters_snapshot_calls_by_id`, `enter_opens_the_session_detail_pane` (TestBackend), `session_click_opens_detail` (Slint e2e), `session_detail_exists_on_both_surfaces`.

## T60.5 — Plugin toggle on the web Plugins page

The TUI Plugins tab toggles `plugins.<id>.enabled` through `config set`; the web page renders the same rows read-only and `src/web/mod.rs` has no inbound WebSocket message at all (survey 2026-09-17) — a D23 defect.
Done when the web Plugins page has the same toggle, sent as one inbound WebSocket message `{"set": {"key": "plugins.<id>.enabled", "value": bool}}` handled by the same `config set` function the TUI and CLI use (keys limited to that allow-list; anything else is refused with a message frame), the next snapshot reflects it, `tests/web.rs` covers accept and refuse, and the Slint e2e test clicks the toggle.

**Result (2026-09-18).** Commits `e66e2e5` (inbound `/ws` `set` through `validate::set`) and `633fbd6` (Slint toggle + WASM send + e2e click). Allow-list is `plugins.<id>.enabled` where `id` is a catalogue plugin from `Registry::manifests` (D23: no second list); anything else, a non-bool `value`, or a `config set` error is a `{"type":"message","text":...}` frame. The next snapshot reloads Config so the Plugins rows match the file. `tests/web.rs` `ws_set_accepts_plugin_enabled` / `ws_set_refuses_other_keys` green; `plugin_toggle_click_sends_the_set` green under `SLINT_EMIT_DEBUG_INFO=1`.

## T60.2 — trycmd goldens for every subcommand

Survey 2026-09-17: trycmd (`tests/cli_trycmd.rs`, `tests/trycmd/*.toml`) covers `help`, `version`, `config-show`, `completions-bash`, `bench-dry-run`, `stats-price` — 6 of 22 commands. The other 16 have behaviour tests but no byte-level snapshot of what the binary prints, so a wording, column or ordering change on `doctor`, `info`, `plugins`, `expand`, `report` and the rest lands unnoticed (T59.4 changed the `mcp` help line and only the top-level `help` golden caught it).
Done when every subcommand has at least one trycmd case of its real output, hermetic the way `stats-price.toml` is (`inherit = false`, `RTOK_HOME` under `target/tmp/`, `--config tests/trycmd/input/<case>.toml`, fixture store or empty dirs), plus a `--help` case for every subcommand and nested subcommand (`agents`, `config`, `demon`, `logs`, `otel`, `memory`, `graph`). `web` and `tui` get `--help` only. Timestamps, ids, versions and absolute paths use trycmd `[..]` / `[EXE]`. One `tests/trycmd/README.md` line per case; the README command table is checked against the trycmd case list.

Execution plan: (1) one `tests/trycmd/help-subcommands.trycmd` with `--help` for every subcommand and nested verb, plus `*.trycmd` in `cli_trycmd.rs` and a `tests/trycmd/README.md` index; (2) hermetic reading goldens (`inherit = false`, `[env.add]` HOME/RTOK_HOME under `target/tmp/`) for stats table/`--json`, `info --json`, doctor/plugins/agents/demon/otel/logs tables, `config init|path|get|validate|set`, `expand --lines/--grep`, completions zsh/fish/powershell, `man`, `report --format md`, `proxy --dry-run` — skip T60.1 `--json` duplicates; (3) stdin cases for `hook`, `mcp tools/list`, `filter --cmd`, `run -- echo`; (4) README command table vs trycmd case list, bless, close.

**Result (2026-09-18).** Every visible clap command has a `--help` golden in `tests/trycmd/help-subcommands.trycmd`; `web`/`tui` stay help-only. Reading commands gained hermetic trycmd cases for stats table/`--json`, `info --json`, doctor/plugins tables, `config init|path|get|validate|set`, completions zsh/fish/powershell, `man`, agents list/sessions, demon/otel/logs tables, `report --format md`, `proxy --dry-run`, plus stdin cases for `hook SessionStart`, `mcp tools/list`, `filter --cmd`, `run -- /bin/echo`, and `expand --lines --grep`. T60.1 `--json` goldens were not duplicated. `tests/cli_trycmd.rs` walks clap and the README command table against the trycmd case list.

## T60.1 — `--json` on every reading command


Survey 2026-09-17 (`src/cli.rs`): 22 user-facing commands, `--json` only on `stats`, `info` and `config show`. `doctor`, `plugins`, `agents list`, `agents sessions`, `logs print`, `demon status` and `otel status` print tables only, so a script or another agent has to scrape text, and the web/TUI model already carries the same rows (D27).
Done when every reading command that prints a table accepts `--json` and emits the `web::model` type that page renders (`DoctorPage`, `PluginPage` list, `SessionTotals`, log lines, demon/otel status) through one `serde` path — no second struct, no hand-built JSON; each command has a trycmd golden on the fixture store next to `stats-price`; `docs/config.md` mapping table lists the flag once; `tests/surface_parity.rs` gains the check that a reading command without `--json` fails the gate.

Execution plan: (1) `--json` on the table-printing readers; serialize `doctor::Report`, `PluginPage`, `SessionTotals`, log lines, `demon::Row`, plus model helpers for agents-list / otel-status — no parallel DTOs. (2) hermetic trycmd goldens like `stats-price`. (3) `docs/config.md` lists `--json` once; `surface_parity` fails a reader without the flag.

**Result (2026-09-18).** `doctor`, `plugins`, `agents list`, `agents sessions`, `logs`, `demon status` and `otel status` accept `--json` and serialize the existing `web::model` / store types (`doctor::Report`, `PluginPage`, `AgentListRow`, `SessionTotals`, log lines, `demon::Row`, `OtelStatus`) through `serde` — no parallel DTOs. trycmd goldens sit next to `stats-price`; `docs/config.md` lists `--json` once; `tests/surface_parity.rs` fails a reading command without the flag. `graph dead` still prints text only (no model page).

## T70.2 — pi `context` hook: the `archive` live zone without a proxy

From the T70 series (pi without MCP). The proxy's live zone — old large `tool_result` payloads swapped for `expand <id>` pointers outside `[plugins.archive] keep_turns` — needs `ANTHROPIC_BASE_URL`; pi has no base-URL setting, so its sessions carry every aged result whole forever. pi's `context` event fires before every LLM call with the full message array (a deep copy) and accepts `{ messages }` back — the one hook that can rewrite what the model sees without a wire hop.

Done when the extension's `context` handler sends the array through `rtok archive rewrite --stdin` (the same live-zone function the proxy filter calls over `ToolResultRef`/`BlobRef` — no second implementation), the rewrite is idempotent (the handler re-runs before every call: decisions persist in the store, nothing-eligible echoes the input bytes back so pi keeps the same array object), pointer strings land in pi's text-block `content[].text` shape, every pointer serves a `Measurement` row (D3), fail open on a missing `rtok` or unparseable output, and the host table / trycmd goldens are re-blessed for the new `archive` command.

**Result.** Implemented as `src/plugins/archive/pi.rs` (`tool_results` over `role: "toolResult"` + camelCase `toolCallId`, turns counted from the end like the proxy's; `live_blobs` for non-result payloads behind the same gate) and the `pi.on("context")` handler in `plugins/pi/extensions/rtok.ts`; CLI `rtok archive rewrite --stdin` (cli.rs, one carrier entry point). Verified against pi 0.85.1 extensions docs and a real `~/.pi/agent/sessions/` file. Landed through the #116 repair PR and the `4701646` follow-up (re-wrap into pi text blocks, `keep_turns = 1` turn semantics, `surface_parity` exemption, blessed `help.stdout`/`completions-bash.stdout` and the `docs/agents.md` pi row — archive now in the reached set).

**Check (2026-09-21, this closure).** `--test pi_plugin` 7/7 (includes `plugins/pi/tests/rtok.test.ts` via node + binary-level link/unlink), `--test archive_rewrite` (shrink only outside keep turns, expand recovers, replay stable), `--lib plugins::archive` 21/21, `--test host_docs` / `agents_doc` / `filter` / `surface_parity` 14/14, full `just check` green in the isolation worktree `apps/rtok-wt-t702` (branch `t70.2`). The todo.md row is dropped with this entry; no code changed in the closure commit.

---

## T70.1 — pi extension shortens every tool result, not only bash

From `research.md` §15.3. D2's constraint is that a PostToolUse hook can only add context, so on Claude Code every tool except `Bash` (rewritten to `rtok run` in PreToolUse) enters context whole; on a host with no proxy there is no second chance. pi's `tool_result` event is documented to return replacement `content` for **any** tool, and `plugins/pi/extensions/rtok.ts` uses it for bash only. Read is 15 % of tool-result tokens and its largest single results are 9.5–17 K tokens each (§2), so the tools worth adding are pi's file and search tools.
Done when:
1. Step 1 (decides the task): verify against pi's current docs (`## Docs` links in `plugins/pi/README.md`, re-checked as `tests/host_docs.rs` requires) and one real pi session that a `tool_result` handler's returned `content` replaces what the model sees for a non-bash built-in tool, and record pi's tool names in the card. If only bash may be replaced, close the task with that finding and no code.
2. The extension routes the result of pi's read / grep / find / list tools through `rtok filter --stdin --cmd "<tool> <path-or-pattern>"`, keeping the existing bash path unchanged and reusing the one `rtok()` helper already in the file — no second spawn path (D21: one call path per capability). Every shortened result carries the `expand <id>` trailer (D4).
3. Fail open exactly as today: a missing `rtok`, a spawn error, or empty stdout returns the original content; the ketch hint is printed once.
4. `plugins/pi/tests/rtok.test.ts` covers a large read result (shortened, trailer present), a small one (byte-identical passthrough) and a spawn failure (original returned); `Measurement { plugin = "cmd", kind = "rule" }` rows appear per tool family.
5. `plugins/pi/README.md` and `src/agents/pi/README.md` list the new call path and the reached plugins; `RTOK_BLESS=1 mise exec -- cargo test --test agents_doc` re-blesses the host table in `docs/agents.md` if the reached set changes.

**Result (2026-09-18).** Docs (pi 0.85.1 `https://pi.dev/docs/latest/extensions`): `tool_result` **Can modify result** for any tool; handlers return `{ content }` patches. Built-in names: `read`, `bash`, `powershell`, `edit`, `write`, `grep`, `find`, `ls`. The card's "list" is pi's `ls`. Real session: pi 0.85.1 `createAgentSession` with an inline `tool_result` handler — replacement `content` is what `afterToolCall` (the model) sees for `read`/`grep`/`find`/`ls`. Not bash-only → implemented.

The extension keeps bash as `rtok filter --stdin` and routes those four tools through the same `rtok()` helper as `filter --stdin --cmd "<tool> <path-or-pattern>"`. Missing `rtok` / empty stdout fail open; the ketch hint prints once. `rtok filter` archives, prints the `expand <id>` trailer, and records `Measurement { plugin = "cmd" }` per family (`read`/`grep` → `kind = "rule"`; `find`/`ls` → `kind = "formatter"`). Host table reached set unchanged (measure, cmd, archive); `agents_doc` needed no bless.

Check: `plugins/pi/tests/rtok.test.ts` 11/11; `cargo test --lib cmd::filter` 3/3; `--test pi_plugin` / `--test host_docs` / `--test agents_doc` / `--test filter` green; `clippy -D warnings` on `--lib` clean. Isolated worktree `.worktrees/T70.1` on `t70.1` (`ec34dbe`, `42952a6`, `cd9bb14`).

---

## T62.3 — OpenCode plugin shortens skill bodies in `tool.execute.after`

From `research.md` §10.8. `plugins/opencode/rtok.ts` already replaces bash output through `rtok filter` in `tool.execute.after`; if OpenCode delivers a skill body through a tool call, the same hook sees it.
Step 1 (decides the task): verify against OpenCode's current docs and one real session log (`~/.local/share/opencode/opencode.db`, `part` rows) which tool carries a skill body and whether `tool.execute.after` receives its `output`; record the finding in the card. If skills are injected outside the tool path, close the task with that finding and no code.
Done when (if step 1 passes) the plugin routes that tool's output through `rtok filter --cmd "skill <name>"` with a `[skill]` rule in `rules/default.toml` (head 30 / tail 5, keep headings), the cut is lossless — `filter` archives the raw body and prints the `expand <id>` trailer, adding an `--archive` flag to `filter` if it has none today (check first; one code path with `run`) — `rtok.test.ts` covers a 3,000-line body and a small one, `Measurement { plugin = "cmd", kind = "skill" }`, and `plugins/opencode/README.md` documents it with the verified docs link (`tests/host_docs.rs`).

**Finding (2026-09-18).** On the tool path -- implement. Docs: native `skill` tool, `skill({ name })`, body returned in the conversation (https://opencode.ai/docs/skills/, https://opencode.ai/docs/tools/). `tool.execute.after` already mutates `output.output` for every tool (bash path; apply_patch docs name the same hook). Session `~/.local/share/opencode/opencode.db`: 7 `part` rows `type=tool` `tool=skill` `state.status=completed`, `state.input={"name":"..."}`, `state.output` the body (`<skill_content name="nx-workspace">`, 7628 bytes). `rtok filter` on t70.3 has no `--archive`; add it and share emit with `run`.

**Result (2026-09-18).** On `t62.3`, stacked on `t70.3`. Step 1 passed: skills are the native `skill` tool. `rtok filter --archive` shares `run::emit_filtered` (archive raw body, expand trailer, Measurement). `[skill]` in `rules/default.toml` is head 30 / tail 5, keep `# ` headings. OpenCode `tool.execute.after` routes `skill` through `rtok filter --cmd "skill <name>" --archive`. Guard (T70.5) and compaction (T70.6) unchanged. Tests: 3000-line + small body in `rtok.test.ts`; `Measurement { plugin = "cmd", kind = "skill" }`.

## T70.3 — pi tools without MCP: `read`, `search`, `graph`, `memory` through `pi.registerTool`

From `research.md` §15.3. `src/agents/pi/README.md` records "Not reachable: read, archive, proxy, inject, guard, memory, graph, toon, compress" because pi's philosophy is no MCP. `pi.registerTool` is documented as pi's own tool registration, which is not MCP, so the MCP-surface plugins have a path in on pi after all. The cost is description tokens in every pi request, which is the thing D15 holds `graph` and `memory` to (4 tools / 94 tokens, 3 memory tools).
Done when:
1. Step 1 (decides the task): verify `pi.registerTool`'s signature and result shape against pi's current docs and one real session; confirm a registered tool's description rides the request the way an MCP tool's does, and measure the byte cost of the set. If registration is not available to an extension, close with the finding.
2. One call path per capability (D21): the extension's registered tools are thin callers of the same `rtok mcp` tool implementations through a CLI shim (`rtok mcp --call <tool> --json <args>` or the existing subcommands), never a second implementation of `read` / `search` / `symbol` / `mem_search`.
3. Which tools: the measured-value set only — `read`, `search`, `tree`, `symbol`, `callers`, `expand`, `mem_search`, `mem_get` — with the total description budget at or under what `rtok doctor` prices for the same tools on an MCP host, recorded in the card. A tool that does not fit the budget is not registered.
4. Off by default until step 1 and step 3 numbers are in: `[setup.pi] tools = false` (D12: config key + `docs/config.md` row in the same commit).
5. Tests: `plugins/pi/tests/rtok.test.ts` registers against a fake `rtok` and asserts one call path per tool and fail-open on a missing binary; `src/agents/pi/README.md` module table and the reached set updated, host table re-blessed.



**Result (2026-09-18).** On `t70.3`, stacked on `t70.5`. Step 1: pi 0.85.1 docs (`pi.registerTool` at https://pi.dev/docs/latest/extensions) plus one SDK session. Registration is available to extensions at load. A tool registered with a plain JSON-schema `parameters` object appears in `session.getAllTools()` with its description, the same list built-in `read`/`bash` ride. Measured set (estimator prose 4.2, same as `rtok doctor`): read 17, search 12, tree 12, symbol 30, callers 27, expand 22, mem_search 11, mem_get 7 = **138 tokens**. All eight fit; none dropped.

`[setup.pi] tools = false` (off by default). One call path: `rtok mcp --call <tool> --json <args>` → `mcp::invoke` (no `rtok read` / second search). The extension registers on `session_start` when the config key is `true`. Missing binary: do not register; execute still fails open with the ketch hint. Host table: pi now lists read, memory, graph; `toon (off)` appears because toon declares MCP, but those tools are not registered.

## T70.5 — `guard` on pi and OpenCode through the plugin

From `research.md` §15.3. `guard` denies a repeated identical read or command within N turns, and it answers on `PreToolUse` — so it is unreachable on pi, OpenCode and Codex, which have no hook events. pi documents `tool_call` returning a block with a reason, and OpenCode documents `tool.execute.before`, which is the same position.
Done when:
1. Step 1: verify both APIs (block shape and whether the reason reaches the model) against their current docs and one real session each; a host where the block has no reason string is closed with the finding, because a silent deny violates fail-open expectations.
2. Each plugin calls one new CLI path — `rtok guard check --tool <name> --json <input>` printing the same allow/deny verdict the hook path produces from `plugins::guard` — with no second key-building or dedup implementation.
3. Fail open everywhere: missing `rtok`, non-zero exit, unparsable output, or any spawn error allows the call. T57.1's false-deny concern carries over: a wrong "read-only" verdict must not deny a call whose output changed, so the same tests run against this path.
4. Tests: `plugins/pi/tests/rtok.test.ts` and `plugins/opencode/rtok.test.ts` each cover allow, deny-with-reason and fail-open; `Measurement { plugin = "guard", kind = "deny" }` rows; both READMEs and the host table updated.

**Result (2026-09-18).** On `t70.5`, stacked on `t70.6` plus cherry-pick `1e58c43` (T57.1 flag-aware keys). One CLI: `rtok guard check --tool --json --session` calls `Guard::pre_tool` (canonical tool names, `filePath` → `file_path`). Cache seed/clear is existing `rtok hook PostToolUse`. Plugins fail open unless `allow === false` and `reason` is a non-empty string.

pi (docs 2026-09-18: https://pi.dev/docs/latest/extensions): `tool_call` → `{ block: true, reason?: string }`. Installed `@earendil-works/pi-agent-core` 0.85.1 `applyBeforeToolDecision` writes the reason as `isError` tool-result text the model reads. A missing reason is empty text — we do not ship that. OpenCode (https://opencode.ai/docs/plugins/): `tool.execute.before` `throw new Error(reason)` becomes the tool-error the model summarises (opencode#6862, #27900). Denial measurements keep existing `kind = "guard"` (same row the hook path writes), not a second `deny` kind. T57.1 `sed -n` keyed / `find -delete` then `ls` allowed on this CLI path (`tests/guard_check.rs`). Host table: pi and OpenCode now list `guard`.

## T70.6 — Compaction on pi and OpenCode through the plugin

From `research.md` §15.3; the plugin-side half of T58.2, which registers host **hook** events and therefore cannot reach pi or OpenCode. Both document a compaction event that owns the summary — pi's may supply it or cancel, OpenCode's may replace the prompt — which is stronger than Claude Code's checkpoint note (T2.5), where rtok writes a note and hopes the summary keeps it.
Done when:
1. Step 1: verify both events against current docs and one real session; record what each accepts back.
2. Each plugin calls `rtok hook PreCompact --host <host>` (or the CLI equivalent) so the existing `checkpoint::save` runs unchanged — the checkpoint content, its budget and its archive ids (T58.2 step 2) are not re-implemented in TypeScript.
3. Where the host accepts a summary, the plugin returns the rendered checkpoint **appended to** the host's own summary, never replacing it: rtok's checkpoint is prompts, paths, errors and ids, not a conversation summary, and replacing the summary would lose what the host knows.
4. Restore: the next call injects the checkpoint the way `inject::session_start` does on `source = "compact"`, inside the same budget (D5).
5. Tests per plugin for a compaction with and without rtok present (fail open), a Rust test that the injected bytes equal Claude Code's for the same store, and both READMEs updated with verified links; cross-reference T58.2 so the two cards do not both claim the host list.

**Result (2026-09-18).** Commits on `t70.6` (not merged), stacked on `t58.2`. T58.2 owns Claude/Cursor/Codex/Copilot hook registration; this card owns only the pi and OpenCode plugins. `checkpoint::save` and compact restore are unchanged in TypeScript — plugins shell `rtok hook PreCompact --host <host>` and `PostCompact` / `SessionStart source=compact`.

pi (docs 2026-09-18: https://pi.dev/docs/latest/compaction, https://pi.dev/docs/latest/extensions): `session_before_compact` returns `{ cancel: true }` or `{ compaction: { summary, … } }` which **replaces** the host summarizer. No append field. Real sessions on this machine (`~/.pi/agent/sessions`, jsonl version 3) have no `type: compaction` rows. Closed the summary-return half: the extension does not return `compaction.summary`. Save still runs; restore is the next `context` call.

OpenCode (docs 2026-09-18: https://opencode.ai/docs/plugins/): `experimental.session.compacting` `output.context.push` appends to the default prompt; `output.prompt` replaces it. Real `opencode.db` messages have `mode=compaction`, `agent=compaction`, `summary=true`. Plugin appends the budgeted checkpoint to `context` and never sets `prompt`. Restore: next `experimental.chat.system.transform` injects PostCompact `additionalContext`. Missing rtok fails open on both hosts. `docs/agents.md` not re-blessed (reached set unchanged; inject still has no host hook path).

## T59.6 — `handoff` MCP tool for sub-agents

From I-46 (lean-ctx `ctx_handoff` / `ctx_agent`). Agent tool results were 23 K of 2.83 M tokens on the measured workload (§2), so this ships only with a number.
Done when:
1. Evidence: `stats` splits Agent/Task tool inputs and results per session; the card records the share, and closes with the number if sub-agents are below 5 % of tokens.
2. `handoff(budget_tokens)` returns one budgeted digest: the session's memory notes (titles first), archive ids of live tool results with tool and bytes (T58.2 field), touched paths, and the last N user prompts (`checkpoint::extract` reused, not copied); deterministic order; the digest itself is archived and carries an `expand <id>`.
3. Description ≤ 40 tokens; Vfs unit test on a fixture store; docs next to the memory tools.
Execution plan (Cursor / grok 4.6): evidence first in `src/measure/stats.rs` — `Report` gains an `agents` row that splits `Agent` and `Task` tool-input bytes vs result bytes and counts sessions that used either; `to_table` prints the two shares against all tool-result tokens and all tool-input bytes (the T58.3 denominator). Unit test on a two-session fixture (one Agent, one Task). Run `rtok stats --since 30d` on the author's transcripts; if Agent+Task result tokens are < 5 % of tool-result tokens, close with the number and do not add a `handoff` MCP tool.

**Result (2026-09-18, `rtok stats --since 30d`, 939 sessions).** 42 sessions used `Agent` (449 calls); `Task` 0. Agent in 1,042,386 B / out 632,586 B (158,299 est. tokens) = **0.7 % of tool-result tokens** (JSON 0.662 % of 23,905,777) and 2.7 % of tool-input bytes. Under the 5 % gate → `handoff` MCP tool not built. Landed: `AgentRow` in `src/measure/stats.rs` (`agents` in `--json`, one `agent` line in the table), unit test on an Agent + Task + Bash-only fixture; row in `research.md` §2. I-46 keeps the number.

## T71.2 — Session handoff: SessionEnd checkpoint, injected at the next SessionStart

From I-56 (engram `mem_context`, `research.md` §13; MemPalace Stop-hook checkpoint). T2.5 writes a checkpoint only at `PreCompact`, so a session that ends without compacting leaves nothing: on this machine ≥ 80 % of Claude sessions over 20 KB since 2026-09-14 ended with no note (96 sessions touched, 18 checkpoints — a rough mtime count, `ideas.md` I-56). `SessionEnd` is already registered and dispatched (`src/agents/claude/mod.rs`, unhandled), so the save is one call site. The injection half costs up to `checkpoint_tokens` on every startup, which is why it stays off until measured.
Done when:
1. Evidence: `stats` counts sessions with and without a checkpoint note (next to the T58.2 compaction count) and the number replaces the rough one in `research.md` §13 with date and command.
2. `rtok hook SessionEnd` runs the existing `checkpoint::save` (same extractor and render as `PreCompact`, no second implementation) under kind `session:<session-id>` with the project from the hook cwd; hook ≤ 10 ms, fail open; nothing is injected by this half.
3. `[plugins.memory] startup_recall = false` (D12 row in the same commit): when `true`, `SessionStart` with `source = "startup"` offers the newest `session:*` note of the project at the checkpoint priority inside `checkpoint_tokens`, rendered by the same function as the compact restore, byte-stable for an unchanged store; `Measurement { plugin = "memory", kind = "handoff" }`.
4. Hook e2e: end → note exists; start with the key off → bytes identical to today; on → the restore lines present and within budget; a second start → the same bytes.
5. Stays off by default until a P7-style A/B (T53.1 shape) shows cost per passed task does not rise; the dry result is recorded on the card. Hosts other than Claude Code that register `SessionEnd` get it through the same dispatcher (`docs/agents.md` re-blessed if the reached set changes).

**Result (2026-09-18).** Commits `2623c9a` `c192fae` `a68750a` `b3b3ce6` `04358f9` `56aa4ef` on `t71.2` (not merged). `rtok stats --since 30d`: 939 transcript sessions, **0 with** a `checkpoint:<id>` / `session:<id>` note, **939 without**; 25 legacy unscoped `kind=checkpoint` rows in the store are not joinable to a stem. Header is `sessions N  compact N  checkpoint N  no_checkpoint N`. `SessionEnd` calls `checkpoint::save_session` (same extract/render as PreCompact) under `session:<id>` with project from cwd; nothing injected. `[plugins.memory] startup_recall = false` (D12 in `docs/config.md`); when true, `SessionStart` `source=startup` offers the newest project `session:*` note via `render_offer`, priority 9, `checkpoint_tokens`, `Measurement { plugin = "memory", kind = "handoff" }`. Hook e2e `session_end_note_and_startup_recall`. Dry A/B: `rtok bench --dry-run` prints the default a/b schedule only (no startup_recall variant); live cost-per-passed-task A/B not run (needs `RTOK_BENCH_LIVE` + approval). Default stays false. Copilot `sessionEnd` already maps to the same dispatcher; `docs/agents.md` unchanged.

## T58.2 — Compaction checkpoint on every host, with archive ids

From the competitive gap review (`research.md` §9.2, §9.4 item 3; idea I-42). What exists (T2.5): on Claude Code `agents install` registers `PreCompact` and `PostCompact`; `checkpoint::save` stores the last 20 prompts, touched paths and 8 error lines as a memory note, and `inject::session_start` re-emits it (priority 9) plus the modes when `source == "compact"`. Two gaps remain. (a) No other host registers its compaction event — Codex (`PreCompact`/`PostCompact`), Cursor (`preCompact`), Gemini CLI (compression hook), Copilot CLI (auto-compact at 80 %) are listed in `research.md` §9.2 as of 2026-09-17, ZCode has none — so on those hosts the modes and the checkpoint vanish after the summary. (b) The checkpoint carries no archive ids, so `expand <id>` of a tool result that the summary dropped needs the id from a transcript the model no longer sees. Neither rtk, headroom nor caveman handle compaction at all (§9.3), so closing (a) and (b) is "better", not parity.
Done when:
1. Evidence: compactions per session counted from transcripts by `rtok stats` (a `compact` count next to the session rows) and recorded in `research.md`; the current checkpoint's injected bytes on the T2.5 fixture recorded as the baseline.
2. `Checkpoint` gains `ids: Vec<String>`: the archive ids of this session's tool results that are still in the live window (from the store, not the transcript), newest first, capped so the rendered note stays under the existing checkpoint budget (`offer_fits_checkpoint_tokens` extended); rendered as `id <archive-id> <tool> <bytes>` lines. Unit test: a fixture with three archived results yields three `id` lines and the restore injection contains them.
3. Per host, the compaction events verified against the current hooks doc (links in `src/agents/<host>/README.md` `## Docs`) and registered by `agents install` where they exist (Codex, Cursor, Gemini if a host, Copilot): the pre-event maps to `pre_compact`, the post-event to `session_start` with `source = "compact"`; hosts without the event are untouched. `tests/agents_doc.rs` regenerated with `RTOK_BLESS=1`. One commit per host if the 3-file limit needs it.
4. Hook e2e per new host: pre-event → note exists; post-event → injection bytes equal Claude Code's for the same store; fail open, ≤ 10 ms.

**Result (2026-09-18).** Commits `6333958` `2d08e2a` `954a612` `ac50d96` `ac97b9c` `76e1d3c` on `t58.2` (not merged). `rtok stats` prints `sessions N  compact N` by counting transcript `subtype=compact_boundary` (30d: 923 sessions, 271 compacts, 75 sessions with at least one). T2.5 fixture checkpoint body is 144 B before archive-id lines (`checkpoint_tokens` = 400). `Checkpoint.ids` lists live `archive_decisions` newest first as `id <id> <tool> <bytes>`, capped by the existing budget. Hosts: Cursor `preCompact` → `pre_compact` (no post event); Copilot `preCompact` → `pre_compact` (no post); Codex `PreCompact`/`PostCompact` via `~/.codex/hooks.json`. Gemini is not a host. Kimi already installed both via Claude `ENTRIES` (docs confirm `PreCompact`/`PostCompact`) — left untouched. ZCode has none. pi/OpenCode stay with T70.6. `PreCompact` without `transcript_path` still saves (Cursor/Copilot). `docs/agents.md` blessed.

## T48.8 — VS Code Copilot Chat host

**T48.8 VS Code Copilot Chat host** · P2, 3/5 · `src/agents/vscode/{mod.rs,README.md}` (new), `src/agents/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `docs/agents.md` (blessed), `src/cli.rs`, `README.md`, `tests/agents_install.rs`, `tests/common/agents.rs`, `tests/trycmd/config-show.stdout`

From I-17. GitHub Copilot Chat in VS Code reads MCP from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in the VS Code profile dir; T46.4 covered only the Copilot CLI and the desktop app.

Do: `rtok agents install vscode` registers `servers.rtok = {type: "stdio", command, args: ["mcp"]}` in each edition's user `mcp.json` through the SDK `register_server`/`unregister_server` (key `servers` — not a copy of Copilot CLI `mcpServers`/`type: local`). User dir per OS: macOS `~/Library/Application Support/<Code|Code - Insiders>/User`, Windows `%APPDATA%/<product>/User`, else `~/.config/<product>/User`. `[setup.vscode] config_path` / `insiders_path` empty means those defaults. One Desktop variant (bins `code` / `code-insiders`). Hooks are `no`: VS Code documents Claude-format agent hooks (Preview; `.github/hooks/*.json`, user `~/.copilot/hooks`, stdin `hook_event_name` / `tool_name`) — that is not the Copilot CLI camelCase the T46.3 mapping serves, and `~/.copilot/hooks` is already the `copilot` host. Proxy and plugin are `no`. Foreign `servers` survive remove. Host table regenerated (`RTOK_BLESS=1` `tests/agents_doc.rs`).

Check: `agents::vscode::tests::user_dir_resolves_code_and_insiders_per_os`; `dry_run_names_the_file_and_creates_nothing`; `apply_is_idempotent_and_remove_keeps_foreign`; `readme_tables_match_support`; `tests/agents_install.rs` matrix (install twice / remove twice); `host_docs`; `agents_doc`; `config_coverage`.

Status: done 2026-09-18 · Model: Cursor / grok 4.6

Evidence: isolated worktree `.worktrees/T48.8` on `t48.8` (not cherry-picked, not pushed). `cargo test --lib agents::vscode` 3/3; `--lib agents::tests::readme_tables_match_support` ok; `--test agents_install` 9/9; `--test host_docs` ok; `--test agents_doc` (blessed) ok; `--test config_coverage` ok. `just check` not run against the dirty main tree.

Deviation: hooks not written (card condition: only if VS Code documents a hook file the T46.3 Copilot mapping can serve — it does not). No `plugins/vscode/` (plugin module is `no`). Commits split over the 3-file cap (host, install tests, config, docs, CLI/README, plan close).

## T53.1 — Coaching nudges under an A/B

From I-18. Short nudges ("do not re-read", "use expand") may cut waste, but they are re-read every turn and dilute instructions.
Done when an opt-in `inject` nudge set exists as data (D7), stays inside the D5 budget and byte-stable, and a P7-style A/B on the bench shows it does not raise cost per passed task; without that result it stays off.

**Result (2026-09-18).** `modes/nudges.md` is D7 data (re-read / expand / outline-first / search-before-Grep), wired as opt-in `builtin("nudges")` in inject (default `modes = []`). Est. **114** prose tokens (cap 250). SessionStart `additionalContext` **0 B off / 478 B on**, byte-stable, absent from UserPromptSubmit. Dry `rtok bench` without `RTOK_BENCH_LIVE`: off and on both **6/6** pass, cost **0** (`live: false`). Live A/B attempted 2026-09-18 after creator spend approval: `claude` 2.1.236 present, `claude auth status` `loggedIn: false`, OAuth expired and `ANTHROPIC_API_KEY` unset (gateway key 401). No live tokens or `stats --price` rows; gate stays **do not enable**. Default `modes` left off.

---

## T68.1 — `explore`: one call answers a code question

From the codegraph / graphify review (2026-09-18). codegraph's single `codegraph_explore`
answers a free-text question with symbols' source, call paths between them and a
blast-radius line (88 % fewer tool calls claimed, unverified); rtok needed three to five
calls (`symbol`, `callers`, `impact`, `read`).
Done when a fifth MCP tool `explore(query, path?)` resolves the query's identifier tokens
(exact, else prefix best-5 by reference count), prints each definition body once per file,
the call paths between the resolved symbols (depth ≤ 3) and one impact depth-1 line per
symbol; tags and LSP backends share one assembler; the answer goes through `cap` with an
archive id; description ≤ 60 tokens, surface ≤ 150; `tests/graph_contract.rs` pins one
two-symbol question byte-exact; `Measurement { plugin = "graph", kind = "explore" }`
records bytes returned versus the sum of the calls it replaced.

**Result (2026-09-18).** Commit `96272ca`. `explore(query, path?)` splits the question into
identifier tokens (`explore_tokens`: alnum/`_` runs, deduped, first 8), resolves each
exactly, else `Store::symbol_name_prefix` (distinct def names, best 5 by reference count,
ties by name). Both backends implement one `ExploreParts` trait — `TagsExplore` over the
indexed rows, `LspExplore` over workspace/symbol + callHierarchy — and
`assemble_explore` produces one answer shape: `= name` + definition bodies (via the
`symbol` text builders, extracted as `defs_text` / `symbol_text`), `paths:` with caller
chains (`symbol_paths`, recursive CTE over the `scope` edges, simple paths only, ≤ 3 hops;
T68.4 reuses it for `impact --to`) or `none`, then `impact:` with one `name ← N` depth-1
line per symbol. The answer goes through `cap_kind` like the other tools; the
`kind = "explore"` Measurement's `before` is the exact bytes the replaced `symbol` +
`impact` calls would have printed. The SDK `Symbols` trait gains `symbol_name_prefix` and
`symbol_paths` as defaulted methods (D25 additive pattern; `MemoryHost` needs nothing);
`Runtime` delegates to the store. The `path` filter narrows printed definitions and the
impact counts, same as `symbol`'s filter; paths stay cross-file like `impact`'s walk.

Surface re-measured: 5 tools, 127 description tokens, each ≤ 60
(`cargo nextest run -p rtok graph_surface`, 2026-09-18) — still ≤ 150. Whole MCP surface
re-measured with `rtok doctor`: 12 tools, ~223 desc tokens (was 11 / ~143); the rows in
`docs/comparison.md` §2/§4, `research.md` §2/§9.3/§14 and the README graph paragraph were
updated to the new numbers. Deviations: 13 files — over the 3-file guide, noted in the
commit; the LSP backend shares the assembler but has no live test (no language server in
CI — same as the other `lsp.*` tools).

Check: isolated worktree at `458f36c` + the 8 code/test files (main tree was red from
concurrent agents' T60.10/T55.15 edits): `cargo clippy --workspace --all-targets
--all-features --exclude rtok-wasm-demo-guest -- -D warnings` clean; `nextest graph::`
57/57; `--test graph_contract` 5/5 (incl. the byte-exact two-symbol question and the
path-filtered case); `--test extra_cover` 9/9. One unexplained one-off flake of the
contract test on a freshly linked binary during an earlier run; 5 consecutive re-runs
green — disclosed, not chased.

---

## T56.1 — Test VFS helper and convention

**All tests must prefer a virtual filesystem** over host `TempDir` / raw `std::fs` as the primary approach. Goal: unit tests run against an in-memory FS so they do not depend on real disk layout, and Windows/macOS path quirks (case fold, spaced profiles) can be simulated. `src/testutil.rs` ships `Vfs` (path → bytes) with `write` / `read` / `read_str` / `len` / `exists` / `paths` / `paths_under`.

**Result.** D29 recorded, AGENTS.md notes the rule, Vfs covers the API above, and read (`search_max_bytes_gate_uses_vfs_sizes`), cmd (`Settings::from_vfs` rules tests), and graph (`file_uri_encodes_spaces`) unit tests use it with no host temp dir. Moved out of `plan.md` on 2026-09-18 (the row had sat there as `done` / 100 %).

---

## T56.4 — Optional walk/VFS adapter for search/tree

If search/tree keep needing real walks, introduce a narrow trait (metadata + read bytes + list dir) with a `Vfs` impl so oversized-file and relative-path tests run without host disk.
Done when search/tree unit tests for the size-cap and relative-path cases can run against `Vfs`, or the card closes with a measured reason to keep WalkBuilder-on-disk.

**Result.** `plugins::read::walk::{WalkFs, walk, search_hits, tree_rows}` + `Vfs::{list_dir, meta}` (dirs inferred). Production `search`/`tree` keep host `WalkBuilder` (gitignore); disk e2e kept. Size-cap / relative-path / skip-`.git` twins run on the adapter. `HostFs: WalkFs` stub landed under test (T56.5); prod swap of WalkBuilder is the T56.5 follow-up, not required here. Moved out of `plan.md` on 2026-09-18 (the row had sat there as `done` / 100 %).

---

### T67.1. `expand --grep` is a regex with numbered hits

From I-53 (`research.md` §12, recursive-llm). The RLM loop is search → slice: the model regex-searches the externalised context and pulls only the span around a hit. `expand --grep` today is a substring match that prints bare lines, so a hit has no position and `--lines a-b` cannot follow; the model's only way to see the context around a match is a full expand, which is the expand-rate cost `report` flags.
Done when `grep` (CLI `--grep`, MCP `expand.grep`) compiles as a regex through the `regex` crate `search` already uses (a pattern that does not compile is matched literally, never an error the model has to retry), every hit prints as `N:line` with its 1-based line number in the archived payload (absolute inside a `--lines` range, the format of `read` mode `lines`), output without `grep` is byte-identical to today, `slice_lines` stays the one range helper shared with `read`; unit test on a four-line fixture (regex hit, literal fallback, numbering inside a range, no-grep unchanged); tool description still ≤ `mcp.max_description_tokens`; README and the `docs/config.md` row updated.
Execution plan (Claude Code / Fable 5.1): `src/expand.rs` (`filter_lines` → `Vec<String>`, generic `slice_lines` / `cap_lines`, one test), `src/mcp.rs` description, `src/cli.rs` flag doc, README, `docs/config.md`. Verify: fmt, clippy `-D warnings`, `nextest -p rtok expand mcp`.

**Result (2026-09-18).** `filter_lines` now returns `Vec<String>`: it numbers every line as it splits (`enumerate` before any range), applies `--lines` through the shared `slice_lines` (made generic, still the one helper `read` uses), then filters with a `Regex`; a pattern that does not compile is retried as `regex::escape`d literal, so the model never gets an error it has to guess its way out of. Hits print `N:line` with the number the line has in the archived payload, absolute inside a `--lines` range, which is the format `read` mode `lines` already uses. Without `grep` the output is byte-identical to before (bare lines), so `cmd`, `archive` and the MCP `expand` trailers are unchanged. `cap_lines` is generic over the element type. Description updated on the MCP tool (still under `mcp.max_description_tokens = 60`), the clap flag doc, the README example and the `docs/config.md` row.

Deviation: the commit is not its own `T67.1:` commit. Another agent ran `git add … && git commit` against the shared working tree while these hunks were staged, so the code landed inside `1e513d3` ("T62.1: guard digests oversized skill bodies on PreToolUse(Skill)"). The history was left alone rather than rewritten under other agents' worktrees. The same race deleted the memory agent's T66.1/T66.2 cards from `plan.md`; `3e3fa91` restored them and renumbered these tasks from T66 to T67.

Check: `cargo nextest run -p rtok` with the expand/mcp filter — 64 passed, including the new `grep_is_regex_numbered_by_archive_line_and_falls_back_to_literal` (regex hit, literal fallback on `[E0308`, numbering inside a range, no-grep unchanged) and `descriptions_at_most_60_tokens`. `cargo fmt` and `clippy -D warnings` clean. Three `agents install cursor` tests failed in that run with empty stdout/stderr and exit 1; the host had 1.5 GiB free while other agents were building. Re-run after freeing the scratch worktree: 3 passed.

---

## T66.1 — `mem_save` updates a note in place: project + kind + title is the topic key

From the engram gap review (`research.md` §13, 2026-09-18). engram's `topic_key` upserts the observation for the same `project + scope + topic_key` and bumps a revision counter, so an evolving decision stays one row; rtok's `mem_save` always inserted, so re-saving "auth model" after a change left two rows with the same title, and SessionStart recall (5 titles) showed the stale one beside the new one. Zero-LLM, no schema change: the title already is the stable key.
Done when `mem_save` with an existing `(project, kind, title)` updates that row's body and `ts` instead of inserting (FTS triggers and the embedding upsert already key by id), returns `{"id", "updated": true}`, an identical re-save is a no-op update, checkpoints keep using `insert_note` (kind `checkpoint:<session>` is per session and `latest_note` orders by id), the tool description says so in one clause, and a unit test saves the same title twice and asserts one row, the new body, and the same id on `mem_search`.
Execution plan: `Store::upsert_note` (select id by project/kind/title, `UPDATE` or `INSERT`) in `src/store/mod.rs`; `plugins::memory::mem_save` returns `(id, updated)`; `mcp.rs` reports it; README/AGENTS lines. Verify: fmt, clippy `-D warnings`, `nextest -p rtok memory`, e2e `memory_save_then_search`.

**Result (2026-09-18).** `Store::upsert_note` selects the newest id for `(project, kind, title)` (`project IS NULL` when none), `UPDATE`s body + `ts = unixepoch()` or falls through to `insert_note`; `mem_save` returns `(id, updated)` and the MCP result is `{"id", "updated"}`; description: "same project+kind+title updates it". Test `same_project_kind_title_updates_in_place`: second save keeps the id, a different kind or project is a new row, FTS finds the new body and not the old one. Checkpoints untouched. Deviation: shipped in one commit with T66.2 (both live in `store/mod.rs` and `memory/mod.rs`; the T66.1 test uses T66.2's `list_notes`). Verified: `cargo nextest run -p rtok memory` 11/11, clippy `--all-targets -D warnings` clean, `just check` (see commit).

---

## T66.2 — `rtok memory export`: the JSONL that `memory import` reads

From the engram gap review (`research.md` §13). engram's Git Sync exports memories as portable chunks a second machine imports; rtok had `memory import <file.jsonl>` (T6.3) and no way to produce that file from its own store, so notes could not move between machines or be backed up outside `rtok.db`.
Done when `rtok memory export [--project <name>]` prints one `{kind,title,body,project}` per line for every note except `checkpoint:*` rows (session-local), in id order, and an export piped into `import` on a fresh store inserts every row and a second pass skips them all (round-trip test on three notes plus one checkpoint); the CLI table in `README.md` and the plugin README name it.
Execution plan: `Store::list_notes(project)` in `src/store/mod.rs`; `plugins/memory/export.rs` writes JSONL to a `Write`; `MemoryCmd::Export` in `cli.rs`; docs rows. Verify: fmt, clippy, `nextest -p rtok memory`.

**Result (2026-09-18).** `Store::list_notes(project)` (`kind NOT LIKE 'checkpoint:%'`, id ascending), `plugins/memory/export.rs::run(cfg, project, out)` writes `serde_json` objects one per line and returns the count, `rtok memory export [--project]` on the CLI. Test `export_round_trips_through_import_without_checkpoints`: three notes + one checkpoint → three lines, `--project q` → none, import into a second store inserts 3 then skips 3. README CLI row and plugin README section added; no config key (no flag beyond `--project`). Verified with T66.1 above.

---

## T62.2 — Compaction checkpoint lists the skills loaded so far

From `research.md` §10.7–10.8 and T2.5 / T58.2. After auto-compaction the skill bodies are gone and nothing tells the model which skills it had loaded; it either re-invokes all of them (248 KB again) or none.
Done when the `PreCompact` checkpoint (`src/plugins/checkpoint.rs`, which already reads the transcript for prompts, paths and errors) also collects the loaded skills: `isMeta` user records with `sourceToolUseID`, joined to the `Skill` tool_use for the name (fallback: the last path component of the `Base directory for this skill:` line), with the body bytes; the restore text on `SessionStart` with `source == "compact"` gains one byte-stable line `Skills loaded before compaction: slint (4.8 KB), update-config (171 KB) — re-invoke only what the next step needs`, kept inside the injection budget (names truncated to the budget, never the line dropped); unit test on a fixture transcript with two invocations; a line on the checkpoint section of the docs. Lands on every host that registers `PreCompact` (T58.2 decides the others).
Execution plan: one commit — `src/plugins/checkpoint.rs` gains `skills: Vec<(name, bytes)>` collected from `isMeta` + `sourceToolUseID` user records (name = last component of the `Base directory for this skill:` line; the tool_use join is not needed since the line always carries the directory), `user_prompt` no longer quotes those bodies as prompts, `render` emits the skills line after prompts and before paths so the budget cut keeps it; unit test with two invocations; `docs/config.md` comment on `checkpoint_tokens`. Verify: fmt, clippy, `nextest -p rtok checkpoint`.

**Result (2026-09-18).** Commit `ab0f29f`. `Checkpoint.skills: Vec<(name, bytes)>` from `type: user` records with `isMeta: true` and a `sourceToolUseID` whose first line is `Base directory for this skill: <dir>`; name = last path component (both separators), so the tool_use join was not needed. `user_prompt` no longer quotes those bodies as prompts (it did before — a 171 KB body showed up as a 300-char "prompt"). Line rendered after prompts, before paths: `skills loaded before compaction: slint (5.0 KB), ponytail (0.1 KB) — re-invoke only what the next step needs`; the budget cut trims paths first, and the line is byte-stable (no timestamps, no ordering by size). Deviations: lower-case `skills loaded …` to match the other checkpoint lines; names are not truncated to the budget — the cut still applies to the whole text, so on a pathological list the line is cut like any other (noted, not worth a second budget path). Test `injected_skill_bodies_are_listed_not_quoted` (two invocations, one Windows path, one `isMeta` prompt without a source tool). Verified: `cargo nextest run -p rtok checkpoint` 4/4, clippy `-D warnings` clean, fmt clean.

---

## T59.4 — Lossless MCP wrapper for foreign servers

From I-44 (atlassian-labs/mcp-compressor; headroom MCP wrapper; `research.md` §9.1). `rtok mcp --wrap -- <server cmd>` spawns the server, proxies stdio JSON-RPC, and shortens `tools/call` results the way `cmd` results are shortened today: raw archived, `expand <id>` trailer, a `Measurement { plugin = "mcp", kind = "wrap", family = <server>/<tool> }` per call. Descriptions, `tools/list`, prompts and resources pass through untouched; the wrapped server keeps its name.
Done when:
1. Evidence first: `stats` over transcripts ranks foreign MCP servers by result bytes (§2 measured 15 K of 2.83 M for this workload); the card records the number and the wrapper stays off by default until a server above 5 % is measured — the code still lands behind `--wrap`.
2. Framing handled for both MCP stdio transports (newline-delimited JSON and `Content-Length` headers); a malformed frame is forwarded byte-for-byte (fail open); server exit code propagated; no third-party MCP crate beyond what `mcp.rs` already uses.
3. Only `result.content[].text` of `tools/call` responses is shortened, by the `cmd` rule engine with a `[mcp]` default rule (`Rule::default()` semantics) and per-`server/tool` overrides in `rules/default.toml`; `isError` results are never shortened.
4. Tests: an in-process fake server (Vfs-free, stdio pipes) with a 3 000-line result → shortened result carries the trailer and `expand <id>` returns the raw text; `tools/list` byte-identical; header-framed and newline-framed fixtures; `isError` untouched. Docs: one section on the MCP docs page with the measured row.
Execution plan (Claude Code / Fable 5.1): (1) `src/mcp/wrap.rs` — frame reader/writer for both transports, child spawn, pass-through loop; (2) result shortening via `plugins::cmd::rules` + `Archive` + `Measurement`; (3) `mcp --wrap` clap flag in `mcp.rs`; (4) tests in `tests/mcp_wrap.rs` with a fake server binary from `assert_cmd::cargo_bin` or a `sh` script; (5) `stats` server ranking row; (6) docs. Three commits: framing + pass-through, shortening + tests, stats + docs.

**Result (2026-09-17).** Evidence: `rtok stats --since 30d` (2026-09-17, 885 sessions), `mcp` table: lean-ctx 8,232 calls, 19.66 MB result bytes, mean 2.4 KB, p95 45.7 KB (≈ 4.9 M est. tokens) — ≈ 27 % of the 71.8 MB in the tool table; rtok 579 KB, engram 279 KB, t3-code 73 KB (mean 18 KB), Claude_Browser 45 KB. lean-ctx is above the 5 % gate, so the wrapper is justified for this workload; caveat: lean-ctx already compresses its own results, so the win is in the p95 tail, not the mean. Landed: `src/mcp/wrap.rs` — `rtok mcp -- <server argv>` (clap `last = true` on `Cmd::Mcp`, not a `--wrap` flag: the host config opts in by spelling the server command that way, so there is nothing to switch off) spawns the server, forwards every frame as written in either framing (newline-delimited or `Content-Length`, detected per frame by the first byte), tracks `tools/call` ids client→server, and for a non-`isError` response archives each text block longer than the `[mcp]` rule's `max_lines` (`Rule::default()` without one), cuts it with `rules::apply` and appends the `rtok run` trailer; `Measurement { plugin = "cmd", kind = "wrap", ref_id = <server>/<tool>:<id> }` — `cmd`, not a new `mcp` plugin id, because the cmd rule engine does the work and `stats` groups by the plugin catalogue. A store that fails to open leaves a plain pipe; the server's exit code is propagated. Tests: `tests/mcp_wrap.rs` (a `sh` fake server, both framings, 3000-line result cut to < 40 lines, `expand <id>` returns all 3000, `tools/list` and `isError` byte-identical, exit code 3 propagated) plus unit tests for the frame reader and the id table. Docs: README command row and a paragraph in `docs/cmd-rules.md` (there is no separate MCP docs page). Not built: per-`server/tool` rule overrides in `rules/default.toml` (a user `[mcp]` section covers today's need; add a `mcp:<tool>` stem when one server needs its own head/tail) and a separate `stats` ranking row (the existing `mcp` table already ranks servers by result bytes).

## T49.2 — Ingest Codex, OpenCode and Cursor session logs

From I-03. `measure` reads Claude Code JSONL only; the other hosts reach the `usage` table only when they go through `rtok proxy`, so their sessions without the proxy are invisible to `stats`, the TUI and the dashboard.
Done when each host's local session store (Codex `~/.codex/sessions/*.jsonl`, OpenCode `opencode.db`, Cursor where it exposes token counts) is read by one reader per host behind the existing `measure` ingest, rows carry the host slug, re-ingest is idempotent, and each reader has a fixture test. A host without token counts is documented as unsupported, not estimated.
Execution plan (Claude Code / Fable 5.1): (1) survey the three stores on this machine (`~/.codex/sessions`, OpenCode's SQLite, Cursor's state dir) and record the schema that carries usage per host in the card — a host without per-turn token counts stops here as "unsupported"; (2) `src/measure/hosts/{codex,opencode}.rs` behind one `HostReader` fn signature `read(dir) -> Vec<UsageRow>` reusing the `usage` table writer `proxy` already uses, host slug column already present; idempotency by `(host, session_id, turn)` unique key; (3) `rtok stats --host <slug>` filter and the host column in the api table; (4) fixture per host under `tests/fixtures/hosts/`; (5) `docs/measure` page rows. One commit per host, ≤ 3 files each.

**Result (2026-09-17).** Survey of the local stores on this machine (Haiku agent, read-only): Codex `~/.codex/sessions/**/*.jsonl` has one `event_msg`/`token_count` line per API request with `last_token_usage` (`input_tokens` ⊇ `cached_input_tokens`, `cache_write_input_tokens`, `output_tokens` ⊇ reasoning); OpenCode `~/.local/share/opencode/opencode.db` (`message`/`part` JSON: role, model, agent — no usage), Cursor `state.vscdb` (`ItemTable`/`cursorDiskKV`: only `cursor.slashUsage.v1` counters) and Copilot CLI `~/.copilot/data.db` (`session_context_usage`: context size and limit per timestamp, no input/output split) carry no per-turn token counts → documented as unsupported in `docs/config.md`, not estimated. Landed: `src/measure/codex.rs` (`collect` → `ApiRow` "codex": input = uncached, cache_read = cached, cache_create = cache_write, output; `jsonl_paths` shared with `stats::collect`), `stats::attach_codex` called from `web::model::stats_report`, `[stats] codex_dir = ~/.codex/sessions`, unit fixture test, trycmd golden. Read on the fly like the Claude Code transcripts instead of writing `usage` rows: no migration, idempotent by construction; the `--host` filter from the execution plan was not needed (the row is one `api` line).

## T58.3 — Measure the `old_string` share of assistant output

From the competitive gap review (`research.md` §9.3, §9.4 item 1; idea I-43). §2 shows assistant output is 8.6 M tokens, 96 % of it tool input, and on Fable/Mythos 5.1 output is 39 % of the bill. Every `Edit` re-emits `old_string` verbatim and nobody has measured what that costs. The number decides T58.4.
Done when:
1. `rtok stats` (the transcripts path `measure::stats::collect` already parses) adds rows: Edit/MultiEdit calls, sum of `old_string` bytes, sum of `new_string` bytes, share of all tool-input bytes and of total assistant output; per host where the edit tool name differs (verify Cursor/Codex names before adding them).
2. Unit test on a fixture transcript with two Edit calls; the measured numbers land in `research.md` §2 with date and command.
3. The card closes with a decision line: T58.4 proceeds only if `old_string` is ≥ 10 % of assistant output on the measured workload; otherwise T58.4 leaves the plan for `ideas.md` with the number.
Execution plan (Claude Code / Fable 5.1): `src/measure/stats.rs` only — `Report` gains an `edits: EditRow { calls, old_bytes, new_bytes, tool_input_bytes, output_tokens }`; `fold_session` sums `old_string`/`new_string` over `Edit`, `MultiEdit.edits[]`, `apply_patch`/`edit_file`-style names verified per host, and `serde_json::to_string(&u.input).len()` over every tool_use; `to_table` prints one `edit` line with the two shares (share of tool-input bytes; est. tokens vs `usage_output`); unit test on a two-Edit fixture next to `ctt_and_tool_totals_on_mini_session`; run on the real transcripts dir, paste the row into `research.md` §2 with the date and command.

**Result (2026-09-17, `rtok stats --since 90d`, 925 sessions):** 5,990 Edit/MultiEdit calls, `old_string` 1.61 MB, `new_string` 3.60 MB; `old_string` = 3.8 % of tool-input bytes, ≈ 1.3 % of output tokens. Under the 10 % gate → T58.4 not built, I-43 keeps the number. Landed: `EditRow` in `src/measure/stats.rs` (`edits` in `--json`, one `edit` line in the table), unit test on an Edit + MultiEdit fixture; row in `research.md` §2.


## T55.16 — Guard deny reads archive metadata, not the body

**T55.16 Guard deny loads the whole archive on the PreToolUse hot path** · P3, 1/5 · `src/plugins/guard/mod.rs`, `src/plugins/guard/AGENTS.md`, `src/store/mod.rs`, `src/plugin.rs`, `crates/rtok-plugin-sdk/src/{host,testing}.rs`

From review 2026-09-17, second pass (code read, then fixed). `guard::pre_tool` called `cx.get_archive` and estimated tokens over the full body just to fill the denial Measurement — megabytes across the ≤ 10 ms hook path, against the plugin's own "no filesystem reads" invariant.

Do: `Archive::archive_size` on the SDK contract (default `Ok(None)` = fail open, so in-memory hosts need no change), backed by `Store::archive_size` (row `bytes` + a file-existence stat on `dir/<id>` else the stored path — never a body read). The deny's `est_before` becomes the same `bytes/4` heuristic `record_context_path` and the semantic-cache measurement use.
Check: `deny_does_not_read_the_archive_body` (unix; a chmod-000 body file still denies — the old body read would have failed open), `missing_archive_fails_open` and the rest of the guard suite unchanged; `just check` green.
Status: done 2026-09-17 · Model: ZCode / GLM-5.3
Evidence: `just check` green — 845/845 tests, fmt, clippy `-D warnings`, build-min, jscpd.
Deviation: none.

## T55.14 — Semantic-cache key folds tool results, ids and image hashes

**T55.14 Semantic-cache key drops tool_result content** · P3, 1/5 · `src/proxy/semantic_cache.rs`

From review 2026-09-17, second pass (reproduced, then fixed). `content_text` kept only `text` fields, so a `tool_result` block contributed nothing to `CachePrompt` — neither to the direct hash nor to `scope_hash`: two requests differing only in tool-result text hashed identically (both eligible under defaults).

Do: one `block_text` helper per content block — text as-is; `tool_result {id}: {nested content}`; `tool_use {id} {name}`; `image`/`document` source data and OpenAI `image_url` contribute their sha256 — used by `content_text` for both the direct hash and the scope.
Check: `tool_result_text_joins_the_cache_key` (the two repro bodies hash differently, shape stays eligible), `tool_use_ids_join_the_cache_key`, `image_blocks_hash_into_the_cache_key`; the `only_near_identical_prompts_clear_the_threshold` family, `p9_fixture_audit_zero_false_hits` and both proxy semantic-cache e2e tests unchanged and green.
Status: done 2026-09-17 · Model: ZCode / GLM-5.3
Evidence: `just check` green — 844/844 tests, fmt, clippy `-D warnings`, build-min, jscpd.
Deviation: OpenAI Chat `tool_call_id`s (the wire's analogue of Anthropic `tool_use_id`) are not folded separately — its tool results already join as `role: "tool"` message text, which is what the card's repro shape needed.

## T55.11 — `expand` freezes pointers by archive id, not session

**T55.11 `expand` never freezes the owning session's pointer** · P2, 3/5 · `src/store/mod.rs`, `src/expand.rs`, `src/plugin.rs`, `src/plugins/archive/mod.rs`, `tests/{proxy,report}.rs`

From review 2026-09-17, second pass (reproduced, then fixed). Every `expand` caller ran under a session that owns no decisions (CLI session `expand`, MCP `mcp-<pid>`), while `mark_expanded` filtered `WHERE session = ?` — it matched 0 rows, pointers were rewritten forever, toon attribution never fired, and the expand-rate honesty metric read 0 %.

Do: freeze and attribute by `archive_id` alone — `Store::mark_expanded(archive_id)` and `Store::live_zone_pointer(archive_id)` (`ORDER BY tool_use_id, session` for a deterministic pick). One expand freezes every session following that pointer: those sessions start receiving the original (more tokens; never wrong bytes — the blast radius the card sanctioned, because no expand surface can know the writer's session).
Check: store `expand_freezes_every_session_pointing_at_the_archive` (rewritten from the T45.3 per-session twin), expand `cli_expand_freezes_the_owning_sessions_pointer` + `expand_measurement_attributes_toon_pointers`, proxy e2e `proxy_expand_freezes_the_pointer_for_the_next_request` (the post-expand request carries the original turn-1 block whole while the un-expanded turn-2 pointer stays).
Status: done 2026-09-17 · Model: ZCode / GLM-5.3
Evidence: `just check` green — 841/841 tests, fmt, clippy `-D warnings`, build-min, jscpd. The load-sensitive `tests/otel.rs::hooks_stay_fast_with_an_unreachable_endpoint` failed twice at load avg 14–23 (p95 212 ms vs the 200 ms budget) from parallel agent work, passed in isolation and in the final full run; the diff touches no hook/otel path.
Deviation: none.

## T55.13 — Copilot `path`-keyed Read events reach guard and read-advice

**T55.13 Copilot `Read` events use `path`, guard and read-advice match `file_path` only** · P3, 1/5 · `src/plugins/guard/mod.rs`, `src/plugins/read/{mod,hook,cache}.rs`

From review 2026-09-17, second pass (reproduced with a scratch test, then fixed). `hooks::types::adapt_copilot` maps `view`/`read_file` to tool name `Read` but keeps the Copilot input key `path`, while `guard::cache_key` and `read::hook::pre_tool` read `file_path` only — on the Copilot host the re-read deny and the large-file advice never fired (`cache::invalidate` already accepted both keys).

Do: one `read::path_arg` helper (`file_path` **or** `path`) used by `read::hook::pre_tool` and `cache::invalidate`; `guard::cache_key` takes the same two keys inline (no cross-plugin feature dependency for a one-liner).
Check: `copilot_path_key_dedups_like_file_path` (guard deny fires for both keys, reason names `rtok expand`), `copilot_path_key_gets_the_read_advice` (100 KiB file denied with `rtok read` in the reason), existing `file_path` tests unchanged; `just check` green (fmt, clippy `-D warnings`, nextest 838 passed, build-min, jscpd).
Status: done 2026-09-17 · Model: ZCode / GLM-5.3
Evidence: `just check` green; both new tests pass on macOS.
Deviation: none.

## T55.7 — Stats `strip_prefix_cd` and quoted paths

**T55.7 Stats `strip_prefix_cd` and quoted paths** · P3, 1/5 · `src/measure/stats.rs`

From review 2026-09-17. `strip_prefix_cd` split the path on the first whitespace, so `cd 'My Documents' && git status` / `cd "C:\Program Files\…" && …` did not strip and `bash_family` bucketed the call as `cd`.

Do: one `skip_word` helper (bare word or `'…'` / `"…"` segments, `~/'My Documents'/src` mixed; unterminated quote → `None` so the caller fails open and the command stays untouched). `strip_prefix_env` and `strip_prefix_cd` both use it, replacing the two ad-hoc splitters. `guard::strip_cd_and` is untouched (its cwd-blind key is T55.9).

Check: `bash_family_strips_quoted_cd_paths` — spaced single/double-quoted paths, mixed quoting, quoted env before `cd`, tab separators, unterminated quote stays `cd`, `cdx` is not `cd`; `bash_family_strips_cd_and_env` unchanged.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `just check` green (fmt, clippy `-D warnings`, nextest workspace, build-min, jscpd).
Deviation: none. Review of neighbouring code filed T55.8 (guard `read:` keys survive a mutating Bash), T55.9 (guard Bash key cwd-blind), T55.10 (three copies of `cmd_stem`) and idea I-38.

## T55.1–T55.6 — Windows/agent correctness (review 2026-09-17)

**T55.1–T55.6** · P1–P2 · `src/plugins/read/search.rs`, `src/plugins/cmd/{hook,run}.rs`, `src/plugins/graph/lsp.rs`, `src/agents/mod.rs`, `src/plugins/guard/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `plugins/cursor/scripts/mcp.cmd`, `src/testutil.rs`, `tests/trycmd/config-show.stdout`

From review 2026-09-17 (docs PR #48 reverted on main). Six Windows/agent bugs fixed with regression tests; new tests prefer `testutil::Vfs` / pure `Path` where they touch files (D29 / T56).

Do:
- T55.1: `display_rel` strips prefixes ASCII-case-insensitively on Windows (aligned with `under`).
- T55.2: `never_wrap` stem match is ASCII-case-insensitive (`Sudo.exe`, `RTOK.EXE`).
- T55.3: graph `file_uri` percent-encodes spaces/reserved octets; `path_from_file_uri` decodes.
- T55.4: PreToolUse wrap uses `wrap_quote` (PowerShell-safe on Windows); `shell_quote_bin` uses cmd `""` escapes; guard `strip_wrap` accepts both POSIX and PS forms.
- T55.5: `plugins.read.search_max_bytes` (default 1 MiB); search skips oversized files before `read_to_string`.
- T55.6: `plugins/cursor/scripts/mcp.cmd` invokes `call rtok mcp`.

Check: unit tests in search/hook/run/lsp/agents/testutil; `cargo nextest` + clippy `-D warnings` + fmt; config-show snapshot includes `search_max_bytes`.
Status: done 2026-09-17 · Model: Cursor / Grok
Evidence: see PR for branch `fix/t55-windows-bugs`.
Deviation: T55.7 left open; VFS migration continues as T56.x (`Vfs` helper landed here).

## T48.6 — Zed host

**T48.6 Zed host** · P2, 3/5 · `src/agents/zed/{mod.rs,README.md}` (new), `src/agents/mod.rs`, `src/cli.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `docs/agents.md`, `README.md`, `site/content/docs/commands.md`, `tests/agents_install.rs`, `tests/agent_remove.rs`, `tests/common/agents.rs`, `tests/trycmd/config-show.stdout`

From I-17. Zed configures MCP as `context_servers` in `~/.config/zed/settings.json` (JSON with comments) and has no shell hook events; its agent can also use external agents over ACP.

Do: `rtok agents install zed` writes `context_servers.rtok = {command, args}` with a comment-aware textual edit (no JSONC crate): comments and foreign servers survive install and remove. CLI and desktop share one file (`shared()`). Support: mcp yes; hooks/proxy/plugin `no` with reasons. `[setup.zed] config_path` default `~/.config/zed/settings.json`. Host table regenerated (`RTOK_BLESS=1` `tests/agents_doc.rs`).

Check: unit tests in `zed/mod.rs` (dry-run, missing-file idempotent, comments/foreign servers, comment-only object left in place, malformed refuses to write); `agents_install` matrix row; `zed_remove_keeps_comments_and_foreign_servers`; `host_docs` README `## Docs` links; `agents_doc` table includes Zed CLI + Desktop.
Status: done 2026-09-17 · Model: Cursor / grok 4.6
Evidence: `cargo nextest run` zed unit + `agent_remove` zed + `host_docs` + `agents_doc` pass; `cargo clippy -p rtok --all-targets -- -D warnings` clean. Pre-existing `agents_install::remove_twice` failure on a real OpenCode 1.18.29 on PATH is unchanged (same as T48.7).
Deviation: `zed/mod.rs` is larger than the 200 LOC working agreement because JSONC has to be edited without a new crate; noted on the card.

## T50.4 — Optional deny of native Grep and Glob

**T50.4 Optional deny of native Grep and Glob** · P3, 2/5 · `src/plugins/guard/mod.rs`, `src/doctor.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `tests/commands_e2e.rs`, `tests/fixtures/hooks/pre_tool_{grep,glob}.json`, `src/hooks/types.rs`, `src/report/{ai,pdf}.rs`, `tests/trycmd/config-show.stdout`

From I-08. `[plugins.guard] deny_grep_glob = false` (opt-in, no CLI flag). PreToolUse denies native `Grep`→`search` and `Glob`→`tree` with a pointer reason, only while the `read` plugin is enabled (fail open: no `search`/`tree` to point at; the knob is per-host opt-in so a host without `rtok mcp` never turns it on, and the hook path does no filesystem reads). Each deny records a zero-delta `guard/native_deny` Measurement (countable deny rate, claims no saving per D3). `rtok doctor` prints `read-share grep+glob x% of read-class tokens (read, grep, glob)` from `stats::collect` over `[stats] transcripts_dir`, or `read-share no data` on empty/missing transcripts. Default stays off: my 30 d transcripts show ~5.7 M Read tokens vs ~0 Grep/Glob, so the numbers do not justify it.

Check: guard unit test (default off, on-denies with pointer, other tools untouched, zero-delta row, read-disabled allows); doctor unit tests (synthetic JSONL → 44.4%, empty dir → no data); hook e2e `hook_grep_glob_deny_is_opt_in` via binary stdin with the knob off/on; `every_fixture_round_trips_unchanged` now covers 9 fixtures.

Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: isolated worktree at e2e43a8 + this task's hunks: `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; lib 524/524; `commands_e2e` 13/13; `cli_trycmd` + `report_ai` + `report_pdf` green; `web` + `stats_model` green; binary `rtok doctor` on fixture transcripts prints 33.3% and `no data` on a missing dir; `cargo build --no-default-features --features measure` green. The one `surface_parity` failure there names HEAD's `wrap` (T51.4 landed without its EXEMPT row — fails without this task's changes).

Deviation: 11 files (over the 3-file guideline) — the repo's own gates force the spread: `default_toml_is_the_defaults` (default.toml), `config_coverage` leaf rule (docs row per convention), `config-show` snapshot, fixture-count assertion (types.rs), two `Report` struct literals (ai/pdf), plus the two hook fixtures. No new dependency.

## T50.2 — User filter drop-in directory and schema

**T50.2 User filter drop-in directory and schema** · P3, 2/5 · `src/plugins/cmd/rules.rs`, `src/config/mod.rs`, `src/config/validate.rs`, `src/cli.rs`, `config/default.toml`, `docs/config.md`, `docs/cmd-rules.md` (new), `site/content/docs/reference/_content.gotmpl`, `tests/cmd_rules.rs` (new), `tests/trycmd/config-show.stdout`

From I-06. Users can already override rules through one user rules file, but there is no `rules.d/*.toml` drop-in, no published schema and no example, so writing a filter means reading `src/plugins/cmd/rules.rs`.

Do: `[plugins.cmd] rules_dir` (default `~/.rtok/rules.d`) joins the single `rules` file; `Settings::load` merges built-ins < user file < sorted `rules.d/*.toml`, later files winning per `match_cmd` through the existing `merge_rules`. Both layers now parse strictly (`parse_strict`: TOML syntax, table-only top level, known fields, right types) and a malformed file is skipped whole at runtime (fail open — previously a bad value fell back per-field). `rtok config validate` additionally reports malformed rules files (single file when present + every drop-in; missing paths are not errors) via feature-gated `validate::rules_issues`, reading the dirs through the validated file as the user layer with a loader that creates nothing. `docs/cmd-rules.md` documents every field with defaults, merge order, fail-open semantics and a worked pytest example; site reference row added. D12: `rules_dir` key + docs row, no new flag.
Check: unit tests (drop-ins merge in name order after the file, broken drop-in skipped + missing dir is built-ins, strict rejects syntax/wrong-type/unknown-field, `issues_in` names the file); `tests/cmd_rules.rs` e2e (`config validate` fails naming the file then passes once fixed; `filter --stdin` proves the drop-in wins and a broken sibling does not stop it); `config_coverage`, `cli_trycmd` (config-show row), `filter` green.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: clean worktree at ae54cbf + these files: `cargo nextest run --workspace --no-fail-fast` 743 passed, 1 failed — the pre-existing opencode `remove_twice` env failure (real opencode 1.18.29 on PATH; fails identically without these changes); lib `plugins::cmd` + `config` 94 passed (incl. 4 new rules tests); `cmd_rules`/`filter`/`cli_trycmd`/`config_coverage` 6/6; min-feature build (`--no-default-features --features measure`) green; `cargo fmt --check` and `cargo clippy --workspace --all-targets --all-features -D warnings` clean; `hugo --source site` builds with the new `cmd-rules` page. Full `just check` is not green in the main tree (other agents' concurrent uncommitted WIP); untouched by this task.
Deviation: none; no new dependency.

## T52.1 — Query language over the graph index

**T52.1 Query language over the graph index** · P3, 3/5 · `src/plugins/graph/mod.rs`, `src/plugins/graph/lsp.rs`, `tests/graph_contract.rs`, `src/plugins/graph/README.md`, `research.md`

From I-14. `graph` answers `symbol`, `callers`, `impact` and `outline`; composite questions (callers of X inside path Y of kind Z) take several calls.

Do: measured 172 transcripts in `~/.claude/projects/*/*.jsonl` (2026-09-17, 157 sessions with tool calls): 615 `ctx_search` calls, 610 carrying a path scope; 252 search→search refinement chains in 33 sessions; 528 search→read chains in 46 sessions; 41/98 shell `rg` calls with a path arg — verdict GO. Optional `path` (substring) on `symbol`/`callers`/`impact` plus optional `kind` (exact) on `symbol`, on the existing tools (no new tool, no new required schema fields); a `Filter` rows check threads through the tags path and the LSP backend with the same semantics (`impact` walks the full graph, filters reported lines). Old 3-arg `symbol`/`callers`/`impact` stay as thin wrappers, so benches, `watch.rs` and `graph_lsp_gate.rs` need no churn. Empty answers name the scope (`no definition of b of kind struct`); unfiltered answers stay byte-exact (T8.9). Surface after: 4 tools, 94 description tokens (was 62; bar ≤ 150, each description ≤ 60).
Check: 4 new unit tests in `mod.rs` (path keeps one file, kind picks struct over function, callers/impact keep one subtree incl. empty-scope messages) + `tests/graph_contract.rs::filters_narrow_to_one_subtree` (MCP stdio e2e through the binary, hermetic tmp home); existing contract byte-exact assertions unchanged and green.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: `cargo nextest run -p rtok --lib plugins::graph` 47 passed; `--test graph_contract` 4/4; `--test graph_lsp_gate --test graph_bench --test graph_truth` 10 passed; `mcp::descriptions_at_most_60_tokens` green; `cargo clippy -p rtok --lib --tests --all-features` clean; `rustfmt --check` on the three files clean. One mid-task contract run showed 2 failures from concurrent agents' mid-edit tree breakage (transient `src/plugins/cmd/rules.rs` compile error); green on rerun after the tree settled. Full `just check` stays red on concurrent WIP — reported, not fixed.

Deviation: compat wrappers (3×3 lines) instead of a signature churn across benches/watch/gate tests; code in 3 files, docs (`README.md` surfaces row, `research.md` §2 table) alongside.

## T51.3 — Gemini wire in the proxy

**T51.3 Gemini wire in the proxy** · P3, 4/5 · `src/proxy/gemini.rs` (new), `src/proxy/wire.rs`, `src/proxy/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `tests/proxy.rs`, `tests/fixtures/proxy/gemini_generate_{body,stream}.json` (new), `tests/trycmd/config-show.stdout`

From I-11. The proxy speaks Anthropic Messages and OpenAI Chat/Responses; Gemini `generateContent` / `streamGenerateContent` hosts cannot use rtok's proxy.

Do: `src/proxy/gemini.rs` implements `Wire` — matches `:generateContent`/`:streamGenerateContent` path suffixes; `contents` tool results keyed by `functionResponse.name` (the API carries no stable call id; rewritten payload is `response` so the name stays visible); `usageMetadata` counters (`promptTokenCount` in, `cachedContentTokenCount` cached-read, `candidatesTokenCount` out; body object or end-scanned array, SSE events); `provider_total = input + output` (prompt already contains cached); model slug parsed from the path (the body carries none) via a new defaulted `Wire::model()`. Routes to new `[proxy] gemini_upstream` (default `https://generativelanguage.googleapis.com`; env `RTOK_PROXY_GEMINI_UPSTREAM`; no CLI flag). Shared `UsageFields` gains the `container` key (`usage` vs `usageMetadata`) instead of a fourth usage-walk copy.
Check: `tests/proxy.rs::proxy_gemini_body_records_usage_with_cached_tokens_and_path_model` + `proxy_gemini_stream_is_byte_identical_and_records_usage` (mock on `gemini_upstream` only, Anthropic upstream dead; counters, cached tokens, model slug, `api = "gemini"`, byte-identical SSE, `calls`/`call_io`/`tokens` rows) plus `gemini.rs` unit tests (matches, path model, name-keyed results, body/array/SSE usage).
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: `cargo test --test proxy` 26 passed incl. 2 new; `cargo test --lib gemini` 5 passed, `wire` 4 passed; `--test wrap/config_coverage` + `cli_trycmd` help/version/bench/config-show green (`completions-bash` fails on T53.2's uncommitted work); `fmt --check`, `build-min`, `jscpd` green. A temporary public-API mirror (since deleted) caught and fixed one real bug while lib-test was uncompilable on others' edits (path model accepted action-less paths).

Deviation: 10 files (wire + shared helper + config/docs + 2 fixtures + tests + snapshot); no new dependency.

## T53.2 — Shell completions and man page

**T53.2 Shell completions and man page** · P3, 1/5 · `Cargo.toml`, `Cargo.lock`, `src/cli.rs`, `tests/completions.rs`, `tests/surface_parity.rs`, `tests/trycmd/completions-bash.toml`, `tests/trycmd/completions-bash.stdout`, `tests/trycmd/help.stdout`, `README.md`, `toolchain.md`

From I-20. `rtok` has a large clap surface but no completions or man page.

Do: `rtok completions <shell>` prints bash/zsh/fish/powershell completions via `clap_complete::generate`, `rtok man` prints the roff page via `clap_mangen::Man` — both generated from `Cli::command()`, so they stay byte-exact with the CLI surface. The shell is a positional `ValueEnum` (no long flag), so no D12 config key and no `config_coverage` ALLOW change; both commands are classified in `surface_parity.rs` EXEMPT as helpers (D27 gate). Workspace `rust.md` already listed both crates — no change there.
Check: `tests/trycmd/completions-bash.toml` + blessed `.stdout` (one shell), `tests/completions.rs` (assert_cmd: every shell renders non-empty output naming the binary, unknown shell refused, `man` carries `.TH`/rtok/completions markers — no `man` snapshot, the page embeds the git-sha version), regenerated `help.stdout`, README install snippet.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: isolation worktree at 7d1e2a1 + own files only — `cargo fmt --check` green; `cargo clippy --workspace --all-targets --all-features --exclude rtok-wasm-demo-guest -- -D warnings` green; `cargo nextest run --test completions --test cli_trycmd --test surface_parity --test config_coverage` 9 passed. Full `just check` on main stays red on concurrent agents' in-progress work (graph test arity errors, windsurf unused import, transient mid-edit lib breakage) — reported, left for the owners.

Deviation: ~100 hand-written LOC but 10 files (the D27 gate, both snapshots and the docs each demand their file); `completions-bash.stdout` is 2796 generated lines, not counted. Includes the 4-line rustfmt normalization of the `wrap` EXEMPT entry — HEAD was not fmt-clean there, and without it no commit can pass `fmt --check`.

## T48.5 — Windsurf host

**T48.5 Windsurf host** · P2, 3/5 · `src/agents/windsurf/{mod.rs,README.md}` (new), `src/agents/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `docs/agents.md` (blessed), `src/cli.rs`, `README.md`, `site/content/docs/commands.md`, `tests/agents_install.rs`, `tests/agent_remove.rs`, `tests/common/agents.rs`, `tests/trycmd/config-show.stdout`
Do: `rtok agents install windsurf` registers `rtok mcp` in Cascade's `~/.codeium/windsurf/mcp_config.json` (`[setup.windsurf] config_path`) as `mcpServers.rtok = {command, args}` with no `type` field, the stdio shape the Windsurf MCP docs show, through the SDK's `register_server`/`unregister_server`; foreign servers survive. One Desktop variant (app bundles macOS + Windows, no CLI binary claimed). Hooks are `no`: Cascade hooks (`~/.codeium/windsurf/hooks.json`, twelve `agent_action_name`/`tool_info` events) are a different contract from `hook_event_name`/`tool_name`, so `rtok hook` needs a `--host windsurf` payload mapping first (T46.3 is the pattern) — the card's hook condition is evaluated and documented, not silently skipped. Proxy and plugin are `no` (no documented base-URL setting; no local plugin dir — rules/memories live in `.windsurf/`). README carries the module table, `Reachable:`/`Not reachable:` lines and `## Docs` with the two verified doc links (MCP, hooks, fetched 2026-09-17). The e2e matrix gains windsurf; the unknown-host probe is renamed `windsurf` → `notahost`.
Check: `agents::windsurf::tests::dry_run_names_the_file_and_creates_nothing`; `apply_is_idempotent_and_remove_keeps_foreign` (no `type` field, second apply `no changes`, remove keeps `foreign`, second remove `no changes`); `readme_tables_match_support`; `config::tests` path leaves + `default_toml_is_the_defaults`; `tests/agent_remove.rs::windsurf_remove_keeps_foreign_servers`; `tests/agents_install.rs` matrix + `an_unknown_host_is_refused_before_any_backup`; `host_docs`, `config_coverage`, `agents_doc` (blessed), `cli_trycmd`.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test --lib -- agents:: config::` 126 passed; `agent_remove` 12 passed; `agents_install` 8 passed + 1 pre-existing failure (`remove_twice…` on opencode Desktop: install skips it as not-found, remove still applies as `— no changes` — untouched by this task, needs its owner); `host_docs`, `config_coverage`, `agents_doc`, `cli_trycmd` green; `cargo clippy --lib --all-features -- -D warnings` clean; rustfmt clean. `just check` not green in this tree (concurrent agents' WIP broke `src/proxy/cli.rs` and graph mid-verification; verified with those files at HEAD via temporary stashes, all restored/dropped without loss). Not verified on a live install: the Windsurf app bundle paths.
Deviation: registry + config + docs + e2e by construction (T46.4 precedent); `src/cli.rs` host help also names `aider` (landed without updating it).

## T49.1 — `rtok stats --price`

**T49.1 `rtok stats --price`** · P2, 3/5 · `src/measure/stats.rs`, `src/store/mod.rs`, `src/config/mod.rs`, `src/config/validate.rs`, `src/cli.rs`, `src/web/model.rs`, `config/default.toml`, `docs/config.md`, `tests/stats_price.rs` (new), `tests/trycmd/stats-price.{toml,stdout,stderr}` (new) + fixture config, `tests/trycmd/config-show.stdout`

From I-02. `rtok stats` reports tokens but not money, so a saving cannot be compared with a model's cost; cache reads are priced very differently from input (research.md §8).

Do: `[stats] price=false` plus `[stats.prices."<model>"]` (USD per MTok: input, cache_write, cache_read, output) with four shipped rows dated 2026-09-17 — Anthropic Sonnet 5 (2.0/2.5/0.2/10.0) and Haiku 4.5 (1.0/1.25/0.1/5.0) from platform.claude.com/docs/en/about-claude/pricing, OpenAI gpt-5 (1.25/1.25/0.125/10.0) and gpt-5-mini (0.25/0.25/0.025/2.0) from platform.openai.com/docs/pricing (no separate write price there, so cache_write = input). `rtok stats --price` attaches per-model costs from the same proxy `usage` rows (new `Store::usage_by_model`): cost = Σtok/1e6×rate, saved = cache_read×(input−read)/1e6 — the only saving computable from usage alone. Models without a row print `-` for both dollar columns (token counts still print), stay out of the totals, and are named. Costs attach only when the flag is set, so default table/JSON output is byte-identical. `config validate` exempts the open-ended `stats.prices` subtree (bench.configs precedent); every new key has its default.toml row, docs row and flag mapping (D12).
Check: unit tests on the arithmetic (`row_cost` legs/saving/rounding, `attach_costs` priced+unknown+NULL-model on an in-memory store); `tests/stats_price.rs` fixture-ledger e2e (table totals $14.70 / saved $15.30, `-` rows, JSON cost object, plain `stats` mentions no costs); hermetic trycmd `stats-price` snapshot (empty ledger); `stats_model.rs` goldens unchanged; `config_coverage`, `host_docs`, `cli_trycmd` green.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: clean worktree at e2e43a8 + these files: `cargo nextest run --workspace` 705 passed, 2 failed — both pre-existing (opencode `remove_twice`: real opencode 1.18.29 on PATH, fails identically without these changes; `surface_parity` on T51.4's `wrap` command, fails on clean HEAD too); `stats_price` 3/3; `stats_model` goldens unchanged; lib price unit tests pass; `cli_trycmd`, `config_coverage`, `host_docs` green; `cargo fmt --check` and `cargo clippy --workspace --all-targets --all-features -D warnings` clean. Full `just check` is not green in the main tree (other agents' concurrent uncommitted WIP breaks the build); untouched by this task.
Deviation: none; no new dependency.

## T51.2 — Anthropic native context editing

**T51.2 Anthropic native context editing** · P3, 3/5 · `src/proxy/anthropic.rs`, `src/proxy/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `tests/proxy.rs`, `tests/trycmd/config-show.stdout`, `README.md`

From I-10. Anthropic can clear old tool uses server-side (`context_management`, `clear_tool_uses_*`), which competes with or complements `archive`.

Do: `[proxy] context_management = false` (opt-in; env `RTOK_PROXY_CONTEXT_MANAGEMENT`; no CLI flag) arms server-side clearing on Anthropic Messages only — the proxy adds `context_management.edits: [{type: clear_tool_uses_20250919}]` plus the `context-management-2025-06-27` beta header (API shape verified against the platform docs 2026-09-17), never overwriting a caller-set field, in both proxy modes. Each armed request records a zero-delta `proxy/context_management` Measurement naming the path (semantic-cache precedent; the platform's saving is not locally observable so none is claimed, D3). `archive` stands down on armed Anthropic requests (no double-shrink, no cache churn). No `Wire` trait change.
Check: `tests/proxy.rs::proxy_anthropic_context_edits_arm_platform_path` (mock upstream matches the beta header; body carries the field; old turns unrewritten; 0 archive rows; 1 path row; 1 usage row) plus `anthropic.rs::context_edits_are_opt_in_and_never_overwrite`.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: `cargo test --test proxy` 24 passed; `--test wrap/cli_trycmd/config_coverage` green; `--lib proxy/config` green; `fmt --check`, `build-min`, `jscpd` (exit 0) green. README compares both paths on the same six-turn request: archive 103 729 → 70 837 B with 4 Measurements; platform 103 729 → 103 798 B (+69 B field) with 1 path row.

Deviation: 8 files / ~240 LOC (card needs config + docs per D12, a proxy test, and the README comparison). `just check` clippy stays red on another agent's untracked `src/agents/windsurf/mod.rs` (`unused import std::fs`), left for its owner.

## T54.1 — Agents support table in docs

**T54.1 Agents support table in docs** · P2, 2/5 · `docs/agents.md`, `tests/agents_doc.rs`, `site/content/docs/reference/_content.gotmpl`, `AGENTS.md`

Creator request: one documentation table of every agent app (CLI / Desktop), whether rtok links a plugin into it, and which rtok features reach it; agents must keep it current.

Do: `docs/agents.md` holds the table between `agents-table` markers: host, app, kind, the four install modules (hooks, MCP, proxy, plugin as `yes`, flag or `—`) and the catalogue plugins reached. `tests/agents_doc.rs` builds the same table from `HOSTS`, `Agent::variants`, `Agent::support`, `agents::reaches` and the plugin registry, and fails with the regenerate command when the doc is stale; `RTOK_BLESS=1` rewrites it. Site reference page `Agents`. AGENTS.md D21 line tells agents to regenerate after any host or surface change.

Check: `cargo test --test agents_doc` passes; editing one cell makes it fail with "host table is stale"; `RTOK_BLESS=1` restores it. Table includes aider (T48.7).

Status: done 2026-09-17 · Model: Claude Code / Fable 5.1

Evidence: `just check` in a clean worktree at ef6c6ff: 693 passed, 2 failed, 2 skipped; both failures reproduce at HEAD without this change (`surface_parity` unclassified `wrap` from T51.4; `agents_install` `remove_twice_says_no_changes_and_the_second_takes_no_backup` for OpenCode). `agents_doc` and `host_docs` pass after rebase on 0dfcad1.

Deviation: 4 files. Claimed and closed in one commit without a plan.md row, because plan.md and done.md held other agents' uncommitted edits; only this entry is staged in done.md.

## T52.4 — Dead code report

**T52.4 Dead code report** · P3, 2/5 · `src/store/symbols.rs`, `src/plugins/graph/mod.rs`, `src/cli.rs`, `src/plugin.rs`, `crates/rtok-plugin-sdk/src/host.rs`, `tests/surface_parity.rs`

From I-29. `Store::symbol_dead_candidates` lists defs with no same-name ref row under the root (one `NOT EXISTS` query, name-based like `callers`); `graph::dead` filters to actionable dead code — skips `macro` kind, `main`, test paths (`tests/`, `test_`/`_test`), `pub` lines, `#[test]`/`#[cfg(test)]` fns, methods inside trait/`impl X for Y` ranges and any `impl` target type (the tags query records no ref for a trait impl's type, so `S` in `impl T for S` would otherwise read as dead; one tree-sitter parse per `.rs` file, no new dependency); `rtok graph dead [path]` prints `path:line kind name` through the existing cap (one `graph/cap` Measurement per call, empty report is `no dead code in <root>`). MCP surface stays four tools (name-stability rule); `surface_parity` classifies `graph dead` as on-demand reading with no snapshot page yet, like `stats`.

Check: `plugins::graph::tests::dead_lists_only_the_private_orphan` (fixture repo: pub fn, used private fn, trait + trait-impl method + impl-target struct, `macro_rules!`, `#[test]` fn, `tests/` file, `main` — only the orphan is listed); e2e `rtok graph dead` on a fixture repo prints one line and `rtok stats --plugin graph` shows the `cap` row; `graph_contract`/`graph_truth` byte-exact.

Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: isolated worktrees with this task's hunks only. At 77af448: `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; 43/43 `plugins::graph` lib tests; `graph_contract` + `graph_truth` 6/6; `surface_parity` 4/4; full nextest 694 passed / 2 failed / 2 skipped (both failures pre-existing on clean HEAD: `agents_install remove_twice…` opencode backup, plus the `surface_parity` row this commit adds); `cargo build --no-default-features --features measure` green. Re-verified at 0dfcad1: fmt/clippy clean, 43/43 graph, contract+truth green, `graph dead` classified; the one `surface_parity` failure there names HEAD's `wrap` (T51.4 landed without its EXEMPT row — not this task's command).

Deviation: 6 files / 228 insertions (over the 200 LOC / 3-file guideline) — D25 forces the SDK trait seam (`Symbols::symbol_dead_candidates` with an empty default so out-of-tree hosts keep compiling, plus the `Runtime` delegation) beside the store query, the filter, the CLI and the parity row; no way to add a store-backed capability in fewer files without breaking the plugin contract. No new dependency (tree-sitter + tree-sitter-rust already behind the `read` feature `graph` requires).

## T48.7 — aider host

**T48.7 aider host** · P3, 2/5 · `src/agents/aider/{mod.rs,README.md}` (new), `src/agents/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `README.md`, `site/content/docs/commands.md`, `tests/agents_install.rs`, `tests/agent_remove.rs`, `tests/common/agents.rs`, `tests/trycmd/config-show.stdout`

From I-17. aider has no MCP and no hooks, but reads `~/.aider.conf.yml` / `.env`, where a base URL can point it at `rtok proxy`, which is the only rtok surface it can use.

Do: `rtok agents install aider --proxy` writes `openai-api-base: http://<bind>:<port>/v1` into `~/.aider.conf.yml` (`[setup.aider] config_path`) with a line edit, so comments and foreign keys survive — no YAML crate, no new dependency. Correction to the card: there is no `anthropic-api-base` in aider's options reference (only `--openai-api-base`; verified against the fetched options page 2026-09-17) — Anthropic models reach the same URL with an `openai/` model prefix, and the README says so. Support: proxy ``--proxy``, hooks/mcp/plugin `no` with reasons. Remove strips only `openai-api-base` lines pointing at this proxy (exact URL or loopback+our-port); a foreign base URL stays, and remove on a foreign value is `no changes`. Install without `--proxy` is `no changes` (flag-gated like codex proxy). `installed()` reads the key back for the e2e matrix. Unreadable file is an error, never an overwrite.
Check: unit tests in `mod.rs` (dry-run shows one key + revert and touches nothing; apply idempotent, keeps comments/foreign keys; quoted value + trailing comment keep shape; commented `# openai-api-base` untouched; foreign-URL remove is no-op; non-UTF-8 refused with file untouched); `agents_install` matrix + `agent_remove` entries; README parity (`Reachable: measure, archive, proxy, toon, compress`); `## Docs` links re-verified 2026-09-17.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: clean worktree at 77af448 + these files: `cargo nextest run --lib agents:: config::` 124 passed; `--test agent_remove` 11 passed (incl. the new aider test); `--test agents_install` 22/23 — the aider legs pass, the 1 failure is pre-existing `remove_twice` on opencode (fails identically without these changes; a real opencode 1.18.29 on PATH); `--test host_docs --test config_coverage --test cli_trycmd` pass; `cargo fmt --check` and `cargo clippy --workspace --all-targets --all-features -D warnings` clean. Full `just check` is not green in the main tree (other agents' concurrent uncommitted WIP breaks the build); untouched by this task.
Deviation: none; verified in a clean worktree at 77af448 + these files (main tree holds other agents' uncommitted work).

## T51.4 — `rtok wrap -- <agent>`

**T51.4 `rtok wrap -- <agent>`** · P3, 2/5 · `src/proxy/cli.rs`, `src/cli.rs`, `tests/wrap.rs` (new), `tests/trycmd/help.stdout`

From I-12. Pointing a host at the proxy means editing its config; for a one-off run it is simpler to set `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` for one process.

Do: `rtok wrap -- <cmd> [args]` ensures the proxy answers `/health` (hand-rolled TCP GET, no new dep — reqwest has no blocking feature), starting it in-process on a background thread with its own runtime when down; execs the child with both base URLs from the same helpers the installers write, inherits stdio, prints nothing itself, and exits with the child's code (signal death → 128+signo; terminal signals already reach the child via the shared foreground pgroup, so no signal crate). Fail open: an unstartable proxy runs the command with the operator's own environment. No long flags, so no new config keys (T12.4 walk skips positionals).
Check: `tests/wrap.rs` — fake agent (`sh -c` / `cmd /c`) echoes both URLs with byte-exact silent stdout, `exit 3` → 3, and the ensured proxy records one `usage` row plus `calls`/`call_io`/`tokens` rows against the Anthropic body fixture.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3

Evidence: `cargo test --test wrap` 3 passed; `cargo nextest run --test wrap --test proxy --test config_coverage` green; `help.toml` trycmd passes with the new `wrap` line; `cargo fmt --check` clean.

Deviation: the 129-line `src/proxy/cli.rs` bulk landed inside G1 `2b8c266` (T48.3), swept from the shared dirty tree in pre-fix form — HEAD did not compile (E0382 moved `cfg`, missing `anyhow::Context`, two clippy lints). This commit adds the `src/cli.rs` wiring, those fixes, the tests and the help snapshot instead of re-committing the swept lines. Files: 3 source/test + 1 snapshot line. `just check` clippy stays red on another agent's untracked `src/agents/windsurf/mod.rs` (`unused import std::fs`), and `config-show`/`agents_install` trycmd cases fail on other agents' uncommitted host/config WIP — both unrelated to this task and left for their owners.

## T52.5 — Type-position and scoped-call references

**T52.5 Type-position and scoped-call references** · P3, 3/5 · `src/plugins/read/outline.rs`, `src/plugins/graph/index.rs`, `tests/graph_truth.rs`, `tests/graph_lsp_gate.rs`, `research.md`, `src/plugins/graph/PLAN.md`, `docs/lsp.md`

Do: rtok's own extra tags queries appended to the grammar queries (no new crate — queries are data). `RUST_EXTRA_REF`: bare `type_identifier`, `scoped_type_identifier` path, and both `scoped_identifier` arms so every `a::b` segment (`plugin` in `crate::plugin::Surface::Mcp`, `Registry` in `Registry::new(..)`, `store`/`Store` in `use …`) counts as a reference; `self`/`crate`/`super` never match (own node types). `TS_CALL_TYPE_REF`: plain/member/nested-member calls, member constructions, bare `type_identifier` (generic args the `type_annotation` arm misses), namespace modules. No post-filter needed: tree-sitter-tags keeps one tag per node with the earlier pattern winning, and rtok's extras come last (verified: no doubles, no def-line self-refs on the constructs fixture). Extractor fingerprint hashes both strings (stale roots re-index, T35.5).
Check: `reference_capture_matches_the_known_misses` (OnlyTyped/Recv/outer/middle/leaf + TS rows flip to hit; macro bodies stay missed), new `new_constructs_group_under_the_enclosing_definition` (Rust groups under `user`, TS at file level — the upstream TS query has no `function_declaration` defs, noted as follow-up), `tags_backend_hits_onlytyped_type_position` + new `tags_backend_misses_macro_body` (the old P30 gate pin is a tags hit now; macro bodies are the discriminating fixture), `graph_contract` byte-exact, lib graph/outline/store 46 green, `plugins_e2e` + `import_index_e2e` green, min-feature build green, fmt/clippy clean on touched files.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: T8.8 rescore `cargo test -p rtok --test graph_truth` — defs 40/40 (1.000/1.000), refs 96/105 recall 0.914 (was 32/105, 0.305), overall 136/145 = 0.938; all 9 remaining misses are macro-body call sites (opaque `token_tree`, query-unreachable ceiling); recorded in `research.md` §2. Regression floor in the test raised 0.30 → 0.85.
Deviation: 7 files (≤3) — the behavior change flips a P30 gate pin, so the gate test, its `docs/lsp.md` line and the `PLAN.md` Known-misses/P30 notes move in the same commit; `plan.md`/`todo.md`/`done.md` moves are bookkeeping.

## T48.3 — Cursor plugin MCP goes through the ketch-hint launcher

**T48.3 Cursor plugin MCP goes through the ketch-hint launcher** · P1, 2/5 · `plugins/cursor/mcp.json`, `plugins/cursor/scripts/mcp.cmd`, `tests/cursor_plugin.rs`

From I-37. The bundle `mcp.json` ran `rtok mcp` directly, so the ketch-hint launchers never ran and a missing binary was a silent MCP failure; it also lacked the Agent Plugins `$schema`/`type`.
Do: `mcp.json` is now closed-spec-conformant (`$schema` `mcp.schema.json`, one server `type: "stdio"`) with `command` `./scripts/mcp.cmd` — one launcher Cursor resolves (single plugin-relative token per spec §7.2.1; the spec explicitly allows a client interpreter for `.cmd` on Windows). `mcp.cmd` gained a 2-line sh preamble (`#!/bin/sh` + `exec` sibling `mcp.sh`) +x, so the same file runs on macOS/Linux (verified: direct kernel exec, missing-rtok → ketch hint exit 1) and Windows (cmd skips the preamble as noise, runs the unchanged batch body). Root `plugin.json` already conforms (closed-schema fields `$schema`/name/version/description only), so it stays — no drop, no README reason owed.
Check: `d21_mcp_json_invokes_launcher_not_rtok_directly` (`$schema`, `type`, launcher path, no args), `d21_no_duplicate_call_paths` (launcher + still no read/search duplication), new unix `d21_bundle_launcher_names_ketch_when_rtok_missing` (execs the bundle launcher with rtok missing → ketch hint).
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: clean-worktree run at 77af448 + these 3 files: `cargo nextest run --test cursor_plugin` 10 passed; `cargo fmt --check` clean; `cargo clippy --test cursor_plugin -D warnings` clean. Full `just check` not run (workspace-wide; main tree holds other agents' uncommitted work).
Deviation: 3 files; bundle README line `mcp.json — mcpServers.rtok → rtok mcp` now routes via the launcher — one-line doc touch deferred to keep the 3-file limit. The plain-install `~/.cursor/mcp.json` entry (`register_mcp`, bare `rtok`) is unchanged: out of this card's scope (plugin bundle only).

## T48.4 — DeepSeek Harness host

**T48.4 DeepSeek Harness host** · P2, 4/5 · `plan.md`, `todo.md`, `done.md` (no code: the card's escape clause)

Do: verified the official sources (fetched 2026-09-17) and closed the card with evidence instead of code. DeepSeek Harness (`dsh`, https://github.com/deepseek-ai/deepseek-harness) is an open-source agent harness in developer preview — its README says "THERE WILL BE COMPATIBILITY-BREAKING CHANGES". It has no stable user-level config surface rtok could install into: models are configured through the Web UI Settings → Models form into `$DSH_HOME/settings.yaml` with keys in `$DSH_HOME/.credentials.yaml` (https://deepseek-harness.github.io/deepseek-harness/en/guide/providers); MCP servers attach as Cordis overlay YAML patches passed per-run as `dsh web --patch …` or merged by hand into `$DSH_HOME/cordis.patch.yml` ("do not copy over an existing file: it may already contain unrelated user patches"), as `@deepseek-ai/dsh-mcp-client` plugin rows (`serverName`/`transport`/`command`), not a server map rtok could merge into (https://deepseek-harness.github.io/deepseek-harness/en/guide/mcp-memory); no shell-hook event protocol `rtok hook` could serve is documented. Re-check when the harness leaves preview with a versioned config schema: then MCP is one `dsh-mcp-client` row (`serverName: rtok`, stdio `rtok mcp`) and providers already speak `anthropic-messages`, so the proxy is a base-URL away.
Check: the card's escape clause ("If the harness has no stable config surface, the card closes with that evidence instead of code").
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: five official pages fetched and linked above (API docs agent-integration note, repo README, quickstart, providers, mcp-memory); no repo file besides the plan trio changed (`git status` clean apart from them).
Deviation: none; evidence-close, no code, no new host in the registry, config, docs or e2e matrix.

## T48.2 — pi install hint reaches the model

**T48.2 pi install hint reaches the model** · P1, 1/5 · `plugins/pi/extensions/rtok.ts`, `plugins/pi/tests/rtok.test.ts`, `plugins/pi/README.md`

From I-36. `pi.appendEntry` is TUI-only (pi docs: "do NOT participate in LLM context"), so the missing-rtok ketch hint never reached the model.

Do: the extension now calls `pi.sendMessage({customType: "rtok-missing", content: KETCH_HINT, display: true})` once per session (module guard flag; falls back to `appendEntry` only when `sendMessage` is missing for old pi). Fail-open holds: the bash command still runs unchanged.
Check: `plugins/pi/tests/rtok.test.ts` asserts one `sendMessage` with the ketch hint, once per session across two calls, zero `appendEntry` when `sendMessage` exists, plus the fallback case; `plugins/pi/README.md` states the `sendMessage` path.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3
Evidence: `node --test plugins/pi/tests/rtok.test.ts` 6 passed; `cargo nextest run --test pi_plugin` 5 passed. `just check` not green in this tree: `cargo clippy --all-features` fails on uncommitted concurrent work (`src/plugins/graph/mod.rs` T52.4 `symbol_dead_candidates` errors), untouched by this task; `cargo fmt --check` shows the same pre-existing diff.
Deviation: none; 3 files, ≤200 LOC.

## T48.1 — pi loads the rtok extension from its linked directory

**T48.1 pi loads the rtok extension from its linked directory** · P0, 2/5 · `plugins/pi/tests/load.test.ts`, `tests/pi_plugin.rs`, `plugins/pi/README.md`

From I-35. The pi docs list only `extensions/*.ts` and `extensions/*/index.ts`, so the `extensions/rtok` link (no `index.ts`) looked unloaded.

Do: read pi 0.85.1's loader (`dist/core/extensions/loader.js`): `discoverExtensionsInDir` follows a symlinked subdirectory and `resolveExtensionEntries` reads its `package.json` `pi.extensions` before `index.ts`. The link already loads, so no `index.ts` was added (it would be dead code). Added a Node test that calls pi's own `discoverAndLoadExtensions` on the linked directory; `tests/pi_plugin.rs` runs it right after `rtok agents install pi --yes` against the real installed dir. README states the real loading rule.

Check: the load test expects no errors, exactly one extension from `extensions/rtok/extensions/rtok.ts` with `tool_call` and `tool_result` handlers; skipped when pi is not on PATH. Negative control: with `pi.extensions` emptied the test fails (0 pass, 1 fail).

Status: done 2026-09-17 · Model: Claude Code / Fable 5.1

Evidence: `cargo test --test pi_plugin` 5 passed; `just check` 716 passed, 2 skipped, jscpd 70 clones.

Deviation: none; the idea's premise (missing `index.ts` breaks loading) was wrong for pi 0.85.1, so the fix is a proof plus docs, not a new entry file.

## T47.5 — Host plugin Node tests on every OS

**T47.5 Host plugin Node tests on every OS** · P2, 2/5 · `tests/node/fake-rtok.ts` (new), `plugins/pi/tests/rtok.test.ts`, `plugins/opencode/rtok.test.ts`, `plugins/pi/README.md`
Do: the pi and OpenCode unit tests skipped on Windows because their fake `rtok` was a `sh` script, and Node's `execFile`/`spawnSync` only find `.exe` there. `tests/node/fake-rtok.ts` links (or copies) the running `node` binary as `rtok[.exe]` once per process and points `NODE_OPTIONS=--require` at a per-case script that plays `rtok` with `args` and `input` in scope; node answers `--version` itself. `fakeRtok(null)` puts an empty dir on PATH. Both tests use the one helper, so no case is skipped on any OS.
Check: `tests/pi_plugin.rs::pi_extension_unit_test_with_fake_rtok`, `tests/filter.rs::opencode_plugin_unit_test_with_api_mock`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: Node pi 5/5 and OpenCode 5/5 with 0 skipped on macOS; `just check` exit 0 (716 passed). Not run on Windows here.
Deviation: 4 files (≤3) — one shared helper replaces a fake in each of the two tests, and the pi README names it.

## T45.6 — Extra hook/expand/guard/read/toon coverage

**T45.6 Extra hook/expand/guard/read/toon coverage** · P2, 2/5 · `tests/extra_cover.rs` (new)
Do: nine cases through the binary and the library: hook fail-open on garbage and empty stdin, `expand` of an unknown id with `--lines`, the 11-row `plugins` listing, PreToolUse rewrite plus deny-wins merge, a guard deny naming an expandable id, the read cap marker within `max_chars`, toon comma-cell round-trip. The `collapsible_if` lint left by T45.4 is gone.
Check: `cargo test --test extra_cover` (9); clippy `-D warnings` on all targets.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3 (code); closed by Claude Code / Fable 5.1
Evidence: the file landed inside `a423aca`; `just check` exit 0 (716 passed, clippy clean).
Deviation: committed by another agent's snapshot commit (`a423aca`), not as its own `T45.6` commit; this commit only moves the task to done.md.

## T45.5 — Gates cover tests, webui and examples

**T45.5 Gates cover tests, webui and examples** · P2, 2/5 · `.jscpd.json`, `examples/mcp_tool.rs`, `tests/trycmd/config-show.*`
Do: jscpd now scans `crates/rtok-plugin-sdk`, `crates/rtok-agent-sdk`, `crates/rtok-webui` and the mirrored tests (`config_coverage.rs`, `surface_parity.rs`) besides `src/`, with the gate still green. `examples/mcp_tool.rs` records one `Measurement` per tool call and asserts exactly one row, like `hello_plugin`. `config show` has a trycmd snapshot.
Check: `just dup` (jscpd) exit 0; `cargo run --example mcp_tool`; `tests/cli_trycmd.rs` with `config-show`.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3 (code); closed by Claude Code / Fable 5.1
Evidence: the code landed inside `a423aca`; `just check` exit 0 (716 passed, jscpd 70 clones under threshold).
Deviation: committed by another agent's snapshot commit (`a423aca`), not as its own `T45.5` commit; this commit only moves the task to done.md.

## T45.2 — Proxy cache hits and errors keep usage rows

**T45.2 Proxy cache hits and errors keep usage rows** · P0, 3/5 · `src/proxy/mod.rs`, `src/proxy/semantic_cache.rs`, `tests/proxy.rs`
Do: every proxied request owes one `usage` row (T5.1). A semantic-cache hit now writes the usage and provider token rows with the request bytes, beside its measurement and `call_io`; an upstream error or a non-2xx / no-usage response writes a minimal usage row instead of none.
Check: `tests/proxy.rs::proxy_cache_hit_records_usage_and_request_bytes`, `proxy_upstream_error_still_records_usage_row`.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3 (code); closed by Claude Code / Fable 5.1
Evidence: the code landed inside `a423aca`; `just check` exit 0 after T47.4 (716 passed), which runs `tests/proxy.rs`.
Deviation: committed by another agent's snapshot commit (`a423aca`), not as its own `T45.2` commit; this commit only moves the task to done.md.

## T45.1 — OTel flush survives a traces error

**T45.1 OTel flush survives a traces error** · P0, 2/5 · `src/otel/export.rs`, `tests/otel.rs`
Do: `flush_into` no longer aborts the whole flush on a non-404 traces POST error. Each stream's `post` result is matched on its own: the error is kept in `rep.error` (joined when several streams fail), the traces mark stays, and logs and metrics still post and advance in the same round.
Check: `tests/otel.rs::traces_500_still_posts_logs_and_metrics` beside `a_traces_404_still_posts_logs_and_metrics`.
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3 (code); closed by Claude Code / Fable 5.1
Evidence: the code landed inside `a423aca`; `just check` exit 0 after T47.4 (716 passed), which runs `tests/otel.rs`.
Deviation: committed by another agent's snapshot commit (`a423aca`), not as its own `T45.1` commit; this commit only moves the task to done.md.

## T45.3 — Archive decisions scoped per session

**T45.3 Archive decisions scoped per session** · P1, 3/5 · `migrations/0014.sql`, `src/store/mod.rs`, `src/expand.rs`, `src/plugin.rs`
Do: `archive_decisions` was keyed by bare `tool_use_id` while reads and writes scoped by `(session, tool_use_id)`, so a repeated id in a second session hit `INSERT OR IGNORE` and was never stored, and `mark_expanded` froze every session. `0014.sql` rebuilds the table with `PRIMARY KEY (session, tool_use_id)` and an `archive_id` index; `mark_expanded(session, id)` and `live_zone_pointer(session, id)` filter by session, and `expand` passes the calling session.
Check: `store::tests::archive_decision_repeated_id_persists_per_session` (same id, two sessions, two pointers; a third session sees none); `store::tests::expand_in_one_session_does_not_freeze_another` (`mark_expanded` 1 then 0, only session a expanded, counts `(2, 1)`).
Status: done 2026-09-17 · Model: OpenCode / Muse Spark 1.3 (migration, callers); Claude Code / Fable 5.1 (store methods in T47.4, tests)
Evidence: both tests pass; `just check` exit 0 (716 passed).
Deviation: the migration and callers landed inside `a423aca`, the store methods in T47.4; this commit adds the two tests the card asked for and closes the task.

## T47.3 — Unit and e2e tests for every host plugin

**T47.3 Unit and e2e tests for every host plugin** · P1, 3/5 · `plugins/pi/tests/rtok.test.ts` (new), `plugins/opencode/rtok.test.ts`, `tests/opencode_plugin.rs` (new), `tests/pi_plugin.rs`, `plugins/pi/README.md`
Do: each host plugin now has a unit test of its own file and an e2e through the binary. pi had none of its own: `plugins/pi/tests/rtok.test.ts` (`node:test`, no dependency) loads the extension against a stub `pi` and a fake `rtok` shell script first on PATH — a bash call becomes one single-quoted `rtok run -- '…'` and is never wrapped twice, other tools are untouched, a missing `rtok` leaves the command and the result alone and appends the ketch hint once, a shorter `rtok filter` result replaces the bash output, and unchanged or empty filter output keeps the original; it lives outside `extensions/` so pi never loads it, and `tests/pi_plugin.rs::pi_extension_unit_test_with_fake_rtok` runs it. OpenCode's unit test gains `filterStdin` itself: the argv `filter --stdin --cmd <command>` and stdin reach `rtok`, a non-zero exit fails open, and a missing `rtok` fails open and prints the ketch hint once per process. `tests/opencode_plugin.rs` is the OpenCode D21 e2e, mirroring `cursor_plugin.rs`: the plugin registers no tool and no `tool.execute.before`; `--dry-run` offers `plugins/opencode/rtok.ts` at `<config dir>/plugins/rtok.ts` with the ketch hint and writes nothing; a plain install writes `mcp.rtok` and leaves the offer open; `--yes` links exactly one file that reads as the source, keeps a foreign MCP server, says `already installed` the second time; remove unlinks it, drops `mcp.rtok`, keeps the foreign server and the source. Cursor already had both (`cursor_plugin.rs`: manifests, `scripts/mcp.sh|cmd` ketch hint, link and unlink); its plugin is JSON and shell, so there is no script unit to add.
Check: `cargo test --test opencode_plugin --test pi_plugin --test cursor_plugin --test filter`; `just check`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `opencode_plugin` 4, `pi_plugin` 5, `cursor_plugin` 9, `filter` 2 passed; Node: pi 5/5, OpenCode 5/5; `just check` exit 0, 714 tests passed, jscpd clone count unchanged (70). The Node cases skip on Windows, where the fake `rtok` would need a `.cmd`.
Deviation: 5 files (≤3) — two plugins, each with a unit file and an e2e file, plus the pi README line for the new test file. The pi test sits in `plugins/pi/tests/`, not `extensions/` as the card planned, so pi cannot load it as an extension.

## T47.2 — Remove and list e2e for every host

**T47.2 Remove and list e2e for every host** · P1, 2/5 · `tests/agent_remove.rs`, `tests/agents_install.rs`
Do: `tests/agent_remove.rs` covered claude, cursor, codex, opencode and pi; the three newer hosts get their own install → remove cases with foreign entries seeded first. ZCode: a foreign `hooks.events.Stop` chain and `mcp.servers.foreign` survive, every `hook` command and `mcp.servers.rtok` go. Kimi Code: the `# mine` comment and a foreign `[[hooks]]` table survive in `config.toml`, nothing named rtok is left, `mcp.json` keeps `foreign` and loses `rtok`. Copilot: `hooks/rtok.json` is deleted after one backup that still holds `version: 1`, a sibling `hooks/other.json` stays, `mcp-config.json` keeps `foreign`. Each ends with a second remove that says `no changes`. `tests/agents_install.rs` gains `list_reports_installed_modules_per_host`: over the eight-host matrix, `agents list` shows no `✓` module row before install, at least one after, none after remove, in the blocks whose `config` line names the host's file (pi, which edits no file, by its `CLI: pi` header).
Check: `cargo test --test agent_remove --test agents_install`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `agent_remove` 10 passed, `agents_install` 9 passed; clippy `-D warnings` on rtok tests clean; fmt clean.

## T47.4 — Repair main after the swept snapshot commit

**T47.4 Repair main after the swept snapshot commit** · P0, 3/5 · `src/store/mod.rs`, `AGENTS.md`, `src/agents/{claude,codex,cursor,pi}/README.md`, `src/plugins/graph/lsp.rs`, `tests/plugins_e2e.rs`, `src/mcp.rs`, `src/demon.rs`, `src/proxy/live.rs`, `src/tui/view.rs`, `src/web/model.rs`
Do: `a423aca` committed a working tree in which a broken rename script (T47.1) had emptied files and a restore from the previous HEAD had dropped uncommitted edits, so `main` did not compile and `just check` was red. Store: migrations `0013.sql` and `0014.sql` are registered; `mark_expanded(session, archive_id)` and `live_zone_pointer(session, archive_id)` are scoped per session, as `0014.sql` keys `archive_decisions` by `(session, tool_use_id)` and the callers in `expand.rs` / `plugin.rs` already passed the session; `recent_session_totals(since, limit)` is `session_totals` with an SQL `LIMIT` (`session_totals` delegates with `-1`). T44.6 leftovers: the `## Docs` sections of the claude, codex, cursor and pi host READMEs and the AGENTS.md D21 sentence are back, taken from the T44.6 session's own edits. Regressions in the swept T45.x work: `graph` `outline` keys the LSP session by cwd only when the file sits under cwd and its nearest manifest picks the same server (a Dart package outside a Cargo cwd went to rust-analyzer and outlined empty); `read_dedup_on_second_mcp_read` sends both reads down one `rtok mcp` stdin, since one process is now one session; the min-feature build no longer warns on an `anyhow` import used only under `memory`. Flaky unit tests: the TUI and web tests that clear and fill the process-wide live-call ring take `proxy::live::test_lock()`; the demon lock-release test polls for 500 ms, because a child spawned by a parallel test can hold the inherited lock fd between fork and exec.
Check: `just check` — fmt, clippy `-D warnings` (workspace and wasm guest), nextest, min-feature build, jscpd.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `just check` exit 0, 705 tests passed, 2 skipped; `cargo test --lib` three runs in a row green; `graph_lsp_gate` 5 passed with `dart` on PATH; `host_docs` passed.
Deviation: 12 files (≤3) and not claimed in `plan.md` first: the repair unblocks every open task on `main`. The T45.x rows stay `in progress` under their agent; this commit only makes their swept code compile and pass.

## T47.1 — Rename `rtok agents setup` to `rtok agents install`

**T47.1 Rename `rtok agents setup` to `rtok agents install`** · P1, 2/5 · `src/cli.rs`, `tests/agents_install.rs` (renamed), docs, READMEs and tests that name the command; `plugins/{cursor,pi,opencode}/README.md`
Do: the host installer is `rtok agents install <host>`; `setup` stays a clap alias, and the deprecated top-level `rtok setup` points to `rtok agents install`. Every doc, README, help snapshot and test says `agents install` (done.md history keeps the old spelling). `tests/agents_setup.rs` is `tests/agents_install.rs`. The plugin READMEs for Cursor, pi and OpenCode are rebuilt with the new command and their `## Docs` lists.
Check: `cli_trycmd`, `agents_install`, `agent_remove`, `cursor_plugin`, `pi_plugin`, `surface_parity`, `config_coverage`, `host_docs`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `just check` exit 0 (705 passed), which runs every test above.
Deviation: the code rename landed inside another agent's commit `a423aca`, together with three plugin READMEs my rename script had emptied; this commit restores those READMEs and closes the task.

## T44.6 — Host plugin docs links

**T44.6 Host plugin docs links** · P1, 2/5 · `plugins/{cursor,pi,opencode}/README.md`, `src/agents/{claude,codex,cursor,opencode,pi}/README.md`, `plugins/pi/{package.json,skills/rtok/SKILL.md}`, `tests/host_docs.rs`, `AGENTS.md`, `ideas.md`
Do: rule (AGENTS.md, D21 line): every `plugins/<host>/README.md` and `src/agents/<host>/README.md` carries a `## Docs` list linking the host's current config and plugin documentation, re-verified on each change. Every link was fetched on 2026-09-17: Cursor plugins / manifest reference / hooks / MCP, Agent Plugins spec, pi extensions / packages / skills, OpenCode config / MCP / plugins, Codex config reference and MCP (developers.openai.com now 308-redirects to learn.chatgpt.com), Claude Code hooks / settings-reference#env / MCP, Claude Desktop via modelcontextprotocol.io. Fixes the audit found: the OpenCode plugin dir is `~/.config/opencode/plugins/` (README said `plugin/`); pi `SKILL.md` got the frontmatter the skills spec requires (`name`, `description`) and `package.json` a `pi.skills` entry (a `pi` manifest disables default discovery, so the skill was never loaded). Findings not changed, filed as I-35..I-37 in `ideas.md`: pi loads `extensions/*/index.ts` and the linked dir has none; `pi.appendEntry` never reaches the model; Cursor `mcp.json` bypasses `scripts/mcp.sh|cmd`.
Check: `tests/host_docs.rs::every_host_readme_links_its_docs` — every dir under `plugins/` and `src/agents/` has a README with a `## Docs` section and at least one `- … https://` bullet.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo test --test host_docs --test pi_plugin --test cursor_plugin` 14 passed; rustfmt clean on the new test. `agents::tests::readme_tables_match_support` fails on OpenCode's `Reachable:` line because of the uncommitted T44.5 `src/agents/mod.rs` (mcp + plugin now reach `cmd`, `read`, `memory`, `graph`); the `## Docs` sections are not read by that test.
Deviation: 13 files — the rule is one section per host, and the audit fixes are one-liners in the pi bundle.

## T46.4 — Copilot CLI and GitHub Copilot app host

**T46.4 Copilot CLI and GitHub Copilot app host** · P1, 3/5 · `src/agents/copilot/{mod.rs,README.md}` (new), `src/agents/mod.rs`, `crates/rtok-agent-sdk/src/lib.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `src/cli.rs`, `README.md`, `site/content/docs/commands.md`, `tests/agents_setup.rs`, `tests/common/agents.rs`
Do: `rtok agents setup copilot` installs into GitHub Copilot CLI (`copilot`) and the GitHub Copilot desktop app as one shared host (`shared()`, Cursor pattern; app bundle `/Applications/GitHub Copilot.app`, `$LOCALAPPDATA/Programs/GitHub Copilot/GitHub Copilot.exe`). One key, `[setup.copilot] dir` (default `~/.copilot`), holds both files: `mcp-config.json` gets `mcpServers.rtok = {type: "local", command, args: ["mcp"], tools: ["*"]}` through the SDK's `register_server`; `hooks/rtok.json` is rtok's own file — `{version: 1, hooks: {preToolUse, postToolUse, userPromptSubmitted, sessionStart, sessionEnd: [{type: "command", bash, powershell, timeoutSec}]}}`, each command `rtok hook <ClaudeEvent> --host copilot` (T46.3) — written as a whole document (identical file = no change), deleted on `remove` after the usual backup; `Apply::writes` is now `pub` so that delete honours dry-run the same way writes do. Support: hooks `yes` on the CLI, `no` on the app (undocumented there), mcp `yes`, proxy `no` (BYOK is env-only), plugin `no` (`installed-plugins/` is the marketplace's). The e2e matrix gains copilot with `hooks/rtok.json` as the backup-checked file.
Check: `agents::copilot::tests::dry_run_names_the_hook_file_and_creates_nothing`; `apply_writes_five_events_is_idempotent_and_remove_deletes` (`version == 1`, five events, `preToolUse[0].bash` ends with `rtok hook PreToolUse --host copilot`, `powershell` equal, `timeoutSec == 5`, second apply `no changes`, remove reports `- <path>` and the file is gone); `mcp_entry_is_local_with_all_tools_and_the_app_reports_no_hooks`; `readme_tables_match_support` per kind (cli: hooks + mcp; desktop: mcp only); `config::tests` path leaves; `tests/agents_setup.rs` over eight hosts.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo test --lib -- agents:: config::` 118 passed; `agents_setup` 8, `agent_remove` 7, `host_docs`, `cli_trycmd`, `config_coverage`, `surface_parity` 4 all green; clippy `-D warnings` on rtok + rtok-agent-sdk clean; fmt clean. Not verified on a live install: Copilot's tool ids, the app bundle paths, whether the app runs `hooks/*.json` at all.
Deviation: 12 files (≤3) — a new host touches the registry, the config section, its docs and the e2e matrix by construction. `tests/trycmd/config-show.stdout` (another agent's uncommitted T45.5 file) got the `setup.copilot.dir` line in the tree but is not in this commit.

## T46.3 — Copilot hook payload map

**T46.3 Copilot hook payload map** · P1, 2/5 · `src/hooks/types.rs`, `src/hooks/mod.rs`, `src/cli.rs`, `config/default.toml`, `docs/config.md`
Do: `rtok hook <Event> --host copilot` (or `[hook] host = "copilot"`) reads GitHub Copilot CLI's camelCase stdin as the Claude event and answers in Copilot's flat stdout. `HookInput::adapt_copilot` maps `sessionId` → `session_id`, `toolName` → `tool_name`, `toolArgs` → `tool_input`, `toolResult` → `tool_response`, takes the event from the CLI argument (Copilot's own camelCase names are accepted too: `preToolUse`, `postToolUse`, `sessionStart`, `sessionEnd`, `userPromptSubmitted`), and renames the tool so the plugins match: anything with `bash`/`shell`/`terminal`/`powershell` in it becomes `Bash`, `read*`/`view*` become `Read`; unmapped fields stay in `extra`. `copilot_output` turns the Claude `HookOutput` into `{permissionDecision, permissionDecisionReason, modifiedArgs}` (from `hookSpecificOutput.updatedInput`) or `{additionalContext}`; a top-level `decision: block` becomes `permissionDecision: deny`; `{}` stays `{}`. Wired in `dispatch_owned_strict` beside the Cursor branch, after the normal dispatch, so every plugin path is unchanged. `--host` docs and the `[hook] host` comments list `copilot`.
Check: `hooks::types::tests::copilot_pre_tool_use_maps_camel_case_and_tool_names` (`run_in_terminal` → `Bash`, `view` → `Read`, `timestamp` kept in `extra`, `pre_tool()`/`post_tool()` views resolve); `hooks::tests::copilot_output_shapes_pre_post_block_and_empty`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `mise exec -- cargo test -p rtok --lib -- hooks::` 18 passed; clippy `-D warnings` clean; fmt clean. Not verified on a live Copilot CLI: its exact tool names (the docs name `sessionId`, `toolName`, `toolArgs` and the stdout keys, not the tool ids), so the rename is by substring.
Deviation: 5 files (≤3) — the three config/CLI doc lines are one-word mentions of the new host value.

## T46.2 — Kimi Code host

**T46.2 Kimi Code host** · P1, 3/5 · `src/agents/kimi/{mod.rs,README.md}` (new), `src/agents/claude/mod.rs`, `src/agents/mod.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `src/cli.rs`, `README.md`, `site/content/docs/commands.md`, `tests/agents_setup.rs`, `tests/common/agents.rs`
Do: `rtok agents setup kimi` installs into Moonshot's Kimi Code CLI (`kimi`; `[setup.kimi] config_path`, default `~/.kimi-code/config.toml`, with `mcp.json` read beside it). Hooks are `[[hooks]]` tables — `event`, `matcher`, `command = "rtok hook <event>"`, `timeout` in seconds — for all eight Claude entries (Kimi documents every one of those events), written through `toml_edit` so comments and foreign hooks survive; ours are recognised by Claude's `is_ours` (now `pub(super)`), so a user chain that merely contains `rtok hook` is left alone on remove, and an empty `hooks` array goes with the last entry. MCP is `mcpServers.rtok = {command, args}` in `mcp.json` through the SDK's `register_server`, without the `type` field the Kimi docs do not show. Proxy and plugin are `no` (`[providers.<name>]` tables carry keys; `plugins/managed/` belongs to `kimi plugin install`). The README says `updatedInput` on PreToolUse is not in the Kimi docs, so the `cmd` rewrite path is unverified there and deny is the promised path.
Check: `agents::kimi::tests::dry_run_names_eight_tables_and_touches_nothing`; `apply_keeps_comments_and_foreign_hooks_and_is_idempotent` (comment and `echo other` kept, nine tables, `timeout = 5`, second apply `no changes`, `8 removed`, nothing named rtok left); `mcp_lands_beside_the_config_without_a_type_field`; `readme_tables_match_support`; `config::tests` path leaves; `tests/agents_setup.rs` over seven hosts.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo test --lib -- agents:: config::` 115 passed; `agents_setup`, `agent_remove`, `host_docs`, `cli_trycmd`, `config_coverage`, `surface_parity` all green; clippy `-D warnings` on rtok + rtok-agent-sdk clean; fmt clean; jscpd exit 0. Not verified on a live Kimi install: `updatedInput` on PreToolUse (docs list `permissionDecision` only).
Deviation: 12 files (≤3) — a new host touches the registry, the config section, its docs and the e2e matrix by construction. `tests/trycmd/config-show.stdout` (another agent's uncommitted T45.5 file) got the `setup.kimi.config_path` line in the tree but is not in this commit.

## T46.1 — ZCode host

**T46.1 ZCode host** · P1, 3/5 · `src/agents/zcode/{mod.rs,README.md}` (new), `src/agents/claude/mod.rs`, `src/agents/mod.rs`, `crates/rtok-agent-sdk/src/lib.rs`, `src/config/mod.rs`, `config/default.toml`, `docs/config.md`, `src/cli.rs`, `README.md`, `site/content/docs/commands.md`, `tests/agents_setup.rs`, `tests/common/agents.rs`
Do: `rtok agents setup zcode` installs into Z.ai's ZCode desktop app (one Desktop variant: `/Applications/ZCode.app`, `$LOCALAPPDATA/Programs/ZCode/ZCode.exe`; `[setup.zcode] config_path`, default `~/.zcode/cli/config.json`). Hooks reuse Claude's entry helpers, now `pub(super)` and parameterised (`insert_ours(hooks, entries, bin, timeout_key, timeout)`, `strip_ours(Option<&mut Value>)`, `desktop_command()`): ZCode gets `hooks.enabled = true` and the first five Claude entries (PreToolUse Bash/Read, PostToolUse, UserPromptSubmit, SessionStart — ZCode has no PreCompact, PostCompact or SessionEnd) under `hooks.events` with `timeoutMs = hook_timeout_s * 1000` and the absolute binary, since the app starts without a shell PATH. MCP is `mcp.servers.rtok = {command: <abs rtok>, args: ["mcp"]}` through the SDK's `register_server`/`unregister_server`, which now walk a dotted key. `is_rtok_bin` also accepts the running executable by its exact path (`cargo test` names it `rtok-<hash>`), so an absolute command reads back as ours in tests too. Proxy and plugin are `no` (providers are per-id tables with keys; plugins come from the marketplace). The e2e matrix in `tests/agents_setup.rs` gains zcode, and opencode now takes `--yes` there — since T44.5 it offers a plugin, and an unanswered offer never reads as `already installed`.
Check: `agents::zcode::tests::dry_run_names_five_entries_and_creates_nothing` (report starts with `+ hooks.enabled = true`, `5 additions`, no SessionEnd, no file); `apply_is_idempotent_and_remove_keeps_foreign` (`hooks.enabled` true, `PreToolUse[0].matcher == Bash`, `timeoutMs == 5000`, second apply `no changes`, `installed()` = hooks + mcp, remove strips ours and keeps `echo other` and the foreign `mcp.servers.other`); `readme_tables_match_support` on the new README (Reachable: measure, cmd, read, archive, inject, guard, memory, graph, toon; not proxy, compress); `config::tests` path leaves; `tests/agents_setup.rs` over six hosts.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo test --lib -- agents::` 52 passed; `agents_setup`, `agent_remove`, `cursor_plugin`, `pi_plugin`, `filter`, `host_docs`, `cli_trycmd`, `surface_parity` 36 passed; `config_coverage` 1 passed; `rtok-agent-sdk` 15 passed; clippy `-D warnings` on rtok + rtok-agent-sdk clean; fmt clean; jscpd exit 0. Not verified on a live ZCode install: the bundle paths and whether its shell tool is named `Bash` (the matchers follow https://zcode.z.ai/en/docs/hooks).
Deviation: 13 files (≤3) — a new host touches the registry, the config section, its docs and the e2e matrix by construction; the Claude helper generalisation is the reuse the task asked for. `tests/trycmd/config-show.stdout` (another agent's uncommitted T45.5 file) got the `setup.zcode.config_path` line in the tree but is not in this commit.

## T44.5 — OpenCode CLI+Desktop MCP and plugin parity with Cursor

**T44.5 OpenCode CLI+Desktop MCP and plugin parity with Cursor** · P1, 3/5 · `src/agents/opencode/{mod.rs,README.md}`, `plugins/opencode/{rtok.ts,rtok.test.ts}` (moved from `hosts/opencode/`), `crates/rtok-agent-sdk/src/lib.rs`, `src/agents/mod.rs`, `tests/filter.rs`
Do: `rtok agents setup opencode` now installs three modules, one call path each (D21). `mcp.rtok` is written in OpenCode's own shape (`{type: "local", command: ["rtok", "mcp"], enabled: true}`, under `mcp`, not `mcpServers`) through the new SDK pair `register_server`/`unregister_server` (key + entry + summary); `register_mcp`/`unregister_mcp` became one-line wrappers, so nothing is duplicated. `offer_plugin` links `plugins/opencode/rtok.ts` to `<config dir>/plugins/rtok.ts` via `PluginLink` for the CLI and the desktop app separately (`for_kind` swaps `config_path`); `support` is proxy/mcp `yes`, plugin `--yes`, hooks `no`; `installed` reads `plugin` back from the link; `markers` adds the link. The plugin and the MCP entry are two capabilities (bash output via `rtok filter`, read/search/memory/graph via MCP), so there is no singleton clear here. The plugin prints the ketch install line once on ENOENT and still fails open. `hosts/opencode/` moved to `plugins/opencode/` so the release archive ships it (`Cargo.toml` includes `plugins/`); the SDK's `copy_owned` and `resolve_plugin_src`/`ketch_store_plugin` accept a single-file plugin (`exists()` instead of `is_dir()`).
Check: `agents::opencode::tests::mcp_entry_is_local_argv_idempotent_and_remove_keeps_foreign` (shape, `NO_CHANGES` on the second call, foreign server kept, `installed()` says `mcp`), `plugin_offer_links_one_file_beside_the_config` (dry run names `plugins/opencode` and `ketch install listepo/rtok`, `--yes` links one file, second apply no changes, remove unlinks), `desktop_variant_uses_its_own_config_dir`; `readme_tables_match_support` on the new rows (Reachable: measure, cmd, read, archive, proxy, memory, graph, toon, compress); `tests/filter.rs::opencode_plugin_unit_test_with_api_mock` follows the move.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo test --lib agents::` 50 passed; `filter`, `cursor_plugin`, `pi_plugin`, `agent_remove`, `host_docs` 23 passed; `rtok-agent-sdk` 15 passed; clippy `-D warnings` on rtok + rtok-agent-sdk clean; fmt clean; jscpd under the gate. `just check` over the whole workspace not run: the tree carries other agents' uncommitted edits in ~60 files.
Deviation: 7 files (≤3) — the move of the plugin and the SDK generalisation are what the task is; a per-file split would leave `support(plugin)` promising a link that does not exist. `src/agents/opencode/README.md` also carries the T44.6 Docs section that was uncommitted in the tree.

## T44.4 — Checks, e2e and platforms

**T44.4 Checks, e2e and platforms** · P1, 3/5 · `src/agents/mod.rs`, `src/agents/claude/mod.rs`, `tests/agents_setup.rs` (new), `tests/common/agents.rs` (new), `tests/common/mod.rs`, `tests/agent_remove.rs`, `.github/workflows/ci.yml`, `README.md`
Do: three checks around `rtok agents setup`. A setup whose configs would spawn a bare `rtok` that does not resolve on PATH prints `warning: rtok is not on PATH …` first (Windows writes the absolute exe and has nothing to warn about; remove never warns). After a non-dry install each variant is read back: `agents::expected` is every `Yes` module (`mcp` only with `[setup] mcp`) plus a flag module whose flag was given, `agents::missing` is that minus `installed()`, and each miss prints `  warning: <module> did not read back as installed` under the block. A run whose steps all report no changes removes the backups it took up front, so `already installed` and `no changes` leave the directory as they found it (content-identical copies were already skipped by T44.1). `run` split into the backup/warning frame and `apply_all`. The e2e helpers (`tmp`, `write_cfg`, `raw`, `rtok`, `json`, `backups`, `claude_desktop_config`) moved from `tests/agent_remove.rs` into `tests/common/agents.rs`, which also sets `APPDATA` to the temp home so Windows lands there too. A `windows` job (`windows-latest`, `continue-on-error: true`, nextest without fail-fast) joins `ci.yml`; it stays advisory because `revert-on-failure` reads job results.
Check: `tests/agents_setup.rs` over claude, cursor, codex, opencode and pi — setup twice → one backup, `already installed`, no `backup` line and identical module/plugin rows the second time; remove twice → `— no changes`, one backup; `--dry-run` → `dry run, nothing written`, no file created, no copy; `agent list|setup` equals `agents …`; `--cli`/`--desktop` pick the Cursor block and `--desktop` leaves OpenCode's CLI file alone; Claude Desktop under a temp home gets `mcpServers.rtok` with an absolute command next to a foreign server, `already installed` on the second run, gone after remove; `agents setup|remove windsurf` refused with no backup; a setup with an empty PATH starts with the warning. Unit: `expected_modules_follow_support_and_flags_and_missing_reads_them_back`, `expand_app_resolves_home_and_env_vars`, `#[cfg(windows)] desktop_path_lives_under_appdata_on_windows`.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo nextest run --lib agents` 47 passed; `agents_setup`, `agent_remove`, `cursor_plugin`, `pi_plugin` 28 passed; workspace run 691 passed with the same two failures outside this change as T44.3 (`plugins_e2e::read_dedup_on_second_mcp_read` from another agent's uncommitted MCP work, `graph_lsp_gate::lsp_backend_outlines_dart_main` dart LSP timing out locally); clippy `-D warnings` (check recipe) exit 0; fmt clean; jscpd 1.50 % < 2 %; `build-min` ok. The Windows job itself runs on the next push.
Deviation: 8 files — the helper move touches the old test, the new test and the shared module together.

## T44.3 — Plugins per app and Claude Desktop

**T44.3 Plugins per app and Claude Desktop** · P1, 3/5 · `src/agents/mod.rs`, `src/agents/claude/{mod.rs,README.md}`, `src/agents/{cursor,pi}/mod.rs`, `src/agents/{cursor,codex,opencode,pi}/README.md`
Do: every app block now ends with a `plugins` section that groups rtok's own plugins the way the modules are grouped — `✓ installed`, `✗ not installed`, `− not supported` — derived from each manifest's surfaces and the host's module rows: hooks carry `hook` and the bash call path (`cli`), MCP carries `mcp`, the proxy carries `proxy`, and the linked `plugin` module carries what `Agent::plugin_surfaces()` declares (Cursor: hook + mcp; pi: cli). A plugin is installed when one carrying module is installed, not installed when one is still available, not supported when none is; disabled plugins print `(off)`. Claude Desktop joins as the `Desktop` variant of `claude`: `claude_desktop_config.json` under `~/Library/Application Support/Claude` (macOS), `%APPDATA%\Claude` (Windows), `~/.config/Claude` (Linux); MCP only, written with the absolute `rtok` binary because the app starts without a shell PATH; hooks, proxy and plugin are `not supported` with their reasons, `--replace` acts as a plain install there. `support()` is now per kind, the Claude README carries `<module> (desktop)` rows and each host README a `Reachable:` / `Not reachable:` line (per kind where they differ) that the parity test checks against the catalogue. `module_lines` and `plugin_lines` share one `ModuleState::line` painter.
Check: `agents::tests::plugins_group_by_the_modules_that_carry_their_surfaces` (Codex with an MCP block: `read`/`graph` installed, `proxy`/`measure` not installed, `cmd`/`guard` not supported, `compress (off)`; `reaches` for pi and Cursor); `agents::claude::tests::desktop_writes_absolute_rtok_into_claude_desktop_config` (platform path, absolute command, register then unregister at a temp path); `readme_tables_match_support` now also fails when a README `Reachable:` line and the manifests disagree; `list_prints_one_block_per_app_with_kind_name_app_and_config` sees `Desktop: Claude Desktop` and the `plugins` section; `variant_filter_defaults_to_all` on the two `claude` variants.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo nextest run --lib agents` 45 passed; `agents`, `agent_remove`, `cursor_plugin`, `pi_plugin`, `surface_parity`, `config_coverage`, `cli_trycmd` 41 passed; workspace run 617 passed with two failures outside this change (`plugins_e2e::read_dedup_on_second_mcp_read` from another agent's uncommitted `src/mcp.rs` / `tests/plugins_e2e.rs` edits; `graph_lsp_gate::lsp_backend_outlines_dart_main`, the dart language server timing out on this machine); clippy `-D warnings` (check recipe) clean; fmt clean; jscpd 1.51 % < 2 %; `build-min` ok.
Deviation: 9 files (four of them one-line README sections) — the per-host READMEs are the parity test's input and had to move together.

## T44.2 — `rtok agents` with one folder per host

**T44.2 `rtok agents` with one folder per host** · P1, 4/5 · `src/agents/{mod.rs,claude/,cursor/,codex/,opencode/,pi/}` (moved from `src/setup/`), `src/cli.rs`, `src/doctor.rs`, `src/proxy/cli.rs`, `src/lib.rs`, tests, docs
Do: the command is `rtok agents` (`agent` stays as an alias); `--desktop` replaces `--gui` (kept as an alias). `src/setup/` moved to `src/agents/` with one folder per host: `<host>/mod.rs` implements the `Agent` contract — `variants()` (kind CLI/Desktop, display name, binaries, desktop app paths), `support(kind, module)` → `Yes | Flag("--proxy") | No(reason)`, `files()`, `markers()`, `installed()`, `apply(mode)` — and `<host>/README.md` carries the module table (which modules the host takes, which flag installs them, why the rest cannot be taken today). The five per-host `match` arms in `cli.rs` and the six string-keyed `match`es in `setup/mod.rs` collapsed into one generic `agents::run` loop: unknown hosts are refused before any backup, backups are taken once per file up front, then one block per selected app. The block is the new output for both `setup` and `list`: `CLI: Codex` / `Desktop: Cursor`, `app <path> (<version>)`, `config <files>`, the diff lines of steps that changed something, then `✓ hooks installed` / `✗ proxy not installed (--proxy)` / `− plugin not supported: <reason>`. A setup that changed nothing heads the block with `already installed` (remove keeps `no changes`); `--dry-run` says so; an absent app prints its header, app and config lines only. `rtok doctor` reads the same `ModuleRow`s (state plus note). Docs, site, `architecture.md`, help snapshot, parity and coverage gates renamed.
Check: `agents::tests::readme_tables_match_support` fails when a host README row and `support()` disagree (support cell and, for `no`, the very reason string); `list_prints_one_block_per_app_with_kind_name_app_and_config`; `a_flag_module_names_its_flag_and_an_unknown_bin_has_no_path_or_version`; `claude_modules_read_back_hooks_and_a_proxy_on_any_port` on the new rows; `present_*`/`absent_*` through the contract; `tests/cursor_plugin.rs` and `tests/pi_plugin.rs` now assert `already installed` on the second apply; `tests/agent_remove.rs` unchanged in substance (`agents` spelling); `tests/fixtures/graph_truth.toml` follows the moved files.
Status: done 2026-09-17 · Model: Claude Code / Fable 5.1
Evidence: `cargo nextest run --lib agents` 43 passed; `agent_remove`, `cursor_plugin`, `pi_plugin`, `agents`, `config_coverage`, `surface_parity`, `commands_e2e` 40 passed; `graph_truth` 3 passed after the fixture move; workspace run 558 passed with one unrelated failure (`graph_lsp_gate::lsp_backend_outlines_dart_main`, `dart language-server` outline timing out on this machine, graph code untouched); clippy `-D warnings` (check recipe) clean; fmt clean; jscpd 1.30 % < 2 %; `build-min` ok.
Deviation: far above ≤200 LOC / ≤3 files — a rename of a command plus a module move cannot land in pieces without a broken intermediate tree; the per-host folders are the task itself.

## T45.4 — Dead config keys and stale public numbers

**T45.4 Dead config keys and stale public numbers** · P2, 2/5 · `src/hooks/mod.rs`, `src/config/mod.rs`, `README.md`, `docs/comparison.md`
Do: `core.session_env`, `[hook] max_ms`/`fail_open` were declared and read nowhere. The hook now resolves the session as stdin id, then `$<core.session_env>` (empty key disables the fallback), else `unknown` (`resolve_session`); over-budget events print one `rtok: hook <event> slow` line when `[hook] max_ms` is non-zero (`slow_note`); `run` honours `[hook] fail_open` with a strict panic path for debugging behind `false`. README/`docs/comparison.md` factual rows fixed: 7 → 8 hook entries across 7 events, `rtok web` current with `dashboard` as the deprecated spelling, p95 8.25 ms cited to `research.md` §2 (Gate P17 serialized run 2026-09-09).
Check: every previously-dead key is read on the hook path (`session_env` at `src/hooks/mod.rs:87`, `max_ms` at `:129`, `fail_open` at `:21`); `cargo test -p rtok --lib config::` 60 passed; `cargo test -p rtok --lib hooks::` 16 passed (new `resolve_session_prefers_stdin_then_env`, `slow_note_fires_only_over_budget`, strict-path cases); `cargo test --test extra_cover` 9/9 still green; `cargo clippy -p rtok --lib -- -D warnings` clean.
Complexity: 2/5
Status: done 2026-09-16 · Model: OpenCode / Muse Spark 1.3
Evidence: config 60 passed / 0 failed; hooks 16 passed / 0 failed; extra_cover 9 passed / 0 failed; clippy lib zero warnings; `grep` confirms no documented key left unread.
Deviation: 4 source files instead of ≤3 (wiring lives in `src/hooks/mod.rs`, docs fixes in `README.md` + `docs/comparison.md`, comment/expansion touch-ups in `src/config/mod.rs`) — kept as one task because wiring and its docs are one behavior. Also collapses the `collapsible_if` at `src/hooks/mod.rs:47` that blocked the gate.

## T44.1 — backup once per content

**T44.1 Backup once per content** · P1, 1/5 · `crates/rtok-agent-sdk/src/lib.rs`
Do: `rtok agent setup` and `remove` copied every host file to `<name>.bak-<ts>` on every run, so an unchanged file collected one identical copy per run. `rtok_agent_sdk::backup` now reads the `<name>.bak-*` siblings first and returns `None` when one already holds the same bytes, whatever its suffix; changed content still gets its own copy, and another file's copies never count.
Check: unit test `backup_skips_when_an_identical_copy_exists_under_any_name` (same bytes → `None`; renamed identical copy → `None`; same size, different bytes → new copy; other file's copy ignored); `tests/agent_remove.rs` still green (setup then remove keeps two distinct copies, dry-run takes none).
Status: done 2026-09-16 · Model: Claude Code / Fable 5.1
Evidence: `cargo nextest run -p rtok-agent-sdk` 14 passed; `cargo nextest run --test agent_remove` 7 passed; fmt + clippy `-D warnings` clean on the crate.

## T43 — `rtok info`: version, config, store and proxy in one place

**T43 `rtok info`: version, config, store and proxy in one place** · P2, 2/5 · `src/info.rs` (new), `src/cli.rs`, `src/lib.rs`, `tests/commands_e2e.rs`, `tests/surface_parity.rs`, `tests/trycmd/help.stdout`
Do: `rtok doctor` reports the host chain and `rtok config show` reports every key, but no command answered "what does rtok use and how much disk does it take". New `src/info.rs` (`collect` + `to_text`, unit tests inside), `Cmd::Info` in `src/cli.rs` with `--json` only (already in the `config_coverage` allow-list, so no new config key), `pub mod info` in `src/lib.rs`, one `EXEMPT` row in `tests/surface_parity.rs` (helper: version, paths, disk usage, proxy status), e2e cases in `tests/commands_e2e.rs`.
Check: `rtok info` on a fresh home prints version, binary path + bytes, home, config path + bytes, db path + bytes, archive files + bytes, log lines + error count, proxy `host:port` + status, `otel off`; `rtok info --json` parses as JSON with the same fields in bytes; a missing DB prints `-` and still exits 0; unit tests in `src/info.rs` plus e2e cases in `tests/commands_e2e.rs` are green. `just check` green.
Complexity: 2/5
Status: done 2026-09-15 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test -p rtok --lib info` 4 passed; `cargo test --test commands_e2e info` 2 passed; fresh-`RTOK_HOME` probe prints every Check line and `--json` parses with `db.bytes: null`; `just check` green (fmt-check, workspace + guest clippy `-D warnings`, `cargo test --workspace` with lib 433 passed, build-min, jscpd 1.10% lines under threshold 2).
Deviation: two clippy lints in the new file (`collapsible-if`, `obfuscated-if-else`) plus a stale trycmd `help.stdout` snapshot missing the `info` line — all fixed inside the task scope so the Check's `just check` passes.

## T42 — module status in `agent setup` and `doctor`

**T42 Module status in `agent setup` and `doctor`** · P2, 2/5 · `src/setup/mod.rs`, `src/cli.rs`, `src/doctor.rs`
`rtok agent setup <host>` printed only the installer's diff, and `rtok doctor` counted Claude hooks only; neither said which rtok modules a host carries.
Do: after each host, `agent setup` prints every module (`hooks`, `mcp`, `proxy`, `plugin`) as green `✓ installed`, red `✗ not installed` or grey `− not supported`, for all five hosts; `rtok doctor` prints the same block per host variant under `agents` (plain words, no marks, in the text the PDF/HTML reports and the TUI embed). `setup::{MODULES, supported_modules, module_states, module_lines}` over `installed_modules`; `doctor::Report.agents` reads files only (no `--version` probe). Claude proxy detection now parses `env.ANTHROPIC_BASE_URL` against `[proxy] bind:port` instead of matching `8790` anywhere.
Check: a unit test reads a Claude settings file back as hooks ✓ / mcp ✗ / proxy ✓ (port 9123) / plugin −; `just check`.
Complexity: 2/5
Status: done 2026-09-15 · Model: Claude Code / Opus 5
Evidence: `claude_modules_read_back_hooks_and_a_proxy_on_any_port` green; fmt + workspace clippy `-D warnings` clean; `cargo test --workspace` 428 passed; build-min ok; jscpd within threshold; `agent setup claude --dry-run` and `doctor` in a throwaway `HOME` print the marks for every host.

## T40 — drop `demon list` and `demon update`

**T40 drop `demon list` and `demon update`** · P2, 1/5 · `src/cli.rs`, `src/demon.rs`, `tests/demon.rs`, `tests/surface_parity.rs`, `config/default.toml`
Do: both verbs duplicated another one. `list` was `status` over every service; `update` was `restart` plus a line comparing the binary *path*, which never changes when `ketch install` replaces the binary in place. Removed both, the `State.exe` field only `update` read, and their surface-parity rows; `status` with no names now shows every service. Approved by the creator 2026-09-14.
Check: `rtok demon list` / `rtok demon update` are clap errors; `rtok demon status` with nothing running prints all three services as stopped; `cargo test --test demon --test surface_parity --test config_coverage`; `just check`.
Complexity: 1/5
Status: done 2026-09-14 · Model: Claude Code / claude-opus-5
Evidence: `demon list` and `demon update` exit 2 (unrecognized subcommand); `demon status` rc=0 lists proxy/mcp/web stopped; `demon status proxy` rc=0; `demon`, `surface_parity`, `config_coverage` green; fmt + clippy clean. `cli_trycmd` `version` fails on `main` before this change (fixture says 0.1.0 after the v0.1.1 release) — fixed by PR #26, not here.
Deviation: 5 files instead of ≤3 (two are test tables, one a config comment); net −30 LOC.

## T38.3 — e2e `memory import` + `graph index`

**T38.3 e2e `memory import` + `graph index`** · P2, 2/5 · `tests/import_index_e2e.rs` (new, 132 LOC)
Do: `memory import` of a runtime-written JSONL fixture is found by `mem_search` over `mcp` stdio; `graph index` of a runtime-written two-file Rust tree is found by `symbol` over `mcp` stdio. Hermetic temp homes (`rtok-t383-`, removed at end), harness from `commands_e2e.rs`, `tools/call` shape from `graph_contract.rs`.
Check: both cases green; `just check` green.
Complexity: 2/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test --test import_index_e2e` 2 passed / 0 failed (`memory_import_is_searchable`, `graph_index_finds_symbol`); fmt + clippy clean; full `just check` green (fmt-check, workspace clippy `-D warnings`, 39 test binaries ok incl. `plugins_e2e` 11 + `import_index_e2e` 2, build-min, jscpd 1.26% under threshold 2).

## T38.6 — translate all `.md` to English

**T38.6 translate all `.md` to English** · P3, 1/5 · `toolchain.md`, `plan.md`, `AGENTS.md`
Do: a repo-wide grep for Cyrillic hit three project files — `toolchain.md` (headers + every row), one line in `plan.md` (claiming rule), one line in `AGENTS.md` (Workflow). Rewrite all three in English: parent-spec table headers with English cells, `Status`/`Agent`/`Complexity` in the two one-liners. Nothing else changes.
Check: a repo-wide grep for Cyrillic across `*.md` finds no matches.
Complexity: 1/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `grep '[Cyrillic] --include='*.md'` — no matches (the only hit during work was the Check line quoting the pattern itself, reworded).

## T38.5 — `commands_e2e.rs` on `assert_cmd`

**T38.5 `commands_e2e.rs` on `assert_cmd`** · P2, 2/5 · `Cargo.toml`, `Cargo.lock`, `toolchain.md`, `tests/commands_e2e.rs`
Do: add `assert_cmd = "2"` to dev-deps (already in `rust.md`, same line as ketch/cox — reuse, no new approval), record it in `toolchain.md`, rewrite `tests/commands_e2e.rs` on `Command::cargo_bin` with `cmd`/`ok`/`hook` helpers (`.assert().success()/.failure()`, `write_stdin` for `hook`) — same 9 cases, same asserts, no `std::process::Command` left.
Check: `cargo test --test commands_e2e` 9/9 green; fmt + clippy clean.
Complexity: 2/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test --test commands_e2e` — 9 passed / 0 failed; `cargo fmt --check` clean; `cargo clippy --test commands_e2e -- -D warnings` clean; test file 173 LOC; `Cargo.lock` gains the resolved `assert_cmd` subtree only.
Deviation: none. (Also removed the leftover T38.4 card and a duplicated T38.2 card from `plan.md` while claiming — dead text only.)

## T38.2 — e2e plugin matrix

**T38.2 e2e plugin matrix** · P1, 3/5 · `tests/plugins_e2e.rs` (new, 199 LOC)
Do: one e2e case per catalogue plugin through its surface — `cmd` via `run`, `read`/`graph`/`memory` via `mcp` stdio, `proxy`/`archive`/`toon` via in-process proxy (`httpmock` upstream), `inject`/`guard` via `hook` stdin, `measure`/`compress` via `stats --json`. `Measurement` rows asserted where owed (cmd, read `dedup`, graph `cap`, inject `inject`, guard `guard`, archive/toon counts, compress `summary` in stats JSON); surface behavior asserted where no row is owed (measure reports, memory round-trip, proxy `usage` row). Hermetic temp homes (`rtok-t382-`, `Drop` cleanup), binary harness copied from `commands_e2e.rs`.
Check: 11 cases green; `just check` green.
Complexity: 3/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test --test plugins_e2e` 11 passed / 0 failed; full `just check` green (fmt-check, workspace clippy `-D warnings`, 39 test binaries ok incl. `plugins_e2e` 11 + `import_index_e2e` 2, build-min, jscpd 1.26% under threshold 2).

## T38.4 — drop stale `divan`/`lbug` rows from `toolchain.md`

**T38.4 drop stale `divan`/`lbug` rows from `toolchain.md`** · P3, 1/5 · `toolchain.md`
Do: delete the `divan` and `lbug` rows — neither crate is in any manifest (`lbug` removed by P39; only historical prose in `src/plugins/graph/PLAN.md` mentions it, `divan` nowhere). Nothing else changes.
Check: `divan`/`lbug` no longer appear in `toolchain.md`; the diff touches only `toolchain.md`.
Complexity: 1/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `grep 'divan|lbug' toolchain.md` — no matches; neighbouring rows (`dotenvy`, `figment`, `libsqlite3-sys`, `notify`) intact.
Deviation: the task was scoped from a rust.md audit that first misreported `figment`/`rmcp`/`similar`/`indicatif`/`owo-colors` as missing — re-read showed all five present (`rust.md:456,477,478,506,579`), so `rust.md` needed no change and the task shrank to the two stale `toolchain.md` rows.

## T38.1 — e2e for uncovered commands

**T38.1 e2e for uncovered commands** · P1, 3/5 · `tests/commands_e2e.rs` (new)
Do: one case per uncovered command through the binary with an isolated HOME — `hook PreToolUse`/`SessionStart` exit 0 with a JSON object, `run echo` prints its output, a 50-line `run` prints a `[rtok <id> …]` trailer and `expand <id>` round-trips it (`--lines` slices, unknown id fails), `plugins` lists the 11 catalogue ids, `config init/path/show/get/set/validate` round-trips, `bench --dry-run` lists 6×2×3, `doctor` reports the chain, `stats --json` parses.
Check: `cargo test --test commands_e2e` green; `just check` green.
Complexity: 3/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `cargo test --test commands_e2e` — 9 passed / 0 failed; `just check` green — fmt-check PASS, clippy PASS (`-D warnings`, workspace + wasm-guest), workspace tests PASS, build-min PASS, jscpd 1.26% lines under threshold.
Deviation: none — one new file, 187 LOC, no source changes.

## T37.1 — e2e tests for the Slint web UI

**T37.1 e2e tests for the Slint web UI** · P2, 3/5 · `crates/rtok-webui/{Cargo.toml,src/lib.rs,ui/app.slint,tests/e2e.rs}`, `toolchain.md`
Do: drive the real compiled `MainWindow` through Slint's recommended headless backend (`i-slint-backend-testing`, `=`-pinned to `slint` 1.17.1 — run tests with `SLINT_EMIT_DEBUG_INFO=1`, which `slint-build` already honours and re-runs on). Extract shared `pub fn apply_snapshot` (one call path for the WASM client and tests); label `NavItem` touch areas (`accessible-role: button` + label, a11y + queryable); mark derived `page-id`/`selected-plugin`/`selected-call` as `out property` so external tests can read them. Five e2e tests: fresh-window defaults, full snapshot → every bound property/model, real tab clicks → `page-id` binding, cursors → selected rows, fail-open empty snapshot.
Check: `SLINT_EMIT_DEBUG_INFO=1 cargo test --manifest-path crates/rtok-webui/Cargo.toml` green (4 unit + 5 e2e); webui clippy `-D warnings` green; root `just check` green.
Complexity: 3/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: webui 9 passed / 0 failed; root gate green — fmt-check, workspace clippy, 390+ unit/integration tests, build-min, jscpd 1.26% under threshold 2. `tab_click_switches_page` clicks every tab by accessible label via `mock_single_click` (the sync no-event-loop path); plain `invoke_accessible_default_action` did not deliver the click, and `ElementHandle` queries need the debug-info build var (documented in `tests/e2e.rs` header).
Deviation: 5 product files + toolchain row + workflow surface — over the ≤200 LOC / ≤3 files guideline; kept as one task because backend, shared apply path, labels and tests are one behavior. New dev-dependency `i-slint-backend-testing`: Slint's official test backend, same repo/version as `slint`, test-only (one-line reason per the dependency rule).

## T37.0 — agent list plus CLI/GUI setup variants

**T37.0 `rtok agent list` + CLI/GUI setup variants** · P1, 3/5 · `src/cli.rs`, `src/setup/mod.rs`, `tests/config_coverage.rs`, `tests/surface_parity.rs`, `docs/config.md`
Do: `rtok agent setup opencode,cursor` installs into every named host; new `rtok agent list` prints every known host with app type (cli/gui), app version (`<bin> --version`, `-` when unknown), rtok installed state and installed modules (hooks/mcp/proxy/plugin markers the installers write); `rtok agent setup cursor` covers cli+gui (shared `hooks.json`/`mcp.json`, one run), opencode cli+gui keep separate configs (desktop path per OS); `--cli`/`--gui`/`--all` select variants (default all); a missing agent prints `skip <host> (<kind>): not found, not installed` and no files are created; removal always covers all variants. `--cli`/`--gui` are action flags (allow-listed in `config_coverage`, like `--all`); `agent list` is exempt in `surface_parity` (helper, like `config path`).
Check: `agent list` prints the 7-row table (claude/cursor×2/codex/opencode×2/pi); `setup cursor --dry-run` shows hooks+plugin+MCP lines; `setup opencode,cursor --dry-run` installs both; `--cli` drops the gui skip line, `--gui` drops the cli install lines; a missing variant prints `skip … not found`; `unknown host` still exits 1; `just check` green.
Complexity: 3/5
Status: done 2026-09-14 · Model: OpenCode / Muse Spark 1.3
Evidence: `just check` green — fmt-check PASS, clippy PASS (`-D warnings`, workspace + wasm-guest), 530 passed / 0 failed / 2 ignored (graph_bench ignores), build-min PASS, jscpd 1.26% lines under threshold 2. Runtime probes on isolated HOME: list prints versions for installed CLIs and `-` for opencode-gui; comma + variant flags behave as above. `config_coverage` 1/1, `surface_parity` 4/4.
Deviation: ~410 new source lines (2 source files) plus required test/docs/workflow surface — over the ≤200 LOC guideline; kept as one task because list, variant filter and skip-if-missing are one behavior. `agent_present` treats an existing host dir as present (needed so seeded temp homes install; means an empty leftover dir defeats the skip). `app_version` spawns `<bin> --version` with no timeout and `cursor` CLI also answers to bare `agent` (fail-open: `-` on any failure).


## P36 — second bug-hunt residue — T36.1–T36.20 done (Gate P36 still needs `just check`)

**T36.6 dead read/graph surface: wire or delete** · — · `src/plugins/read/outline.rs`, `src/config/mod.rs`, `docs/config.md`
Do: `outline::supported()` is never called and `plugins.read.languages` is never read. Either restrict `mode = map|signatures` to the configured languages, or delete the key and the helper (a config key that changes nothing is worse than none — the D26 argument).
Check: whichever way, `just check` green and no documented key is unread (`docs/config.md` and the schema agree).
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib default_toml_is_the_defaults` — 1 passed; `mise exec -- cargo test --lib validate::` — passed; `mise exec -- cargo test --test config_coverage` — passed; `mise exec -- cargo test --lib plugins::read::` — passed. Deviation: deleted `plugins.read.languages` (D26); kept `outline::supported()` because graph calls it.

**T36.15 the web Doctor page carries the instruction audit** · — · `crates/rtok-webui/src/lib.rs`
Do: `doctor_of` stops after `autoCompactWindow`; `doctor::Report::to_text` appends the `instructions` section (per-file rows and duplicates) that the snapshot already carries, so the D23 parity claim is false for that page.
Check: the page renders the same instruction rows in the same order as `rtok doctor`; `tests/surface_parity.rs` covers it.
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --test surface_parity` — 4 passed, including `web_doctor_instruction_audit_matches_cli_order`; `mise exec -- cargo test --manifest-path crates/rtok-webui/Cargo.toml doctor_of` — 1 passed (`doctor_of_renders_instruction_audit`).

**T36.20 archive rows and inline bodies keep their attribution** · — · `src/store/mod.rs`
Do: (a) `spill` writes every `call_io` body with `archive.session = ""`, so archived bodies are unattributable — thread the session through; (b) inline bodies are stored from `String::from_utf8_lossy`, so `request_json` is not the byte string `request_sha256` hashes — store the bytes or hash the lossy form; (c) `insert_measurement` clamps with `unwrap_or(i64::MAX)`, a silently wrong saving where a checked conversion should error.
Check: an archived `call_io` body carries its session; the recorded sha256 matches the stored text; an out-of-range estimate is an error, not a clamp.
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib store` — `spill_archive_carries_session`, `inline_sha256_matches_stored_text`, `insert_measurement_rejects_out_of_range_estimates` pass (rstest); T36.2 `live_zone_pointer` retained.

**T36.9 `~` expands for every path key** · — · `src/config/mod.rs`
Do: `report.out`, `bench.tasks`, `bench.configs.*` and `plugins.read.allow_paths` are missing from the expansion list, so `[report] out = "~/rtok-report.md"` fails with `No such file or directory` although `docs/config.md` says paths accept `~`.
Check: a test walks every `PathBuf` leaf of `Config::default()` and fails if one is not expanded.
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test -p rtok --lib default_expands_every_pathbuf tilde_expands` — 6 passed (`default_expands_every_pathbuf`, four `tilde_expands_for_every_path_key` cases, `expand_covers_bare_tilde_and_rtok_home_dir`).

**T36.8 legacy-key fold cannot outrank env or flags** · — · `src/config/mod.rs`, `src/config/layers.rs`
Do: `[dashboard]`, `core.log_file`, `core.log_level`, `core.log_to_db` and `core.inject_budget_tokens` are folded after `extract()`, so a stale file key overrides `RTOK_*` and `--flags` (`rtok web --port 5555` binds the file's 4444), and `config show --sources` reports the pre-fold value and source. Fold inside the figment below project/env/flag, or apply a legacy value only while the new key is still at its default, and build `--sources` rows from the folded result.
Check: a legacy file key loses to `RTOK_*` and to a flag; `show --sources` names the layer whose value is in effect.
Complexity: 4/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test config::` — 39 passed, including `legacy_dashboard_port_loses_to_higher_layers`, `legacy_dashboard_port_folds_when_unset`, `legacy_core_log_level_loses_to_env`; T36.7 `user_path`, T36.9 `finish()` tilde expansion, T36.10 cap field docs retained.

**T36.10 the two documented output caps exist** · — · `src/expand.rs`, `src/mcp.rs`, `src/config/mod.rs`
Do: `expand.max_lines` and `mcp.max_result_chars` are declared, documented and read by nothing. Apply both in one shared line-slicing helper used by `expand::run` and the MCP `expand` tool (they already duplicate `take(b).skip(a-1)`).
Check: `[expand] max_lines = 100` truncates a larger payload and the output says so; the MCP result honours `max_result_chars`.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib max_lines` — 1 passed (`max_lines_truncates_and_reports`); `mise exec -- cargo test --lib max_result_chars` — 2 passed (`expand_honours_max_result_chars`); T36.2 `live_zone_pointer` and T36.9 `finish()` retained.

**T36.11 retention actually runs** · — · `src/store/mod.rs`, `src/proxy/mod.rs`, `src/mcp.rs`
Do: `purge_calls_older_than` has no caller and `core.retain_calls_days` is read nowhere, so `calls`/`call_io`/`tokens`/`logs`/`usage` and `~/.rtok/archive/` grow without bound on exactly the long-running surfaces `demon` keeps alive.
Check: with `retain_calls_days = 1` and an old row seeded, a proxy/mcp session purges it and its archive file; a test asserts the row count falls.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib retention` — 3 passed (`run_retention_purges_old_call_and_archive`, `retention_runs_on_mcp_session_start`, `retention_runs_on_proxy_session_start`); `mise exec -- cargo test --lib purge_drops` — 1 passed (`purge_drops_old_calls_and_detaches_their_ledger_rows`); T36.2 `live_zone_pointer`, T36.10 `filter_lines`/`max_result_chars`, and T36.20 session/spill/try_from retained.

**T36.13 PDF page numbers match the pages** · — · `src/report/pdf.rs`
Do: `starts.push(pages.len())` is recorded before the pagination loop may push a fresh page, so the ToC and every bookmark name the previous page for sections that start one.
Check: in a multi-page report the ToC entry and the outline destination agree with the heading's real page.
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib report::pdf` — 4 passed, including `section_page_numbers_match_heading_pages`.

**T36.7 `--config` is honoured by every `config` subcommand** · — · `src/cli.rs`, `src/config/validate.rs`, `src/config/mod.rs`
Do: `config init|set|path|validate` resolve `<home>/config.toml` and ignore `--config`/`RTOK_CONFIG`, while `show`/`get` honour it: `rtok --config ci.toml config set proxy.port 2222` reads ci.toml and writes the home file. Add one `Config::user_path(home, config_file)` and use it everywhere.
Check: `--config /tmp/ci.toml config set/get/validate/path` all act on `/tmp/ci.toml`; a test pins the path in the output.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test config` — 32 passed, including `config_subcommands_honour_config_flag` (`path`/`get`/`set`/`validate` all honour `--config /tmp/ci.toml`).

**T36.14 live passthrough rows are not labelled tokens** · — · `src/web/model.rs`, `src/tui/view.rs`
Do: a plain-proxy row stores byte counts in `input`/`output`, and the Calls page renders their sum as `… tok` while no `usage` row exists for it.
Check: a live row shows bytes (or `-`), and a linked api_request row still shows tokens. `cargo test --lib call_size_label_distinguishes live_passthrough_rows_show_bytes_not_tokens calls_tab_lists_rows_newest_first`, all pass (rstest).
Complexity: 1/5
Status: done 2026-09-11 · Model: Composer 2.5

**T31.1 config flag (opt-in, default off)** · T31.0 · `config/default.toml`, `docs/config.md`, config schema (`src/config/mod.rs`)
Do: add the opt-in flag and threshold knobs; default off. Document them. NO cache implementation, no embeddings, no new deps. Proxy behaviour must stay unchanged.
Check: default proxy behaviour unchanged; enabling the flag is visible in `config show --sources`; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Check: `cargo fmt --check`, `cargo clippy --lib -D warnings`, `cargo test --lib config::` (40 passed incl. `semantic_cache_defaults_overlay_and_unknown_key`, `default_toml_is_the_defaults`), `cargo test config_coverage` green; no `src/proxy/` diff; `RTOK_PLUGINS_PROXY_SEMANTIC_CACHE_ENABLED=true rtok config show --sources` → `plugins.proxy.semantic_cache.enabled = true (env)`.
Model: Composer 2.5


**T36.16 `graph` reads each file once** · — · `src/plugins/graph/mod.rs`, `src/plugins/graph/index.rs`
Do: (a) `symbol` re-reads the whole file for every definition row (500 one-line definitions = 500 reads before the cap truncates); cache the last `(path, contents)`; (b) the cap budget scales `text.len()` (bytes) against a char-based estimate, so a CJK-heavy file's head can exceed `plugins.graph.max_tokens` by ~3× — scale and compare in chars; (c) a file that cannot be decoded or parsed is never recorded, so it is re-read and re-parsed on every call forever — record the stat with an empty-sha sentinel.
Check: `symbol` on the 500-definition fixture reads the file once (counting fixture or a stat counter); a CJK fixture's capped output estimates ≤ `max_tokens`; a latin-1 fixture is not re-read on a second warm call. `cargo test --lib graph`, all pass, including `symbol_reads_each_source_file_once`, `cjk_capped_output_respects_max_tokens`, and `latin1_file_is_not_reread_on_warm_index` (rstest).
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


**T36.17 a removed directory leaves the index** · — · `src/plugins/graph/watch.rs`
Do: `relevant()` accepts only supported *files*, so `rm -rf src/<dir>` never reaches `run_changed` and rows for the deleted files keep being served (`callers`/`impact` name them; `symbol` prints an empty body). Treat a non-relevant, non-`.git` event path as a rescan trigger.
Check: deleting a directory removes its rows without a full walk being needed for the call that follows. `cargo test --lib graph::watch`, all 10 pass, including `removed_directory_drops_index_rows` (rstest).
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


**T36.18 one path→provider/api mapping and a real URL join** · — · `src/proxy/wire.rs`, `src/proxy/mod.rs`
Do: `Wire::api()` re-derives the name from `matches()` while each wire already knows its own provider, the `provider` fallback arm for `/v1/chat/completions` is unreachable, and the upstream URL is built by `format!("{base}{path}")` plus a hand-appended `?`. Give each wire constants for provider/api and join the URL with `Url`, so a base with a path or query cannot produce `//` or a doubled `?`.
Check: every wire reports its own provider and api; a base URL with a trailing path still forwards to the right target (test with a mock).
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5
Evidence: `mise exec -- cargo test --lib wire_reports` — 1 passed (`wire_reports_own_provider_and_api`); `mise exec -- cargo test --lib join_upstream` — 1 passed (`join_upstream_avoids_doubled_slashes_and_queries`); `mise exec -- cargo test --lib upstream_base_with_trailing` — 1 passed (`upstream_base_with_trailing_path_forwards_to_target`); `mise exec -- cargo test --test proxy` — 21 passed; T36.11 `run_retention` at serve start retained.

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

**T36.1 toon escapes control characters in a quoted cell** · — · `src/plugins/toon/mod.rs`
Do: `encode_cell` detects `\n` and then emits it literally, so a row physically spans two lines while the header claims N; escape `\n`, `\r`, `\t` and unescape them in `decode_cell`.
Check: a fixture with a newline inside a cell round-trips (`round_trip_values`), and the emitted block has exactly N row lines. `cargo test --lib toon`, 7/7 pass, including `round_trip_values` rstest cases for `\n`, `\r`, `\t`.
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5




**T36.2 toon persists a live-zone decision and archives the original string** · T36.1 · `src/plugins/toon/mod.rs`
Do: `rewrite_block` never calls `archive_decision`/`put_archive_decision`, so `expand <toon-id>` does not stick (the next request re-encodes) and `expand::fetch` never records the expand Measurement; it also archives `serde_json::to_vec(&table)` — a re-serialised copy — while toon/README.md promises the original.
Check: `rtok expand <toon-id>` freezes the id for the next request and increments the toon expand row; the archived bytes equal the original text. `cargo test --lib toon`, 9/9 pass; `cargo test --lib expand`, 9/9 pass; `archived_bytes_match_original_text`; `expand_freezes_id_and_records_toon_expand_row`.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


**T36.3 `cmd` honours `plugins.cmd.rules` and `fail_tail_lines`** · — · `src/plugins/cmd/rules.rs`, `formatters.rs`, `run.rs`, `filter.rs`
Do: both keys are documented in `docs/config.md` and read nowhere: the user rule file is only tilde-expanded, and the non-zero-exit tail is the `FAIL_TAIL = 80` constant. Thread the `[plugins.cmd]` knobs into `pick()`/`apply()` through one settings value so both call paths agree.
Check: a rules file with a `match_cmd` rule changes the output for that command; `fail_tail_lines = 3` keeps three lines on a failing command. `cargo test --lib cmd`, 21/21 pass, including `user_rules_file_changes_output_for_match_cmd` and `exit_nonzero_fail_tail_lines` rstest cases.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

**T36.4 rules trailer counts every line it dropped** · T36.3 · `src/plugins/cmd/rules.rs`
Do: the omitted counter only increments for lines the pick loop skipped, not for `take` lines dropped by `max` (nor for lines after the mid-loop `break`), so the trailer under-reports and the promised tail can disappear.
Check: a fixture whose `max_lines` cap drops `take` lines reports the true omitted count. `cargo test --lib cmd`, 22/22 pass, including `max_cap_reports_true_omitted_count` (10 lines / head3 / tail3 / max4 → 7 omitted).
Complexity: 2/5
Status: done 2026-09-11 · Model: Composer 2.5


**T36.12 the OTel exporter cannot lose or block a stream** · — · `src/otel/export.rs`, `src/store/otel.rs`, `tests/otel.rs`
Do: (a) the `only_ties` shortcut skips the whole traces block when the only new session ended in the second the watermark already covers, so that session's `invoke_agent` span is never posted (make the sessions watermark identity-based, or drop the shortcut and let the backend dedupe by `span_id`); (b) a 404 on `/v1/traces` returns `Err`, so logs and metrics are never attempted and every flush re-posts a doomed traces batch — the module header promises the stream is skipped and its mark kept.
Check: a session that ends in a watermark second is posted exactly once; a traces-404 collector still receives logs and metrics, with one `skipped` line. `cargo test --test otel`, 10/10 pass; `cargo test --lib store::otel`, 5/5 pass, including `session_in_watermark_second_posts_once` and `a_traces_404_still_posts_logs_and_metrics`.
Complexity: 4/5
Status: done 2026-09-11 · Model: Composer 2.5


## P35 — Graph index speed (open; Gate P35 met 2026-09-11) — T35.1–T35.5

**T35.1 compile each tags query once** · T8.1 · `src/plugins/read/outline.rs`
Do: `outline::config` compiled the language's tags query on every `tags` call. That was 19 ms of a 26.5 ms call on `graph/index.rs`, paid by each file of an index run and by each outline.
Check: one `OnceLock` per language, kept for the process. A failed compile keeps its message, so every call gets the same `Err` and never a retry. `each_language_compiles_once` checks that two `.rs` files get the same configuration and a `.ts` file a different one. `golden_per_language` and the graph tests are unchanged: definitions 42/42, ref recall 0.305. The cold debug index of this repo fell from 3.42 s to 1.38 s and 1.30 s over two runs (127 files, 18 100 rows, 2026-09-11). The release `graph_bench` cold index of 3 000 files fell from 13.8 s to 341 ms (research.md P8c). A `TagsContext` per thread was not added: it builds a parser and a cursor and does not compile a query, so no measurement points at it.
Complexity: 2/5
Status: done 2026-09-11 · Model: Opus 5

**T35.3 batched store I/O for the index** · T35.2 · `src/store/symbols.rs`, `src/plugins/graph/index.rs`, `src/plugin.rs`, `crates/rtok-plugin-sdk/src/host.rs`
Do: measure first. Then one query for the root's `(path, sha, mtime, size)` instead of a `symbol_stat` per file; multi-row INSERTs chunked under SQLite's variable limit; one DELETE for vanished paths. `graph-lbug` keeps its own path.
Check: `symbol_stats` loads every file stat in one DISTINCT query; cold `run_with` stages writes and flushes them through `replace_symbol_files` in transactions of 64 files with multi-row INSERTs chunked at 90 rows; `delete_symbols_missing` issues one `DELETE` (or `eq_any` for partial roots). Twelve `index` tests and `graph_truth` stay green; release `graph_bench` cold index of 3 000 files still yields 9 000 rows. Measured on this machine before patch (release, three runs): 251–328 ms (median 309 ms). After: 153–223 ms (median 197 ms, −36 %). Debug `labelled_symbols_are_found` cold index 380 ms (high run-to-run variance on this host). `graph-lbug` untouched.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5

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

**T28.2 implement optional compress / memory extractor** · T28.1 · plugin sources under `src/plugins/`
Do: implement the chosen path behind the flag. Record a `Measurement`. When the source is not regenerable, `expand` still recovers the original. Default-off path is byte-identical to today's lossless behaviour.
Check: with the flag off, proxy/archive/inject bytes match pre-P28; with the flag on, a fixture compresses and `expand` recovers where required.
Complexity: 4/5
Status: done 2026-09-12 · Model: Composer 2.5
Check result: `plugins::compress` 4 passed (default-off byte-identical after archive; fixture shrinks + `get_archive` recovers original; deterministic). `registry_matches_catalogue`, `every_plugin_has_a_plan`, `cargo fmt --check`, `cargo clippy --lib -D warnings` green. Deviation: 5 files (added `tests/plugin_plans.rs` SURVEYS drop + `Cargo.toml` feature); `compress/mod.rs` 248 LOC (extractive ranker + wire tests). Memory observation extractor deferred (optional per PLAN). `just check` hit unrelated flakes (`agents`, `otel`, `rtok-agent-sdk` backup) on this machine — re-run on landing.

**T28.1 config / feature flag (default off)** · T28.0 · `config/default.toml`, `docs/config.md`, config schema
Do: add a config flag (and matching CLI override if needed) that enables the compressor / extractor; default off. Document the key. No compression logic yet — reading the flag and refusing unknown keys is enough.
Check: `rtok config show` lists the new key as off by default; turning it on via config or env is visible in `config show --sources`; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5
Check result: `plugins.compress.enabled = false` in `rtok config show`; `RTOK_PLUGINS_COMPRESS_ENABLED=true rtok config show --sources` → `true (env)`; user file edit → `true (user)`; `cargo fmt --check`, `cargo test --lib config::` 40 passed; `just check` blocked by disk full (errno 28) on this machine — re-run on landing.


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


**T29.2 implement optional embed search** · T29.1 · `memory` / `graph` plugin sources
Do: implement the embed backend behind the flag. Progressive disclosure / existing MCP tool names stay. A fixture note is indexed so both FTS5 and embed find it.
Check: flag off → FTS5-only bytes/behaviour; flag on → the fixture note is returned by both search paths.
Complexity: 4/5
Status: done 2026-09-12 · Model: Composer 2.5
Check: `cargo test --test p29_memory` (2 passed); `cargo test --lib plugins::memory store::embed::tests` green. Flag off keeps FTS5-only ranking; embed on finds `p29-gate-arctic-tern` via vector leg, hybrid RRF, and FTS `Diesel sync`; records `p29_hybrid_recall` Measurement. Deviation: >200 LOC / >3 files — `src/store/embed.rs` (BLOB KNN not sqlite-vec vec0), `migrations/0012.sql`, `tests/p29_memory.rs`, `tests/fixtures/p29_memory.toml`, `src/mcp.rs` wiring; deterministic `hash_embed`/`hash_embed_note` (no ONNX).

**T29.1 config flag (FTS5 default)** · T29.0 · `config/default.toml`, `docs/config.md`, config schema
Do: add a config flag that selects the embed path; FTS5 remains default when the flag is off/absent. Document it.
Check: default config keeps FTS5-only behaviour; enabling the flag is visible in `config show --sources`; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5
Check: `default_toml_is_the_defaults` + `memory_embed_*` tests; `RTOK_PLUGINS_MEMORY_EMBED_ENABLED=true rtok config show --sources` → `plugins.memory.embed.enabled = true (env)`; `cargo fmt --check`; `cargo clippy --lib -D warnings`; `cargo test --lib config::` (41 passed).

## P30 — LSP graph backend — T30.2 done 2026-09-12

**T30.0 design/survey: LSP behind tags MCP** · — · `src/plugins/graph/PLAN.md`
Do: survey serena-grade LSP backends and how they map onto the existing MCP tool names. Tags remain default; LSP is optional. Name one fixture where tags miss and LSP hits. No implementation.
Check: PLAN (`src/plugins/graph/PLAN.md` P30 survey): rust-analyzer / clangd / tsserver surveyed; MCP names stable; Gate P30 `OnlyTyped` type-position miss.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5


**T30.1 config flag (tags default)** · T30.0 · `config/default.toml`, `docs/config.md`, config schema
Do: add a config flag that enables the LSP backend; tags-only when off. Document it.
Check: default keeps tags-only; enabling the flag is visible in `config show --sources`; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5

**T30.2 implement optional LSP backend** · T30.1 · `src/plugins/graph/lsp.rs`, `src/plugins/graph/mod.rs`, `tests/graph_lsp_gate.rs`
Do: implement `plugins.graph.backend = "lsp"` behind the T30.1 flag. Same MCP names (`symbol`/`callers`/`outline`). Off → tags-only bytes. On → Gate fixture where tags miss and LSP hits (rust-analyzer on PATH; skip if absent). Native JSON-RPC client; spawn rust-analyzer/clangd/tsserver from PATH (D6: do not spawn serena). Record a Measurement row.
Check: same MCP names; LSP off → tags-only bytes; LSP on → the fixture hits on LSP and misses on tags.
Complexity: 4/5
Status: done 2026-09-12 · Check: `cargo test --test graph_lsp_gate --test graph_contract --lib graph` — graph_contract 3 passed (tags MCP bytes unchanged); graph_lsp_gate 4 passed (`mcp_tool_names_are_unchanged`, `tags_backend_callers_bytes_match_contract`, `tags_backend_misses_onlytyped_type_position`, `lsp_backend_hits_onlytyped_type_position`); lib `graph` 39 passed. `cargo fmt --check` on touched files green. Gate P30: `callers("OnlyTyped")` is `no references to OnlyTyped` on tags and contains `user` on lsp; `plugin=graph` measurement kind `lsp.callers`. Skip: `lsp_backend_hits_onlytyped_type_position` returns after `eprintln!("skip: rust-analyzer not on PATH")` when `rust-analyzer --version` fails.
Model: Cursor / grok 4.6
Deviation: **~594 LOC** `src/plugins/graph/lsp.rs` + dispatch in `mod.rs` + `tests/graph_lsp_gate.rs` (121) exceeds ≤200 LOC / ≤3 files — native JSON-RPC client + rust-analyzer handshake cannot fit the cap. Extra comment-only edits: `config/default.toml`, `docs/config.md`. Clangd/tsserver adapters are spawn+framing only; the gate is rust-analyzer. Crate-wide `clippy -D warnings` still fails on pre-existing P31 `semantic_cache`/`proxy` lints; not touched. `just check` skipped (fmt-check whole repo, lint, dup, build-min).

## P31 — Semantic response cache — T31.2 done 2026-09-12

**T31.2 implement opt-in semantic cache** · T31.1 · `src/proxy/` / proxy plugin
Do: implement the cache behind the flag. Off → identical proxy bytes to today. On → cache hits only under the documented threshold; record measurements.
Check: off → identical proxy bytes; on → documented false-hit rate runnable on the P9 set.
Complexity: 4/5
Status: done 2026-09-12 · Check: `cargo test --lib semantic_cache` (6 passed: `semantic_cache_disabled_proxy_bytes_identical`, `semantic_cache_enabled_direct_hit_skips_upstream`, `p9_fixture_audit_zero_false_hits`, `direct_hit_on_exact_replay`, `hash_backend_skips_semantic_tier`); P9 synthetic corpus `tests/fixtures/p9_semantic_cache_corpus/` — false-hit pairs **0**, semantic hit rate **0%** at threshold 0.99 (orthogonal fixture embeddings; `embed_backend = hash` keeps tier 2 off in production).
Model: Composer 2.5
Deviation: **451 LOC** `src/proxy/semantic_cache.rs` + **169 LOC** `src/proxy/mod.rs` + fixture corpus (exceeds ≤200 LOC / ≤3 files).

## P31 — Semantic response cache (open) — T31.2

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


**T32.1 config / feature flag for WASM host** · T32.0 · `config/default.toml`, `docs/config.md`, `src/config/mod.rs`
Do: add the flag / Cargo feature that enables loading `.wasm` plugins; default off so in-tree builds stay unchanged.
Check: default build and config do not load WASM; enabling the flag is documented and visible; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5
Check result: `[plugins.wasm]` (`enabled = false`, `dir = "~/.rtok/plugins"`) in schema, `default.toml`, and `docs/config.md`; not in `CATALOGUE`; `RTOK_PLUGINS_WASM_ENABLED` in env leaf table; `mise exec -- cargo test --lib config::` — 43 passed; `cargo fmt --check` + `clippy -p rtok --lib -D warnings` green; `Cargo.lock` unchanged (no wasmi). `just check` lint step blocked here by missing `sccache` shim in cmake (pre-existing env).

**T32.2 implement WASM host + example Measurement** · T32.1 · host loader, example `.wasm`, `Registry::from_plugins`
Do: `Registry::from_plugins` loads one example `.wasm` that records a `Measurement`. In-tree plugins unchanged. D6: example may live as a build artefact / docs sample, not a vendored third-party plugin.
Check: `Registry::from_plugins` plus the example `.wasm` records a `Measurement`; in-tree plugin tests unchanged.
Complexity: 5/5
Status: done 2026-09-12 · Model: Composer 2.5
Check result: Cargo feature `wasm-host` (default off) + optional `wasmi` 2.0; `src/plugins/wasm.rs` Wasmi host (`WasmPlugin`, `env` imports `rtok_estimate` / `rtok_record_measurement` / `rtok_log`); `Registry::from_plugins` native map path unchanged when feature off or `[plugins.wasm] enabled = false`, appends `*.wasm` from `plugins.wasm.dir` when both on; rejects manifests listing `hook`. First-party guest `crates/rtok-wasm-demo-guest` (not in `all()`); artefact built at test time. `mise exec -- cargo test --features wasm-host -p rtok --lib from_plugins_loads_demo_and_records_measurement` — 1 passed (`wasm-demo`, `kind=demo`); `mise exec -- cargo test -p rtok --lib plugins::` — 106 passed (no wasm-host). Deviations: 5 files, ~480 LOC (task budget ≤200 / ≤3).


## P33 — Tiered session context (design; open) — T33.0

**T33.0 design/survey: L0/L1/L2 tiers + AGPL** · — · `src/plugins/archive/PLAN.md` and/or `inject` PLAN
Do: survey OpenViking L0/L1/L2 and at least two other tiered-context schemes. Call out AGPL (or other) license implications in the PLAN before any code. Define how tiers compose with v0.1 `archive`+`inject` and what "measured against" means for Gate P33. No implementation.
Check: OpenViking, MemGPT/Letta, Claude Code compaction; AGPL-3.0 call-out (do not vendor); Gate P33 measurement vs v0.1 archive+inject CTT.
Complexity: 3/5
Status: done 2026-09-11 · Model: Composer 2.5

**T33.2 implement optional L0/L1/L2 loading** · T33.1 · `src/plugins/archive/mod.rs`, `tests/fixtures/tier_context/`
Do: implement tiered loading behind the flag. Lossless `expand` still required where the source is not regenerable. Measure against v0.1 archive+inject on a documented fixture/session window.
Check: flag off → v0.1 byte-identical (`tiers_off_matches_v0_1`); flag on → L0 line-only cold blocks + L1 hot promotion (`tier_l0`/`tier_l1` kinds); fixture `tests/fixtures/tier_context/session.jsonl` (12 tool results); Gate P33 CTT baseline=126872 treatment=109908 ratio=86.6% (≤90% bar met); `expand` unchanged (L2 in store); inject untouched; `cargo test --lib plugins::archive::tests` 7 passed.
Status: done 2026-09-12
Model: Composer 2.5
Complexity: 4/5


**T33.1 config flag (default off)** · T33.0 · `config/default.toml`, `docs/config.md`, config schema
Do: add the opt-in flag for tiered loading; default off (v0.1 archive+inject unchanged). Document license note beside the key.
Check: default behaviour unchanged; flag visible in `config show --sources`; `just check` green.
Complexity: 2/5
Status: done 2026-09-12 · Model: Composer 2.5
Check: `plugins.archive.tiers` default false; TOML/env overlay; `RTOK_PLUGINS_ARCHIVE_TIERS=true` → `(env)` in `config show --sources`; unknown archive keys denied; `cargo test --lib config::` 41 passed; clippy clean; fmt ok.

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

### P39 — Replace LadybugDB — closed 2026-09-12 (SQLite only)

**T39.0 survey: LadybugDB replacement** · — · `src/plugins/graph/PLAN.md`, `research.md`, `Cargo.toml`, `src/store/`
Do: survey keep-SQLite / Grafeo / frozen-lbug; name winner; delete losers.
Check: SQLite only; `graph-lbug` and `graph-grafeo` gone; `just check` green; numbers archived.
Status: done 2026-09-12 · Check: Winner = SQLite. Removed feature `graph-lbug`, optional dep `lbug`, `src/store/symbols_lbug.rs`, `.cargo` `LBUG_BUILD_FROM_SOURCE`, mise `cmake` pin for liblbug. Grafeo never merged (PR #21/#22 closed). `src/store/mod.rs` is SQLite-only `symbols`. Docs: D18 amended, P39 closed, `research.md` keeps P8c + P8e tables as archive. Supersedes further lbug/grafeo work.

**T41.1 Dart in the LSP `graph` backend** · T30.2 · `src/plugins/graph/lsp.rs`, `tests/graph_lsp_gate.rs`
Do: a `pubspec.yaml` root picks `dart language-server` (Dart SDK on PATH); `workspace_of` stops at `pubspec.yaml`; `.dart` files open as `languageId = "dart"`.
Check: unit test for `pick`/`workspace_of` on a `pubspec.yaml` root; `graph_lsp_gate`-style test skipping when `dart` is not on PATH, else `outline` of a two-symbol `lib/main.dart`; `just check`.
Complexity: 2/5 — one backend match arm, one languageId, one root probe; no new dep, no schema, contract untouched.
Status: done 2026-09-15
Check result: `cargo test --lib plugins::graph::lsp` 2 passed (incl `dart_pubspec_root_picks_dart_language_server`); `cargo test --test graph_lsp_gate` 5 passed incl `lsp_backend_outlines_dart_main`, live on Dart 3.13.1 (not skipped); `cargo test --test graph_contract` 3 passed untouched; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; full `just check` exit 0 (`test --workspace`: 41 suites ok, lib 429 passed; build-min; jscpd under threshold). 2 files, no new dependency.
Model: OpenCode / Muse Spark 1.3 Contributor

**T41.2 LSP setup page with Rust and Dart examples** · T41.1 · `docs/lsp.md`, `docs/config.md`, `site/content/docs/reference/_content.gotmpl`
Do: new `docs/lsp.md` walking through Rust (rust-analyzer via `rustup component add rust-analyzer`) and Dart (Dart SDK): install the server, set `backend = "lsp"` in `.rtok.toml` or `RTOK_PLUGINS_GRAPH_BACKEND=lsp`, which file marks the workspace root (`Cargo.toml` / `pubspec.yaml`), confirm via MCP `outline` / `callers` plus the `lsp*` measurement rows, and fallback without the server (error, set `backend = "tags"` back). Linked from the `backend` line in `docs/config.md` (that line is verbatim `config/default.toml`, left intact); gotmpl row added after `configuration`.
Check: `just site` builds; every command on the page run once and dated.
Complexity: 2/5 — one docs page plus link plus gotmpl row; `src/` and `tests/` untouched.
Status: done 2026-09-15
Check result: `mise exec -- just site` builds, `reference/lsp/index.html` rendered. Commands run 2026-09-15: `rustup component add rust-analyzer` (already installed), `rust-analyzer --version` 1.97.1, `dart --version` 3.13.1, `config get plugins.graph.backend` → `tags` bare / `lsp` with env, `config show --sources` confirms `lsp (env)`, `cargo test --test graph_lsp_gate` 5 passed. Note: this machine's `~/.rtok/config.toml` carries stale keys, so bare `config show/get` reads empty here — pre-existing, page verified with a clean `RTOK_CONFIG`. No numbers claimed (D3).
Model: OpenCode / Muse Spark 1.3 Contributor


**T59.2 Canonicalize `cwd` once per `search` / `tree` call** · I-40 · `src/plugins/read/search.rs`
Do: hoist the per-row `dunce::canonicalize(cwd)` out of `display_rel`: `search` / `tree` compute the canonical base once per call (`canonical_base`, threaded through the `ReadFs` adapter so tests can count), `display_rel` takes the base. The undocumented raw-cwd middle strip arm is gone; the doc contract (canonical cwd → walk root → raw) is now the code.
Check: `just check` green; existing search/tree/display_rel tests pass with assertions unchanged; a Vfs unit test with a counting `ReadFs` adapter asserts exactly one `canonicalize` per call and zero per displayed row.
Complexity: 1/5 — one file, ~30 LOC incl. test.
Status: done 2026-09-17
Check result: `cargo test --lib read::search` 20 passed incl. new `display_rel_canonicalizes_base_once_per_call_from_vfs`; full `just check` exit 0 (fmt, clippy `-D warnings`, nextest, build-min, jscpd 1.90 % under threshold) in an isolation worktree (HEAD + own file — the main checkout carried T55.8 WIP). Note: a bare `nextest` rerun flaked on `otel::hooks_stay_fast_with_an_unreachable_endpoint` under full parallel load; it passes standalone twice and passed inside the gate run.
Model: ZCode / GLM-5.3-Flash

**T55.10 One `cmd_stem`** · review 2026-09-17 · `src/agents/mod.rs`, `src/plugins/cmd/formatters.rs`, `src/measure/stats.rs`
Do: one `pub(crate) fn cmd_stem` in the feature-free `agents` module (`measure` builds without the `cmd` feature, so the `formatters` copy could not be shared); `formatters` re-exports it, `bash_family` and `is_rtok_bin` call it; the `cmd_stem_strips_windows_path_and_exe` test moved with the definition.
Check: behavior unchanged (stem rules byte-identical), `cmd_stem_strips_windows_path_and_exe` + `bash_family_*` + `is_rtok_bin` tests pass under `just check` and `just build-min`.
Complexity: 1/5 — three files, net −17 LOC.
Status: done 2026-09-17
Check result: targeted `nextest -E 'test(cmd_stem) or test(bash_family) or test(is_rtok_bin)'` 4 passed; full `just check` exit 0 in the isolation worktree (HEAD `11eef8b` + own three files; main checkout carries T55.8 WIP) — fmt, clippy `-D warnings`, nextest, build-min, jscpd 1.88 % (was 1.90 % before the dedup). 3 files, no new dependency.
Model: ZCode / GLM-5.3-Flash

**T55.8 Guard `read:` keys survive a mutating Bash** · review 2026-09-17 · `src/plugins/guard/mod.rs`, `src/plugins/read/cache.rs`
Do: the guard owns invalidation of its own keys. Key scheme `read\t{path}` (prefix-clearable); a non-keyed mutating `Bash` clears every `bash\t…` and `read\t…` key; `Edit`/`Write` clear the bash keys plus their own path's `read\t{path}` key (all `read` keys when the path is missing) with no dependency on the `read` plugin; `plugins::read::cache::invalidate` clears the guard's key under the new scheme too.
Check: `bash_mutation_allows_the_next_read` and `edit_with_read_plugin_off_allows_the_next_read` unit tests; existing `edit_clears_guard_read_so_the_next_read_is_allowed` still passes.
Complexity: 2/5 — two files, ~100 LOC incl. tests (WIP finished from 60 %).
Status: done 2026-09-17
Check result: targeted nextest `test(guard) or test(cache)` 48 passed; full `just check` in the isolation worktree (HEAD `4f9d18a` + own two files): fmt + clippy `-D warnings` clean, nextest 758 passed with one failure = `otel::hooks_stay_fast_with_an_unreachable_endpoint`, the documented load flake (passes twice standalone in the same worktree), `just build-min` exit 0, `just dup` exit 0. Migration note: `read:{path}` rows written by the old scheme are never read again — dead keys age out with the window; the deny direction only loses dedups, never adds false denies.
Model: ZCode / GLM-5.3-Flash

**T55.9 Guard Bash key is cwd-blind** · review 2026-09-17 · `src/plugins/guard/mod.rs`, `src/plugins/mod.rs`, `src/measure/stats.rs` (+ `guard/AGENTS.md` invariant line)
Do: the Bash key keeps the effective `cd` target — `norm_cmd` folds leading `cd <dir> &&` hops to the *last* hop (that is where the command runs) instead of stripping them; the read-only stem check looks past the folded prefix via `after_cd_prefix`. Quote-aware splitting through `plugins::skip_word`, moved feature-free from `measure::stats` (guard and measure compile without each other's feature), byte-identical behavior on the stats side.
Check: `cd a && ls` ≠ `ls` ≠ `cd b && ls`, `cd a && cd a && ls` = `cd a && ls`, `cd a && cd b && ls` = `cd b && ls`, `cd 'a && b' && ls` is one hop (the `&&` inside quotes is not a split point), spacing normalizes through the fold; the old `bash_repeat_behind_cd_prefix_denies` is rewritten as `bash_repeat_behind_cd_prefix_is_a_new_key` plus a pure `bash_key_keeps_the_cd_target`; `bash_family_*` tests unchanged and green.
Complexity: 2/5 — three code files, ~120 LOC incl. tests.
Status: done 2026-09-17
Check result: targeted `cargo test --lib plugins::guard` 12 passed, `nextest -E 'test(bash_family)'` 2 passed; full `just check` exit 0 in the isolation worktree (HEAD `6f8bd6d` + own files): nextest 855/855 passed (10 slow, 2 skipped), fmt, clippy `-D warnings`, build-min, dup all green. Behavior note: `cd a&&ls` (no spaces, rare spelling) is no longer folded — it stays unkeyed, the fail-open direction (a missed dedup, never a wrong deny).
Model: ZCode / GLM-5.3-Flash

**T62.1 Claude Code `PreToolUse(Skill)`: digest oversized skill bodies** · `research.md` §10.8 · `src/plugins/guard/skill.rs`, `src/plugins/guard/mod.rs`, `src/agents/claude/mod.rs`, `src/config/mod.rs`
Do: `ENTRIES` gains `("PreToolUse", "Skill")`; `guard` handles it behind `[plugins.guard] skills = false` with `skill_max_bytes = 8192` — resolve `tool_input.skill` to its `SKILL.md` (project `.claude/skills` → `doctor::skill_md_path` → plugin roots from `installed_plugins.json`), and a body over the cap whose frontmatter has none of `allowed-tools` / `model` / `context` / `agent` is archived and denied with its heading map plus `[rtok <id> · N lines · expand: rtok expand <id>]`; everything else allows untouched; `Measurement { plugin = "guard", kind = "skill" }`; fails open.
Check: fixture skills of 3 lines, 3,000 lines and one with `allowed-tools`; hook under 10 ms on a ~250 KB body (in the test); host table re-blessed; guard page and `docs/agents.md` note that a digested skill does not apply its frontmatter.
Complexity: 3/5 — one hook entry, one guard branch (~230 LOC incl. tests), two config knobs; no new dependency.
Status: done 2026-09-18
Check result: code landed unclaimed in `c2a42ef` (knobs + goldens), `1e513d3` (`guard/skill.rs`, `pre_tool` branch, `expand`/MCP/README follow-ups), `d3b3f21` (`ENTRIES`, guard and agents docs); this close-out ran the Check. `nextest -E 'test(skill)'` 6 passed: `small_bodies_and_host_keys_pass` (3-line body → allow; `allowed-tools` frontmatter → allow even over the cap), `oversized_body_is_denied_with_map_and_pointer_under_budget` (~3,000 lines / ~230 KB → deny, heading map + fenced code skipped, reason ≤ 8,192 B, `get_archive(id)` returns the exact body), `resolve_prefers_project_then_user_and_reads_the_plugin_manifest` (project → user → plugin `installPath`; `../`, `a/b`, unknown plugin → None = fail open). Release profile: `cargo nextest run --release --lib -E 'test(oversized_body_is_denied)'` passes the 10 ms budget (debug runs assert 100 ms). `nextest -E 'test(guard) and package(rtok)'` 19 passed incl. hook e2e; `--test agents_doc` 1 passed — the blessed `docs/agents.md` table matches the new entry. Deviation (noted in the plan): `read` has no markdown map mode, so the heading outliner lives in `guard/skill.rs` — the first one; T68.8 unifies it.
Model: ZCode / GLM-5.3

**T56.2 Read/search unit tests on VFS** · D29 / T56 · `src/plugins/read/**` (tests)
Do: hottest read/search filesystem tests on `testutil::Vfs` (pure `display_rel`, size gate, regex hits, line numbering/range, WalkBuilder twins via the T56.4 adapter, `ReadFs` twins via T56.5); polish leftover unit-level disk-only read paths.
Check: no unit-level disk-only read/search path remains that the current architecture can carry on Vfs; suites green.
Status: done 2026-09-18 (close-out; earlier halves landed unclaimed)
Check result: final piece: the `read/cache.rs` re-dedup trio gained Vfs twins through `read_with` — `two_identical_reads_second_is_short_from_vfs`, `edit_fixture_between_reads_is_full_from_vfs`, `post_tool_edit_clears_hit_from_vfs` (disk twins kept). Audit of what stays disk: `read/hook.rs` tests (the production hook probes `std::fs::metadata`; swapping the hook onto `ReadFs` is T56.5's deferred follow-up, not this card), the `HostFs: WalkFs` smoke test in `walk.rs` (by design), and the disk e2e twins kept beside their Vfs twins. `nextest -E 'test(read) or package(rtok) and test(agents)'` 183/183 in the isolation worktree (HEAD `cd2c262` + own files), clippy `-D warnings` and fmt clean there.
Model: ZCode / GLM-5.3

**T56.3 cmd/setup path tests on VFS** · D29 / T56 · `src/plugins/cmd/**`, `src/agents/**` (tests)
Do: quoting tests stay pure strings; rules/settings loaders and agent hook writers gain `Vfs` twins where practical; MCP register stays disk (SDK write path).
Check: no host unit test writes hook files to real disk; the real-binary install matrix stays disk e2e by D29.
Status: done 2026-09-18 (close-out; earlier halves landed unclaimed)
Check result: audit closes the "more hosts (cursor/codex/…) if needed" remainder as not needed: `agents/*` unit tests carry zero host-disk writes — claude (7) and kimi (5) hold the Vfs hook-writer twins, every other host has no hook-writer unit tests on disk (their coverage is `tests/agents_install.rs`, a real-binary e2e matrix D29 keeps on disk; it ran green in the same worktree pass as T56.2). Cursor writes through the same `insert_ours`/`strip_ours` helpers the claude twins already cover, so a cursor twin would re-test covered code; codex TOML editing is e2e-covered. `cmd`: the `rules.rs` loader keeps its `*_from_vfs` twins; `run.rs`/`formatters.rs` tests exercise the real archive store and read `tests/cmd_golden` fixture files — repo data, not temp state, so Vfs adds nothing there. No code change in this close-out; evidence is the green 183/183 scope run above.
Model: ZCode / GLM-5.3

**T55.12 Windows `wrap_quote` corrupts apostrophes under POSIX host shells** · - · `src/plugins/cmd/hook.rs`
Do: nothing containing `'` is wrapped on `cfg!(windows)` — the PowerShell `''` rewrite concatenated under Git Bash (Claude Code's Windows Bash shell) silently drops the apostrophe; mirrors the heredoc skip, compression loss is the safe direction.
Check: pure tests `windows_apostrophe_commands_stay_unwrapped` (host-explicit `skip_wrap_host`, both host contracts pinned on one toolchain) and the parse-simulation `ps_quoting_does_not_round_trip_under_sh` (`'echo it''s fine'` splits to the single word `echo its fine`; the POSIX `'"'"'` embedding round-trips); `wrap_keeps_apostrophe_host_safe` updated to the new contract.
Complexity: 2/5 — one skip rule plus a host-explicit refactor of `skip_wrap`.
Status: done 2026-09-18
Check result: `cargo nextest run -p rtok cmd` 54/54 green (the three tests above included); full `just check` green in the isolation worktree (HEAD + own files, never /tmp).
Model: ZCode / GLM-5.3-Flash

**T69.1 Note lifecycle: retire, supersede, pin — never delete** · graymatter gap review (`research.md` §14) · `migrations/0015.sql`, `src/store/{mod,schema,embed}.rs`, `src/plugins/memory/`, `src/mcp.rs`, `src/cli.rs`, READMEs, goldens
Do: migration 0015 adds `notes.retired` / `superseded_by` / `pinned` (existing rows live); one MCP tool `mem_update(id, retire?, superseded_by?, pinned?)` plus CLI `rtok memory retire|pin|unpin|revise`, all calling the same functions in `plugins/memory` (D21); recall and both search paths exclude retired rows in SQL, pinned rows lead recall; `mem_get` of a retired id returns the body prefixed `retired <ts>[, superseded by <id>]`; `revise` = save through the in-place `mem_save` path, then retire the old id; an explicit re-save clears the tombstone.
Check: the four card tests on the in-memory store; the migration applies to a previous-schema db; surface ≤ 60 description tokens; docs (plugin README, AGENTS invariants, README command rows).
Status: done 2026-09-18
Check result: full gate in the isolation worktree (HEAD `62e4a0f` + own files — main carried concurrent T68.1 WIP): `nextest run -p rtok` **849/849 passed** (2 skipped), `cargo fmt --check` clean, `clippy -p rtok --all-targets -D warnings` clean. One documented flake on the way: `demon::a_service_that_exits_comes_back` failed once under the full parallel run and passed standalone (`--test demon` 3/3). New tests: `revise_supersedes_and_retires_the_old_note` (old words stop searching, `mem_get` carries the retired+superseded line, same-title revise updates in place and retires nothing), `retire_removes_from_recall_and_keeps_the_body` (recall omits, search omits, body kept, unknown id errors, re-save revives), `pinned_note_leads_recall_byte_stable` (pinned first over 20 newer, byte-identical second run), `migration_0015_adds_lifecycle_columns_to_a_previous_schema_db` (db built through migrations 0001–0014 + one note reopens through `Store::open`, defaults live, retire works), `mcp_surface_stays_within_sixty_description_tokens` (four tools ≤ 60 by the repo estimator, `doctor`'s measure). Guard updates forced by the new surface: trycmd `completions-bash` re-blessed (only the four subcommands in the completion tree), `config_coverage` ALLOW_KEYS for the three one-shot lifecycle values (per-call values, not settings — D12), `surface_parity` EXEMPT reasons for the four writing commands (D27). Deviation: 13 files, ~650 insertions of which 238 are the generated completion golden and ~170 test LOC — over the ≤200/≤3 shape of a C3 card; a split would have cut the store/plugin/MCP halves of one behavior apart (T51.1 precedent). Ordering note: recall sorts `pinned DESC, id DESC` (pinned first, newest within each block, byte-stable) — the card's "(id order)" read as deterministic order; recorded in `memory/AGENTS.md`.
Model: ZCode / GLM-5.3

**T55.15 `live_blobs` rewrites image/document payloads into invalid blocks** · T51.1 · `src/proxy/anthropic.rs`, `src/proxy/openai_chat.rs`, `src/plugins/archive/mod.rs`
Do: `live_blobs` yields text blocks only — `source.data` of `image`/`document` and `image_url.url` are never rewritten in place, so turning `[plugins.archive] live_blobs = true` cannot produce an invalid (400) request. Text blocks carrying `data:` URIs stay the T51.1 case.
Check: repro tests `image_source_data_is_never_rewritten` and `openai_image_url_is_never_rewritten` — the very same bytes shrink as a text block in the same turn while the binary field stays byte-identical through `rewrite_blobs`; the existing `live_blobs_*` suite passes untouched.
Complexity: 2/5 — two wire arms deleted, two repro tests.
Status: done 2026-09-18
Check result: archive filter 44/44 and proxy filter 72/72 green (new repro tests included); full `just check` green in the isolation worktree (HEAD + own files). clippy's `collapsible_if` on the new Chat arm fixed in the same pass.
Model: ZCode / GLM-5.3-Flash

**T60.10 Sessions tab in the TUI is a static paragraph** · T60.3 · `src/tui/app.rs`, `src/tui/view.rs`, `tests/surface_parity.rs`
Do: Sessions gets the Calls row model — a `sessions` cursor (`↑/↓`, clamped to the visible rows), the selected row bold, `l` toggling the live-only filter, a derived scroll offset keeping the cursor row on screen, and a status line naming the keys. The `placeholder()` fallback is deleted; an unknown page is now a compile-time match error plus a `surface_parity` failure (`every_model_page_has_a_tui_body`).
Check: `sessions_page_claims_its_keys_and_clamps_the_selection` (app), `sessions_tab_lists_rows_and_l_filters_live_only` + `sessions_tab_scrolls_to_keep_the_cursor_visible` (TestBackend, 200-row snapshot), `every_model_page_has_a_tui_body` (surface_parity).
Complexity: 2/5 — one state struct, one page renderer, one parity test.
Status: done 2026-09-18
Check result: tui 31/31 green, surface_parity 5/5 green in the isolation worktree; fmt/clippy clean. Note: the surface_parity test rode into history inside the T69.1 commit (another agent committed the file while this card was in flight); the rest of this task is these two commits.
Model: ZCode / GLM-5.3-Flash

**T51.1 Compress JSON and code inside the live zone** · I-09 · `tests/proxy.rs`
Do: the shrink pass itself landed in `d899760` (SDK `BlobRef` / `live_blobs`, the Anthropic and Chat wire arms, `archive::rewrite_blobs` behind `[plugins.archive] live_blobs = false`, `Measurement { kind = "live_blob" }`); what the card still owed was its acceptance at the proxy level rather than in unit tests. `proxy_compress_shrinks_live_blobs_on_both_wires` replays a six-turn request twice on Anthropic (`user` `text` blocks) and Chat (`user` string content), each turn carrying a 400-row JSON dump, and asserts the whole contract: the two upstream bodies are byte-identical, the request shrinks to under half its size, turns 2 and up carry `[archived …]`, the two working-edge turns stay whole, every `archive` measurement is `kind = "live_blob"` (a blob-only fixture leaves the result pass nothing to do), the four distinct blobs map to four archive ids, and each id reads back through `get_archive` as its original bytes. `t51_server` and `openai_server` now delegate to one `proxy_server(dir_tag, tune)` builder, which is what lets a case set `live_blobs = true` without a third copy of the setup.
Check: `proxy_compress_shrinks_live_blobs_on_both_wires`, plus the existing `live_blobs_*` unit suite untouched.
Complexity: 5/5 as carded — the remaining share was one test target.
Status: done 2026-09-18
Check result: isolation worktree at HEAD `454e4a5` + own files (main carried three other agents' in-flight compile errors in `doctor::Report` and `measure/stats.rs`): `nextest run -p rtok --lib --test proxy` **687/687 passed**, the new test among them, `cargo fmt --check` clean on the touched file. Image and document fields stayed out of scope; T55.15 owned and has since closed that bug, and this test was re-run green on top of it.
Model: Claude Code / opus-5

**T72.1 Shorter dev build and test loop** · `Cargo.toml`, `tools/test-changed.sh`, `justfile`, `README.md`
Do: two independent costs. The linker copied `line-tables-only` debug info for all ~450 dependencies into each of the 42 test binaries, so `[profile.dev.package."*"] debug = false` (plus `build-override`) drops it for dependencies, build scripts and proc macros while workspace code keeps its line tables and its `file:line` backtraces. And `cargo nextest run` links every integration target before the first test runs — `-E` filters only after that build, so only cargo's own target selection can help. `tools/test-changed.sh` turns the diff into that argument list: `tests/<name>.rs` -> `--test <name>`; a genuinely shared input (`Cargo.toml`, `Cargo.lock`, `justfile`, `mise.toml`, `crates/`, `config/`, `.config/`, `.cargo/`, anything under `tests/<dir>/`) -> the whole suite; anything else -> `--lib` plus every `tests/*.rs` whose name or body mentions a segment of the changed path, with a stoplist for segments that name nothing (`mod`, `lib`, `src`, `plugins`, …). Markdown goes through the same name match, so `docs/agents.md` reaches the host-docs test. Past three quarters of the suite the selection stops paying and the script runs everything instead. The same token list then trims the unit-test binary (`-E 'kind(lib) & (test(~seg)|…) | !kind(lib)'`): it links in seconds but ran for 76s, essentially all of it in TUI tests no other module can reach. `RTOK_CHANGED` replaces the git query so the mapping is exercisable without staging a diff.
Check: the mapping on representative diffs, and a measured before/after rather than an assertion.
Complexity: 2/5 — one profile stanza and one script; no crate code.
Status: done 2026-09-18
Check result: artifacts, dev profile before -> after: rtok lib-test 95 -> 88 MB, proxy 67 -> 62 MB, plugins_e2e 62 -> 57 MB (~7-8% each), all 42 test targets rebuilt. Mapping: `tests/proxy.rs` -> 1 target and no unit tests; `src/plugins/archive/mod.rs` -> 10 targets + `test(~archive)`; `src/tui/view.rs` -> 2 targets + `test(~tui)|test(~view)`; `Cargo.toml` -> whole suite. Unit-test trim on a fixed target set: 687 -> 75 tests. End to end in an isolation worktree on one edit to `src/proxy/wire.rs`: `cargo nextest run --workspace` 120s against `tools/test-changed.sh` 54s (10 of 42 targets, 182 tests, all passing) — the 120s is a floor, that run aborted early. `just check` is untouched and stays the gate; the script picks targets by name, so it is an accelerator, not a coverage proof. Note on the worktree: `tests/filter.rs` fails there for an unrelated environment reason — it shells out to `node --test plugins/opencode/rtok.test.ts`, and the byte-identical copy under the scratchpad path loads as CommonJS; in the repository under the pinned node 26.8.2 that file is 5/5 green.
Model: Claude Code / opus-5

**T61.1 `stats` counts injected skill bodies** · T61.2, T61.3, T63.1 · `src/measure/jsonl.rs`, `src/measure/stats.rs`, `src/measure/codex.rs`, `tests/stats_model.rs`, `research.md`
Do: `measure::jsonl` keeps `isMeta` user records with a top-level `sourceToolUseID` as `Injected { tool_use_id, bytes, turn }` (body = the flattened `message.content` text); `stats` folds them per `Skill` tool_use's `input.skill` into a `skills` section — count, bytes, mean, p95, max, est_tokens and `resident` = bytes × the API requests at or after the injection turn — under `mcp` in the table and in `--json` (absent when empty, so the existing goldens hold byte-identical).
Check: `skills_fold_injected_bodies_and_count_resident_requests` (fixture transcript with a 3-line and a 3,000-line body; resident multiplication asserted), `stats_renders_injected_skill_bodies` (binary-level, `tests/stats_model.rs`); measured row in `research.md` §10.2 dated 2026-09-18 — 24 bodies, 792,820 B, `resident` ≈ 96.8 MB over 30 d (`update-config` 391,824 B, `claude-api` 248,816 B, `slint` 35,997 B over 8).
Complexity: 2/5 — one parser arm, one fold, one table section.
Status: done 2026-09-18
Check result: measure 48/48 + stats_model 5/5 green; full `just check` green in the isolation worktree. Deviation: the card expected a `stats_model` golden re-bless — none was needed because the section is absent when no skill body exists; the golden coverage arrives as the new binary test. Also `codex::jsonl_paths` widened to `pub(crate)` for the doctor reuse.
Model: ZCode / GLM-5.3-Flash

**T61.3 `doctor` skill audit** · T63.1, T71.3 · `src/doctor.rs`, `src/report/ai.rs`, `src/report/pdf.rs`
Do: `rtok doctor` gains a `skills` section — one row per listed skill over the documented §10.1 roots (`~/.claude/skills`, `.claude/skills`, `.agents/skills`, the codex/cursor/gemini/copilot user roots, and `<installPath>/skills [id]` of every enabled plugin from `installed_plugins.json`), with description chars, body bytes after frontmatter, invocations in the last 30 d counted the T61.1 way (`-` when no scan ran), and the measured flags `desc>200` / `body>8K (references/)` / `never invoked` (only when invocation data exists) — plus the one-line total; advice only; `doctor --json` carries the same rows.
Check: `skills_audit_flags_the_measured_warns_on_a_vfs_tree` (Vfs tree, four skills, flags + `plugin:<id>` source + fail-open with no data) and `skills_audit_renders_a_section_with_flags` on `to_text`.
Complexity: 2/5 — one probe, one pure row builder, one render section.
Status: done 2026-09-18
Check result: doctor tests green; full `just check` green in the isolation worktree. Deviation: the `docs/` doctor page does not exist — the README's abridged `rtok doctor` example carries the section with the measured totals instead.
Model: ZCode / GLM-5.3-Flash

**T60.8 TUI help overlay and manual refresh** · - · `src/tui/app.rs`, `src/tui/view.rs`
Do: the `KEYS` table in `app.rs` is the one source every hint renders from — the `?` overlay (globals plus the current page's rows), the footer, the Plugins and Sessions status lines are all generated from it; `?` toggles the overlay and `r` re-reads the model before the next tick, both global.
Check: `help_overlay_lists_the_keys_and_toggles` and `r_refreshes_the_snapshot_immediately` (TestBackend), `question_mark_and_r_are_global` and `keys_table_covers_the_row_state_pages` (app), the footer test updated to the generated hints.
Complexity: 1/5 — one table, two keys, one overlay renderer.
Status: done 2026-09-18
Check result: tui 35/35 green, fmt/clippy clean, full `just check` green in the isolation worktree. Deviation: the key handler stays behavioural code — the table is the single rendered source, not the dispatch mechanism; the generated footer replaced the hand-written hint list, and three tests that pinned the old wording were updated in the same commit.
Model: ZCode / GLM-5.3-Flash

**T65.3 `cmd` column-padding collapse** · T65.4 · `src/plugins/cmd/rules.rs`, `tests/cmd_golden/{docker_ps,kubectl_get,ps_aux}.{in,out}`
Do: a rule may set `collapse_columns = true`; `Rule::default()` — every stem without a TOML rule — turns it on after the fixtures won. Runs of two or more spaces fold to one, leading indentation and tabs are kept, a trailing run folds to nothing, and the pass stands down whenever a trace block is in the output (T65.4's indentation is not columnar). `expand <id>` still returns the aligned original.
Check: `collapse_columns_folds_padding_keeps_indent_and_tabs` (pure) and `collapse_shrinks_columnar_output_and_stands_down_for_traces` (≥⅓ saved on a docker-ps fixture; padding survives verbatim beside a traceback). Fixtures per source: docker_ps 3147→1311 B (58 %), kubectl_get 4542→1770 B (61 %), ps_aux 2341→990 B (58 %); beyond the rule cut collapse adds 17/21/15 %. The 16 existing goldens re-blessed unchanged in shape — the 5 trace fixtures byte-identical, `cat` still lossless with the AWS key.
Complexity: 1/5 — one field, one fold pass, one stand-down guard.
Status: done 2026-09-18
Check result: cmd 57/57 green, fmt/clippy clean, full `just check` green up to the in-flight T70.2 red (plugins::archive::pi + its 4 test files — another agent's half-landed work at HEAD, verified pre-existing with my changes stashed; their fixes sit in the working tree).
Model: ZCode / GLM-5.3-Flash

**T59.7 `doctor` names host-native features that duplicate a rtok surface** · I-47 · `src/doctor.rs`, `src/report/ai.rs`, `src/report/pdf.rs`
Do: three duplicate checks under a doctor `overlaps` section — Claude Code auto-memory (on by default where the settings file exists) while `[plugins.memory] recall_tokens > 0` and the plugin is on; OpenCode (its `~/.config/opencode` present) and Cursor (`~/.cursor` present) while `[plugins.archive]` is on — each line naming the rtok config key that turns the duplicate side off and starting with `duplicate:`; no measurement claim, no "saves N".
Check: `overlap_checks_name_the_rtok_off_key_and_stay_off_when_quiet` — all three fire with both sides on, each names its key, none claims a saving, and every single-side-off / host-absent combination is silent; the fixture initializers in `src/report/{ai,pdf}.rs` carry the new field.
Complexity: 2/5 — one probe, one pure rule, one render section.
Status: done 2026-09-18
Check result: doctor 20/20 green (the gate pass also fixed a wrong host in one of the test's quiet cases — `claude` cannot prove the archive side off, `opencode` can), fmt/clippy clean; full `just check` green except the then-in-flight T70.2 reds, which landed fixed in 4701646. Deviation: the `docs/doctor` page does not exist — the checks are visible in the README's abridged `rtok doctor` example via the skills section added with T61.3.
Model: ZCode / GLM-5.3-Flash

**T56.5 ReadFs trait + optional HostFs walk swap** · T56.4 · `src/plugins/read/fs.rs`, `src/plugins/read/mod.rs` (close-out; code landed earlier unclaimed)
Do: production `read`/`resolve` run through `ReadFs`/`read_with`/`resolve_with` with `HostFs`; `Vfs` gains optional symlinks; disk e2e stays, with Vfs twins for three-lines / range / caps / symlink escape. The optional half — swapping production `search`/`tree` from `ignore::WalkBuilder` to `WalkFs`+`HostFs` — only if gitignore parity is measured and the swap stays small.
Check: the audit closes the remainder as not needed: `read` carries the Vfs twins (three_lines/range/numbered/caps/symlink via `read_with`, plus cache twins `*_from_vfs`) and keeps the disk e2e D29 wants; the walk swap stays undone on its own gate — gitignore parity was never measured, and `ignore::WalkBuilder` handles the `.gitignore` semantics (`overrides`, exclude/include from T68.10) a hand-rolled `WalkFs` would have to re-prove, so a swap is a rewrite with no measured win.
Complexity: 2/5 — the landed half was the task; the optional half fails its own precondition.
Status: done 2026-09-18 (close-out; the trait work landed in the T56.x series unclaimed)
Check result: no code change; evidence is the coverage audit above plus the green cmd/tui/doctor scope runs in this session's worktree passes.
Model: ZCode / GLM-5.3-Flash

**T64.3 Prompt-cache FAQ with the measured hit rate per surface** · - · `docs/prompt-cache.md`, `docs/comparison.md`, `README.md`, `site/content/docs/reference/_content.gotmpl`
Do: a `docs/` FAQ section "Does rtok break the prompt cache?" states it per surface with a measured number each — hooks filter once and the host stores the result in its transcript; `inject` byte-stable per turn (tests named); `archive`/`proxy` rewrite only outside `keep_turns` with byte-identical pointers (tests named, hit rate measured and dated); `guard` denials add no bytes; README links the section and `docs/comparison.md` §"The platform itself" points at it instead of restating the number.
Check: the four cited test names verified present (`three_500_budget_800_drops_one_and_is_byte_stable`, `session_start_has_nudges_once_and_stable`, `six_turn_fixtures_archive_only_turns_1_and_2_on_every_wire`, `live_blobs_shrink_stably_outside_the_working_edge`); README link present; `just readme-check` exit 0.
Complexity: 1/5 — one page, one pointer, one link.
Status: done 2026-09-18
Check result: commit 869baa7. Page cites `hit=97.5%` overall and `95.2%` codex (`rtok stats`, 2026-09-18, commands quoted) and zero cache-bust rows from `rtok report`'s Cache section on this machine.
Model: ZCode / GLM-5.3-Flash

### T50.1. More `cmd` filter families

From I-05; re-scoped by the competitive gap review (`research.md` §9.3, "Command output"). Today: formatters for cargo/git/pytest/jest/vitest/go test/ls/find/tree, nine TOML rules (`rules/default.toml`: grep, rg, sed, cat, make, curl, npm, pnpm, node), and `Rule::default()` (40 lines, head 10 / tail 10, dedupe) for every other stem — so docker, kubectl, gh, aws, pip, python, mvn, gradle, dotnet, tsc, eslint are capped, not passed through, but their error lines and summaries are cut by position, not by meaning. rtk ships 100+ per-command filters; the parity target is a per-family rule for every family that carries real bytes, each one measured. Rules are data, so this task adds TOML and fixtures, no Rust.
Done when:
1. Evidence first: `rtok stats` over real transcripts ranks Bash families by after-bytes where `Measurement.kind = rule` fell back to the default rule (`bash_families` split by kind; a `stats` column, not a new command); the top-20 land in `research.md` with date and command.
2. One `[stem]` rule per family from that list (expected from rtk's list and §2: docker / docker compose, kubectl, gh, aws, pip / uv, python tracebacks, go build / vet, cmake / ctest, mvn / gradle, dotnet, tsc, eslint, brew / apt), each with `keep` patterns for its error and summary lines and a `tests/cmd_golden` fixture with before/after bytes that keeps failures and the `expand <id>` trailer.
3. `Measurement` rows per family show the saving; the family table in the cmd docs page cites them. A family whose rule does not beat the default rule on its fixture is not added (the default already wins there).
4. Families where a rule cannot keep the signal (structured tables, grouped diagnostics) are listed in the card for T58.5, with the fixture that shows why.

Execution plan (T50.1, Cursor / composer 2.5): isolated worktree `t50.1`. (1) Add `filter` + `bash_default` columns to `rtok stats` from transcripts + `cmd` `Measurement` rows (`kind = rule`, default stem). (2) Record top-20 default-rule families in `research.md` (`rtok stats`, 2026-09-18). (3) Add `[stem]` rules + `tests/cmd_golden` fixtures where the rule beats `Rule::default()` on the fixture; cite `Measurement` rows in `docs/cmd-rules.md`. (4) List table/grouped families on T58.5 with fixtures. No Rust beyond the stats column.


### T64.1. `cmd` grouping pass: files by directory, diagnostics by type

From `research.md` §11 (rtk's four strategies, 2026-09-18). rtk groups similar items — files by directory, errors by type; rtok's rule engine (`src/plugins/cmd/rules.rs`) only keeps, drops, cuts by position and folds adjacent duplicates, and the `ls`/`find`/`tree` formatters just take the first 40 lines.
Done when a rule may set `group = "dir"` (path-per-line output: `find`, `rg -l`, `git status` untracked, `ls -R`) or `group = "diag"` (diagnostics keyed by code or rule id: `cargo` `error[E…]`, `tsc` `TS…`, `eslint` rule, `pytest` exception class), the pass rewrites the lines as `dir/ (N files): a, b, c …` and `E0308 ×N: first message (file:line, …)` before the head/tail cut, stays lossless (raw output archived as today, `expand <id>` trailer), and a fixture per family in `tests/cmd_golden` records before/after bytes that beat the same rule without `group` — a family that does not win is not switched on. T58.5 keeps its per-family formatters; this is the generic pass a TOML rule turns on.

Execution plan:
1. Parse `group = "dir" | "diag"` on a Rule; apply the pass after drop/keep/collapse/dedupe and before the head/tail cut.
2. `dir` rewrites path-per-line output as `dir/ (N files): a, b, c …`; `diag` keys rustc `E…`, `tsc` `TS…`, `eslint` rules, `dotnet` `CS…`, pytest/python exception classes as `E0308 ×N: first message (file:line, …)`.
3. Golden per family vs the same rule without `group`; enable the field in `rules/default.toml` only on a win. Leave T58.5 docker/kubectl/ps formatters; drop the ls/find take(40) stubs so a TOML rule can run. Do not implement T64.2.

Execution plan (T64.1, Cursor / grok 4.6): isolated worktree `t64.1` from `t58.5`. (1) `group = "dir" | "diag"` on `Rule`, applied after drop/keep/collapse/dedupe and before the head/tail cut. (2) Goldens per family vs the same rule without `group`. (3) Enable in `rules/default.toml` only on a win; leave T58.5 docker/kubectl/ps formatters; drop ls/find take(40). (4) Do not implement T64.2.

Shipped: `dir` on `ls` (248→49 B), `find` (539→83 B), `rg -l` (459→83 B); `diag` on `tsc` (1619→107 B), `eslint` (749→48 B), `cargo check` (749→34 B), `dotnet` (849→71 B). Skipped: `grep` (`-n` hits unchanged), `tree` (keep take(40) formatter), `git status` / `pytest` (formatters), `python` (T50.1 traceback does not shrink as `NameError ×1`), `go` (no error codes). Lossless archive + expand trailer unchanged.

Status: done 2026-09-18
Check result: `cargo test --lib -- cmd::` 60 passed in worktree `t64.1`; `group_goldens_beat_the_same_rule_without_group` pins the table.
Model: Cursor / grok 4.6

### T58.5. `cmd` formatters for structured families

Follow-up of T50.1 step 4 (`research.md` §9.3, "Command output"). A TOML rule keeps lines by pattern and position; families whose signal is a table (`docker ps` / `kubectl get` / `ps aux`) need a formatter, like the existing cargo/git/pytest ones in `formatters.rs`.
Done when:
1. Only families named by T50.1 step 4, each with the fixture that showed the rule losing the signal.
2. One formatter per family in `formatters.rs`, returning `None` on unrecognized output so the rule path stays the fallback; failures and the `expand <id>` trailer kept; golden fixtures before/after.
3. `Measurement` rows `kind = formatter` per family beat the rule's row on the same fixture; the cmd docs page table cites them.
4. ≤ 200 LOC per commit: split by family group (containers, TypeScript tooling, JVM) if needed.

Execution plan (T58.5, Cursor / grok 4.6): isolated worktree `t58.5` from `t50.1`. (1) `docker ps` + `kubectl get` formatters in `formatters.rs` (one row per object, `None` on non-table output) + goldens that beat `Rule::default()` on the T50.1 fixtures. (2) `ps aux` formatter the same way. (3) Cite `kind = formatter` before/after bytes on `docs/cmd-rules.md`. (4) Close the card into `done.md`. No tsc/eslint/mvn/gradle formatters.

Shipped: `docker ps` 3147→1190 vs rule 1311, `kubectl get` 4542→1731 vs rule 1770, `ps aux` 2341→870 vs rule 990 (`kind = formatter`, one row per object). tsc/eslint/mvn/gradle formatters skipped (T50.1 rules already beat default on keep-error lines).

**T70.4 Cursor plugin shortens MCP results the host launched** · T59.4 · `src/mcp/wrap.rs`, `src/hooks/{types,mod}.rs`, `src/agents/cursor/mod.rs`, `plugins/cursor/hooks/hooks.json`
Do: Cursor `postToolUse` (verified 2026-09-18: input `tool_output`; output `updated_mcp_tool_output` replaces MCP results only; `afterMCPExecution` / `afterShellExecution` have no documented replacement) maps onto `rtok hook PostToolUse --host cursor`. Long MCP `content[].text` is shortened through T59.4 `shorten_result`, lossless `expand <id>` trailer, `Measurement { plugin = "archive", kind = "mcp" }`. Skip `mcp_server_name == rtok` and tool `expand`. Fail open; never rewrite the call.
Check: `cursor_mcp_post_tool_use_shortens_only_foreign_long_results`, `shorten_result_records_archive_mcp_above_threshold`, `post_tool_use_shortens_long_mcp_results_and_skips_small_and_rtok`; `tests/host_docs.rs` green; host table unchanged (no bless).
Complexity: 3/5 — one wrap helper, Cursor field map, one replacement stdout shape.
Status: done 2026-09-18
Check result: wrap/hooks/cursor lib tests and `cursor_plugin` / `host_docs` / `agents_doc` / `mcp_wrap` green. `just check` red only on a pre-existing trycmd bash-completion snapshot (`--context` from T67.2), not this hook.
Model: Cursor / grok 4.6

**T73 Cycle demon surfaces around a binary replace** · `src/demon.rs`, `tests/demon.rs`
Do: hidden `rtok demon upgrade` snapshots kernel-live services, stops them (marker + wait), runs `ketch upgrade rtok --yes` / `rtok-update` / `RTOK_UPDATE_CMD`, and starts the same set even when the replace failed; the supervisor respawns from the on-disk path. 2026-09-21 (ZCode / GLM-5.3): `quiesce` between stop and replace — a supervisor that lost the race between its child's death and its own signal could still respawn the surface, rewriting the state file and re-locking the store the replace touches (seen 3× on 2026-09-19–20; one occurrence auto-reverted an unrelated markdown push); the respawn is retired with the `start` this-boot guard and the upgrade waits (≤4 s) until every stopped service's file is gone and stays gone, failing loudly instead of racing the replace; the stop folds into the replace arm so a failed stop still restarts what was up.
Check: `tests/demon.rs` — live mcp is down during the replace command (no state file), up afterwards; a failing replace still leaves mcp running; unit `quiesce_retires_a_late_respawn_state_file`, `quiesce_is_ok_when_nothing_respawned`; `just check`.
Complexity: 2/5 — one loop; reuses `read`/`boot_time`/`process_kill`.
Status: done 2026-09-21
Check result: e2e 10/10 standalone after the fix (3 failures on 2026-09-19–20 before); full gate green.
Model: ZCode / GLM-5.3 (race fix + close-out; feature skeleton by Cursor / grok 4.6)

**T84 Auto-revert opens a PR that brings the reverted work back** · `.github/workflows/ci.yml`
Do: `revert-on-failure` used to push the revert to `main` and stop, leaving the reverted work only in history. After the revert lands (step `id: revert`, `reverted=true` written only after the push — every skip path leaves it unset) a second step pushes `revert-<original branch>` — the head branch of the merged PR the push came from (`gh api repos/{repo}/commits/{sha}/pulls`, merged only), else `revert-<short sha>` for a direct push to `main`, suffixed with the short sha if the branch exists — holding one commit that reverts the revert with the original author, and opens a draft PR back to `main`. Draft because `check` skips drafts and the tree is known-red; a PR opened with `GITHUB_TOKEN` starts no workflow, so the first CI run is the fix-up push or "ready for review". Job permissions gain `pull-requests: write`. Open for the creator: the repo setting "Allow GitHub Actions to create and approve pull requests" is off (`gh api repos/listepo/rtok/actions/permissions/workflow`, 2026-09-21); until it is on, `gh pr create` fails after the revert has already landed.
Check: `actionlint` clean; git commands of both steps replayed in a scratch repo; `just check`.
Complexity: 2/5 — one workflow step, no product code.
Status: done 2026-09-21
Check result: `actionlint` 1.7.12 + `shellcheck` 0.10.0 clean; scratch replay (single commit, multi-commit push, merge commit): `main` equals the pre-push tree, the branch equals the failed push, 1 commit ahead, author preserved; `just check` exit 0. The `gh` calls are not exercised until a real red push.
Model: Claude Code / claude-fable-5-1

**T85 Kimi Code plugin tree (`plugins/kimi/`)** · `plugins/kimi/{kimi.plugin.json,README.md}` (new), `src/agents/kimi/mod.rs`
Do: creator request 2026-09-21 — a host plugin for Kimi Code CLI + Desktop, like Cursor's. Kimi's plugin format is one manifest at the plugin root, `kimi.plugin.json`, that carries `hooks` (the `[[hooks]]` shape: `event` / `matcher` / `command` / `timeout`) and `mcpServers` (`{command, args}`) together — D21's "plugin and MCP as one unit" by construction. The manifest holds the nine Claude `ENTRIES` as `rtok hook <event>` with `timeout: 5` and `mcpServers.rtok` → `rtok mcp` directly: one command works on macOS and Windows, and I-37 records that Cursor's `scripts/mcp.*` launchers never run, so none are copied. A missing `rtok` fails open (only exit 2 blocks); the ketch hint is in the README. Desktop manages the same plugins (Settings → Plugins; its docs link the CLI plugin page and say Desktop and CLI share plugin-related settings; on this machine `Kimi Code.app` keeps its `kimi` binary and server under `~/.kimi-code/`). Kimi copies a plugin into `$KIMI_CODE_HOME/plugins/managed/<id>/` and runs the copy, so `PluginLink` does not apply; install is `/plugins install <path>` inside Kimi, and the installer offer is T86. `plugins/` already ships whole in the release archive (`Cargo.toml` `include`). README states two limits: plugin hooks run with cwd = plugin root (documented by Kimi), so the `.rtok.toml` / `.env` project layer — found from the process directory — does not apply, while the project itself still comes from stdin `cwd`; and the cwd Kimi gives a plugin's stdio MCP server is undocumented.
Check: `agents::kimi::tests::plugin_manifest_matches_the_installer` — manifest hooks equal `ENTRIES` with `Config::default().setup.hook_timeout_s`, every command passes `is_ours`, `mcpServers` is exactly `rtok`; `host_docs`; `just check`.
Complexity: 2/5 — two data files and one unit test; no product code.
Status: done 2026-09-21
Check result: the new test, `host_docs` (2) and `declared_host_plugins_have_distinct_sources` pass; `cargo nextest run --workspace --no-fail-fast` 1079 passed, 4 skipped (a first fail-fast run lost `demon::status_asks_the_kernel_rather_than_believing_the_state_file` once, green on the rerun, untouched by this change); lint, `build-min`, `dup` exit 0; `rustfmt --check` clean on the touched file — workspace `fmt-check` was red only on another session's uncommitted `src/hooks/types.rs`. Not verified on a live Kimi install (the `kimi` binary is outside this agent's shell allowlist and `/plugins install` is TUI-only): that the plugin loads, and the MCP server's working directory — T86 starts with that check.
Model: Claude Code / claude-fable-5-1

**T98 `rtok hook` reads Grok Build's hook envelope** · `src/hooks/types.rs`, `src/hooks/mod.rs`
Do: creator request 2026-09-21 — a host plugin for Grok Build (xAI's `grok` CLI), like Claude's and Cursor's; this is its precondition. Grok answers hooks in Claude's shape (`hookSpecificOutput.updatedInput` / `additionalContext`, `permissionDecision`; only exit 2 or an explicit `deny` blocks), but its stdin is camelCase: `sessionId`, `toolName` with Grok's own names (`run_terminal_command`, `read_file`), `toolInput`, `toolResult`, `toolUseId`, `permissionMode`, `workspaceRoot`; only `hook_event_name` keeps Claude's key. Before this every rtok hook under Grok parsed `tool_name = None` and did nothing — including the `~/.claude/settings.json` / `~/.cursor/hooks.json` hooks Grok imports by default. `HookInput::adapt_grok` lifts those keys, maps only `run_terminal_command` → `Bash` and copies a Bash `output_for_prompt` to `stdout`. `read_file` keeps its name: Grok blocks a call whose `updatedInput` fails the tool schema, and rtok's Read rewrite is unverified against `read_file` (T100). Dispatch detects Grok from the runner's reserved `GROK_HOOK_EVENT` (or `--host grok`) ahead of `--host cursor` / `copilot`, because an imported hook still receives Grok's envelope; output passes through. Evidence: Grok's bundled `~/.grok/docs/user-guide/10-hooks.md` (= https://docs.x.ai/build/features/hooks). No new dependency.
Check: `hooks::types::tests::grok_lifts_camel_case_and_maps_only_the_terminal` (documented Pre/PostToolUse payloads, `read_file` untouched, unknown keys kept); `hooks::tests::grok_pre_tool_use_rewrites_the_terminal_command` (a Grok `git status` comes back with `hookSpecificOutput.updatedInput.command` rewritten); `just check`.
Complexity: 2/5 — one adapter beside `adapt_copilot` / `adapt_devin`, one dispatch arm.
Status: done 2026-09-21
Check result: both tests pass; `just check` exit 0 — 1090 passed, 4 skipped. Not verified on a live Grok session (a headless run bills the creator's xAI account).
Model: Claude Code / claude-opus-5

**T99 Grok Build plugin tree (`plugins/grok/`)** · `plugins/grok/{.grok-plugin/plugin.json,hooks/hooks.json,.mcp.json,README.md}` (new), `tests/grok_plugin.rs` (new)
Do: after T98. Grok plugins are Claude-compatible directories: optional `.grok-plugin/plugin.json`, `hooks/hooks.json` in Claude's nested shape, `.mcp.json`. Hooks: `rtok hook <event> --host grok` with `timeout: 5` (Grok's PostToolUse default is 600 s) on PreToolUse (Bash), PostToolUse (no matcher — Grok's matcher is a regex, so Claude's `*` is not used), UserPromptSubmit, SessionStart, PreCompact, PostCompact, SessionEnd = Claude `ENTRIES` minus Read and Skill. `.mcp.json` → `rtok mcp` (D21: one unit). Install `grok plugin install <path> --trust` or `~/.grok/plugins/rtok/` + `[plugins].enabled`. README states the double-fire: Grok imports rtok's Claude hooks and `~/.claude.json` MCP by default, so keep either the plugin (with `[compat.claude] hooks = false`, `mcps = false`) or the Claude install; plus the limits (no Read/Skill, Grok drops UserPromptSubmit and SessionStart context, live load unverified) and `## Docs`.
Check: `tests/grok_plugin.rs` — manifest name `rtok`, `.mcp.json` exactly `rtok`, hooks exactly the filtered set; `host_docs`; `just check`.
Complexity: 2/5 — data files and one integration test; no product code.
Status: done 2026-09-21
Check result: `grok_plugin` (3) and `host_docs` pass inside `just check` exit 0 — 1090 passed, 4 skipped. The expected hook list is written out in the test because `agents::claude::ENTRIES` is `pub(super)`; T100's `src/agents/grok` moves the check next to `ENTRIES`. Not verified on a live Grok session.
Model: Claude Code / claude-opus-5

**T77 ZCode plugin offered on install (--yes), singleton with the config surfaces** · `src/agents/zcode/{mod.rs,README.md}`, `plugins/zcode/`, `tests/agents_install.rs`, `tests/trycmd/{doctor,report-md}.toml`, `docs/agents.md`
Do: `rtok agents install zcode --yes` links `plugins/zcode` to `~/.zcode/cli/plugins/local/rtok` and lists it in `plugins.dirs` in `~/.zcode/cli/config.json` — read from the installed app (v0.2.0, `glm/zcode.cjs`): every `plugins.dirs` entry is an inline plugin root, enabled by default, marketplace id `inline`; `remove` unlinks and drops the entry. The plugin is hooks and MCP as one unit (D21): `.zcode-plugin/plugin.json` + auto-discovered `hooks/hooks.json` (the five documented entries, `type: "process"` via `${ZCODE_PLUGIN_ROOT}/scripts/hook.sh`) + `.mcp.json` (`scripts/mcp.sh`); the launchers resolve rtok from PATH or the ketch store, fail the hook open and the MCP loudly with the ketch hint. While the plugin is linked it is the only call path: setup strips its own `hooks.events` entries and `mcp.servers.rtok` instead of re-adding them; a declined offer adds no `plugins.dirs` entry and a stale one is dropped.
Check: unit `dry_run_offer_names_plugin_and_local`, `yes_links_plugin_and_lists_dirs`, `linked_plugin_is_the_only_call_path`, `declined_offer_adds_no_dirs_entry_and_drops_a_stale_one`, `remove_unlinks_plugin_and_drops_dirs_entry`; `agents_install` / `agent_remove` with `--yes`; `host_docs`, blessed `agents_doc`; `just check`.
Complexity: 3/5 — mirrors the Cursor offer (T10.5) plus the `plugins.dirs` listing and a hooks+MCP singleton.
Status: done 2026-09-21
Check result: 7/7 zcode unit tests; `agents_install` 9/9 and `agent_remove` green with `--yes`; `just check` green after updating the two trycmd snapshots (`doctor`, `report-md`) that carry the module line; smoke run under a temp HOME shows the first `--yes` run writing only `+ plugin` and `+ plugins.dirs +=`.
Model: ZCode / GLM-5.3

**T112 Restart prompt spinner no longer erases the typed answer** · `src/agents/restart.rs`
Do: the `[y/N]` restart prompt after `agents install|uninstall` redrew its spinner every 80 ms with `\r\x1b[K` + the prompt, wiping the terminal echo of what the user was typing. Now the row is cleared once on the first paint; each frame swaps only the column-0 glyph under cursor save/restore (`ESC 7`/`ESC 8`); an answered prompt blanks the glyph one row up instead of reprinting the prompt, and an unanswered one ends the line.
Check: `agents::restart` tests assert exactly one `\x1b[K` per prompt, in-place glyph frames, one trailing newline on timeout, and the answered-row cleanup; `just check`.
Complexity: 1/5 — three small write helpers and test updates.
Status: done 2026-09-21
Check result: `agents::restart` 13/13 pass. Known limit: an answer that wraps past the terminal width puts the glyph on the wrong row (cosmetic).
Model: Claude Code / claude-opus-5

**T119 CodeQL on GitHub and locally** · `.github/workflows/codeql.yml`, `justfile`, `mise.toml`, `toolchain.md`
Do: creator request 2026-09-21. `codeql.yml` scans `actions`, `javascript-typescript`, `python` and `rust` on push/PR to main, weekly and on dispatch (`github/codeql-action@v4`, `build-mode: none`, `security-and-quality`); repo default setup is `not-configured`, so the advanced workflow does not collide with it. `codeql` 2.27.0 is pinned in `mise.toml`; `just codeql [langs…]` copies the tracked files (working-tree content) to `target/codeql/src`, builds one database per language, writes `target/codeql/<lang>.sarif` and fails on any result. Kept out of `just check` because it takes minutes.
Check: `just codeql` runs all four languages; `actionlint` clean on the workflow; `just check`.
Complexity: 2/5 — one workflow, one recipe, one pin.
Status: done 2026-09-21
Check result: local scan — actions 15 (13 `actions/unpinned-tag` across all workflows, 2 `actions/missing-workflow-permissions` in `ci.yml`), javascript-typescript 0, python 0, rust 1 (`rust/log-injection` in `tests/web.rs:223`, test code); 30 of 190 Rust files extract with errors under `build-mode: none`. Findings are left for a follow-up task. `just check` red only on load flakes (`claude_plugin` ×2, `cli_trycmd`, `graph::watch` watchman fallback, earlier ENOSPC at 2 GB free); each passes alone, none touches this change.
Model: Claude Code / claude-opus-5
