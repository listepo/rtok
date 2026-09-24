# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T83.2 | todo | P1 | 3 | 0% | |
| T83.4 | todo | P1 | 3 | 0% | |
| T87 | in progress | P1 | 2 | 70% | Claude Code / claude-fable-5-1 |
| T88 | todo | P1 | 2 | 0% | |
| T89 | todo | P1 | 3 | 0% | |
| T97 | in progress | P1 | 3 | 95% | Claude Code / claude-fable-5-1 |
| T124 | todo | P3 | 2 | 0% | |
| T131 | todo | P2 | 3 | 0% | |
| T132 | todo | P2 | 2 | 70% | |
| T134 | todo | P1 | 2 | 0% | |
| T156 | todo | P3 | 3 | 0% | |
| T159 | todo | P2 | 4 | 0% | |
| T163 | in progress | P2 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T163.2 | in progress | P2 | 3 | 5% | Claude Code / claude-sonnet-5 |
| T163.3 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T163.4 | in progress | P2 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T163.8 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T163.9 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T178 | in progress | P1 | 4 | 75% | Claude Code / claude-opus-5-5 |
| T241 | todo | P2 | 3 | 0% | |
| T246.5 | todo | P1 | 2 | 0% | |
| T262.3 | todo | P2 | 2 | 0% | |


### T83.2. `plugins::cmd::run::tests` shell-spawn family fails on Windows

The `cfg(windows)` `default-filter` in `.config/nextest.toml` (T82) skips four tests on `windows-latest`: `one_arg_compound_command_runs_as_one_script`, `exit_3_is_preserved`, `printf_two_lines_exit_0_no_trailer`, `three_runs_stats_plugin_cmd_json_has_rows`. Find out whether `plugins/cmd/run.rs` hardcodes a POSIX shell (`sh -c`) or exit-code assumption that needs a `cfg(windows)` branch (`cmd /C` or PowerShell), or the tests themselves assume a Unix shell on PATH; fix accordingly and delete the line. One family split out of the original T83 (all families and sources: `done.md` → T83.1, which fixed the log/demon rotation family). Closing criterion for the whole split: once every T83.x below has emptied its line from the `cfg(windows)` override in `.config/nextest.toml`, delete the override and move `windows` out of `continue-on-error` into `revert-on-failure`'s `needs` (or into the `check` matrix if `just check` runs on Windows).

Check: the four tests pass in the `windows` CI job; `just check` stays green.

### T83.4. `agents_install` / `cursor_plugin` / `pi_plugin` / `opencode_plugin` symlink and path expectations fail on Windows

