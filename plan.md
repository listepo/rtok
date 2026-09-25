# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T87 | in progress | P1 | 2 | 70% | Claude Code / claude-fable-5-1 |
| T88 | todo | P1 | 2 | 0% | |
| T89 | todo | P1 | 3 | 0% | |
| T97 | in progress | P1 | 3 | 95% | Claude Code / claude-fable-5-1 |
| T124 | todo | P3 | 2 | 0% | |
| T131 | todo | P2 | 3 | 70% | |
| T132 | todo | P2 | 2 | 70% | |
| T134 | todo | P1 | 2 | 40% | |
| T156 | todo | P3 | 3 | 50% | |
| T159 | todo | P2 | 4 | 0% | |
| T163 | in progress | P2 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T163.2 | in progress | P2 | 3 | 5% | Claude Code / claude-sonnet-5 |
| T163.3 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T163.4 | in progress | P2 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T163.8 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T163.9 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T178 | in progress | P1 | 4 | 75% | Claude Code / claude-opus-5-5 |
| T262.3 | todo | P2 | 2 | 0% | |
| T266 | todo | P2 | 4 | 0% | |
| T267 | todo | P2 | 5 | 0% | |
| T267.1 | todo | P2 | 4 | 0% | |
| T267.2 | todo | P2 | 4 | 0% | |
| T267.3 | todo | P2 | 3 | 0% | |
| T267.4 | todo | P3 | 2 | 0% | |
| T267.5 | todo | P2 | 3 | 0% | |
| T267.6 | todo | P3 | 2 | 0% | |


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
Progress (2026-09-25): the cost row was already recorded by T130.1 (`memory`/`brief`, before 0, after = brief tokens); `tests/hook_spawn_brief.rs` now asserts one row per fired brief and none otherwise. `rtok stats` splits the `subagents` re-read share and input tokens into `brief` / `no brief`, detected by `measure::subagents::SPAWN_BRIEF_MARKER` in a sub-agent's first user message (a `handoff.rs` test pins it inside the brief's `INSTRUCTIONS`); fixture test `briefed_and_plain_subagents_split_the_reread_share`. Left: the dated window with the flag on, the net in `research.md` §17, and the default decision.
### T132. Ship a Haiku scout agent definition with the Claude Code plugin
`research.md` §17.3(4). Make the cheap path the default one: `plugins/claude/agents/rtok-scout.md` with `model: haiku`, `tools` limited to the rtok MCP `read`, `search`, `outline`, `explore`, `expand`, and a short system prompt — ranged reads only, never a whole file over the outline threshold, answer with `path:line` citations and no file dumps. Verify the plugin `agents/` directory format against the current Claude Code docs first and add the link to the `## Docs` list in `plugins/claude/README.md`.
Check: `rtok agents install claude` offers the agent file and removal takes it away (host matrix e2e); `tests/host_docs.rs` and `tests/agents_doc.rs` (`RTOK_BLESS=1`) green; T128's per-`agentType` split is the measurement — record `rtok-scout` vs `Explore`/`general-purpose` read bytes per sub-agent in `research.md` §17 after a dated window.
Progress (2026-09-24): `plugins/claude/agents/rtok-scout.md` ships with the plugin (`model: haiku`, the five tools under the plugin-scoped names `mcp__plugin_rtok_rtok__<tool>`); unit test on the frontmatter, install/remove e2e in `tests/claude_plugin.rs`. Left: the dated `rtok-scout` vs `Explore`/`general-purpose` read-bytes row in `research.md` §17 once a window of sessions has run with it.
### T134. Probe: does a CLI command hook's `PostToolUse` `updatedToolOutput` replace native tool output?
Gate for I-91 (`research.md` §17.2). The Agent SDK hooks page says `updatedToolOutput` "works for any tool"; rtok's standing rule says PostToolUse can only add context. If the CLI honours it, native Read/Bash output could be shrunk in place (pointer + `expand <id>`) instead of wrapped or denied — that changes the design of `cmd`, `read` and `guard`, so it is a creator decision, not a silent change. No product code in this task.
Plan: throwaway hook script (scratch, not committed) returning `hookSpecificOutput.updatedToolOutput` for `Read` and `Bash` on the current Claude Code; run one Read and one Bash; check what the model received in the transcript. Repeat for an MCP tool.
Check: a dated row in `research.md` §3 with the Claude Code version, the payload sent and what the transcript shows, per tool kind. Honoured → the `AGENTS.md` rule line and I-91 are put to the creator with the row; not honoured → I-91 closes with the date.
Progress (2026-09-25, `research.md` §3 "T134"): Claude Code 2.1.267; the CLI hooks page lists no `updatedToolOutput` for `PostToolUse` (only `additionalContext`, `systemMessage`, `terminalSequence`), the Agent SDK page does. The live run is blocked in agent sessions (`claude -p` fails with an expired OAuth session, as in T53.1); left: the creator runs the scratch probe (hook logging the payload and returning `updatedToolOutput`, one Bash / Read / MCP call on `--model haiku`) from a real terminal, and the per-tool verdict goes into the row.
### T124. Realized `tools_rewrite` saving as a dated `research.md` row
6.2 % (T59.5) is the ceiling, not a saving: no dated row shows what `[proxy.tools_rewrite]` removes with the default `max_description_tokens = 60`. Precondition, by the creator: turn it on for this machine's proxy for at least 20 sessions. Then sum the `kind = tools_rewrite` Measurement rows against session input for the same window (`rtok stats` / `rtok gain`, dated command in the row), and write one row into `research.md` §2 next to the T59.5 row; update `docs/comparison.md` only if it cites the number. If the realized share is under the 3 % gate, say so in the row and leave the default off.
Check: the row cites the command, date, sessions, before/after tokens and the share; no number in prose without it.

### T156. Probe: `WorktreeCreate`/`WorktreeRemove` hooks and reflink-seeded `target/`

No product code. Two open questions from `research.md` §18.3–18.4: (1) Claude Code's `WorktreeCreate`/`WorktreeRemove` hooks replace the default create/remove — can rtok own location, naming and the ownership record there, and what do the desktop app and sub-agent `isolation: worktree` actually send; (2) does seeding a new worktree's `target/` by reflink (`reflink-copy`, APFS `clonefile`) save build time and disk after a real task, or does cargo rewrite most of it anyway.

Plan: throwaway hook script (scratch, not committed) that logs the payloads for `claude --worktree`, a sub-agent worktree and the desktop app, and returns a path under `_worktrees/`. For (2): two fresh worktrees of this repo, one seeded with `cp -c -R target`, one cold; record wall time of `just check` and physical disk delta (`df`, not `du` — clones are double-counted) for each. Write the payloads, the numbers and the dated commands into `research.md` §18. `reflink-copy` is a new dependency: adopting it is a creator decision taken on those numbers, not part of this task.

Progress (2026-09-25, `research.md` §18.4 second data point): part (2) measured. Cold `just check` took 185 s and +6.28 GiB; seeded took 371 s and +3.35 GiB. The clone skipped every dependency rebuild (≈ 23 s saved), but T236's `dunnage` pass then compressed the cloned files (≈ 215 s). Parked as I-99, with no follow-up task. Part (1), the hook payloads from `claude --worktree`, a sub-agent worktree and the desktop app, is still open: it needs live sessions of the creator's.

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

### T262.3. Codex: spawn brief on `SubagentStart`

`research.md` §23: Codex fires `SubagentStart` and adds the hook's stdout (or its hook-specific context) to the subagent as developer context. Add `SubagentStart` to `plugins/codex/hooks/hooks.json` and the Codex installer's list, and make `rtok hook SubagentStart` answer in the shape Codex reads.

