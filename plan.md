# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T79 | todo | P1 | 3 | 0% | |
| T83 | todo | P1 | 4 | 0% | |
| T86 | todo | P1 | 3 | 0% | |
| T87 | in progress | P1 | 2 | 70% | Claude Code / claude-fable-5-1 |
| T88 | todo | P1 | 2 | 0% | |
| T89 | todo | P1 | 3 | 0% | |
| T91 | todo | P1 | 3 | 0% | |
| T94 | todo | P1 | 3 | 0% | |
| T95 | todo | P1 | 2 | 0% | |
| T96 | todo | P1 | 3 | 0% | |
| T97 | in progress | P1 | 3 | 95% | Claude Code / claude-fable-5-1 |
| T100 | todo | P1 | 3 | 0% | |
| T101 | todo | P2 | 2 | 0% | |
| T102 | todo | P2 | 3 | 0% | |
| T105 | todo | P2 | 2 | 0% | |
| T106 | todo | P3 | 2 | 0% | |
| T107 | todo | P3 | 2 | 0% | |
| T116 | todo | P2 | 3 | 0% | |
| T117 | todo | P2 | 3 | 0% | |
| T118 | todo | P2 | 4 | 0% | |
| T122 | in progress | P1 | 3 | 5% | Claude Code / claude-haiku-4-5 |
| T123 | in progress | P2 | 2 | 5% | Claude Code / claude-haiku-4-5 |
| T124 | todo | P3 | 2 | 0% | |
| T125 | in progress | P2 | 2 | 5% | Claude Code / claude-haiku-4-5 |
### T79. `agents install zed` aborts on a real settings.json (JSONC)

Found by T78 on its first run, against this machine's own files. Zed writes **JSONC**: its `settings.json` carries `//` comments and trailing commas. `rtok_agent_sdk::read_json` is strict `serde_json::from_str`, so `edit_json` fails and `rtok agents install zed` exits with `trailing comma at line 44 column 3` and writes nothing. Every synthetic test passes because every synthetic config is strict JSON. VS Code's settings.json is JSONC by the same rule and shares the risk — this machine's happens to be strict JSON, so `vscode_keeps_the_real_settings_json` passes here and is not proof either way.

Reproduce: `rtok agents install zed` with a `~/.config/zed/settings.json` Zed itself wrote. The repro is kept as the `#[ignore]`d `zed_keeps_the_real_settings_json` in `tests/agents_real_config.rs` — un-ignore it as the check.

Plan (needs a decision first): a JSONC read alone is not enough — writing back through `serde_json` would silently delete the user's comments, which for Zed is most of the file. So either (a) edit the JSONC hosts in place with a comment-preserving editor, or (b) keep strict parsing and refuse with a message naming the file and the line the user can paste in themselves. Ask the creator which before starting; (b) is small and honest, (a) is the one users want and likely needs a dependency.

Check: un-ignore `zed_keeps_the_real_settings_json` in `tests/agents_real_config.rs` and it passes against a Zed-written `settings.json` — under (a) the comments and trailing commas are still in the file afterwards, under (b) the command exits non-zero with the message and the test asserts that instead. `just check` green either way.

### T83. Fix the Windows test failures and empty the T82 exclusion list

The `cfg(windows)` `default-filter` in `.config/nextest.toml` (T82) names every test that fails on `windows-latest` (29). Families from run 35244082778: `tests/demon.rs` (3 × 180 s timeout — process-tree start/stop), `plugins::cmd::run::tests` (4 — shell spawn), `log::tests` + `demon::tests` rotation (4 — rename of an open file), `agents_install` / `cursor_plugin` / `pi_plugin` / `opencode_plugin` (8 — symlink and path expectations), `agents::claude` desktop config path, `agents_doc`, `cli_trycmd`, `commands_e2e` run→expand, `otel` unreachable endpoint (131 s), `cmd::formatters::ten_families_and_aws_key_unredacted`. Four more from run 35576438155 (2026-09-21, after T93): `cmd::run::tests::identical_output_from_different_commands_dedups` (dedup count 1 ≠ 0), `read::cache::tests::vfs_small_change_is_hunks_large_is_full_missing_archive_is_full` (`unchanged since …` instead of hunks), `agent_remove::uninstall_clears_the_installed_marks_over_a_materialized_plugin_copy` (os error 4390, not a reparse point), `plugins_e2e::graph_session_start_map_off_by_default_and_on_when_capped` (empty `{}`). T55.12 (Git Bash `wrap_quote`) is in the same area. Each is either a product bug on Windows or a test that assumes Unix; decide per family, fix, and delete its line from the list.