Ten tests across four binaries: `agents_install::{list_reports_installed_modules_per_host, setup_twice_takes_one_backup_and_says_already_installed}`, `opencode_plugin::dry_run_offers_the_plugin_and_writes_nothing`, `cursor_plugin::{setup_cursor_dry_run_offers_plugin, setup_cursor_yes_links_plugin_without_mcp_json, setup_cursor_clears_leftover_mcp_when_plugin_already_linked}`, `pi_plugin::{setup_pi_dry_run_offers_plugin, setup_pi_yes_links_remove_unlinks, pi_extension_unit_test_with_fake_rtok}`, `filter::opencode_plugin_unit_test_with_api_mock`. Likely a symlink family: `std::fs::symlink` needs Developer Mode or admin on Windows, and/or the assertions compare `/`-joined paths against a host that prints `\`. Decide per test whether the installer needs a Windows fallback (junction/hardlink/copy) or the fixtures need `Path`-based comparison instead of string paths. One family split out of the original T83; see T83.2 for the closing criterion.

Check: the ten tests pass in the `windows` CI job; `just check` stays green.

### T87. `rtok hook <event> --host devin` reads Devin's payload

Creator request 2026-09-21: a host plugin for Devin CLI + Devin Desktop, like Claude's and Cursor's. Devin Desktop is the renamed Windsurf (2026-06-02); its local agent and the `devin` CLI read the same files. Devin's hooks are Claude-shaped on the way out — it reads `hookSpecificOutput.updatedInput` / `additionalContext`, `decision: block`, exit 2 blocks and any other non-zero exit is logged without blocking (fail open holds) — so no output translation is needed. The way in differs: tool names are Devin's own (`exec`, `read`, `edit`, `write`, `grep`, `glob`, `mcp__<server>__<tool>`), `tool_response` is `{success, output, error}`, compaction is one event `PostCompaction` (there is no PreCompact), and the project root arrives as env `DEVIN_PROJECT_DIR`. Evidence: https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks, https://docs.devin.ai/cli/extensibility/hooks/overview; on this machine `~/.config/devin/config.json` already carries a `"hooks"` key in exactly that event → `[{matcher, hooks: [{type, command, timeout}]}]` shape.

Plan:
1. `src/hooks/types.rs` — `HookInput::adapt_devin(event)` beside `adapt_cursor` / `adapt_copilot`: `exec` → `Bash` (add `exec` to `canonical_tool_name`; `read` / `edit` / `write` already map), `PostCompaction` → `PostCompact`, `tool_response.output` lifted to where the plugins read a Bash result, `cwd` from `DEVIN_PROJECT_DIR` when stdin has none.
2. `src/hooks/mod.rs` — one `else if cfg.hook.host == "devin"` arm in `dispatch_owned_strict`; output passes through unchanged.
3. Unit tests with the docs' payloads verbatim (`exec` + `{command, shell_id}`, PostToolUse `{success, output, error}`, `PostCompaction` with `summary`).
Verify first: capture one real PreToolUse / PostToolUse / SessionStart payload from Devin with a logging hook (`tee`) and confirm whether stdin carries `cwd` and which key `read` uses for its path (`file_path` vs `path`) — the docs do not say. The agent's shell allowlist blocks the `devin` binary, so the creator runs the capture or allows it.
Progress: steps 1–3 are written and their tests pass (`devin_maps_tool_names_result_project_dir_and_compaction`); the `--host` list grew by `devin` in `config/default.toml`, `docs/config.md`, the `src/cli.rs` help line and its five `tests/trycmd` snapshots — one-line edits, over the 3-file limit by necessity. Open: the capture and the captured-payload half of the Check; not committed until then.

Check: the unit tests above pass; `rtok hook PreToolUse --host devin` on the captured `exec` payload returns the same decision as `rtok hook PreToolUse` on the equivalent Claude `Bash` payload; an unknown tool returns `{}`; `just check`.

Extra tests (creator request 2026-09-21): garbage and empty stdin with `--host devin` print `{}` and exit 0; `mcp__<server>__<tool>` names pass through unmapped; PostToolUse with `{success: false, output: "", error: "…"}` does not panic and lifts no stdout; `cwd` from stdin wins over `DEVIN_PROJECT_DIR`; an integration test that `rtok hook PostCompaction --host devin` reaches the PostCompact plugins.

### T88. Devin plugin tree (`plugins/devin/`)

After T87. Devin's plugin format is a directory: manifest `.devin-plugin/plugin.json` (only `name` is required), `hooks.json` and `.mcp.json` at the plugin root, optional `skills/<name>/SKILL.md`. One tree loads in the CLI and in Devin Desktop (hooks load "in local Devin agents only — the CLI and Devin Desktop"), so it is D21's one unit for both surfaces. Local install is `devin plugins install --local <dir>`. Evidence: https://docs.devin.ai/cli/extensibility/plugins/overview.

Plan:
1. `plugins/devin/.devin-plugin/plugin.json` — `name: "rtok"`, version, description, homepage; `plugins/devin/.mcp.json` — `mcpServers.rtok` → `rtok mcp` directly (I-37: launcher scripts never run; the ketch hint lives in the README).
2. `plugins/devin/hooks.json` — `PreToolUse` (`^exec$`, `^read$`), `PostToolUse` (all), `UserPromptSubmit`, `SessionStart`, `PostCompaction`, `SessionEnd`, each `rtok hook <event> --host devin`, timeout 5.
3. `plugins/devin/README.md` — install by hand, files, `## Docs` (plugins, hooks, MCP, skills, Desktop pages).
4. `agents::devin::tests::plugin_manifest_matches_the_installer` lands with T89; here a `tests/` check that the three JSON files parse, every hook command passes `is_ours`, and `mcpServers` is exactly `rtok`.
Verify first: whether a plugin's `hooks.json` puts event names at the top level (like `.devin/hooks.v1.json`) or under a `"hooks"` key — the overview page does not show the file.

Check: `host_docs` and the new manifest test green; `just check`.

### T89. `rtok agents install devin` — CLI and Desktop, plugin as the singleton