Blocked (found 2026-09-24 while claiming): the brief is built from `PreToolUse` rows whose `tool_name` is `Read|Edit|Write` (`ledger()` in `src/plugins/memory/handoff.rs`), and rtok installs no `PreToolUse` hook for Codex (only `PreCompact`/`PostCompact`), so a Codex brief would always be empty. Needs Codex `PreToolUse` wiring first (idea I-88), which the creator has not approved.

Check: a Codex `SubagentStart` payload through `rtok hook` returns the brief in Codex's shape (test); `just check` green.

### T266. Heavy store queries through sea-query on the Diesel connection

Restored 2026-09-25 from an uncommitted `plan.md` draft in the main checkout, where it was T237; that id went to another task on `main`.

Creator request 2026-09-24: high-load queries use `sea-query` as the query builder, executed on the existing Diesel `SqliteConnection` through `sea-query-diesel` (same SeaQL repo; `sea-query` 1.0.x, `sea-query-diesel` 0.3.0 requires `diesel ^2.1.1`, compatible with our 2.3.13). Diesel stays the ORM, the only DB owner (D13) and the source of truth for `schema.rs`; sea-query only builds statements the typed DSL cannot express (window functions, CTEs, `UNION ALL`, correlated subqueries) with bound parameters — no SQL strings. Candidates: `usage_ctt`, `session_totals`/`recent_session_totals`, `recent_calls`, and the dashboard `snapshot()` queries that run every 2 s tick.

Overlaps T163.9 (in progress, another agent), which plans hand-written `QueryFragment`s in `src/store/sql_ext.rs` for the same statements. Do not edit T163.9's card or branch: once T163.9 lands, this task swaps those fragments for sea-query builders; if its owner has not started, ask the creator whether T163.9 should switch to sea-query instead.

Done when: those statements are built with sea-query and run via `sea-query-diesel`; signatures and row order unchanged; their tests unchanged and green; `rtok stats` and the dashboard identical on a DB clone; a before/after timing on a large DB clone recorded in the PR (no speed claim without it); hook path still ≤ 10 ms; `toolchain.md` and workspace `rust.md` rows for both crates; `just check`.

Execution plan: (1) wait for T163.9 on `main`; (2) add both crates (`sqlite` backend features only) with one-line reasons; (3) port one statement per commit behind the existing `Store` methods; (4) time each on a DB clone before/after; (5) `just check`. Split per statement if the PR passes ≤200 LOC / ≤10 files.

Check: no `sql_query` left in the three functions; tests unchanged and green; `just check`.

### T267. Every rtok TOML file has a schema; config logic lives in one encapsulated engine

Restored 2026-09-25 from an uncommitted `plan.md` draft in the main checkout, where it was T238 (subtasks T238.1–T238.6); that id went to another task on `main`.

Creator request 2026-09-24: every TOML file rtok owns is described by a schema and validated against it, and the code that loads, validates and edits TOML is abstract and lives in one encapsulated module.

Today each format is handled differently. The main config (`config/default.toml`, `~/.rtok/config.toml`, `.rtok/config.toml`) loads through figment, and `src/config/validate.rs` (632 lines) checks it by walking `Config::default()` by hand, with allow-lists such as `GRAPH_GRAMMARS` repeating what the types already say. Command rules (`rules/default.toml` plus drop-in files) use their own `toml_edit` parser in `src/plugins/cmd/rules.rs`, and a drop-in that fails to parse is skipped without a word (`if let Ok(..)`). `bench/tasks.toml` and `bench/graph.toml` are read by a second figment setup in `src/bench.rs`. Host configs (Codex, Grok, Kimi) are edited with `toml_edit` straight from `src/agents/*` and `src/doctor.rs`. Editors cannot autocomplete or check any of these files.

"TOML schema" means JSON Schema applied to TOML: the Tombi and Taplo editors and linters read it from a `#:schema <url>` first line or a glob mapping. The Rust types are the only source of truth. `schemars` (already in `Cargo.lock`, approved in workspace `rust.md`) generates the schemas, which are committed under `schemas/`, and a drift test fails when they are stale.

Design:
- **Engine** (`crates/rtok-config`): generic over a `TomlConfig` trait (`DeserializeOwned + Serialize + JsonSchema + Default`, plus an id and layer paths). It knows no rtok types. It provides layered `load` with per-key provenance (figment, D14), `validate` (JSON Schema plus `toml_edit` spans → `file:line`), `set` (edit in place with comments kept, validate before an atomic write), `schema()` export, and a `HostToml` editor for configs rtok does not own. `figment` and `toml_edit` are imported only here; the crate boundary enforces that.
- **Hot path unchanged:** the hook keeps plain serde deserialization, fails open and stays ≤ 10 ms. Schema validation runs only in cold commands (`config validate`, `config set`, `doctor`, `bench`).
- **Constraints go in the types:** enums, `#[schemars(range(..))]` and `deny_unknown_fields`/`additionalProperties: false` replace the hand-written checks, so validation logic is not written twice.
- **Stable schema URL:** `#:schema` points to the tagged release (`…/v<version>/schemas/<id>.schema.json`), so a user file always matches the binary that wrote it.

Done when: every rtok-owned TOML format has a committed, drift-tested schema; `validate.rs`'s hand walk, `parse_strict` and the direct `toml_edit`/`figment` imports outside the engine are gone; `config validate`, `doctor` and trycmd output say the same things or differ in reviewed ways; the hook latency test is green; `toolchain.md` and workspace `rust.md` list every new crate.

Split: T267.1 → T267.2 → (T267.3, T267.4, T267.5 in parallel) → T267.6.

Check: T267.1–T267.6 are in `done.md`; searching `src/` for `toml_edit::` and `figment::` finds nothing; `just check` is green.

### T267.1. `rtok-config` engine crate

Build the generic engine described in T267 with no rtok types in it: the `TomlConfig` trait, `load` (figment layers + provenance), `validate` (JSON Schema → `file:line` through `toml_edit` spans), `set` (validate before an atomic write, comments kept), `schema()`, and `HostToml`. The validator is the `jsonschema` crate with default features off (no remote `$ref` fetching). It is maintained; if it would add a new transitive dependency to the `rtok hook` path, stop and ask the creator. Tests use a toy config type: an unknown key, a wrong type, an out-of-range value and a bad enum each report the right line; `set` refuses an invalid value and leaves the file untouched.

Check: the crate's tests are green; the `rtok` binary does not change behavior yet; `just check`.

### T267.2. Main config on the engine

Derive `JsonSchema` on the whole `Config` tree and move constraints into attributes (graph grammars become an enum, numeric limits become ranges). Load, `config validate` and `config set` go through T267.1. Delete the hand walk in `src/config/validate.rs`. Add `rtok config schema` (print the schema) and `schemas/config.schema.json` with a drift test. Legacy-key rejection (T24.5) and `plugins.graph.extensions` keep failing, now through the schema. If this goes over ≤200 LOC / ≤10 files, split derive + schema from the validate swap.

Check: the `config-validate` / `config-path` trycmd fixtures are unchanged or deliberately updated; the T24.5 test is green; the drift test fails after a field is added without regenerating; the hook latency test is green.

### T267.3. Command rules on the engine

Give `rules/*.toml` (default, user file, drop-ins) a typed struct, replacing `parse_strict` and the manual `toml_edit` walk in `src/plugins/cmd/rules.rs`. Merge order and fail-open loading stay. A broken drop-in is still skipped at load time, but `doctor` and `config validate` now report it with `file:line` (today it is silent). Add `schemas/rules.schema.json` with a drift test.

Check: the cmd filter tests are unchanged and green; a malformed drop-in shows up in `doctor`; `just check`.

### T267.4. Bench files on the engine