Done when the exclusion list is empty, the override is deleted, and the `windows` job drops `continue-on-error` and joins `revert-on-failure`'s `needs` (or moves into the `check` matrix if `just check` runs on Windows). Split into T83.x per family when claimed.

Check: `ci` run with the `windows` job green and 0 tests skipped by the platform filter.

### T86. `rtok agents install kimi` offers the plugin, and the plugin is the singleton

After T85. Precondition, by the creator on a live Kimi (agents cannot drive its TUI): `/plugins install <repo>/plugins/kimi`, `/reload`, then call the rtok MCP `tree` tool with no path — it must list the session project, not `plugins/managed/rtok/`. If it lists the plugin copy, Kimi starts plugin MCP servers in the plugin root and the singleton rule below must keep `mcp.json` and strip only the hooks. `support("plugin")` stops saying "no": install prints the exact `/plugins install <resolved plugins/kimi path>` line (dry-run and apply alike; rtok never writes `plugins/managed/` or `installed.json` — that format is Kimi's and undocumented). `installed()` reports `plugin` when `<kimi home>/plugins/managed/rtok/kimi.plugin.json` exists. D21 singleton: while the plugin is installed, setup strips rtok's own `[[hooks]]` tables and `mcpServers.rtok` from `config.toml` / `mcp.json` instead of adding them (Cursor's `plugin_is_mcp` rule, for hooks too), so no event fires twice and one `rtok mcp` serves the store. A `Desktop` variant (`Kimi Code.app`) joins `VARIANTS` with the same files. `src/agents/kimi/README.md` module table, `docs/agents.md` (bless), `research.md` §15 sentence listing Kimi among hosts without a plugin directory.

Check: unit tests — offer names `plugins/kimi` and `/plugins install`; with a seeded `managed/rtok/kimi.plugin.json` a second install removes the nine tables and `mcpServers.rtok` and reports `plugin`; remove leaves the managed copy alone and says how to remove it (`/plugins remove rtok`); `agents_doc` blessed; `just check`.

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

### T91. `rtok agents install antigravity` — CLI and desktop, the plugin is the only unit

After T90 (done): `plugins/antigravity/` carries `plugin.json` + `mcp_config.json` and **no hooks** — creator decisions 2026-09-21: the plugin is the only install path (no direct edit of `~/.gemini/config/mcp_config.json`), and per https://antigravity.google/docs/hooks/ `PreToolUse` cannot rewrite tool input and `PostToolUse` cannot add context. New host `antigravity` in `src/agents/antigravity/` (`mod.rs` + `README.md` with the module table and `## Docs`), registered in `HOSTS` and `host()`. Variants: CLI (`agy` on PATH) and Desktop (`Antigravity.app`), same files. `support`: `plugin` → `Flag("--yes")`; `mcp` → through the plugin only; `hooks` → `No` with the reason from T90; `proxy` → `No` (no documented base-URL override). `apply` is `HostPlugin::offer` — `plugins/antigravity` linked to `<plugins_path>/rtok` (default `~/.gemini/config/plugins/rtok`) — plus `skill::sync` of the hub skill into Antigravity's documented user skill root. Missing `rtok`: the offer names `ketch install listepo/rtok`.

Plan:
1. `src/agents/antigravity/mod.rs` on the `pi` pattern (one `HostPlugin`, `installed()` reads `PLUGIN.ours`), `[setup.antigravity] plugins_path` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`; `skill::dest` / `label` arms for `antigravity`.
2. Unit tests: dry-run offer names `plugins/antigravity` and the ketch line and writes nothing; `--yes` links, a second apply is `NO_CHANGES`, remove takes back exactly ours; a foreign directory at the dest is left alone and not reported as installed.
3. `docs/agents.md` via `RTOK_BLESS=1` on `tests/agents_doc.rs`; `tests/trycmd/agents-list*.toml` re-blessed; `research.md` host sentence ("Antigravity on request" → listed).
Verify first: (a) whether Antigravity's plugin loader follows a symlinked plugin directory — the docs do not say; if it does not, the offer prints the exact `agy plugin install <resolved plugins/antigravity path>` line instead of linking (the Kimi rule from T86) and `installed()` reads `<plugins_path>/rtok/plugin.json`; (b) resolved 2026-09-21 from https://antigravity.google/docs/skills: the global skill root differs per surface — `~/.gemini/config/skills/<name>/` for Antigravity 2.0 and IDE, `~/.gemini/antigravity-cli/skills/<name>/` for the CLI — and the CLI also loads plugin-provided skills from `plugins/<name>/skills/`; so `skill::sync` needs two dests for this host (or one, if plugin-provided skills turn out to load on desktop too — then the hub skill is copied into the installed plugin instead, never duplicated in `plugins/antigravity/`). `agy` and the desktop app are not installed on this machine (`~/.gemini/config/` exists), so the creator runs both checks or installs `agy`.
Over the ≤200 LOC / ≤3 files limit as written — split into T91.1 (host + plugin offer + config) and T91.2 (skill root + docs bless) when claiming.

Check: the unit tests above; `rtok agents list` shows `antigravity`; `agents_doc`, `host_docs`, `config_coverage` green; `just check`.

### T94. `rtok hook <event> --host cline` speaks Cline's file-hook JSON both ways

Creator request 2026-09-21: a host plugin for Cline CLI + desktop (the VS Code / JetBrains extension), like Claude's and Cursor's. Creator decisions 2026-09-21: (1) hooks go through Cline's **file hooks**, not a TS Cline plugin — Cline plugins (`~/.cline/plugins`, `cline plugin install`) load in the SDK, CLI and Kanban only ("not applicable on VSCode and JetBrains Extension for now", https://docs.cline.bot/customization/plugins), while core adapts file hooks onto the same runtime hook layer, so they are the one hook path both surfaces have (D21); (2) MCP is written directly into `cline_mcp_settings.json` (T96), not shipped as an agent-plugins.org package. A file hook is an executable named after its event. stdin is JSON: `hookName` (`tool_call` / `tool_result` / `agent_start` …), `taskId`, `workspaceRoots`, and `tool_call: {id, name, input}` or `tool_result: {id, name, input, output, error, durationMs}`. stdout is JSON: `cancel`, `review`, `context` (injected into the next turn), `errorMessage`, `overrideInput` (replaces the tool input; PreToolUse only); `{}` means do nothing. Neither direction is Claude-shaped, so unlike Devin (T87) the adapter translates both ways. The shell tool is `run_commands` with `input.commands: string[]`. Evidence (cline/cline `main`, fetched 2026-09-21): `sdk/examples/hooks/README.md`, `sdk/packages/shared/src/hooks/contracts.ts` (`HookControl`), `sdk/packages/shared/src/agent.ts` (`AgentBeforeToolResult.input` / `appendContext`), `sdk/packages/shared/src/storage/paths.ts`.

Plan:
1. `src/hooks/types.rs` — `HookInput::adapt_cline(event)` beside `adapt_cursor` / `adapt_copilot` / `adapt_devin`: `tool_call` / `tool_result` lifted to `tool_name` / `tool_input` / `tool_response`; `run_commands` → `Bash` and `read_files` → `Read` in `canonical_tool_name`; a one-entry `commands` becomes `command` (a multi-entry call passes through untouched — no rewrite, no measurement); `taskId` → `session_id`; `cwd` from `workspaceRoots[0]` when stdin has none.
2. `src/hooks/mod.rs` — a `cline` arm in `dispatch_owned_strict` plus the way out: `updatedInput.command` → `overrideInput: {commands: [..]}`, `additionalContext` → `context`, a deny → `cancel: true` + `errorMessage`, nothing to say → `{}`. Fail open prints `{}` and exits 0.
3. Unit tests with the README's payloads verbatim (PreToolUse `tool_call`, PostToolUse `tool_result`, a lifecycle event).
Blocked on T87 landing: its uncommitted work sits on the same lines this task edits (the `devin` arm in `dispatch_owned_strict`, the `--host` list in `config/default.toml`, `docs/config.md`, `src/cli.rs` and the trycmd snapshots), and hunks on one line cannot be staged apart in the shared checkout.
Verify first (2026-09-21: no `cline` binary on this machine, only `~/.cline/` data — the creator installs `npm i -g cline` or runs the capture): (a) real payloads from the CLI through a `tee` hook — the event file names Cline fires (`PreToolUse`, `PostToolUse`, `TaskStart`, `UserPromptSubmit`, `PreCompact`, …) and the `run_commands` input shape; (b) the VS Code extension — its hooks live in `.clinerules/hooks/<Event>` / `~/Documents/Cline/Hooks/` with no file extension behind an "Enable Hooks" setting; confirm it sends the same JSON and honours `overrideInput`. If it does not, the extension gets `context` only and the README says so.

Check: the unit tests above; `rtok hook PreToolUse --host cline` on a captured `run_commands` payload prints `overrideInput` whose command starts with `rtok run`; garbage stdin prints `{}` and exits 0; the `--host` list grows by `cline` (`config/default.toml`, `docs/config.md`, the `src/cli.rs` help line and its `tests/trycmd` snapshots); `just check`.

### T95. Cline plugin tree (`plugins/cline/`)

After T94. Cline has no bundle format that reaches both surfaces, so the tree is what the installer places: one POSIX script `plugins/cline/hooks/rtok-hook` that takes its event from its own file name (`${0##*/}`, extension stripped) and runs `rtok hook <event> --host cline`; with `rtok` missing it prints `{}`, exits 0 and says on stderr to install with ketch (`ketch install listepo/rtok`). One script, no per-event copies (T96 links it once per event).

Plan:
1. `plugins/cline/hooks/rtok-hook` (executable) and, if T94's capture shows Windows needs it, a `rtok-hook.ps1` twin.
2. `plugins/cline/README.md` — install by hand (link or copy into `~/Documents/Cline/Hooks/<Event>`, enable hooks in the extension, `cline mcp` / settings file for MCP), files, what the extension does and does not honour, `## Docs` (hooks, plugins, MCP, CLI reference, config pages).
3. A `tests/` check: the script is executable, fails open without `rtok` on `PATH` (`{}`, exit 0), and every event name the installer links is one `adapt_cline` knows.
Verify first: Cline runs a hook that is a symlink (its loader `realpath`s plugin paths; hook discovery is not documented) — if not, T96 copies instead of linking and `installed()` compares bytes.

Check: `host_docs` and the new test green; `just check`.

### T96. `rtok agents install cline` — CLI and the VS Code extension, one hooks directory

After T95. New host `cline` in `src/agents/cline/` (`mod.rs` + `README.md` with the module table and `## Docs`), registered in `HOSTS` and `host()`. Variants: CLI (`cline` on PATH) and the VS Code extension (`saoudrizwan.claude-dev`). D21 singleton for hooks: the CLI scans `~/Documents/Cline/Hooks` **and** `~/.cline/hooks`, the extension scans `~/Documents/Cline/Hooks` — so rtok installs into `~/Documents/Cline/Hooks` only and one directory serves both surfaces with no event firing twice. MCP is per surface, same `mcpServers.{command,args}` entry through `rtok_agent_sdk::register_mcp`: `~/.cline/data/settings/cline_mcp_settings.json` (CLI; `CLINE_DIR` / `CLINE_MCP_SETTINGS_PATH` are path overrides) and `<VS Code globalStorage>/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` (reuse the `vscode` host's per-OS user-dir resolution). `support`: `hooks` → yes, `mcp` → yes, `plugin` → `Flag("--yes")`, `proxy` → `No` (the Anthropic base URL is extension state, not a file rtok may edit). A foreign file at `<hooks_path>/<Event>` is left alone and named in the output — Cline has one slot per event per directory.

Plan:
1. `src/agents/cline/mod.rs`: one `HostPlugin` per event linking `plugins/cline/hooks/rtok-hook`; `[setup.cline] hooks_path`, `mcp_path` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`.
2. Unit tests: dry-run names `plugins/cline` and the ketch line and writes nothing; `--yes` links every event and writes `mcpServers.rtok`, a second apply is `NO_CHANGES`, remove takes back exactly ours and keeps foreign servers and foreign hook files.
3. `docs/agents.md` via `RTOK_BLESS=1` on `tests/agents_doc.rs`; `tests/agents_install.rs` `hosts()`; `tests/trycmd/agents-list*.toml` re-blessed; README / site host lists.
Over the ≤200 LOC / ≤3 files limit as written — split into T96.1 (host + hooks + config) and T96.2 (MCP for both variants + docs bless) when claiming.

Check: the unit tests above; `rtok agents list` shows `cline`; `agents_doc`, `host_docs`, `config_coverage` green; `just check`.

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

### T100. `rtok agents install grok` and the Read path under Grok

After T99. A `grok` host in `src/agents/`: detection (`~/.grok/bin/grok`, `GROK_HOME`), `support("plugin")` prints `grok plugin install <resolved plugins/grok> --trust`, `installed()` reports `plugin` from `grok plugin list --json` or `~/.grok/plugins/rtok`; MCP as `[mcp_servers.rtok]` in `~/.grok/config.toml` via `toml_edit` when the plugin is absent. D21 singleton: while rtok's Claude hooks are installed and `[compat.claude] hooks` is on, say so instead of adding a second set. Separately, on a live Grok: dump one `read_file` PreToolUse payload, and if its `toolInput` matches what the Read plugins rewrite, map `read_file` → `Read` and add the Read matcher to the plugin. `docs/agents.md` blessed, `src/agents/grok/README.md` with `## Docs`.

Check: unit tests for offer, detection and singleton; `agents_doc` blessed; `just check`.

### T101. Hook fail-open matrix over every `--host`

`tests/extra_cover.rs` checks bad and empty stdin for the default host only. One `rstest` matrix in `tests/hook_fail_open.rs`: every value `[hook] host` accepts × every hook event × stdin {empty, garbage, truncated JSON, non-UTF-8, 1 MiB}. Done when each case exits 0, prints the host's no-op reply, and never rewrites the input.

Check: `just test` green; one case per host × event × stdin in the nextest list.

### T102. Lossless round-trip for every plugin that shortens

Rule: anything shortened is retrievable via `expand <id>`. Today each plugin checks its own path (`toon` in `tests/extra_cover.rs`, archive in `tests/archive_rewrite.rs`). One test walks every plugin that writes an archive row: shorten a fixture, take the id, `rtok expand <id>`, compare bytes. Fixtures include CRLF, non-UTF-8 and an empty body.

Check: the test fails if a new shortening plugin is added without a fixture; `just test` green.

### T105. Report renderers: edge-case snapshots

`src/report/markdown.rs` and `src/report/html.rs` have no unit tests; `tests/report.rs` covers one fixture and the empty store. `insta` snapshots on a fixed model: zero savings, one row, very large numbers, text with `<`, `|`, backticks and newlines. Done when no table breaks, no HTML is injected, and no `NaN` or `inf` is printed.

Check: snapshots committed; `just test` green.

### T106. `otel/export.rs` unit tests

`src/otel/export.rs` has no unit tests; `tests/otel.rs` covers the happy path against a mock collector. Add units for `resource()` attributes, an unreachable collector (error returned, no row marked, no panic) and ticker shutdown. Skip cases `tests/otel.rs` already pins.

Check: `just test` green; no new dependency.

### T107. CLI bad-argument fixtures

`tests/trycmd/` pins help and happy output. Add fixtures for a bad value or missing argument on each subcommand (exit 2, clap message) and for `parse_since` rejects (`--since 5x`, `--since -1d`, empty).

Check: one fixture per subcommand; `just test` green.

### T116. Copilot CLI plugin

Copilot CLI plugins bundle hooks and MCP (`plugin.json`); local install is `copilot plugin marketplace add <path>` (https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating). First verify whether Copilot reads the T114 Claude-format marketplace as is; if yes, reuse it (one tree), otherwise a `plugins/copilot` tree with `--host copilot` hook commands. Installer mirrors T115 through the `copilot` CLI; D21 singleton against `mcp-config.json` and `hooks/rtok.json`.

Check: fake `copilot` e2e like T115; `just check` green.

### T117. VS Code agent plugins

VS Code agent plugins carry hooks and MCP and are registered by path in the `chat.pluginLocations` setting (https://code.visualstudio.com/docs/agent-customization/agent-plugins). Verify the accepted format (Claude-format plugins?) and the hook event names first. `rtok agents install vscode --yes` adds the plugin path to `chat.pluginLocations` in the user `settings.json` (JSONC — see T79 before writing it) and strips its own MCP entry while the plugin is listed (D21).

Check: settings round-trip test (add, idempotent, remove keeps foreign entries); `just check` green.

### T118. Gemini CLI host with an extension

New host `gemini`. Gemini CLI extensions (`gemini-extension.json`, hooks in `hooks/hooks.json`, MCP servers in the manifest) install with `gemini extensions install <path>` / `link` (https://geminicli.com/docs/extensions/). Needs a hook adapter for Gemini's event names and I/O shape (`--host gemini`), `src/agents/gemini/` (`mod.rs` + `README.md` with `## Docs`), `plugins/gemini/`, registration in `HOSTS`, config keys, docs table bless. Split into sub-tasks when claimed.

Check: host matrix e2e with a fake `gemini`; hook adapter unit tests; `just check` green.

### T122. A dedup pointer reaches a context that never saw the body

Seen 2026-09-21 in a Claude Code session: a Haiku sub-agent read `research.md` and `ideas.md`; the parent's first MCP `read` of the same files (`mode=lines`, ranges `14-30` and `1230-1260`) answered `[rtok <id> · identical to a result 1 turns ago …]` for both ranges with one id, and `expand <id>` returned the whole file. Two defects: (1) `plugin::identical_result` (T65.1) keys on the host session, which sub-agents share with the parent, so the pointer names a body that is not in the caller's context and every such read costs a second `expand` round trip — a loss, recorded as a `dedup` saving; (2) the ranged read was hashed or archived as the whole file, so two different ranges are "identical". The same question holds after a `compact_boundary`: the earlier body is gone from context. Reproduce first with a test, then fix at the responsible layer: hash the bytes actually returned, and return a pointer only when the earlier result was delivered to the same context (the hook payload's agent/transcript id where the surface has one; when the surface cannot tell — MCP — a body under a size threshold is returned as is). Lossless rule unchanged.

Plan: failing test first in `src/plugins/read/mod.rs` tests (two ranges of one file → distinct results; body archived under one context, read from another → body). Fix in `plugin::identical_result` / `read::mod` hash of the returned bytes; context key from the hook payload where present, size threshold on MCP. `mise exec -- cargo nextest run read:: plugin::`.

Check: a test where session S archives body B under sub-agent context A, then context P reads B → P gets the body, not a pointer; two different ranges of one file never share an id; `just test` green; no `dedup` Measurement row on the returned-body path.

### T123. `rtok doctor` names `[proxy.tools_rewrite]` when it applies

`research.md` §2 (T59.5 row): 8,951 MCP description tokens × 40,402 turns = 6.2 % of session input on a host without Tool Search — the largest measured share with a shipped lever that is off by default. `doctor` already prints `mcp_tool_search likely disabled` and per-server `desc tokens` (`src/doctor.rs` `render`), and stops there. Add one advice line when all hold: Tool Search likely disabled, rtok's proxy is a hop in the Anthropic chain, `proxy.tools_rewrite.enabled = false`, and the summed description tokens are above a threshold (config key under `[doctor]`, default from the 3 % gate). The line names the total and the config key; per T59.7 it never says "saves N". Same field in the JSON report.

Plan: field + advice line in `src/doctor.rs` (`Report`, `render`), threshold key under `[doctor]` in `src/config/mod.rs` + `config/default.toml`; bless trycmd config fixtures; unit tests on `render` for each condition.

Check: unit tests on `Report::render` for the four conditions (line present only when all hold); `just test` green; ≤ 100 LOC.

### T124. Realized `tools_rewrite` saving as a dated `research.md` row

6.2 % (T59.5) is the ceiling, not a saving: no dated row shows what `[proxy.tools_rewrite]` removes with the default `max_description_tokens = 60`. Precondition, by the creator: turn it on for this machine's proxy for at least 20 sessions. Then sum the `kind = tools_rewrite` Measurement rows against session input for the same window (`rtok stats` / `rtok gain`, dated command in the row), and write one row into `research.md` §2 next to the T59.5 row; update `docs/comparison.md` only if it cites the number. If the realized share is under the 3 % gate, say so in the row and leave the default off.

Check: the row cites the command, date, sessions, before/after tokens and the share; no number in prose without it.

### T125. `rtok stats`: thinking-block share — the gate for I-86

I-86 (strip or pointer prior reasoning blocks on replay) has no number. First read the provider docs for what is already dropped server-side from earlier turns and cite it in the row. Then measure in `measure::stats` (same walk and unique-`message.id` rule as the other rows): bytes of `thinking` content blocks in assistant messages, per session and as a share of session input across the turns that re-send them. Gate 3 % of session input: above → promote I-86 to a task with an A/B Check; below → move I-86 to Rejected with the row as evidence.

Plan: count `thinking` blocks in `src/measure/stats.rs` (same unique-`message.id` walk), text + JSON line, fixture unit test; run `rtok stats --since 30d`, add the dated row to `research.md` §2, update I-86 in `ideas.md` by the 3 % gate.

Check: a `thinking` line in `rtok stats --since 30d` (text and JSON), a unit test on a fixture transcript, a dated row in `research.md` §2, and I-86 updated either way; ≤ 150 LOC.