After T88. New host `devin` in `src/agents/devin/` (`mod.rs` + `README.md` with the module table and `## Docs`), registered in `HOSTS` and `host()`. Variants: CLI (`devin` on PATH) and Desktop (`Devin.app`), same files. Without the plugin, install edits the user files directly: the `"hooks"` key of `~/.config/devin/config.json` (`%APPDATA%\devin\` on Windows) and `mcpServers.rtok` in `~/.config/devin/mcp_config.json`. The plugin offer prints the exact `devin plugins install --local <resolved plugins/devin path>` line — rtok does not write Devin's plugin store, its on-disk location is undocumented (the Kimi rule from T86). D21 singleton: while the plugin is installed, setup strips rtok's own hooks and `mcpServers.rtok` from the user files instead of adding them. The existing `windsurf` host stays untouched for machines that still run Windsurf; retiring or aliasing it is not part of this task.

Plan:
1. `src/agents/devin/mod.rs` — `Agent` impl on the Kimi/Cursor pattern, reusing `edit_json` and the Claude `ENTRIES` mapped to Devin event and matcher names (one source of truth); `[agents.devin]` paths in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`.
2. Unit tests: install writes the hooks and the MCP entry and is idempotent; remove takes back exactly ours; foreign hooks in `config.json` survive both (this machine's file has other tools' hooks under every event); the offer names `plugins/devin` and `devin plugins install --local`; the plugin manifest equals the installer's entries.
3. `docs/agents.md` via `RTOK_BLESS=1` on `tests/agents_doc.rs`; `tests/trycmd/agents-list*.toml` re-blessed; `tests/agents_real_config.rs` case for the real `~/.config/devin/config.json`.
Verify first: how an installed plugin can be detected (a documented path or a `devin plugins list` output) — if neither is stable, `installed()` reports `plugin` only from a marker rtok can honestly read, otherwise says "unknown" rather than guessing.
Over the ≤200 LOC / ≤3 files limit as written — split into T89.1 (host + hooks/MCP install) and T89.2 (plugin offer + singleton + docs) when claiming.

Check: the unit tests above; `rtok agents list` shows `devin`; `agents_doc`, `host_docs`, `config_coverage` green; `just check`.

### T97. `rtok agents install kilo` — Kilo Code: the shared OpenCode plugin plus `kilo.json` MCP

Creator request 2026-09-21: a host plugin for Kilo Code CLI + desktop. Kilo Code 7 is rebuilt on the OpenCode server: the CLI (`kilo`, `npm i -g @kilocode/cli`) and the VS Code extension (`kilocode.kilo-code`) share one config — `~/.config/kilo/kilo.json[c]` globally, `kilo.jsonc` / `.kilo/kilo.jsonc` per project; the legacy `mcp_settings.json` is no longer read (v7.0.33+). Plugins are OpenCode-shaped TS modules (`tool.execute.before` / `tool.execute.after`, `shell.env`, …) loaded from `~/.config/kilo/plugin/` or `.kilo/plugin/`; MCP is `mcp.<name> = {type: "local", command: [..], enabled}` — the entry `agents::opencode::register_mcp` already writes. Creator decision 2026-09-21: reuse `plugins/opencode/rtok.ts` as is — it imports only `node:child_process` — so there is no `plugins/kilo/` tree (the omp rule from T92). Evidence (fetched 2026-09-21): https://kilo.ai/docs/automate/extending/plugins, https://kilo.ai/docs/automate/mcp/using-in-kilo-code, https://kilo.ai/docs/code-with-ai/platforms/cli.

Plan:
1. `src/agents/kilo/mod.rs` + `README.md` (`## Docs`: plugins, MCP, CLI, skills, custom rules): `HostPlugin { src_rel: "plugins/opencode/rtok.ts", host: "Kilo Code", dest: <config dir>/plugin/rtok.ts }`; MCP through the OpenCode helpers given Kilo's path (the `for_kind` trick or a path parameter — no copy of the logic). Variants: CLI (`kilo` on PATH) and the VS Code extension, same files. `support`: `plugin` → `Flag("--yes")`, `mcp` → yes, `hooks` → `No` (in-process plugin hooks; the plugin owns that path), `proxy` → decide from the OpenCode host's rule for `provider.*.options.baseURL`. `[setup.kilo] config_path = "~/.config/kilo/kilo.json"` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`. Registered in `HOSTS` and `host()`.
2. `plugins/opencode/README.md` gains the Kilo section and links.
3. Unit tests: offer names `plugins/opencode/rtok.ts` and the ketch line; `--yes` links and writes `mcp.rtok`, second apply `NO_CHANGES`, remove takes back exactly ours and keeps foreign servers; `docs/agents.md` blessed; `tests/agents_install.rs` `hosts()`; `tests/trycmd/agents-list*.toml` re-blessed.
Verify first (check `command -v kilo` and `~/.config/kilo/`; otherwise the creator runs it): (a) the linked plugin loads under `kilo` and a bash call goes through `rtok run`; the plugin directory is `plugin/` (Kilo docs) and not OpenCode's `plugins/`; (b) the extension reads the same `~/.config/kilo/` plugin and `mcp` entries; (c) the plugin passes `--host opencode`, so Kilo rows are labelled `opencode` — accept, or let the host name come from the environment; (d) a user's `kilo.jsonc` with comments hits T79: until T79 lands the host refuses with T79's message and never rewrites a JSONC file.
Over the ≤200 LOC / ≤3 files limit as written — split into T97.1 (host + plugin link + config) and T97.2 (MCP + docs bless) when claiming.

Progress: host written and green in one change (not split — the rest were one-line list edits). Settled from Kilo's source (`packages/opencode/src/config/config.ts`, `config/plugin.ts`, fetched 2026-09-21): the global config merges `config.json`, `kilo.json`, `kilo.jsonc`, `opencode.json[c]`, so rtok writes `kilo.json` and never touches a user's `kilo.jsonc` — (d) is moot, T79 does not block; plugins are scanned as `{plugin,plugins}/*.{ts,js}` with `symlink: true`, so the link to `plugins/rtok.ts` loads. Both variants share the files (`shared() = true`); the desktop variant is detected by VS Code, because the extension's directory name carries its version. (c) accepted: rows are labelled `opencode`. `proxy` → `No`. Open: (a)/(b) live — one bash call through `rtok run` in `kilo` and in the extension; the agent's shell allowlist blocks the `kilo` binary, so the creator runs it (`rtok agents install kilo --yes`, then any bash command, then `rtok stats`).
Extra tests: written and green (`agents::kilo::tests`); the dangling-link one found that `PluginLink::run` reported `no changes` for a broken symlink on every host — fixed in `rtok-agent-sdk` (`plugin_link_replaces_a_dangling_link`). Only the live check above is left.

Check: the unit tests above; `rtok agents list` shows `kilo`; `agents_doc`, `host_docs`, `config_coverage`, `opencode_plugin` green; `just check`.

Extra tests (creator request 2026-09-21): `--dry-run` writes nothing (tree unchanged byte for byte); a user's `kilo.jsonc` is byte-identical after install and remove; an existing foreign file at `plugin/rtok.ts` is neither overwritten nor removed; a dangling `rtok.ts` symlink is repaired; remove on a clean home prints `NO_CHANGES`.

### T131. Measure the spawn brief: cost row and on/off re-read share
Rule: a saving that is not a `Measurement` row does not exist, and the brief is a cost first. Needs T128 and T130.2.
Plan: T130.1's hook records a `Measurement` (`plugin: "memory"`, `kind: "brief"`) with the tokens it added (before = 0, after = brief) so the cost shows as negative saving; `rtok stats` `subagents` row splits the re-read share and sub-agent input tokens by "spawned with a brief" (the brief's archive id in the sub-agent's first user message) vs without.
Check: fixture with one briefed and one plain sub-agent asserts the split; after a dated window with the flag on, `research.md` §17 gets the measured net; default flips to on only if net tokens saved > 0 — otherwise the card closes with the number and T130 stays off.
### T132. Ship a Haiku scout agent definition with the Claude Code plugin
`research.md` §17.3(4). Make the cheap path the default one: `plugins/claude/agents/rtok-scout.md` with `model: haiku`, `tools` limited to the rtok MCP `read`, `search`, `outline`, `explore`, `expand`, and a short system prompt — ranged reads only, never a whole file over the outline threshold, answer with `path:line` citations and no file dumps. Verify the plugin `agents/` directory format against the current Claude Code docs first and add the link to the `## Docs` list in `plugins/claude/README.md`.
Check: `rtok agents install claude` offers the agent file and removal takes it away (host matrix e2e); `tests/host_docs.rs` and `tests/agents_doc.rs` (`RTOK_BLESS=1`) green; T128's per-`agentType` split is the measurement — record `rtok-scout` vs `Explore`/`general-purpose` read bytes per sub-agent in `research.md` §17 after a dated window.
Progress (2026-09-24): `plugins/claude/agents/rtok-scout.md` ships with the plugin (`model: haiku`, the five tools under the plugin-scoped names `mcp__plugin_rtok_rtok__<tool>`); unit test on the frontmatter, install/remove e2e in `tests/claude_plugin.rs`. Left: the dated `rtok-scout` vs `Explore`/`general-purpose` read-bytes row in `research.md` §17 once a window of sessions has run with it.
### T134. Probe: does a CLI command hook's `PostToolUse` `updatedToolOutput` replace native tool output?
Gate for I-91 (`research.md` §17.2). The Agent SDK hooks page says `updatedToolOutput` "works for any tool"; rtok's standing rule says PostToolUse can only add context. If the CLI honours it, native Read/Bash output could be shrunk in place (pointer + `expand <id>`) instead of wrapped or denied — that changes the design of `cmd`, `read` and `guard`, so it is a creator decision, not a silent change. No product code in this task.
Plan: throwaway hook script (scratch, not committed) returning `hookSpecificOutput.updatedToolOutput` for `Read` and `Bash` on the current Claude Code; run one Read and one Bash; check what the model received in the transcript. Repeat for an MCP tool.
Check: a dated row in `research.md` §3 with the Claude Code version, the payload sent and what the transcript shows, per tool kind. Honoured → the `AGENTS.md` rule line and I-91 are put to the creator with the row; not honoured → I-91 closes with the date.
### T124. Realized `tools_rewrite` saving as a dated `research.md` row
6.2 % (T59.5) is the ceiling, not a saving: no dated row shows what `[proxy.tools_rewrite]` removes with the default `max_description_tokens = 60`. Precondition, by the creator: turn it on for this machine's proxy for at least 20 sessions. Then sum the `kind = tools_rewrite` Measurement rows against session input for the same window (`rtok stats` / `rtok gain`, dated command in the row), and write one row into `research.md` §2 next to the T59.5 row; update `docs/comparison.md` only if it cites the number. If the realized share is under the 3 % gate, say so in the row and leave the default off.
Check: the row cites the command, date, sessions, before/after tokens and the share; no number in prose without it.

### T156. Probe: `WorktreeCreate`/`WorktreeRemove` hooks and reflink-seeded `target/`

No product code. Two open questions from `research.md` §18.3–18.4: (1) Claude Code's `WorktreeCreate`/`WorktreeRemove` hooks replace the default create/remove — can rtok own location, naming and the ownership record there, and what do the desktop app and sub-agent `isolation: worktree` actually send; (2) does seeding a new worktree's `target/` by reflink (`reflink-copy`, APFS `clonefile`) save build time and disk after a real task, or does cargo rewrite most of it anyway.

Plan: throwaway hook script (scratch, not committed) that logs the payloads for `claude --worktree`, a sub-agent worktree and the desktop app, and returns a path under `_worktrees/`. For (2): two fresh worktrees of this repo, one seeded with `cp -c -R target`, one cold; record wall time of `just check` and physical disk delta (`df`, not `du` — clones are double-counted) for each. Write the payloads, the numbers and the dated commands into `research.md` §18. `reflink-copy` is a new dependency: adopting it is a creator decision taken on those numbers, not part of this task.

Check: `research.md` §18 gains the hook payloads and a dated table (cold vs seeded: seconds, bytes); T159's card is corrected against the recorded payloads; seeding gets a follow-up task or an `ideas.md` entry from the numbers; no file under `src/` changes.

### T159. Claude Code `WorktreeCreate`/`WorktreeRemove` hooks route through `rtok worktree`

Depends on T156 (the real payloads), T158 (create) and T153 (remove). A skill is advice an agent may skip; the host's own worktree hooks are the only place where the rules cannot be skipped: `claude --worktree`, the desktop app and sub-agent `isolation: worktree` all create worktrees without asking the agent, which is where the `agent-<hex>` directories and reason-less locks come from (`research.md` §18.1, §18.3).

Plan: `rtok hook WorktreeCreate` maps the host's `name` to T158's rules and prints the created path; `rtok hook WorktreeRemove` applies T153's single-worktree rules to `worktree_path` — never forced: a dirty worktree, or one locked by another owner, is left in place and reported, and its tagged caches are cleaned (T152) either way. Installed by `rtok agents install claude` with the plugin, removed with it, singleton per D21; the host docs link for these events joins `plugins/claude/README.md` `## Docs`; regenerate the host table (`tests/agents_doc.rs`, `RTOK_BLESS=1`). **One decision to take before the Do, by the creator:** these hooks replace the host's default behaviour and must spawn git, so they cannot meet "exit 0 in ≤ 10 ms with unmodified input". Proposed reading: the 10 ms rule binds the per-tool-call hot path; `WorktreeCreate` fires once per worktree, and fail-open here means "on any rtok error, create the worktree exactly where the host would have (`<repo>/.claude/worktrees/<name>`) with plain git, print that path, exit 0" — the host never loses the ability to create a worktree because of rtok. Record the outcome as a decision row (D31 or the next free id) in this task's PR. Other hosts have no such hook today (§18.3); they keep the skill (T155).

Check: hook fixture tests with T156's recorded payloads — create returns a path under the T158 root with the owner lock; a simulated failure of `rtok worktree add` still yields a usable worktree at the host default path and exit 0; remove deletes a merged clean worktree, keeps a dirty one and a foreign-locked one with the reason on stderr, and cleans the tagged cache in all three; host matrix e2e — install adds both hooks exactly once and removal takes them away; `tests/host_docs.rs` and `tests/agents_doc.rs` green; `just check`.


### T163. Replace raw SQL in `src/store/` with Diesel's query builder

Creator request 2026-09-22: no raw SQL anywhere (AGENTS.md rule, D13). `src/store/` still has 104 `sql_query`/`sql::<>`/`batch_execute` calls: `mod.rs` 92, `otel.rs` 6, `embed.rs` 4, `schema.rs` 2 (`symbols.rs`'s 15 are done — T163.1). Plain CRUD moves to the typed DSL over `schema.rs`; FTS5 `MATCH`, `bm25()` and PRAGMA become Diesel extensions (`define_sql_function!` / a custom `QueryFragment`) in one module; DDL moves to `diesel_migrations` (listed in workspace `rust.md`; creator approved wiring it into rtok on 2026-09-23). Split into ≤200 LOC / ≤10 file PRs per file when claimed.

Check: `grep -rE 'sql_query|sql::<|batch_execute' src` finds nothing; existing store tests unchanged and green; hook path still ≤ 10 ms; `just check`.

**Split (2026-09-23).** T163.1 (`symbols.rs`, done — see `done.md`) created the shared `src/store/sql_ext.rs` extension module. T163.2 takes `otel.rs` and `embed.rs`, reusing it. `mod.rs` (92 sites, including migration DDL and PRAGMA) stays in this card and is split further when claimed; `diesel_migrations` is approved (2026-09-23).

**Split of `mod.rs` (2026-09-23).** Six slices by area, each ≤ 200 LOC: T163.3 PRAGMA, `unixepoch()` and FTS5 in the shared extension module; T163.4 migrations; T163.5 sessions, calls, measurements and `kv`; T163.6 archive, `call_io` and `read_cache`; T163.7 usage and stats aggregates; T163.8 retention and the last test helpers, which also runs this card's full Check and closes T163. Raw SQL in `mod.rs` tests moves with the slice that owns the table it touches. Execution: T163.3 waits for T163.1's `sql_ext.rs` to land on `main` (one module, never a second); T163.4–T163.7 do not depend on each other; T163.8 goes last. T163.9 (window and CTE queries T163.7 could not express) was split off T163.7 on 2026-09-23 and also waits for `sql_ext.rs`.

### T163.2. `src/store/otel.rs` and `src/store/embed.rs` without raw SQL

Second slice of T163: the 6 sites in `otel.rs` and 4 in `embed.rs` move to the Diesel DSL over `schema.rs` (aggregates via `diesel::dsl::{min, count}` and `group_by`). Anything the DSL cannot express goes through the shared extension module from T163.1 — whichever slice lands first creates it.

Execution plan: (1) map each site to `schema.rs`; (2) rewrite, keeping signatures and result order; (3) run the otel, embed and store tests unchanged; (4) move this card to `done.md`.

Check: `grep -nE 'sql_query|sql::<|batch_execute' src/store/otel.rs src/store/embed.rs` finds nothing; tests unchanged and green; `just check`.

### T163.3. PRAGMA, `unixepoch()` and FTS5 through the shared extension module

`mod.rs` sites the typed DSL cannot express: the PRAGMAs in `set_busy`, `connect`, `init`, `set_query_only` and `purge_calls_older_than`; `sql::<>("unixepoch()")` in `upsert_note` and `retire_note`; FTS5 `MATCH`/`bm25()` in `search_notes`; tests `open_on_disk_uses_wal`, `fts5_match_finds_inserted_note`. They become typed helpers in T163.1's `src/store/sql_ext.rs` (`define_sql_function!` for `unixepoch`, a `QueryFragment` per PRAGMA and for the FTS5 match), the only home for non-DSL SQL; `schema.rs`'s `notes_fts` comment is updated. The typed SQL functions declared in `mod.rs` move there too: `coalesce` (T163.5), `length` and `sum_bigint` (T163.7), `substr` (T163.6).

Execution plan: (1) wait for T163.1 on `main`, reuse its module; (2) add the helpers with unit tests; (3) swap the call sites, signatures unchanged; (4) store tests unchanged and green, `just check`.

Check: no `sql_query|sql::<|batch_execute` left in the listed functions and tests; `note_search_treats_query_text_literally` and the WAL test green; `just check`.

### T163.4. Migrations through `diesel_migrations`

`migrate()` (`schema_migrations` bookkeeping plus `batch_execute` of each file) moves to `diesel_migrations` (approved 2026-09-23). Existing databases must not re-run anything: the names already in `schema_migrations` map onto Diesel's version table in a one-time, idempotent bridge, and a DB that was never migrated still gets every file once. Tests move with it: `migration_is_idempotent`, `concurrent_opens_of_a_fresh_store_all_migrate`, `migration_0015_adds_lifecycle_columns_to_a_previous_schema_db`, `schema_0002_seeds_hosts_and_rejects_bad_fk`, `migrations_list_matches_the_directory`, `schema_rs_matches_the_migrated_tables`. The `.sql` files stay raw SQL (the rulebook allows it in migrations).

Execution plan: (1) choose between Diesel's `<version>/up.sql` layout and a `MigrationSource` over the flat `migrations/NNNN.sql` files — the layout move alone touches every file, so if chosen it lands as its own mechanical PR; (2) write the bridge and a test that opens a DB migrated by the current code and sees no re-run; (3) toolchain row for `diesel_migrations`; (4) `just check`.

Check: `migrate()` and its tests hold no `sql_query|batch_execute`; a pre-T163.4 database opens, keeps its data and applies only newer migrations; fresh and concurrent opens green; `just check`.

### T163.8. Retention without raw SQL; close T163

`purge_calls_older_than` and `run_retention` (dynamic `DELETE`s, archive path collection) and the remaining test sites (`purge_drops_old_calls…`, `retention_keeps_plugin_archives…`, `archive_in_session…`), then T163's full Check. Last slice: it runs after T163.3–T163.7 and removes `use diesel::sql_query` from `mod.rs`; it runs after T163.9 too.

Execution plan: (1) rewrite with `diesel::delete(...).filter(...)` and typed updates inside the existing transaction; (2) T163's `grep` over `src` finds nothing; (3) hook path still ≤ 10 ms (`rtok bench` or the existing timing test); (4) move T163 and all its slices to `done.md`.

Check: T163's Check.

### T163.9. Window and CTE queries through the shared extension module

Left over from T163.7: `usage_ctt` (`COUNT() OVER`, `ROW_NUMBER() OVER`), `session_totals`/`recent_session_totals` (four CTEs, `UNION ALL`, per-group `MAX(id)` subqueries) and `recent_calls` (correlated `MAX(id)` subquery in a `LEFT JOIN`) have no form in Diesel 2.3.13's typed DSL. They move into T163.1's `src/store/sql_ext.rs` as typed `QueryFragment`s with bound parameters, each with a comment naming the construct the DSL lacks (the rulebook's exception for statements the ORM cannot express).

Execution plan: (1) wait for T163.1 on `main`; (2) move the three statements, signatures and row order unchanged; (3) the `session_totals` and `recent_calls` tests unchanged and green, `rtok stats` unchanged on a DB clone; (4) `just check`.

Check: no `sql_query` left in the three functions; tests unchanged and green; `just check`.

### T178. Hook wall-clock time as Claude Code sees it

Found in the 2026-09-22 audit: in-process hook time is p50 0.3 ms, but Claude Code records p50 18–19 ms and p95 206–255 ms for PreToolUse/PostToolUse — process start of a 27 MB binary dominates and the ≤ 10 ms rule is broken on every call without rtok noticing. Ten hooks were cancelled at Claude Code's 5 s timeout (5 PreToolUse, 5 UserPromptSubmit with p50 5.6 s — no UserPromptSubmit rows exist in the store, so the owner is unconfirmed). SessionEnd (p50 18.9 ms) and PreCompact (p50 15.0 ms) are over budget in-process.

Plan: research first — measure cold/warm start (`hyperfine`), find what runs before `main` dispatches (config parse, DB open, migrations), confirm who owns the UserPromptSubmit timeouts; then pick: lazy store open, a smaller hook path, or a resident process (`rtok demon`) the hook talks to. Record findings in `research.md`.

Execution plan: (1) `hyperfine` `rtok hook PreToolUse`/`PostToolUse` with recorded payloads against `rtok --version`, release build, cold and warm; (2) trace what runs before dispatch (config load, store open, migrations, plugin registry) and the binary's load/page-in cost; (3) read the `~/.claude` hook config to name the owner of the `UserPromptSubmit` timeouts; (4) fix the largest cost at the responsible layer (lazy store open, no migrations on the hook path, lighter hook entry) — a resident process only if the rest cannot reach 10 ms, and then as its own proposed task; (5) dated `research.md` section with before/after.

Step 2 plan (D32): (a) `crates/rtok-hook`, the std-only wire format; (b) `rtok hook --serve`, one resident per home that runs `hooks::run` for each request in the client's cwd and refuses another version or config environment; (c) the `demon` service `hook`; (d) the `rtok-hook` client: connect, 50 ms answer timeout, fallback to `rtok hook`, detached autostart; (e) `hooks.json` order `rtok-hook` → `rtok` → `hook.sh`, shipping, before/after as Claude Code sees it. One PR each.

Check: a dated `research.md` row with measured start time before/after; hook p50 as seen by Claude Code under 10 ms on this machine; `just test` green.

Progress (research.md §19): the plugin launcher (a second `/bin/sh` per call) was the largest cost; `hooks.json` now execs `rtok` from PATH directly, p50 as Claude Code sees it 20.9 → 14.6 ms (PreToolUse) and 19.0 → 13.3 ms (PostToolUse). Remaining: the node + `/bin/sh` floor (5 ms) plus `rtok --version` (5.6 ms) already exceed 10 ms, so the Check needs a resident process with a small hook client — proposed as its own task. Locked store (§19.6): the hook now waits 5 ms on another writer, not 1 s per statement, and fails open with the input unchanged — 1.06–2.13 s → ~20 ms. Remaining: the resident process and hook client.

### T241. Replay bench: saving over a fixed session corpus

The golden and surface tests measure one call at a time; no test shows the saving over a whole session mix of Bash, Read, Grep and MCP results, so a change that helps one family and hurts the mix goes unnoticed.

Plan: `tests/fixtures/replay/session.jsonl` — about 30 anonymised hook payloads shaped like a real Claude Code session (tool mix taken from `rtok stats` on this machine, bodies written or scrubbed by hand; no real paths, names or secrets). `tests/replay_bench.rs` feeds them through `rtok hook` in a temp home, sums the `Measurement` rows, prints a per-plugin table (`--nocapture`) and asserts the total saving stays over a floor set a few points below the first run. Record the first run as a dated `research.md` §2 row with the command.

Check: the test fails when a plugin is disabled in the temp config; the `research.md` row cites the command; `just check` green. Needs T239.

### T246.5. zed and grok MCP entries

T246.1–T246.6 (T246.1–T246.4 and T246.6 done), creator request 2026-09-24: removing rtok (`agents remove <host>`, and the plugin-supersedes strips of T243) must take back only what rtok itself wrote; anything the user changed in it is asked about — remove or keep. Today `rtok_agent_sdk::unregister_server` drops any entry named `rtok` whatever its command, and `skill::sync` removes a marked rtok skill even after the user edited it. Hooks already go through `strip_ours` + `is_rtok_bin`, but a user-edited rtok hook (other matcher, timeout, extra args) goes silently too.

Outcomes, one ownership check per kind: **ours, unchanged** (equal to what the installer writes now, any rtok binary path counting as the same): remove; **ours, changed by the user** (it runs rtok, but differs): ask `? remove <what> in <file>? you changed it [y/N]` through a new SDK prompt whose default (Enter, EOF) is keep; `--yes` removes; no terminal keeps and reports `leave … (changed by you; remove by hand)`; **not ours** (named `rtok` but not running rtok): leave it and report `leave … (not rtok's; remove by hand)`. A `leave` report writes nothing. Split below so each PR stays under 10 files.

zed (JSONC editor) and grok (TOML) take the T246.1 ownership check on their own writers; then the name-only `rtok_agent_sdk::unregister_server` goes private or goes, so no remove path drops an entry by name alone.

Check: `tests/agent_remove.rs` leaves an edited zed and grok entry without `--yes`; `just check` green.


### T262.3. Codex: spawn brief on `SubagentStart`

`research.md` §23: Codex fires `SubagentStart` and adds the hook's stdout (or its hook-specific context) to the subagent as developer context. Add `SubagentStart` to `plugins/codex/hooks/hooks.json` and the Codex installer's list, and make `rtok hook SubagentStart` answer in the shape Codex reads.

Blocked (found 2026-09-24 while claiming): the brief is built from `PreToolUse` rows whose `tool_name` is `Read|Edit|Write` (`ledger()` in `src/plugins/memory/handoff.rs`), and rtok installs no `PreToolUse` hook for Codex (only `PreCompact`/`PostCompact`), so a Codex brief would always be empty. Needs Codex `PreToolUse` wiring first (idea I-88), which the creator has not approved.

Check: a Codex `SubagentStart` payload through `rtok hook` returns the brief in Codex's shape (test); `just check` green.

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
| D16 | **One task = one PR.** Each task gets its own branch (or worktree) off `origin/main` and lands through its own pull request; never commit to `main` directly. The PR carries the `<task-id>: <title>` commit and the `plan.md` → `done.md` move. Delete the branch after merge. | Every change passes CI before it reaches `main`; concurrent agents stop colliding in one checkout. |
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
| D30 | **HTTPS uses webpki Mozilla roots (`use_preconfigured_tls`); one binary.** Corporate CAs via `SSL_CERT_FILE` (curl parity, fail closed). reqwest 0.13 `rustls` still links `rustls-platform-verifier`; `otool` showed Security.framework still present (T53.3). A second hook binary was rejected. | I-32: 1.3–1.5 ms dyld; dropping the `rustls` feature does not compile. |
| D32 | **An optional resident hook process (T178).** `rtok hook --serve` answers `rtok-hook`, a std-only client, over a Unix socket (Windows: a named pipe); `rtok demon` supervises it as the service `hook`, or the hook starts it detached, rate-limited by a lock file. This supersedes D1's "no daemon on the hook path" and D22's "nothing in it is on the hook path" for the `hook` service only. Without it everything works as today: the client runs `rtok hook` when the resident is absent or refuses (another version or config environment), and prints `{}` when it does not answer within 50 ms. | Process start is ~11 ms of the ~14 ms Claude Code waits per hook (research.md §19); a fresh process cannot meet the 10 ms budget. |

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
| `toon` (on by default) | caveman toon, TOON | proxy/MCP | tabular JSON → TOON |

### Working agreement

- Graph backends: do not claim tasks that reintroduce `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` / `graph-grafeo` / cmake-for-liblbug. Symbol index is SQLite only.
- One task = one branch = one PR (D16). Never skip the Check.
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
2. ~~**`expand::parse_range` when start > line count**~~ — done 2026-09-23: a start past the last line errors (`start exceeds line count N`) instead of printing nothing with exit 0.
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

- ~~**T55.11 (P2)**~~ — done: `expand` never froze the owning session's pointer (every expand caller runs under session `expand` / `mcp-<pid>`, decisions belong to the proxy session; expand rate stayed 0, toon attribution dead). Reproduced.
- **T55.12 (P2)** — Windows `wrap_quote` `''` quoting is wrong under Git Bash (Claude Code's Windows shell): apostrophes silently dropped from the rewritten command. Code read.
- ~~**T55.13 (P3)**~~ — done: Copilot `Read` inputs use `path`; `guard::cache_key` and `read::hook` match `file_path` only → dedup and advice never fire on that host. Reproduced.
- ~~**T55.14 (P3)**~~ — done: the semantic-cache key ignored tool_result text; two requests differing only in tool results hashed identically and were both eligible under defaults. Reproduced.
- **T55.15 (P3)** — `live_blobs` overwrites image/document `source.data` / `image_url.url` with pointer text → invalid request when the flag is on. Reproduced.
- ~~**T55.16 (P3)**~~ — done: the guard deny read the full archived body (and estimated over it) on the ≤ 10 ms PreToolUse path. Code read.

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