`bench/tasks.toml` and `bench/graph.toml` load through T267.1 instead of the private figment setup in `src/bench.rs`. Add `schemas/bench-tasks.schema.json` and `schemas/bench-graph.schema.json` with drift tests.

Check: the `bench-dry-run` / `bench-graph-dry-run` trycmd fixtures are unchanged; `just check`.

### T267.5. Host configs through `HostToml`

`src/agents/{codex,grok,kimi}` and the Codex probe in `src/doctor.rs` edit and read host files only through `HostToml`: comments are preserved, the write is atomic, a parse error names the file. rtok does not own these schemas and does not validate host keys; it only guarantees that its own entry (for example `mcp_servers.rtok`) has the right shape. Output of the setup and uninstall tests stays byte-identical.

Check: the agents and doctor tests are unchanged and green; `src/agents/` and `src/doctor.rs` no longer import `toml_edit`; `just check`.

### T267.6. Editor and CI wiring

Put a `#:schema` line at the top of `config/default.toml`, `rules/default.toml`, `.rtok/config.toml` and the bench files, and have `rtok config init` / `config set` write it (a version-pinned URL) when a file is created. Add a `tombi.toml` that maps these globs to `schemas/`, so editors check the files even without the header. Add `tombi lint` over the repo's TOML to `just check` and CI, which also checks `Cargo.toml`, `cliff.toml`, `release-plz.toml` and `dist-workspace.toml` against their SchemaStore schemas. Add rows for `tombi` in `toolchain.md`.

Check: `tombi lint` is green in CI; a deliberate typo in `config/default.toml` fails it; `just check`.

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




# Roadmap directions — implementation checklist

Per direction in [`roadmap.md`](roadmap.md) (plugin plans). For each: read the
roadmap section, land the open cards already in this `plan.md` / `done.md`, then
close the Check. Batch/Flex has its own research + cards below.

## Core (not a plugin — blocks all of them)

- [ ] Read `roadmap.md` § `Core (not a plugin — blocks all of them)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `measure`

- [ ] Read `roadmap.md` § ``measure`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `inject`

- [ ] Read `roadmap.md` § ``inject`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `cmd`

- [ ] Read `roadmap.md` § ``cmd`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `read`

- [ ] Read `roadmap.md` § ``read`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `proxy`

- [ ] Read `roadmap.md` § ``proxy`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `archive`

- [ ] Read `roadmap.md` § ``archive`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `memory`

- [ ] Read `roadmap.md` § ``memory`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `graph`

- [ ] Read `roadmap.md` § ``graph`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `guard`

- [ ] Read `roadmap.md` § ``guard`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## `toon`

- [ ] Read `roadmap.md` § ``toon`` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## Dependency sketch

- [ ] Read `roadmap.md` § `Dependency sketch` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## Plugin SDK (the contract — not a plugin)

- [ ] Read `roadmap.md` § `Plugin SDK (the contract — not a plugin)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## Dashboard (operator surface — not a plugin)

- [ ] Read `roadmap.md` § `Dashboard (operator surface — not a plugin)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## TUI (operator surface — not a plugin)

- [ ] Read `roadmap.md` § `TUI (operator surface — not a plugin)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## OpenTelemetry (export surface — not a plugin)

- [ ] Read `roadmap.md` § `OpenTelemetry (export surface — not a plugin)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

## Later (v0.2+)

- [ ] Read `roadmap.md` § `Later (v0.2+)` — list open ids vs `done.md`
- [ ] Implement remaining plan cards for this direction (see matching `### T…` above)
- [ ] Tests / Check from those cards green on Mac (`just check` / cited commands)
- [ ] Docs touch if behaviour shipped; move finished ids out of roadmap open lists

# Research — Batch / Flex API pass

Recovered technical design for the Batch/Flex API pass (was `batch-flex-plan.md`,
then briefly in `plan.md`, then `roadmap.md` on `docs/batch-flex-pass`).
Worktree `_worktrees/rtok-research-batch-flex` has no separate `research/design`
file — this section is that design, kept here so `plan.md` stays the executable
surface. Do not drop the checklist cards (T250–T259) below.

# Batch / Flex API pass — implementation plan

Implementation plan for the cost levers documented in [`docs/batch-flex.md`](docs/batch-flex.md)
and [`architecture.md`](architecture.md) §12. Design-first branch:
`docs/batch-flex-pass`. Code lives under `src/proxy/` only (hooks/MCP never see
LLM HTTP).

Status: **plan** (no implementation in this document). Ground truth for
“today” is `origin/main` as of the design pass (`bcb03778` docs commit on this
branch; proxy behaviour matches current `src/proxy/`).

## Goals

1. **Batch observe** — when clients already use OpenAI `/v1/batches*` or
   Anthropic `/v1/messages/batches*` through `rtok proxy`, rtok records those
   hops in the ledger and can attribute usage/price once results are fetched.
2. **Flex inject** — optional `[proxy.flex]` so OpenAI sync Chat/Responses
   requests get `service_tier = "flex"` via `prepare` without the agent knowing
   about Flex.
3. **Keep boundaries** — never convert a live sync agent turn into a Batch job;
   never put Batch/Flex logic in hooks or MCP; do not break prompt-cache sticky
   behaviour documented in [`docs/prompt-cache.md`](docs/prompt-cache.md).
4. **Measurable savings** — `rtok stats` / `rtok report` can distinguish sync vs
   Flex vs Batch once observe + Flex pricing rows exist.

Non-goals for this feature (separate tracks):

- Full **model routing** (D9 / `[proxy.routing]` policy + classifier) — stub
  config only until a Check exists; called out as a later stage, not a Blocker
  for Batch/Flex MVP.
- Automatic sync→Batch enqueue, Batch JSONL rewriting, or a new MCP “submit
  batch” tool (optional follow-on, explicit opt-in only).
- Semantic response cache (P31) behaviour beyond a hard skip on Batch paths.

## Scope

| In scope | Out of scope |
|----------|--------------|
| Config parse for `[proxy.batch]` / `[proxy.flex]` (and stub `[proxy.routing]`) | Transparent sync→Batch |
| Path recognition for Batch create/poll/list/cancel/results + related `/v1/files` | Rewriting Batch JSONL member bodies |
| Ledger tags + optional result→`usage` parsing | Anthropic Flex twin (none exists) |
| OpenAI `prepare` Flex injection / force / documented 429 policy | Changing compress/`proxy_filter` semantics for Batch |
| Docs already on this branch stay authoritative; update when behaviour ships | Porting Flex/Batch into host plugins |

Primary modules (today):

- `src/proxy/mod.rs` — `handle` / `shape_request` / fallback / bookkeeping
- `src/proxy/wire.rs` + `openai_chat.rs` / `openai_responses.rs` / `anthropic.rs` —
  exact `Wire::matches` for sync shapes only
- Config load / validate (same path as other `[proxy.*]` tables)
- Store: `calls`, `call_io`, `usage`, `tokens`

## Current baseline (do not regress)

- Batch paths miss every `Wire::matches` → axum **fallback** byte-forward;
  optional `calls` row; **no** compress / prepare / usage parse.
- Client-set `service_tier` on OpenAI sync bodies already forwards.
- `prepare` today shapes OpenAI `stream_options.include_usage` (and related),
  not Flex.
- MCP / Claude hooks never see LLM HTTP.

## Stages

### S0 — Align plan + config surface (docs done on this branch)

**Done when:** `docs/batch-flex.md`, `docs/config.md` stubs, architecture §12,
proxy `AGENTS.md`, and this plan agree on semantics.

**Work:** keep this file updated if decisions change (especially Flex `429`
fallback and Batch `calls.kind` naming).

**Exit:** no code yet; `rtok config validate` still rejects uncommented
`[proxy.batch|flex|routing]` until S1.

### S1 — Config types + validate (no behaviour change)

**Work:**

- Add `ProxyBatch`, `ProxyFlex`, `ProxyRouting` structs with defaults matching
  [`docs/batch-flex.md`](docs/batch-flex.md) (batch observe defaulting carefully:
  `enabled` is effectively always-on via fallback today; prefer `observe` /
  `parse_results` as the real switches).
- Wire into `Config` deserialize + `rtok config validate`.
- Document that unknown-key failure goes away for these tables.

**Tests:** unit parse; golden TOML fixtures; validate accepts commented→live
examples from `docs/config.md`.

**Exit:** config loads; proxy behaviour **unchanged** with defaults.

### S2 — Batch path observe (create / poll / list / cancel)

**Work:**

- Detect Batch path classes (OpenAI + Anthropic tables in
  `docs/batch-flex.md`) without folding them into sync `Wire` adapters.
- Tag `calls` (kind/metadata) so Batch is distinguishable from sync
  `api_request`.
- Keep bodies pass-through; do **not** run compress/`proxy_filter` on Batch
  create payloads or JSONL file bytes.
- Guard: if semantic cache ever runs on the proxy hop, **skip** Batch paths.

**Tests:** proxy integration — POST/GET Batch paths record tagged rows; body
byte-identical; sync wires still match and parse usage.

**Exit:** `rtok stats` can filter or show Batch hop counts (even without
token totals yet).

### S3 — Batch results → usage (opt-in `parse_results`)

**Work:**

- On Anthropic results fetch / OpenAI output file download through the proxy,
  when `parse_results = true`, parse per-line usage into `usage` (or a rollup
  linked to the batch `calls` row).
- Fail-open on parse errors; never block the client download.
- Price rows: use existing provider rates; Batch discount factors as provider
  documents (document any hard-coded assumptions).

**Tests:** fixture JSONL / results streams → expected `usage` rows; disabled
flag → no parse; malformed line → fail-open.

**Exit:** `rtok stats --price` reflects Batch traffic when observe+parse are on.

### S4 — Flex inject via `prepare` (OpenAI wires)

**Work:**

- In OpenAI Chat + Responses `prepare`: if `[proxy.flex] enabled`, set
  `service_tier = "flex"` when omitted; if `force`, overwrite; if client sent
  `default`/`auto` and `force = false`, leave alone.
- Record effective `service_tier` on the `calls` row.
- **429 policy:** implement only after product decision:
  - `fallback = "none"` (default): surface error to client;
  - `fallback = "default"`: one retry without Flex (**confirm before coding** —
    still marked TODO in design docs).
- Add Flex USD/MTok rows under `[stats.prices]` when rates are chosen (dated).

**Tests:** prepare unit tests for omit/force/respect; integration with mock
upstream; 429 behaviour matrix once policy locked.

**Exit:** enabling `[proxy.flex]` changes request JSON only on OpenAI sync
wires; Anthropic unchanged; prompt-cache docs still accurate.

### S5 — Reporting + docs polish

**Work:**

- Breakdown in `rtok report` / stats: sync vs Flex vs Batch.
- Update `docs/batch-flex.md` “planned” → “today” for shipped pieces; keep
  routing as planned.
- Optional: pointer from `next.md` / plan cards when promoting to `plan.md`.

**Exit:** operator can prove savings from rtok ledger without provider console.

### S6 — Model routing stub only (optional track)

**Work:** parse `[proxy.routing]` (`enabled=false`, `sticky`, `default_model`);
no classifier. Sticky upstream flag may land with prompt-cache work (I-84)
independently.

**Exit:** config ready; no model rewrite until D9 Check exists (follow-on
plan, not part of Batch/Flex MVP definition of done).

## Suggested order / sizing

| Stage | Approx. effort | Risk |
|-------|----------------|------|
| S1 Config | 0.5–1 d | Low — validate regressions |
| S2 Batch observe | 1–2 d | Medium — path matching false positives (`/v1/messages` vs `/v1/messages/batches`) |
| S3 Results parse | 1–2 d | Medium — provider result shapes drift |
| S4 Flex prepare | 1–2 d | Medium — 429 fallback / cache interaction |
| S5 Report + docs | 0.5–1 d | Low |
| S6 Routing stub | 0.5 d | Low |

Batch observe (S2) and Flex (S4) can proceed in parallel after S1; S3 depends
on S2.

## Risks

| Risk | Mitigation |
|------|------------|
| Accidental sync→Batch or treating Batch as a Wire | Explicit non-goal; exact path tables; tests that `/v1/messages` still wires and `/v1/messages/batches` does not |
| Compress/`proxy_filter` mutates Batch JSONL | Never attach Batch paths to sync Wire; integration asserts byte identity |
| Flex 429 silent fallback surprises agents | Default `fallback = "none"`; document; require explicit config for retry |
| Prompt-cache miss from Flex/routing churn | Flex only sets `service_tier`; no system/tools rewrite; sticky stays separate |
| Ledger noise / double-count usage | Tag Batch distinctly; parse_results opt-in; fail-open |
| Config validate breaks existing TOMLs | Defaults off; ship behind this branch’s documented keys only |
| Scope creep into D9 routing | S6 stub only; classifier is a separate plan |

## Tests (summary)

- **Unit:** config parse; Batch path classifier; Flex `prepare` field matrix;
  result-line usage parser fixtures.
- **Integration (proxy):** sync wire regression (usage still parsed); Batch
  create/poll tagged + pass-through; Flex enabled request body; semantic-cache
  skip if that code path exists.
- **Manual / live (optional):** one OpenAI Flex call and one small Anthropic
  Message Batch through `rtok proxy` against a throwaway key — document in PR,
  not required for CI.

Reuse existing proxy test harness patterns under `src/proxy/` / `tests/` (same
style as current prepare / wire tests). Prefer named constants over magic
numbers per listepo Rust norms.

## Definition of done (MVP)

MVP = **S1 + S2 + S4 + S5** (S3 strongly recommended for “savings in rtok”;
S6 not required).

- [ ] `[proxy.batch]` / `[proxy.flex]` parse and validate
- [ ] Batch hops tagged in `calls`; bodies unchanged
- [ ] Flex inject/force on OpenAI Chat/Responses via `prepare`; effective tier
      recorded
- [ ] Flex `429` policy implemented as documented (no silent default unless
      configured)
- [ ] Stats/report can separate Flex vs sync (Batch counts at minimum; usage if
      S3 shipped)
- [ ] `docs/batch-flex.md` updated for shipped behaviour; no sync→Batch
- [ ] CI green for new unit/integration tests on Mac; no CloudAgent for listepo
      work

## Related

- [`docs/batch-flex.md`](docs/batch-flex.md) — semantics and endpoint tables
- [`docs/config.md`](docs/config.md) — planned TOML keys
- [`docs/prompt-cache.md`](docs/prompt-cache.md) — sticky vs Batch/Flex
- [`architecture.md`](architecture.md) §12 — pipeline placement
- [`src/plugins/proxy/AGENTS.md`](src/plugins/proxy/AGENTS.md) — agent notes
- `next.md` (PR #266) — token-saving backlog card for Batch/Flex/routing

## Feature checklist — Batch / Flex (T250–T259)

Executable checklist: what to build for each stage/feature. Inherits global
non-goals from the research section above.

## Batch / Flex API pass — detailed task cards (T250–T259)

Second pass after merging `batch-flex-plan.md` into this file. Expands each stage into
`plan.md`-style cards grounded in `docs/batch-flex.md`, `architecture.md` §12, and the
real `src/proxy/` + `src/config/` + `src/store/` surfaces as of this branch. Do not treat
the verbatim plan section above as obsolete — these cards are the executable checklist.

**MVP definition of done (from merged plan):** S1 + S2 + S4 + S5 (T251, T252, T254, T255);
S3 (T253) strongly recommended; S6 (T256) stub only. T257–T259 are cross-cutting.

**Global non-goals (every card inherits):** no transparent sync→Batch conversion; no Batch
JSONL member-body rewrite; no Batch/Flex logic in hooks or MCP; Anthropic has no Flex
`service_tier` twin; no CloudAgent for listepo work; do not break prompt-cache sticky
behaviour in `docs/prompt-cache.md`.

**Data-flow baseline (architecture §12 / `src/proxy/mod.rs`):**
`client → axum fallback (`app` → `proxy` → `handle`) → optional `shape_request`
(`record` → `compress` → `rewrite_tools` → `context_edits` → `prepare`) → upstream →
response tee → `record_usage` / `finish` when a `Wire` matches.** Batch paths miss
`wire::for_path` today and stay on the byte-forward arm.

### T250. Epic: Batch / Flex API pass overview

Parent epic for T251–T259. Cost levers: Batch observe + optional result→usage, Flex inject
on OpenAI sync wires via `Wire::prepare_request`, measurable sync vs Flex vs Batch in
`rtok stats` / `rtok report`. Authoritative semantics: [`docs/batch-flex.md`](docs/batch-flex.md).
Pipeline placement: [`architecture.md`](architecture.md) §12. Config stubs already in
[`docs/config.md`](docs/config.md) (`[proxy.batch]` / `[proxy.flex]` / `[proxy.routing]` —
not loaded by the binary yet; `rtok config validate` rejects them as unknown keys).

**Goals**

1. When clients already hit OpenAI `/v1/batches*` or Anthropic `/v1/messages/batches*`
   through `rtok proxy`, tag those hops in `calls` and optionally attribute usage once
   results are fetched (`parse_results`).
2. Optional `[proxy.flex]` so OpenAI Chat/Responses get `service_tier = "flex"` in
   `prepare` without the agent knowing about Flex.
3. Keep boundaries: never sync→Batch; never hooks/MCP; do not regress prompt-cache sticky.
4. `rtok stats` / `rtok report` can distinguish sync vs Flex vs Batch once observe + Flex
   pricing exist.

**Primary modules (touch only these for behaviour)**

| Area | Paths |
|------|--------|
| Proxy HTTP | `src/proxy/mod.rs` (`handle`, `shape_request`, `prepare`, `record`, `record_usage`, `finish`, `cache_response`, `app`) |
| Wires | `src/proxy/wire.rs` (`Wire`, `for_path`, `Usage`, `prepare_request` default), `openai_chat.rs`, `openai_responses.rs`, `anthropic.rs`, `gemini.rs` |
| Semantic cache skip | `src/proxy/semantic_cache.rs` (`eligible`, `build_prompt`) |
| Config | `src/config/mod.rs` (`section! { Proxy {…} }`, nest new sections), `src/config/validate.rs` |
| Store | `src/store/mod.rs` (`insert_call`, `insert_usage`, `insert_call_io`), `src/store/schema.rs` (`calls`, `usage`, `call_io`) |
| Report/stats | `src/report/` (and the `rtok stats` path that reads `usage` + `[stats.prices]`) |
| Docs | `docs/batch-flex.md`, `docs/config.md`, `architecture.md` §12, `src/plugins/proxy/AGENTS.md` |

**`calls` columns today (migrations `0002_schema_v2`, diesel `schema.rs`):**
`id`, `ts`, `session_id`, `host_id`, `provider_id`, `model_id`, `plugin`, `surface`,
`kind`, `parent_id`, `name`, `ms`, `ok`, `error`. Sync proxy hops use
`surface = "proxy"`, `kind = "api_request"`, `name = Some(path)`. **No JSON metadata
column.** Prefer tagging via `kind` / `name` / `call_io.request_json` (T258) over a new
column unless report queries prove otherwise.

**`usage` today (`0005_usage_api` added `api`):**
`id`, `ts`, `session`, `model`, `input`, `cache_create`, `cache_read`, `output`,
`call_id`, `api`.

Plan: keep this epic card as the index; implement only through child cards. Claim children
individually in the summary table.

Check: every child T251–T259 has a card below; `rg '^### T25[0-9]' plan.md` lists ten
headings; no collision with `done.md`; MVP checklist in the merged plan section remains
the acceptance spine.

### T251. S1: Config `[proxy.batch|flex|routing]` structs, defaults, validate

**Stage:** S1 — config only; proxy behaviour unchanged with defaults.

**Files**

- `src/config/mod.rs` — add `section!` blocks (same macro as `ToolsRewrite` / `Proxy`):
  - `ProxyBatch { enabled: bool = true, observe: bool = true, parse_results: bool = false }`
  - `ProxyFlex { enabled: bool = false, force: bool = false, fallback: String = s("none") }`
  - `ProxyRouting { enabled: bool = false, sticky: bool = true, default_model: String = String::new() }`
- Nest on existing `Proxy` section: `batch: ProxyBatch = ProxyBatch::default()`,
  `flex: ProxyFlex = ProxyFlex::default()`, `routing: ProxyRouting = ProxyRouting::default()`.
- `src/config/validate.rs` — accept the new dotted keys; reject
  `proxy.flex.fallback` unless `"none"` | `"default"` (mirror `proxy.mode` style around
  the `"proxy.mode" if !matches!(…)` arm ~line 324).
- `docs/config.md` — flip the “not loaded / validate fails” warnings for these three
  tables once the fields exist (full “planned→today” narrative is T259).

**Every config key**

| Table | Key | Type | Default | Meaning |
|-------|-----|------|---------|---------|
| `[proxy.batch]` | `enabled` | bool | `true` | Master switch (fallback already forwards Batch paths) |
| `[proxy.batch]` | `observe` | bool | `true` | Record distinguishable Batch ledger rows |
| `[proxy.batch]` | `parse_results` | bool | `false` | Parse result streams/files into `usage` (T253) |
| `[proxy.flex]` | `enabled` | bool | `false` | `prepare` may set OpenAI `service_tier = "flex"` when omitted |
| `[proxy.flex]` | `force` | bool | `false` | Overwrite client `service_tier` |
| `[proxy.flex]` | `fallback` | string | `"none"` | `none` \| `default` on Flex 429 (policy in T254) |
| `[proxy.routing]` | `enabled` | bool | `false` | D9 stub only (T256) |
| `[proxy.routing]` | `sticky` | bool | `true` | Prompt-cache affinity flag (I-84); not Batch vs Flex |
| `[proxy.routing]` | `default_model` | string | `""` | Empty = leave client `model` |

**Tests (snake_case)**

- `proxy_batch_flex_routing_defaults_match_docs` — `Config::default()` equals the table above.
- `config_validate_accepts_proxy_batch_flex_routing_tables` — live TOML from `docs/config.md` examples → zero unknown-key errors.
- `config_validate_rejects_proxy_flex_fallback_typo` — `fallback = "retry"` errors.
- `config_deserialize_nested_proxy_batch_observe_false` — nested parse round-trip.
- `rtok_config_validate_trycmd_batch_flex_stub` (trycmd or unit) — previously rejecting fixture now green.

**Risks**

| Risk | Mitigation |
|------|------------|
| Existing user TOMLs that already pasted the stubs start loading unexpected behaviour | Defaults keep observe on but S2/S4 code must no-op until those stages; S1 ships config only |
| `section!` macro surprises on nested structs | Follow `tools_rewrite: ToolsRewrite` precedent on `Proxy` |
| Validate walk misses nested keys | Extend the same dotted-key walker tests that cover `proxy.tools_rewrite.*` |

**Non-goals:** no path classifier, no `prepare` Flex, no report changes.

Check: `cargo nextest run -E 'test(proxy_batch_flex_routing) \| test(config_validate_accepts_proxy_batch)'`; `rtok config validate` on a fixture that includes the three tables; `just check` green; with defaults, a sync `/v1/messages` integration still records `kind = api_request` and parses usage unchanged.

### T252. S2: Batch path classifier + observe tagging (no Wire match, byte-identical)

**Stage:** S2 — observe create/poll/list/cancel (+ files) without folding Batch into sync `Wire` adapters.

**Files**

- New helper module preferred: `src/proxy/batch.rs` (or private fns in `mod.rs`) —
  `BatchClass` enum + `classify(method: &Method, path: &str) -> Option<BatchClass>`.
- `src/proxy/mod.rs` — `handle` / `shape_request` / `record`: when `classify` hits and
  `cfg.proxy.batch.enabled && cfg.proxy.batch.observe`, insert a tagged `calls` row;
  **skip** `compress`, `rewrite_tools`, `context_edits`, and Wire `prepare`; forward
  body bytes unchanged.
- `src/proxy/wire.rs` — do **not** add Batch to `WIRES` / `for_path`; keep
  `Anthropic::matches` as exact `path == "/v1/messages"` (already excludes
  `/v1/messages/batches…`).
- `src/proxy/semantic_cache.rs` — hard skip: if Batch classified, never call
  `eligible` / `lookup` (guard in `handle` before the cache block that currently gates on
  `wire` + `eligible`).
- `src/proxy/openai_chat.rs` / `openai_responses.rs` / `anthropic.rs` / `gemini.rs` —
  regression only (exact `matches` unchanged).

**Path tables (`docs/batch-flex.md`) — classifier must accept**

OpenAI:

| Method | Path pattern | Suggested `calls.kind` |
|--------|--------------|------------------------|
| `POST` | `/v1/batches` | `batch_create` |
| `GET` | `/v1/batches/{id}` | `batch_poll` |
| `GET` | `/v1/batches` | `batch_list` |
| `POST` | `/v1/batches/{id}/cancel` | `batch_cancel` |
| `*` | `/v1/files` and `/v1/files/{id}` (+ `/content`) | `batch_files` (or `files` — document choice; still observe when Batch-related traffic shares the route) |

Anthropic:

| Method | Path pattern | Suggested `calls.kind` |
|--------|--------------|------------------------|
| `POST` | `/v1/messages/batches` | `batch_create` |
| `GET` | `/v1/messages/batches/{id}` | `batch_poll` |
| `GET` | `/v1/messages/batches` | `batch_list` |
| `GET` | `/v1/messages/batches/{id}/results` | `batch_results` |
| `DELETE` | `/v1/messages/batches/{id}` | `batch_delete` |

**Data-flow (Batch create example)**

1. `handle` reads body (`MAX_BODY_BYTES`).
2. `wire::for_path("/v1/batches")` → `None`.
3. `batch::classify(POST, path)` → `Some(BatchCreate { provider: openai })`.
4. If `batch.enabled && batch.observe`: `record`-like insert with
   `surface = "proxy"`, `kind = "batch_create"`, `name = Some(path)` (and provider_id when
   known); **do not** parse as sync chat JSON for compress.
5. Skip semantic cache (no wire / explicit Batch guard).
6. `join_upstream` → OpenAI upstream; forward **byte-identical** request body.
7. Response tee: no `Wire::usage_from_body` (no wire); still set `calls.ms` /
   `call_io` best-effort like other recorded hops.

**False-positive guard:** `/v1/messages` must still match `Anthropic` and run compress/
prepare/usage; `/v1/messages/batches` must **not**.

**Tests**

- `batch_classify_openai_create_poll_list_cancel`
- `batch_classify_anthropic_create_poll_list_results_delete`
- `batch_classify_does_not_match_sync_messages_or_chat_completions`
- `batch_observe_posts_tagged_call_kind_batch_create`
- `batch_observe_body_byte_identical_no_compress`
- `batch_observe_disabled_skips_ledger_still_forwards` (`observe = false` or `enabled = false`)
- `sync_messages_still_wires_and_parses_usage` (regression)
- `semantic_cache_skips_batch_paths_even_when_cache_enabled`

**Risks**

| Risk | Mitigation |
|------|------------|
| `/v1/messages` vs `/v1/messages/batches` confusion | Exact prefix rules + dedicated tests |
| Compress/`proxy_filter` mutates Batch JSONL | Never attach Batch to sync `Wire`; assert byte identity |
| `/v1/files` noise (non-batch file traffic) | Document; optionally require Batch observe only when path is under batches* and treat files as optional observe — record decision in card Check notes |
| Ledger noise | Distinct `kind` values; `observe` switch |

**Non-goals:** result→usage (T253); Flex (T254); sync→Batch.

Check: `cargo nextest run -E 'test(batch_)'`; manual curl through proxy to `/v1/batches` against a mock; `just check`.

### T253. S3: `parse_results` → usage (Anthropic results + OpenAI files); fail-open

**Depends on:** T252. Opt-in via `[proxy.batch] parse_results = true`.

**Files**

- `src/proxy/batch.rs` (or `batch_results.rs`) — parsers:
  - Anthropic `GET …/messages/batches/{id}/results` JSONL / SSE-ish stream → per-line
    usage → `Store::insert_usage` linked to the Batch `calls` row (parent or same hop).
  - OpenAI output file download (`/v1/files/{id}/content`) when content is Batch output
    JSONL → same.
- `src/proxy/mod.rs` — after upstream response for classified `batch_results` /
  `batch_files` download paths, if `parse_results`, spawn fail-open parse (never delay or
  corrupt the client byte stream: parse a tee copy / post-forward buffer like existing
  usage tee patterns in `finish`).
- `src/store/mod.rs` — reuse `insert_usage` / `insert_provider_tokens`; set `usage.api` to
  a stable discriminator (e.g. `"openai_batch"` / `"anthropic_batch"`) distinct from sync
  `"openai"` / `"anthropic"` (`0005_usage_api`).
- Price: existing `[stats.prices]` rows × provider Batch discount factors — **document**
  any hard-coded factor and date it (do not invent silent discounts).

**Fail-open rules**

- Malformed line → log + skip line; continue.
- Parse panic / store error → log; client download already completed or streams unaffected.
- `parse_results = false` → zero `usage` rows from Batch results.

**Tests**

- `parse_anthropic_batch_results_jsonl_inserts_usage_rows`
- `parse_openai_batch_output_file_jsonl_inserts_usage_rows`
- `parse_results_disabled_inserts_no_usage`
- `parse_results_malformed_line_fail_open_skips_line`
- `parse_results_does_not_block_or_alter_response_bytes`

**Risks**

| Risk | Mitigation |
|------|------------|
| Provider result shape drift | Fixtures from current provider docs; fail-open |
| Double-count if create + results both counted | Only `parse_results` writes token `usage`; observe rows stay hop metadata |
| Large JSONL memory | Stream line-by-line; cap like `MAX_BODY_BYTES` policy |

**Migration:** none (T258) — `usage.api` + `call_id` suffice.

Check: `cargo nextest run -E 'test(parse_) \| test(batch_results)'`; `rtok stats --price` on a DB fixture with Batch usage; `just check`.

### T254. S4: Flex `prepare_request` on OpenAI wires; force/omit; 429 policy; tier metadata; prices

**Stage:** S4 — OpenAI only. Anthropic / Gemini unchanged.

**Files**

- `src/proxy/openai_chat.rs` — extend `OpenAiChat::prepare_request` (today only sets
  `stream_options.include_usage` when streaming). After include_usage logic, apply Flex:
  - if `!cfg` passed in: either extend signature
    `prepare_request(&self, body, include_usage, flex: FlexOpts)` **or** read Flex in
    `prepare` wrapper in `mod.rs` before/after `wire.prepare_request` (prefer keeping
    `Wire` free of `Config` — apply Flex in `mod.rs::prepare` for OpenAI wires only, or
    pass a small `PrepareOpts { include_usage, flex_enabled, flex_force }`).
- `src/proxy/openai_responses.rs` — same Flex field rules on Responses bodies.
- `src/proxy/wire.rs` — only if signature of `prepare_request` changes; default remains
  no-op for Anthropic/Gemini.
- `src/proxy/mod.rs` — `prepare`; optional 429 retry arm in `handle` when upstream
  returns 429 and `proxy.flex.fallback == "default"` (**product decision must be locked
  before coding the retry**; default shipped config is `"none"` = surface error).
- `src/proxy/mod.rs` `record` / post-prepare bookkeeping — record effective tier:
  prefer encoding in `calls.name` (e.g. keep path, add nothing if default) **or** rely on
  `call_io.request_json` containing `service_tier` after prepare (T258: **no migration**
  unless report needs a first-class column).
- `src/config/mod.rs` / `docs/config.md` — add dated Flex `[stats.prices."<model>"]`
  rows (or document Flex multiplier) when rates chosen.
- `docs/prompt-cache.md` — confirm Flex only sets `service_tier`, no system/tools rewrite.

**Flex field matrix**

| Client body `service_tier` | `enabled` | `force` | Result |
|----------------------------|-----------|---------|--------|
| omitted | false | * | unchanged |
| omitted | true | false | set `"flex"` |
| `"flex"` | true | false | unchanged |
| `"default"` / `"auto"` / other | true | false | **leave alone** |
| any | true | true | set `"flex"` |
| any | false | true | unchanged (force only applies when enabled — document) |

**429 policy**

- `fallback = "none"` (default): return upstream 429 to client; no silent retry.
- `fallback = "default"`: **one** retry with `service_tier` removed or set to default —
  only after explicit product confirm (still TODO in `docs/batch-flex.md`). Tests must
  cover both once locked.

**Tests**

- `flex_prepare_sets_tier_when_omitted_and_enabled`
- `flex_prepare_force_overwrites_client_tier`
- `flex_prepare_respects_client_default_when_not_force`
- `flex_prepare_noop_when_disabled`
- `flex_prepare_anthropic_body_untouched`
- `flex_prepare_preserves_include_usage_behaviour`
- `flex_429_fallback_none_surfaces_error`
- `flex_429_fallback_default_retries_once_without_flex` (gate on policy lock)
- `flex_effective_tier_visible_in_call_io_or_name`

**Risks**

| Risk | Mitigation |
|------|------------|
| Silent 429 fallback surprises agents | Default `none`; require explicit config |
| Prompt-cache miss from body churn | Only `service_tier` mutation; no tools/system rewrite |
| Breaking `include_usage` prepare | Compose Flex after existing logic; shared tests |

**Non-goals:** Anthropic Flex; model routing (T256); Batch path changes.

Check: `cargo nextest run -E 'test(flex_)'`; integration with mock upstream asserting JSON body; `just check`.

### T255. S5: report/stats breakdown sync vs Flex vs Batch

**Depends on:** T252 (kinds), T254 (tier visibility); T253 improves Batch USD.

**Files**

- Stats/report codepaths that aggregate `calls` / `usage` (locate current
  `rtok stats` / `rtok report` modules under `src/` — extend grouping by `calls.kind`
  and by effective tier from `call_io.request_json` or recorded name convention).
- `docs/batch-flex.md` — operator-facing “how to read the breakdown”.
- Optional web/tui proxy pages only if they already surface usage (do not invent a new
  dashboard epic here).

**Breakdown requirements**

- Counts: sync `api_request` vs `batch_*` kinds.
- Flex: sync OpenAI calls whose effective `service_tier == "flex"` vs not.
- Price: `--price` uses `[stats.prices]` (+ Batch discount docs from T253).

**Tests**

- `stats_breakdown_counts_batch_kinds_separately`
- `stats_breakdown_flags_flex_tier_rows`
- `report_sync_flex_batch_sections_smoke`

Check: fixture DB → `rtok stats` / `rtok report` output contains three buckets; `just check`.

### T256. S6: `[proxy.routing]` stub only (`enabled = false`)

**Files:** `src/config/mod.rs` (already in T251), `src/config/validate.rs`,
`docs/config.md`. **No** model rewrite in `prepare` / `shape_request` until D9 has a
measurement Check (separate plan). Sticky upstream may land with prompt-cache work (I-84)
independently — do not block Batch/Flex MVP.

**Tests:** `routing_stub_deserializes_enabled_false`; `routing_enabled_true_still_does_not_rewrite_model` (explicit no-op assert).

Check: config loads; grep/prepare paths unchanged; `just check`.

### T257. Tests matrix master checklist

Aggregate gate for the pass (run on Mac; no CloudAgent). Every name below must exist
before closing the epic (implementing cards own the code; this card closes when the
matrix is green).

**Unit**

- Config: `proxy_batch_flex_routing_defaults_match_docs`,
  `config_validate_accepts_proxy_batch_flex_routing_tables`,
  `config_validate_rejects_proxy_flex_fallback_typo`
- Classifier: `batch_classify_*` (OpenAI + Anthropic + sync negative)
- Flex prepare: `flex_prepare_*` matrix
- Parsers: `parse_anthropic_batch_results_jsonl_inserts_usage_rows`,
  `parse_openai_batch_output_file_jsonl_inserts_usage_rows`,
  `parse_results_malformed_line_fail_open_skips_line`

**Integration (proxy harness, same style as existing prepare/wire tests under
`src/proxy/` / `tests/`)**

- `sync_messages_still_wires_and_parses_usage`
- `batch_observe_body_byte_identical_no_compress`
- `batch_observe_posts_tagged_call_kind_batch_create`
- `semantic_cache_skips_batch_paths_even_when_cache_enabled`
- `flex_prepare` integration against mock upstream
- `flex_429_*` once policy locked
- `parse_results_does_not_block_or_alter_response_bytes`

**Manual / live (optional, PR notes only):** one OpenAI Flex call + one small Anthropic
Message Batch through `rtok proxy` with a throwaway key.

**Commands:** `cargo nextest run -E 'test(batch_) \| test(flex_) \| test(parse_results) \| test(proxy_batch)'`; `just check`.

### T258. Migrations decision: prefer no schema change

**Verify (already true on this branch):** `calls` has
`id, ts, session_id, host_id, provider_id, model_id, plugin, surface, kind, parent_id,
name, ms, ok, error` — **no** `metadata` / `service_tier` column. `usage` has `api` +
`call_id` (`0005`, `0013` indexes).

**Decision for this pass:** **no migration** by default.

| Need | Store in |
|------|----------|
| Batch vs sync | `calls.kind` (`batch_create`, `batch_poll`, … vs `api_request`) |
| Path | `calls.name` (already `Some(path)` in `record`) |
| Effective Flex tier | `call_io.request_json` after prepare (authoritative body) and/or stats join; optional convention documented in T254 |
| Batch usage source | `usage.api` = `openai_batch` / `anthropic_batch` |

**Only if** report queries cannot filter Flex without JSON extract at unacceptable cost:
propose `0023_calls_service_tier` (`ALTER TABLE calls ADD COLUMN service_tier TEXT`) and
update `src/store/schema.rs` + diesel models — separate mini-PR, not required for MVP.

**Tests if no migration:** schema-drift guard still green; document the decision in
`docs/batch-flex.md`.

Check: `rg 'service_tier' migrations/` empty (unless mini-PR); `just check`.

### T259. Docs flip planned→today + acceptance

**Files:** `docs/batch-flex.md` (planned→today for shipped S1/S2/S4/S5 pieces; keep
routing planned), `docs/config.md` (remove “validate fails” for loaded keys),
`architecture.md` §12 table, `src/plugins/proxy/AGENTS.md`, optional pointer from
`next.md` when promoting.

**Acceptance (MVP = T251+T252+T254+T255; T253 strongly recommended)**

- [ ] `[proxy.batch]` / `[proxy.flex]` parse and validate
- [ ] Batch hops tagged in `calls`; bodies unchanged
- [ ] Flex inject/force on OpenAI Chat/Responses; effective tier recorded without hooks/MCP changes
- [ ] Flex 429 policy as documented (no silent default unless configured)
- [ ] Stats/report separate Flex vs sync (Batch counts at minimum; usage if T253 shipped)
- [ ] `docs/batch-flex.md` updated; explicit **no** sync→Batch
- [ ] CI green for new tests on Mac

**Explicit non-goals restate:** no sync→Batch; hooks/MCP untouched; Anthropic has no Flex;
S6 routing stub only; no local LLM disk inventory in this plan.

Check: docs prose matches behaviour; `just check`; epic T250 can move to `done.md` when
all MVP boxes are ticked.

## Batch / Flex — second-pass gap fill

Appended after T250–T259 to close gaps found by re-reading the live code. Does **not**
replace the verbatim merged plan or the T-cards; implementers must read all three.

### G1. Correct `shape_request` order (fix T250 data-flow shorthand)

Real order in `src/proxy/mod.rs` `shape_request` (as of this branch):

1. `serde_json::from_slice` → `parsed`
2. `record(state, wire, path, parsed, headers, raw)` → `Option<Recorded>`
3. if `state.mode == "compress"` and `wire` is `Some`: `compress(...)` (else body unchanged;
   **unwired paths including Batch never compress**)
4. if `wire` is `Some`: `prepare(state, wire, body)` → `Wire::prepare_request`
5. if `wire` is `Some`: `context_edits` (Anthropic `context_management` only)
6. `rewrite_tools(state, body)` (runs even without a wire today — **Batch observe must
   either skip this for classified Batch paths or prove `tools_rewrite` no-ops on
   non-JSON-chat bodies**; prefer an explicit Batch early-return after `record` that
   returns `(original_bytes, recorded, false)` and skips steps 3–6)

Implication for T252: do not only “skip compress”; also skip `prepare`, `context_edits`,
and `rewrite_tools` for Batch-classified paths so JSONL/`/v1/files` bytes stay identical.

### G2. `plain()` and `dry_run` interactions

- `ProxyState::plain()` (`src/proxy/mod.rs`): when true (`proxy.enabled` / `core.enabled`
  false), `handle` forwards byte-identical with **no** `shape_request` / record / cache.
  Batch observe must not invent bookkeeping in plain mode.
- `[proxy] dry_run` (config field on `Proxy`): confirm current behaviour before Batch/Flex
  (existing proxy semantics); Flex inject and Batch parse must honour the same dry-run
  contract as sync traffic — document in T254/T253 Checks once read.

### G3. Exact `WIRES` set (do not extend for Batch)

`wire::for_path` (`src/proxy/wire.rs`):

```text
WIRES = [&ANTHROPIC, &OPENAI_CHAT, &OPENAI_RESPONSES, &GEMINI]
```

API constants: `API_ANTHROPIC`, `API_OPENAI_CHAT`, `API_OPENAI_RESPONSES`, `API_GEMINI`.
Gemini (`src/proxy/gemini.rs`) has no Batch/Flex role in this pass — classifier must not
treat `:generateContent` as Batch; Flex must not touch Gemini bodies.

Exact sync matches today:

| Wire | `matches` |
|------|-----------|
| `Anthropic` | `path == "/v1/messages"` |
| `OpenAiChat` | `path == "/v1/chat/completions"` |
| `OpenAiResponses` | (Responses path — keep as in `openai_responses.rs`) |
| `Gemini` | generate/streamGenerate actions only |

### G4. Stats / report concrete modules (refine T255)

Not only `src/report/`:

| Surface | Path |
|---------|------|
| `rtok stats` | `src/cli.rs` → `crate::web::model::stats_report` (`src/web/model.rs`) + `src/measure/stats` |
| `rtok report` | `src/report/mod.rs` `document`, renderers `markdown.rs` / `html.rs` / `pdf.rs` |
| Prices | `[stats.prices]` / `ModelPrice` in `src/config/mod.rs`; `--price` via `stats_flags` in `cli.rs` |

T255 should extend `stats_report` grouping and, if the markdown report’s Calls section is
the operator-facing breakdown, add a sync/Flex/Batch subsection in `src/report/markdown.rs`
fed by model structs — prefer one shared aggregation helper to avoid three total formulas
(see historical T207 lesson in `done.md`).

### G5. `insert_call` / `Recorded` / `parent_id`

- `Store::insert_call(session, surface, kind, host_id, provider_id, model_id, plugin, name)`
  — proxy sync uses `surface="proxy"`, `kind="api_request"`, `plugin` often via other
  helpers; `record` passes `plugin` as implicit through surface and `name=Some(path)`.
- `Store::set_call_parent(id, parent)` exists — T253 may parent result-parse usage’s
  `call_id` under the `batch_create` row when the batch id is known from the path.
- `Recorded { session, model, call_id }` in `mod.rs` — Batch observe can reuse or mirror.

### G6. Semantic cache config path

Cache is `[plugins.proxy.semantic_cache]` (`SemanticCache` on plugins proxy section),
**not** under `[proxy.]`. Guard in `handle` uses `state.cfg.plugins.proxy.semantic_cache`.
T252 skip must use that path.

### G7. OpenAI Responses `matches` + prepare today

Confirm in `openai_responses.rs`: `matches` path string and whether `prepare_request`
already mutates the body (include_usage analogue). Flex composition must not drop that
behaviour (T254 test `flex_prepare_preserves_include_usage_behaviour` covers Chat;
add `flex_prepare_preserves_responses_prepare_behaviour` if Responses has its own prepare).

### G8. Additional test names discovered in gap pass

- `batch_shape_request_skips_rewrite_tools_and_context_edits`
- `batch_observe_noop_in_plain_mode`
- `batch_classify_rejects_gemini_generate_content`
- `flex_prepare_preserves_responses_prepare_behaviour`
- `stats_report_includes_sync_flex_batch_buckets` (wire to `web::model::stats_report`)
- `report_markdown_mentions_batch_or_flex_when_present`

### G9. Risks added

| Risk | Mitigation |
|------|------------|
| `rewrite_tools` runs without a wire and could alter Batch JSON | Batch early-return in `shape_request` (G1) |
| Stats totals diverge across cli/report/web | One aggregation helper (G4); regression test |
| Flex prepare signature fight with `Wire` trait | Prefer `PrepareOpts` in `mod.rs::prepare` without forcing Config into every wire |
| `usage.api` new discriminators break dashboards that assume only anthropic/openai | Document; update any match/`GROUP BY api` readers |

### G10. Acceptance command pack (copy into PR)

```bash
just check
cargo nextest run -E 'test(batch_) | test(flex_) | test(parse_results) | test(proxy_batch_flex)'
rg -n 'proxy\.(batch|flex|routing)' src/config/mod.rs
rg -n 'batch_create|service_tier' src/proxy/
# plain-mode smoke: with proxy.enabled=false, Batch POST must not insert calls
```
