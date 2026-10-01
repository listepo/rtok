# rtok

https://github.com/pyrlyn/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T124 | todo | P3 | 2 | 0% | |
| T131 | todo | P2 | 3 | 70% | |
| T132 | todo | P2 | 2 | 70% | |
| T134 | todo | P1 | 2 | 40% | |
| T156 | in progress | P3 | 3 | 50% | Claude Code / claude-opus-5-5 |
| T159 | todo | P2 | 4 | 0% | |
| T262.3 | todo | P2 | 2 | 0% | |
| T261 | in progress | P2 | 3 | 95% | Cursor / grok 4.7 |
| T271 | todo | P1 | 2 | 40% | |
| T275 | in progress | P1 | 4 | 80% | Claude Code / claude-opus-5-5 |
| T275.1 | in progress | P2 | 3 | 80% | Cursor / grok 4.7 |
| T276 | in progress | P2 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T277 | in progress | P2 | 5 | 20% | Claude Code / claude-opus-5-5 |
| T278 | in progress | P1 | 3 | 90% | Claude Code / claude-opus-5-5 |
| T279 | in progress | P1 | 5 | 90% | Claude Code / claude-opus-5-5 |
| T279.1 | todo | P2 | 2 | 0% | |
| T281 | in progress | P1 | 3 | 70% | Claude Code / claude-opus-5-5 |
| T283 | in progress | P1 | 3 | 60% | Claude Code / claude-opus-5-5 |
| T284 | in progress | P1 | 3 | 50% | Claude Code / claude-opus-5-5 |
| T285 | in progress | P1 | 4 | 50% | Claude Code / claude-opus-5-5 |
| T286 | in progress | P1 | 3 | 40% | Claude Code / claude-opus-5-5 |
| T287 | in progress | P1 | 4 | 50% | Claude Code / claude-opus-5-5 |
| T288 | in progress | P2 | 3 | 40% | Claude Code / claude-opus-5-5 |
| T289 | in progress | P2 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T290 | todo | P1 | 3 | 0% | |
| T310 | todo | P1 | 5 | 0% | |
| T310.3 | todo | P1 | 3 | 0% | |
| T310.4 | todo | P1 | 3 | 0% | |
| T310.5 | todo | P1 | 3 | 0% | |
| T310.6 | todo | P1 | 3 | 0% | |
| T310.7 | todo | P2 | 3 | 0% | |
| T310.8 | todo | P2 | 3 | 0% | |
| T310.9 | todo | P1 | 4 | 0% | |
| T310.10 | todo | P1 | 3 | 0% | |
| T310.11 | todo | P2 | 3 | 0% | |
| T310.12 | todo | P2 | 3 | 0% | |
| T314 | in progress | P2 | 3 | 60% | Grok Bot |
| T315 | in progress | P2 | 3 | 60% | Grok Bot |
| T317 | todo | P2 | 4 | 0% | |



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

Execution (2026-09-27): (1) a probe kit in the session scratchpad (never committed), like T281, that logs `WorktreeCreate`/`WorktreeRemove` payloads and returns a `_worktrees/` path; the creator runs it with `claude --worktree`, a sub-agent `isolation: worktree` and the desktop app. The documented payload fields go into `research.md` §18.3 now, with sources. (2) Seeded (`cp -c -R target`) and cold worktrees of this repo: wall time of `just check` and physical disk delta (`df` before/after, not `du`). The cold run only happens with ≥ 30 GiB free; otherwise the row says so. Result: a dated row in `research.md` §18.4.

### T159. Claude Code `WorktreeCreate`/`WorktreeRemove` hooks route through `rtok worktree`

Depends on T156 (the real payloads), T158 (create) and T153 (remove). A skill is advice an agent may skip; the host's own worktree hooks are the only place where the rules cannot be skipped: `claude --worktree`, the desktop app and sub-agent `isolation: worktree` all create worktrees without asking the agent, which is where the `agent-<hex>` directories and reason-less locks come from (`research.md` §18.1, §18.3).

Plan: `rtok hook WorktreeCreate` maps the host's `name` to T158's rules and prints the created path; `rtok hook WorktreeRemove` applies T153's single-worktree rules to `worktree_path` — never forced: a dirty worktree, or one locked by another owner, is left in place and reported, and its tagged caches are cleaned (T152) either way. Installed by `rtok agents install claude` with the plugin, removed with it, singleton per D21; the host docs link for these events joins `plugins/claude/README.md` `## Docs`; regenerate the host table (`tests/agents_doc.rs`, `RTOK_BLESS=1`). **One decision to take before the Do, by the creator:** these hooks replace the host's default behaviour and must spawn git, so they cannot meet "exit 0 in ≤ 10 ms with unmodified input". Proposed reading: the 10 ms rule binds the per-tool-call hot path; `WorktreeCreate` fires once per worktree, and fail-open here means "on any rtok error, create the worktree exactly where the host would have (`<repo>/.claude/worktrees/<name>`) with plain git, print that path, exit 0" — the host never loses the ability to create a worktree because of rtok. Record the outcome as a decision row (D31 or the next free id) in this task's PR. Other hosts have no such hook today (§18.3); they keep the skill (T155).

Check: hook fixture tests with T156's recorded payloads — create returns a path under the T158 root with the owner lock; a simulated failure of `rtok worktree add` still yields a usable worktree at the host default path and exit 0; remove deletes a merged clean worktree, keeps a dirty one and a foreign-locked one with the reason on stderr, and cleans the tagged cache in all three; host matrix e2e — install adds both hooks exactly once and removal takes them away; `tests/host_docs.rs` and `tests/agents_doc.rs` green; `just check`.

Update (2026-09-27, D34): also depends on T285 and T286. `WorktreeCreate` calls T285's `add` bound to the session's rtok agent id (the hook payload carries `session_id`, so the agent resolves without T281) and prints the path; `WorktreeRemove` calls T286's `remove` rules for that agent. The rest of the plan stands.


### T262.3. Codex: spawn brief on `SubagentStart`

`research.md` §23: Codex fires `SubagentStart` and adds the hook's stdout (or its hook-specific context) to the subagent as developer context. Add `SubagentStart` to `plugins/codex/hooks/hooks.json` and the Codex installer's list, and make `rtok hook SubagentStart` answer in the shape Codex reads.

Blocked (found 2026-09-24 while claiming): the brief is built from `PreToolUse` rows whose `tool_name` is `Read|Edit|Write` (`ledger()` in `src/plugins/memory/handoff.rs`), and rtok installs no `PreToolUse` hook for Codex (only `PreCompact`/`PostCompact`), so a Codex brief would always be empty. Needs Codex `PreToolUse` wiring first (idea I-88), which the creator has not approved.

Check: a Codex `SubagentStart` payload through `rtok hook` returns the brief in Codex's shape (test); `just check` green.

### T261. CI takes ~9.5 min on macOS; the webui check recompiles 183 crates every run

Creator request 2026-09-24: find what makes CI and the tests slow, try fixes in a draft PR, do not merge. Measured on #326:
- `check (macos-latest)` is the critical path. Of its ~9.5 min: mise 53 s, cache restore 65 s, fmt 3 s, clippy 46 s, nextest's test-profile build 1 min 47 s, 1733 tests 79 s, then about 39 s of `build-min`, jscpd, oxlint and pytest.
- `just webui-check` takes 2 min 6 s on macOS and 1 min 33 s on ubuntu. `crates/rtok-webui` is excluded from the workspace, so its `target/` is not in `rust-cache`, and 183 crates compile from scratch on every run.
- 78 `tests/*.rs` files mean 78 test binaries to link.

Plan (a draft PR; each change measured with `workflow_dispatch` runs on the branch, warm cache):
1. `rust-cache` also caches `crates/rtok-webui/target`.
2. A `lint` job on ubuntu takes fmt, clippy, `build-min`, jscpd, JS, Python and `webui-check`. The `check` matrix keeps its name and runs only the tests and examples, so macOS stops paying for lint. Trade-off to report: clippy no longer runs on macOS-only `cfg` code.
3. `CARGO_PROFILE_DEV_DEBUG=0` in CI only (smaller test binaries, less linking).
4. Estimate folding `tests/*.rs` into one integration binary (78 links become 1); report it, do not do it here.

Check: warm-cache `workflow_dispatch` runs on the draft PR are green, and a PR comment gives before/after timings per job.

Progress (Cursor / grok 4.7): items 1–3 already on `origin/main` via #332 — do not redo. Draft PR https://github.com/pyrlyn/rtok/pull/411 (`t261-ci-timings`). Two `workflow_dispatch` runs posted timings in a PR comment; both runs **failed** (Check not met — card stays open).

**Blocker (exact errors from the logs):**
1. `lint` — `just … dup` / jscpd: `ERROR: jscpd found too many duplicates (2.1%) over threshold (2.0%)` (`.jscpd.json` `threshold: 2`; 198 clones). Same on both runs. `webui-check` / `publish-dry` and lint’s rust-cache save were skipped because of this.
2. `windows` — `cargo nextest` exit 1. Failures (warm run [36202660499](https://github.com/pyrlyn/rtok/actions/runs/36202660499)): `agents::devin::tests::plugin_manifest_matches_the_installer`; `agents_install::{dry_run_setup_creates_nothing_and_copies_nothing, setup_twice_takes_one_backup_and_says_already_installed, remove_twice_says_no_changes_and_the_second_takes_no_backup, list_reports_installed_modules_per_host}` (5 failed; cold run [36201850046](https://github.com/pyrlyn/rtok/actions/runs/36201850046) also failed `plugins::checkpoint::tests::session_end_on_a_large_transcript_is_bounded`).

`check (ubuntu-latest)` and `check (macos-latest)` were green both times; cold→warm job wall: ubuntu 161 s → 146 s, macOS 362 s → 222 s (see PR #411 comment).

Fold estimate (not folding): **82** `tests/*.rs` bins. macOS log `Finished test profile`: cold **2 m 05 s**, warm **1 m 16 s**. No per-binary `Linking` lines in the CI log. Fold not in this PR (82-file mechanical move; own task).

### T271. The Claude desktop Code tab sees rtok's MCP twice while the plugin is installed

Observed by the creator on 2026-09-26: rtok 0.8.0; Claude Code 2.1.267 in the Claude desktop app's Code tab; `rtok@rtok` installed. Every session sees rtok's 14 MCP tools twice, and so does every subagent it spawns:

- `mcp__rtok__*` comes from `mcpServers.rtok` → `/Users/<user>/.ketch/bin/rtok mcp` in `~/Library/Application Support/Claude/claude_desktop_config.json`.
- `mcp__plugin_rtok_rtok__*` comes from the plugin's `scripts/mcp.sh`, which execs the same binary.

The cost is two `rtok mcp` processes and 28 tool schemas instead of 14 in every agent's context. It breaks D21 (one call path per capability, a singleton), and with it D18 and measurement.

Why T243/T244 did not catch it:
- **Write side.** `Claude::apply` for `Kind::Desktop` already drops that entry while `code_serves_mcp` is true. `rtok agents install claude --desktop --dry-run` plans `- mcpServers.rtok` on this machine now. The removal only runs when `agents install`/`update` runs for the desktop variant after the plugin exists. An entry written before the plugin was installed, or left when the plugin came through `/plugin install` inside Claude Code, stays until then. (Within one `agents install claude` the CLI variant runs first, so a fresh install is not affected.)
- **Read side.** The read side hides it. `Claude::installed(Kind::Desktop)` returns `mcp` when the file has an `"rtok"` entry *or* Code serves MCP. So `rtok agents info claude` and `rtok doctor` print `✓ mcp installed` for Claude Desktop and never say the server is there twice.

Done means:
1. **Detect.** `rtok doctor` and `rtok agents info|list` report a desktop `mcpServers.rtok` entry while `code_serves_mcp` is true as a warning: rtok's MCP is served twice in the desktop Code tab. The warning names the fix, `rtok agents install claude --desktop`. The `✓` line no longer covers that state.
2. **Sweep.** Every rtok path that leaves Code serving rtok's MCP also runs the desktop removal: the plugin step of `agents install claude`, `agents update`, and `doctor --fix` if it exists. That removal is `unregister_mcp_ours`, with T246's ownership check, so an entry the user edited is left and reported. Decide in this task whether the plugin's own `SessionStart` may do the sweep too. It fires once per session, but it must stay fail-open, and the 10 ms rule applies. Record the outcome as a decision row if it is yes.
3. **Tests (Vfs).** A desktop entry plus the installed plugin → doctor warns and `agents info` shows the warning. The plugin step alone removes an unchanged desktop entry. An edited entry is kept with a `leave` line.

Check: the tests above pass; on the creator's machine the Code tab lists one set of rtok tools after the fix path runs; `just check`.

### T275. Install/update removes rtok's MCP entry from an agent's config while a plugin serves MCP (every host)

Observed by the creator on 2026-09-27: rtok 0.10.0, Claude Code 2.1.267, `rtok@rtok` plugin installed. After `rtok agents update claude` the plugin (commit `12c7e91`) and its hooks are current, but rtok's MCP server is missing from Claude Desktop's MCP settings. It must be available in both Claude Desktop and Claude Code.

Evidence on that machine:
- `~/Library/Application Support/Claude/claude_desktop_config.json` has no `mcpServers` key. The backup written by the same run (`_backup/claude_desktop_config.json.bak-1790511161`, 15:12) still holds `mcpServers.rtok` = `/Users/<user>/.ketch/bin/rtok mcp`, so the update removed it.
- Claude Code gets rtok's MCP only from the plugin (`.mcp.json` runs `scripts/mcp.sh`, which runs `rtok mcp`). `~/.claude.json` and `~/.claude/.mcp.json` have no `mcpServers.rtok`.
- `rtok agents info claude` still prints `✓ mcp installed` for Claude Desktop.

Cause (code on `main`, `12c7e91`). The D21 singleton rule ("while a plugin serves MCP, the config-file entry goes") is applied on every install and update, and the Claude Desktop case extends it to a different app:
- `src/agents/claude/mod.rs:724-737`, `Claude::apply` for `Kind::Desktop`: when `code_serves_mcp(cfg)` is true it calls `unregister_mcp_ours(cfg, &desktop_path(), "rtok")` instead of `register_mcp`. `Mode::Update` takes the same branch as install.
- `src/agents/claude/mod.rs:422-426`, `code_serves_mcp` is true whenever the Claude Code plugin is installed, so the desktop entry is always removed.
- T243/T244 assumed the desktop file only feeds the desktop app's Code tab. It is also the only place Claude Desktop's own chat reads MCP servers from, so removing it takes rtok out of Claude Desktop entirely.
- The read side hides it. `Claude::installed(Kind::Desktop)` (`mod.rs:688-694`) reports `mcp` when the file has `"rtok"` *or* `code_serves_mcp`. `Claude::installed(Kind::Cli)` (`mod.rs:700-706`) and most hosts below count `mcp` as installed when the plugin is present.

Hosts with the same pattern (install/update removes, or never writes, the agent-config MCP entry while a plugin serves MCP):
- Claude Desktop: `claude/mod.rs:724-737`, removes `mcpServers.rtok` from `claude_desktop_config.json` while the Claude Code plugin is installed.
- Claude Code: `claude/mod.rs:764-767`, removes `mcpServers.rtok` from `~/.claude.json` while the plugin is installed.
- Cursor: `cursor/mod.rs:113-117` with `plugin_is_mcp` (`:246`), never writes `~/.cursor/mcp.json` while the plugin is linked.
- Copilot CLI and Gemini CLI: the shared `d21_plugin_apply` (`agents/mod.rs`, the `if remove || plugin_installed(cfg)` branch), removes `mcpServers.rtok` from `~/.copilot/mcp-config.json` and from Gemini's settings.
- Codex: `codex/mod.rs:334-337`, `run(cfg, true)` removes `[mcp_servers.rtok]` from `~/.codex/config.toml` while the plugin is enabled.
- VS Code and VS Code Insiders: `vscode/mod.rs:140-148`, removes `servers.rtok` from `mcp.json` while the plugin is linked (T196).
- ZCode: `zcode/mod.rs:257-260`, removes `mcp.servers.rtok` while `plugin_serves`.
- Kimi: `kimi/mod.rs:127-130` (`plugin_detected`), removes rtok's tables.
- Grok Build: `grok/mod.rs:97-101` (`plugin_detected`), removes `[mcp_servers.rtok]`.
- Not affected (MCP is written on install/update regardless of a plugin): Kilo, OpenCode, Cline, Devin, Windsurf, Zed, MiMo, CodeWhale. Check omp, pi and Antigravity (they link a plugin; verify whether their plugin serves MCP and whether their config entry is skipped) and add them here if they match.

Decision (record as a decision row; it amends D21 for MCP and replaces T271's "sweep" item and the MCP half of T243/T244):
1. Install and update always write rtok's MCP entry into each agent's own config. Only `remove` takes it out. A plugin being installed no longer suppresses or strips it.
2. Duplicates with the same name are left to the agent to merge (Claude Code merges same-name servers across scopes). All config-file entries keep the name `rtok`.
3. Where two entries could conflict by name on one surface (the agent errors or refuses on a duplicate name instead of merging), use a different name per surface, recorded in the host's README row.
4. Hooks keep the D21 singleton rule; this task changes MCP only.

Architecture principle (applies to every step below and to T275.1):
- One shared MCP core for all hosts: install, update, remove, status (`installed`), the `doctor` check and `ping` are one implementation in `src/agents/mod.rs` (or a new `src/agents/mcp.rs`), parameterised only by data: the host's config file path(s), the JSON/TOML key path (`mcpServers`, `servers`, `context_servers`, `mcp.servers`, `[mcp_servers]`), the server name(s) per surface, the command form (bare `rtok` or absolute path), and the entry shape (`type`, `args`, `env`).
- Host-specific differences live in one table, not in code branches: a per-host `McpSpec` entry (for example a `const` slice or a method on `Agent` that returns data) with those fields plus how the agent treats duplicate names (merges same-name entries, overrides by scope, errors, or shows two servers), whether a plugin also serves MCP and under what name, and the headless ping command for T275.1. `docs/agents.md` is generated from that table.
- No per-host `if plugin …` / `if kind == Desktop …` branches for MCP. The host `apply` / `installed` code calls the shared core with its spec. A new host is added by adding a table row and a Vfs test row, not new MCP logic. The per-host MCP functions this replaces (`register_mcp`, `unregister_mcp`, `plugin_is_mcp`, `code_serves_mcp`, the MCP half of `d21_plugin_apply`) are deleted once every host uses the core.
- Tests are table-driven too: one Vfs test iterates every row and runs install, update, remove, status and ping against it.

Fix:
1. For each host above, verify against the agent's docs and one real run how it treats two MCP servers named `rtok` from different sources (merge, override by scope, error, or two servers), and whether a plugin-served server is namespaced (Claude: `plugin_rtok_rtok`, so it never merges with `rtok`). Record per host in `research.md` with the agent version and a docs link. This decides where rule 3 applies.
2. Write side: change each listed `apply` so that install and update call `register_mcp` whenever `cfg.setup.mcp` is set, and `unregister_mcp` only on `Mode::Remove`. For the Claude Desktop case, drop the `code_serves_mcp` branch in `claude/mod.rs:724-737`. For Copilot and Gemini, change `d21_plugin_apply` once. Keep T246's ownership check, so a user-edited entry is left and reported.
3. Name conflicts: where step 1 shows a surface sees both a plugin server and the config entry as two servers (for example the Claude desktop Code tab, T271), give that pair distinct names or drop the plugin's MCP server (keeping its hooks) so the agent merges one `rtok`. Pick per host from step 1 and note it in the decision row.
4. Honest status: `installed()` reports `mcp` only when the agent's own config file really holds rtok's entry. Remove the `|| code_serves_mcp(cfg)` fallback in `claude/mod.rs:688-694` and the "plugin implies mcp" shortcut in the other hosts; show a plugin-served MCP as its own line (for example `✓ mcp (plugin)`) so it never stands in for the config entry.
5. `doctor`: warn per host when the agent is installed but its config has no rtok MCP entry, naming `rtok agents install <host>`. Keep T171's duplicate check only where step 1 shows the agent does not merge.
6. Tests (Vfs, no host disk, D29), for every listed host: (a) plugin installed plus `install`, then the config has rtok's MCP entry with the right command and args; (b) the same after `update`; (c) `remove` takes it out; (d) config without an entry plus the plugin, then `installed()` does not report `mcp` from the config and `doctor` warns; (e) a user-edited entry is kept with a `leave` line; (f) the T244 surface count is updated to the new rule: at most one merged rtok server per surface, or distinct names where step 3 applies. Re-bless `docs/agents.md` and each host's README MCP row with `RTOK_BLESS=1`.

Check: the tests above pass; on the creator's machine `rtok agents update claude` leaves `mcpServers.rtok` in `claude_desktop_config.json`, Claude Desktop's MCP settings list rtok, the Code tab and Claude Code list one rtok server, `rtok agents info|list` reports every host truthfully, and a Cursor/Codex/Copilot update keeps their config entries; `just check`.

Execution plan (lands before T277; T277 then moves the core into its crate):
1. Worktree `_worktrees/rtok-T275`, branch `t275-mcp-entry-always`. Research (Fix 1) per host into `research.md`, with version and docs link.
2. PR A, Claude Code and Desktop: shared MCP core in `src/agents/mcp.rs` (write/remove/status by spec), decision row amending D21, drop `code_serves_mcp` in `apply` and `installed`, Vfs tests (a)-(f) for Claude.
3. PRs B-D, the other affected hosts in groups of at most 10 files: Copilot and Gemini (`d21_plugin_apply`); Cursor, Codex, VS Code; ZCode, Kimi, Grok. Each deletes that host's MCP branch and adds its table rows to the shared Vfs test.
4. PR E: `doctor` warning (Fix 5), re-bless `docs/agents.md` and README MCP rows, then the creator-machine Check.
Progress (2026-09-28): PR A and B-D merged for Claude, Copilot, Gemini, Codex, Cursor (#444), VS Code (#455), ZCode (#459), Kimi (#467); Grok is #468. Left: PR E.

### T275.1. `rtok mcp ping <agent>`: prove the agent's rtok MCP server is alive and answering

`installed` only says a config entry exists (and today not even that, see T275). This command checks the real path: the agent starts rtok's MCP server from its own config, calls a tool, and writes the answer into its chat.

Syntax: `rtok mcp ping <agent> [--cli|--desktop] [--timeout <secs>] [--json]`. `<agent>` takes the same host names as `rtok agents install|info` (`claude`, `cursor`, `codex`, `copilot`, `gemini`, `vscode`, …); `--cli` / `--desktop` picks the variant like `agents install`; no agent means every host `agents list` shows with MCP installed. Place it in `src/cli.rs` as a `ping` subcommand of `rtok mcp`, next to `--call`. The foreign-server wrap stays behind `--` (`rtok mcp -- <argv>`), so `rtok mcp ping …` must not be parsed as a wrap argv: add a clap test for both forms. Add a matching line to `rtok agents info` help that points at it.

What it sends:
1. A new MCP tool `ping` in rtok's own server (`src/mcp`), taking `{"agent": "<display name>"}` and returning exactly `MCP <display name> жив`, where the display name is the host name `agents list` prints (for example `MCP Claude Desktop жив`, `MCP Claude Code жив`, `MCP Cursor CLI жив`). The call is logged like any MCP call, with no measurement row.
2. For hosts with a headless CLI, rtok runs the agent once with one prompt: `Call the rtok MCP tool "ping" with agent="<display name>" and reply with its result only, nothing else.` For example `claude -p …`, `codex exec …`, `cursor-agent -p …`, `copilot -p …`, `gemini -p …`. The exact non-interactive flag per host is verified against its docs and recorded in the host README.
3. For desktop-only hosts (Claude Desktop, VS Code chat, Windsurf, Zed …) there is no documented way to send a prompt from outside. rtok prints the same prompt for the user to paste into the app's chat, then does the server-side part itself: it spawns the MCP command exactly as written in that host's config (`command` / `args` / `env`), runs `initialize`, `tools/list` and `tools/call ping`, and checks the reply. That proves the configured server starts and answers, but not that the app loaded it; the output says which of the two was checked.

Success: the agent's chat reply (captured stdout for headless hosts) is exactly `MCP <display name> жив`, after trimming whitespace. Anything else is a failure: no rtok entry in the host's config, the server fails to start, the `ping` tool is missing, a timeout (default 60 s), or a different reply. Each failure prints its reason and the fix (for a missing entry, `rtok agents install <agent>`). Exit code 0 only when every checked host succeeds. `--json` prints one object per host: `agent`, `variant`, `mode` (`chat` or `server-only`), `reply`, `ok`, `reason`.

Tests: unit test for the `ping` tool reply text; clap tests for `rtok mcp ping` vs `rtok mcp -- …`; a Vfs test that the server-only check reads `command` / `args` from each host's config (rtok's entry, not a hardcoded path); an e2e that spawns `rtok mcp` through a fake host config and gets `MCP Test жив`; headless hosts are covered by a fake agent binary on PATH that runs the MCP call and echoes the result.

Check: on the creator's machine `rtok mcp ping claude --cli` prints `MCP Claude Code жив` from `claude -p`, `rtok mcp ping claude --desktop` checks the server from `claude_desktop_config.json` and prints the prompt, and after T275 both succeed; `just check`.

Execution plan:
1. `src/mcp.rs`: tool `ping`, always listed, reply `MCP <agent> жив`, logged through the existing call row (no measurement).
2. `src/cli.rs`: `ping` subcommand of `rtok mcp` beside `--call`; wrap stays behind `--`. `agents info` long help points at `rtok mcp ping`.
3. `src/mcp/ping.rs`: headless rows for claude (`-p`), codex (`exec`), cursor (`cursor-agent -p`), copilot (`-p`), gemini (`-p`), each recorded in that host README with the docs link. Every other variant is server-only: read `command` / `args` / `env` from `Agent::files`, spawn through `doctor`'s MCP round-trip (`initialize`, `tools/list`, `tools/call ping`).
4. Tests: ping reply text; clap `mcp ping` vs `mcp --`; Vfs bodies for every host's MCP shape; e2e spawn of `rtok mcp` from a fake config (`MCP Test жив`); fake `claude` on PATH.
5. Verify with `just check`. The creator-machine `claude --cli` / `--desktop` run stays open.

### T276. Spinner audit: every place rtok runs an external command and the user waits for its output

Goal: whenever rtok starts an external process and a person at a terminal waits for its result, a spinner is on screen from the moment the process starts until its output arrives, with a message that says what is happening (`installing plugin via claude…`, `probing MCP server context7…`, `creating worktree…`). Use the existing helpers in `src/render.rs` (`loader`, `spinner`, both silent when stderr is not a TTY) and `with_loader` in `src/cli.rs`; no new spinner code.

Audit (first pass, `rg 'Command::new|tokio::process'` on origin/main `12c7e91`): 39 call sites in 16 production files (about 10 more are in tests and do not count). No `tokio::process` use. Step 1 of the fix confirms this list.

Already covered (spinner shown today):
- `rtok agents list` / `rtok agents info`: `app_version` (`agents/mod.rs:297`, `<bin> --version`) runs inside `with_loader("listing hosts" / "reading host")`.
- `rtok agents install/update/remove` when stdin is not a TTY: the 10 calls in `agents/restart.rs` (`tasklist`, `pgrep`, `osascript` x2, `taskkill`, `killall`, `open`, `cmd`, direct spawn, `xdg-open`) and the host plugin CLI calls through `run_cli` / `spawn_cli` (`agents/mod.rs:248,252`) run inside `with_loader("updating host")`.
- `rtok graph index`: LSP servers spawned in `graph/lsp.rs:202` run under `render::spinner("indexing")`.

Missing, output awaited, spinner to add:
1. `rtok agents install/update/remove` in an interactive terminal (`apply_hosts`, `cli.rs`): T81 turned the loader off entirely so it never draws over the plugin question. Instead, show a loader per step (`installing plugin via <bin>…`, `closing <app>…`, `reopening <app>…`) and stop it before any prompt and restart it after the answer. Covers the same `restart.rs` and `run_cli` sites.
2. `rtok doctor` MCP probe (`doctor.rs:924-938`, `spawn_mcp` / `mcp_command`): starts each configured MCP server (often `npx` / `uvx`, several seconds cold) and waits for `tools/list`, with nothing on screen. Add `probing MCP server <name>…` per server.
3. `rtok worktree add/gc/clean` (`worktree/git.rs:11`, the shared `git` helper): `git worktree add` and removal can take seconds on a large repo. Add a loader around the long `git` calls (`creating worktree…`, `removing worktree <id>…`); keep quick reads (`rev-parse`, `worktree list`) without one.
4. `rtok bench` (`bench.rs:268`, `bench.rs:378`, `claude -p`): each run takes tens of seconds to minutes and prints nothing until it ends. Add a loader with the arm, the prompt number and elapsed time (`bench mcp 3/10…`).
5. `rtok run <command>` (`plugins/cmd/run.rs:150`, `shell_command`): if the output is captured and printed only when the command ends, add `running <command>…`; if it streams live, add nothing (a spinner would interleave with it). Decide from the code in step 1.

No spinner needed (checked, with the reason):
- No person waits, or no terminal: `build.rs:12` (build time), `demon.rs:182,518` (supervised children), `bin/rtok-hook.rs:85` and `hooks/mod.rs:467` (hooks run by the agent), `otel/export.rs:496` (detached flush), `mcp/wrap.rs:44` (stdio proxy), `graph/watch.rs:720,740,759` (background watcher).
- The child owns the terminal: `log.rs:196` (`tspin` viewer), `demon.rs:315,317,326` (`rtok demon upgrade` hands the terminal to `ketch` / `rtok-update`, which print their own progress).
- Returns in milliseconds: `demon.rs:603` (`sysctl`), `bench.rs:88` (`sh` probe), `graph/lsp.rs:21,32` (`on_path`, `rustup which`), `graph/mod.rs:755` (`git diff --name-only`).

Architecture: one `ProgressRunner` for every external command (replaces adding spinners site by site):
- Where it lives: a new module `src/proc/` (`mod.rs` for the runner, `indicator.rs`, `parse.rs`). It is the only place in rtok that calls `std::process::Command::new`; `clippy.toml` gets `disallowed-methods = ["std::process::Command::new"]` with an `#[allow]` only inside `src/proc/`, so a new call site that bypasses the runner fails `just check`. The Windows shim logic now in `spawn_cli` (`agents/mod.rs:246`) and `mcp_command` (`doctor.rs:932`) moves into the runner too.
- Interface:
  - `ProgressRunner::new(label: &str, program, args)` returns a builder with `.cwd()`, `.env()`, `.stdin()`, `.timeout()`, `.progress(Progress)`, `.output(Output)`, then `.run() -> Result<Captured>` or `.spawn() -> Result<Running>`.
  - `enum Output { Capture, Stream, Inherit, Detached }`. `Capture` keeps stdout/stderr and shows an indicator until exit; `Stream` forwards lines live and suspends the indicator around each write; `Inherit` hands the terminal to the child (`tspin`, `ketch` upgrade) with no indicator; `Detached` is for background children (demon, otel flush, deferred hook) with no indicator.
  - `enum Progress { Unknown, Parse(Box<dyn ProgressParser>), Callback }`. `Unknown` draws a spinner. The other two draw a progress bar as soon as the first measurable update arrives and fall back to the spinner until then.
  - `trait ProgressParser: Send { fn feed(&mut self, stream: Stream, line: &str) -> Option<Update>; }` with `struct Update { pos: u64, total: Option<u64>, unit: Unit, msg: Option<String> }` and `enum Unit { Bytes, Percent, Items }`. Bytes render as MB with speed and ETA, percent as a 0-100 bar, items as `pos/total`.
  - `trait Indicator { fn set(&self, u: &Update); fn message(&self, m: &str); fn suspend<R>(&self, f: impl FnOnce() -> R) -> R; fn finish(&self); }` with two implementations: `TtyIndicator` (indicatif, on stderr) and `Hidden`. The runner picks `Hidden` when stderr is not a TTY, under `--json`, inside MCP and hook contexts, and when `RTOK_NO_PROGRESS=1`, so their output stays byte-identical.
- How a process reports progress:
  - Parsing: the runner reads stderr and stdout line by line (splitting on `\r` too, which is how `git`, `curl` and `cargo` redraw) and passes each line to the parser. Built-in parsers in `parse.rs` are `GitProgress` (`Receiving objects: 45% (120/267)`, `Updating files`), `Percent` (any `NN%`), `CargoProgress` (`Building [==> ] 120/300`), and `JsonLines` (a line `{"progress":{"pos":..,"total":..,"unit":".."}}`, for rtok's own child processes and plugin CLIs). The runner adds the flag that turns progress on where the tool has one (`git --progress`).
  - Callback: for work rtok measures itself, `ProgressRunner::batch(label, total, Unit::Items)` returns a `Batch` whose `.step(msg)` moves the bar; each command inside the batch runs with its own spinner line under the bar. `rtok bench` (runs), `rtok doctor` (servers), `rtok worktree gc/clean` (worktrees) and `rtok agents install/update/remove` over several hosts use this.
  - Prompts: `proc::suspend(|| ask(...))` hides every live indicator while rtok waits for an answer and redraws it after. This replaces the T81 rule that turned the loader off for the whole interactive run.
- Result: a new host, plugin CLI or subcommand that runs a program through `ProgressRunner` gets the right indicator with no extra code; the point fixes 1-5 above become one-line `label` / `progress` / `output` choices.

Migration plan:
1. Add `src/proc/` with the runner, both indicators, the four parsers and `Batch`; unit tests feed recorded `git` / `cargo` / `curl` stderr into the parsers, and a test asserts `Hidden` writes nothing.
2. Move the shared helpers first: `worktree/git.rs::git`, `agents/mod.rs::run_cli` / `spawn_cli` / `app_version`, `doctor.rs::spawn_mcp` / `mcp_command`. That converts most user-facing sites at once.
3. Convert the remaining sites file by file, choosing `Output` per the audit: `agents/restart.rs` (10, `Capture`), `bench.rs` (3, `Batch` plus `Capture`), `plugins/cmd/run.rs` (`Stream` or `Capture` per step 1), `graph/lsp.rs` and `graph/mod.rs` (`Capture`, long-lived LSP children via `.spawn()`), `graph/watch.rs` (`Capture`, hidden in the watcher), `demon.rs` (`Detached` for children, `Inherit` for upgrade, `Capture` for `sysctl`), `log.rs` (`Inherit`), `otel/export.rs`, `hooks/mod.rs`, `bin/rtok-hook.rs`, `mcp/wrap.rs` (`Detached` / `Stream`, always hidden). `build.rs` stays on `Command` (build scripts cannot use the crate) and is the one listed exception.
4. Delete the point indicators: `with_loader` and the T81 `interactive` switch in `cli.rs`, and `render::loader` / `render::spinner` once `graph index` uses a `Batch` over files. `render.rs` keeps only styles.
5. Turn on the `clippy.toml` ban and fix whatever it still finds, including `crates/`.
6. Update `docs/` (contributor notes: "run external programs only through `proc::ProgressRunner`") and `CHANGELOG.md`.

Fix:
1. Confirm the audit list above with `rg` over the whole workspace (including `crates/`), and record each site as covered / add / not needed in a table in this card.
2. Implement the `ProgressRunner` migration above; items 1-5 are covered by it, not by separate spinners. Every message names the action and the target in present tense and ends with `…`; the loader is cleared (`finish_and_clear`) before the command's own output or any error is printed.
3. Interactive rule (T81) stays: a loader never runs while rtok is waiting for an answer. Put the pause and resume in one helper so every prompt uses it.
4. Nothing changes when stderr is not a TTY: pipes, CI, `--json` and MCP output stay byte-identical.
5. Progress bar over spinner: when an operation can report progress (megabytes, percent, files or items done out of a known total), show an `indicatif` progress bar with that count, not a spinner. A spinner is only for a wait that cannot be measured, where the command gives no intermediate data until its output arrives. For the items above this means: `rtok bench` shows a bar over runs (`3/10`), `rtok doctor` a bar over servers when it probes more than one, `rtok worktree gc/clean` a bar over worktrees removed; a single `claude -p` run, one MCP server start or one `git worktree add` stays a spinner.

Check: on a TTY, each of items 1-5 shows its message from the moment the process starts until output appears; `rtok agents update claude` in a terminal shows step loaders and the plugin question is readable; `rtok doctor 2>/dev/null` and `--json` output are unchanged; a Vfs or snapshot test asserts no spinner bytes on non-TTY stderr; every operation with a known total shows a progress bar with its count and a spinner appears only on unmeasurable waits; `just check`.

Execution plan (after T275, T277 and T279, which touch the same `agents` spawn helpers):
1. Worktree `_worktrees/rtok-T276`. Confirm the audit with `rg` over the workspace and write the site table into this card (Fix 1).
2. PR 1: `src/proc/` with `ProgressRunner`, `TtyIndicator` / `Hidden`, the four parsers, `Batch`, `proc::suspend`; parser tests on recorded stderr and a no-bytes test for `Hidden`.
3. PR 2: the shared helpers (`worktree/git.rs::git`, `run_cli` / `spawn_cli` / `app_version`, `spawn_mcp` / `mcp_command`), with `proc::suspend` replacing the T81 switch.
4. PRs 3-4: the remaining sites file by file, per the audit's `Output` choice.
5. PR 5: delete `with_loader` / `render::loader` / `render::spinner`, turn on the `clippy.toml` ban, contributor docs and `CHANGELOG.md`.

### T277. Move rtok's MCP core into its own crate `crates/rtok-mcp`

Problem: install, update, remove, status and ping of rtok's MCP entry are written separately in each host (`register_mcp` / `unregister_mcp` / `installed` in 20+ `src/agents/<host>/mod.rs`, plus `plugin_is_mcp`, `code_serves_mcp`, `installed_mcp_only` and the MCP half of `d21_plugin_apply`). That is how the Claude Desktop entry removal (T275) was repeated on Cursor, Codex, VS Code, ZCode, Kimi, Grok, Copilot and Gemini. T275's architecture principle states the rule; this task makes it a crate boundary so a host cannot grow its own MCP logic again.

Crate layout (`crates/rtok-mcp`, workspace member, not published, no dependency on the `rtok` crate):
- `spec.rs`: `McpSpec` (the per-host data from T275): config path(s) per surface, format (`Json`, `Jsonc`, `Toml`), key path (`mcpServers`, `servers`, `context_servers`, `mcp.servers`, `mcp_servers`), server name, command form (bare or absolute), entry shape (`type`, `args`, `env`), duplicate-name behaviour (`Merges`, `OverridesByScope`, `Errors`, `ShowsBoth`), plugin-served name, headless ping command. `Surface { Cli, Desktop, Ide }`.
- `registry.rs`: the one list of rtok's MCP servers (today `rtok`, plus graph or plugin servers when they get their own entry): name, command, args, env. Hosts never build an entry by hand.
- `config.rs`: read and write the host config through a `Fs` trait (so the existing `Vfs` tests plug in): `write_entry`, `remove_entry`, `read_entry`, keeping unrelated keys, comments in JSONC and TOML formatting, with a backup like today.
- `status.rs`: `McpStatus { surface, entry: Present | Missing | Stale(diff), plugin_serves: Option<name> }`; the truth source for T278.
- `ping.rs`: T275.1's ping: headless run through the spec's command, or spawn the configured server and call the `ping` tool.
- `doctor.rs`: the per-host MCP check `rtok doctor` runs (entry present, command resolves, server starts, `tools/list` answers).
- `ops.rs`: `apply(spec, Mode::{Install, Update, Remove}, fs) -> Report`, the only entry point hosts call.

What moves: from `src/agents/mod.rs` the MCP read/write helpers, `installed_mcp_only`, the MCP half of `d21_plugin_apply`; from every host its `register_mcp` / `unregister_mcp` and the MCP branch of `installed`; `plugin_is_mcp` (cursor) and `code_serves_mcp` (claude); from `src/doctor.rs` the MCP probe (`spawn_mcp`, `mcp_command`, using T276's `ProgressRunner` for the spawn); the `ping` tool body for T275.1. What stays in `rtok`: hooks, proxy, plugin install via host CLIs, desktop restart, and each host's `McpSpec` value.

Migration by host:
1. Create the crate with `spec`, `registry`, `config`, `status`, `ops` and a table-driven `Vfs` test over sample specs. No host uses it yet.
2. Claude (Code and Desktop) first, because it is where T275 was found: implement its `McpSpec`, route `apply` and `installed` through `rtok_mcp::ops` / `status`, delete `code_serves_mcp`. Land with T275's fix and T278's status.
3. The hosts T275 lists as affected: Cursor, Copilot, Gemini, Codex, VS Code and Insiders, ZCode, Kimi, Grok. One commit per host, each deleting that host's MCP functions.
4. The unaffected hosts: Kilo, OpenCode, Cline, Devin, Windsurf, Zed, MiMo, CodeWhale, omp, pi, Antigravity.
5. Move `doctor`'s MCP probe and add `ping` (T275.1) on top of the crate.
6. Delete the now-empty helpers in `src/agents/mod.rs`, add a test that fails when a file under `src/agents/` touches an MCP key directly, generate the host table in `docs/agents.md` from the specs.

Check: every host's install, update, remove, status and ping pass the same table-driven test; `rg 'mcpServers|context_servers|mcp_servers' src/agents` finds only `McpSpec` values; `just check`.

Execution plan (after T275 PR A, which gives the core in `src/agents/mcp.rs`):
1. Worktree `_worktrees/rtok-T277`. PR 1: crate `crates/rtok-mcp` with `spec`, `registry`, `config` (behind an `Fs` trait the `Vfs` implements), `status`, `ops`, and the table-driven test over sample specs; `toolchain.md` row.
2. PR 2: Claude moves onto the crate; `src/agents/mcp.rs` shrinks to spec rows.
3. PRs 3-4: the other hosts, one commit per host, at most 10 files per PR.
4. PR 5: `doctor`'s MCP probe into the crate; `ping` joins when T275.1 lands.
5. PR 6: delete leftover helpers, add the guard test against MCP keys under `src/agents/`, generate the `docs/agents.md` host table.

### T278. `rtok agents info <agent>` reports the real MCP state, not "mcp installed" by assumption

Problem: `agents info claude` shows `mcp installed` for Claude Desktop with no `rtok` entry in `claude_desktop_config.json`, because `installed` (`claude/mod.rs:688-694`) returns `mcp` when the Code plugin is present (`code_serves_mcp`), and matches the text `"rtok"` anywhere in the file. That hid today's bug. Other hosts do the same when a plugin is installed (`files.contains("mcp") || plugin`).

Fix:
1. Status comes from `rtok_mcp::status` (T277), or from an equivalent function in `src/agents/mod.rs` if T278 lands first. It parses the config and looks up the exact key path and server name; no substring match.
2. `agents info` prints MCP per surface on its own line, for example `mcp  desktop  missing (claude_desktop_config.json has no "rtok")`, `mcp  cli  plugin rtok@rtok (serves "rtok")`, `mcp  cli  entry ~/.claude.json`. A stale entry (wrong command or args) prints `stale` and the difference. Hooks keep their current line.
3. A plugin only counts for the surface it actually serves. The Claude Code plugin never makes Desktop show as installed.
4. `--json` and the web UI model (`web/model.rs`) carry the same fields: `surface`, `entry` (`present`, `missing`, `stale`), `plugin`.
5. `agents list` and `installed_hosts` use the same status, so "installed" means the same thing everywhere.

Tests: a table-driven `Vfs` test per host writes each combination (no entry, entry, stale entry, plugin only, entry and plugin) and asserts the printed and `--json` status matches the file content; a regression test for today's case (Code plugin installed, Desktop file without `rtok`) expects `desktop missing`.

Check: on the creator's machine, before T275 is fixed `rtok agents info claude` shows Desktop `missing`; after `rtok agents install claude` it shows `present`; `just check`.

Execution (2026-09-27): one PR off main. Status comes from `rtok_mcp::status` (T277, merged in #437) per surface. `agents info`, `agents list`, `installed_hosts` and `web/model.rs` all read it, so a plugin only counts for the surface it serves. Tests: one table-driven `Vfs` test per host, plus the regression "Code plugin installed, Desktop file without `rtok`" → `desktop missing`. The T275 per-host PRs still open (VS Code, ZCode, Kimi, Grok) touch the same host modules, and whichever lands second rebases.
Progress (2026-09-28): merged in #464 (per-surface `mcp` lines, `--json` and web `mcp: [{surface, file, entry, diff?, plugin}]`, table-driven test over every host, the `desktop missing` regression). Left: the creator-machine Check.

### T279. One plugin version scheme for every install source (GitHub, local, marketplace), and `agents update` that skips an up-to-date plugin

Problem: every plugin manifest is still `0.0.1` while rtok is at `0.10.0` (tag `v0.10.0`): `plugins/claude/.claude-plugin/plugin.json` (`rtok@rtok`), `plugins/codex/.codex-plugin/plugin.json`, `plugins/cursor/plugin.json` and `.cursor-plugin/plugin.json`, `plugins/copilot/plugin.json`, `plugins/gemini/gemini-extension.json`, `plugins/kimi/kimi.plugin.json`, `plugins/pi/package.json`. Claude Code caches a plugin by its manifest version (`~/.claude/plugins/cache/rtok/rtok/0.0.1/`, `installed_plugins.json` records `"version": "0.0.1"` for commit `12c7e91`), so a new build with the same number is not a new version to it. rtok itself has no way to tell which plugin build is installed or where it came from, so `agents update` either reinstalls every time or trusts the host. A plugin reaches a user from three sources, and the scheme has to work for all of them:
- GitHub: the host installs straight from the repo (`claude plugin marketplace add listepo/rtok`, Gemini `extensions install https://github.com/listepo/rtok`, similar for others), at a branch or tag.
- Local: installed from the plugin tree of an rtok checkout or install (`plugins/<host>`; Claude via `claude plugin marketplace add <path>`, the pre-T139 flow), used for development and offline installs.
- Marketplace: the host's own catalog entry, which updates when a push to the repo changes the committed catalog (`.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json`, T183's `marketplace.yml` verifies them).

1. Version file: format and location.
   - Each plugin tree carries `plugins/<host>/.rtok-plugin-version`, committed, one JSON object: `{"schema":1,"plugin":"claude","version":"0.10.0"}`. `version` is SemVer and equals the rtok version at the commit. It sits at the plugin root, so every source copies it with the plugin: the GitHub and marketplace installs land it in the host's install dir (for Claude `installPath` from `installed_plugins.json`), a local install copies or links it.
   - Local installs from a git checkout add build metadata when rtok writes the installed copy: `"version":"0.10.0+g12c7e91"`, and `+g12c7e91.dirty` for uncommitted changes, plus `"source":"local"`. rtok writes the same string into the installed copy's manifest `version`, so hosts that cache by version (Claude) also see a new build.
   - rtok keeps an install receipt per host at `$XDG_STATE_HOME/rtok/plugins.json` (`~/Library/Application Support/rtok/plugins.json` on macOS, `%LOCALAPPDATA%\rtok\plugins.json` on Windows): `{"claude":{"source":"github","ref":"v0.10.0","marketplace":"rtok","path":"<installPath>","version":"0.10.0","installed_at":"…"}}`. For a local install `ref` is the checkout path and commit; for a marketplace install it is the marketplace name and its source.
2. How update finds the source and the new version.
   - Source: the receipt first. With no receipt, the host's own records: for Claude, `known_marketplaces.json` (`"source":"github","repo":"listepo/rtok"` means GitHub or marketplace, a `directory` source means local) and `installed_plugins.json` (`installPath`, `version`, `gitCommitSha`); each host's lookup is a field in its plugin spec, next to T275's `McpSpec`, not a code branch.
   - New version, by source. GitHub: read `plugins/<host>/.rtok-plugin-version` at the tag that matches the running binary (`v{CARGO_PKG_VERSION}`), so plugin and binary stay in step; with `--channel main` read it from `main`. Local: read the file in the local plugin tree and add `+g<sha>[.dirty]` from `git describe`. Marketplace: refresh the host catalog first (`claude plugin marketplace update rtok` and the equivalents), then read the file inside the refreshed catalog checkout (for Claude `~/.claude/plugins/marketplaces/rtok/plugins/claude/.rtok-plugin-version`).
   - Installed version: the `.rtok-plugin-version` inside the installed copy, falling back to the receipt, then the host record's `version`.
3. Comparison and decision.
   - Parse both as SemVer. Equal version and equal build metadata: skip, print `plugin rtok@rtok 0.10.0 up to date (github)`. New is greater: update in place through the host (`claude plugin update`, T242.3's reinstall fallback), then rewrite the receipt. New is lower: skip with a warning naming both versions; `--force` (step 6) reinstalls anyway.
   - Same base version but different build metadata (local builds): update, since SemVer ignores metadata and the build did change.
   - A source change (the receipt says local, the user now installs from GitHub, or T139's stale marketplace) is always an update: reinstall from the new source.
   - `--force` bypasses this table entirely (step 6); `--dry-run` prints the decision for each host without acting. The decision table is one pure function with unit tests.
4. Installs without a version file (everything before this task).
   - The installed version comes from the host record if it has one (`0.0.1` today), otherwise it is treated as `0.0.0`. Either is lower than any real release, so the first `agents update` after this task updates once, writes the version file into the new copy and creates the receipt; from then on the normal rule applies.
   - If that update fails, the old plugin stays, the receipt is not written, and the report says `plugin rtok@rtok: legacy install, update failed: …` so the next run tries again.
   - `agents info` shows `legacy (no version file)` for such installs until then (ties into T278).
5. Release-plz and bump.
   - `tools/plugin-versions.sh --set <version>` writes the version into every `.rtok-plugin-version` and every manifest `version` above; `--check <version>` lists each file that differs and exits non-zero.
   - `tools/release.sh` (the version commit `bump.yml` makes) calls `--set` in the same commit that edits `Cargo.toml` and `Cargo.lock`, and the version files join the list of files that commit may touch. `release-plz` only edits Cargo files and today releases only `rtok-plugin-sdk`, so the plugin files never go through it; if `release-plz` ever opens the rtok release PR, that PR runs `--set` in a follow-up step, and the check below blocks it until it does.
   - CI: `ci.yml` runs `--check` against the `Cargo.toml` version on every PR; `release.yml` runs `--check` against the tag (`v` stripped) before building and fails the release on a mismatch. A Rust test asserts every version file and manifest equals `env!("CARGO_PKG_VERSION")`, so `just check` catches drift locally.
   - Raise every manifest and add every version file at `0.10.0` in this task's PR.

6. `--force`: unconditional reinstall.
   - Syntax: `rtok agents update <agent>[,<agent>…] --force`, and `rtok agents update --force` with no host for every host rtok is installed in (the existing "host omitted" rule; `--all` already means "all variants, CLI and desktop" in `UpdateArgs`, so it keeps that meaning and does not also mean "all hosts"). Combines with `--cli` / `--desktop`, `--no-restart` and `--dry-run`.
   - Behaviour: no version lookup, no comparison, no skip. For each selected host rtok removes the installed plugin through the host (`claude plugin uninstall rtok@rtok` and the equivalents, plus a stale marketplace entry as in T139), then installs it again from the source: the receipt's source, else the one step 2 detects, else the default for that host (GitHub at `v{CARGO_PKG_VERSION}`). `--force --source github|local|marketplace` picks the source explicitly and replaces the recorded one.
   - Plugin not installed: `--force` installs it from scratch (the uninstall step is skipped, not an error), then writes the version file and receipt like a normal install.
   - Version file and receipt: after a successful reinstall rtok reads `.rtok-plugin-version` from the new installed copy (writing the `+g<sha>` form for a local source) and rewrites the receipt with the new source, ref, path, version and time. A downgrade or an identical version is reinstalled all the same.
   - Failure: if the uninstall succeeds and the install fails, the report says so plainly (`plugin rtok@rtok removed, reinstall failed: …`), the receipt is deleted so the next `agents update` treats the host as "not installed" and installs, and the exit code is non-zero. MCP entries and hooks follow T275 (always rewritten), not the plugin step.
   - `--dry-run --force` prints `would reinstall rtok@rtok from github v0.10.0 (forced)` per host and changes nothing.
   - Tests (with a fake host CLI on `PATH` that logs its calls, plus `Vfs`): equal versions with `--force` still run uninstall then install; without `--force` they run nothing; a not-installed host with `--force` runs install only and writes the receipt; a newer installed version with `--force` is replaced by the older source version; `--force --source local` switches the receipt source; install failure after uninstall deletes the receipt and exits non-zero; `--force` with no host touches every installed host and only those; `--dry-run --force` calls no CLI and writes no file; the forced path never calls the version comparison function.

7. Documentation (required; T279 and T279.1 are not done without it).
   - A new page `docs/plugin-versions.md`, written for a developer who has not read the code. Sections, in this order:
     1. Why: hosts cache plugins by version, three install sources, what went wrong with `0.0.1`.
     2. The version file: location `plugins/<host>/.rtok-plugin-version`, the JSON fields (`schema`, `plugin`, `version`, `source`), who writes it (committed by the release, rewritten with `+g<sha>[.dirty]` for local installs), where it ends up after install for each host.
     3. The receipt `plugins.json`: path per OS, every field, when it is written, rewritten and deleted.
     4. Sources: GitHub, local, marketplace; how each is detected (receipt, then host records) and where the new version is read from for each, with the Claude paths as the worked example.
     5. The decision: a table of every case (equal, newer, older, same base with different build metadata, source change, legacy without a file) and its result (skip, update, reinstall, warn), then `--force`, `--source` and `--dry-run`, and what happens when a reinstall fails.
     6. `rtok agents outdated` and `rtok agents update --check`: what is listed and what is hidden, the two "nothing to do" messages, the `--json` schema, `--exit-code`, why it works offline.
     7. Releasing: `tools/plugin-versions.sh --set` / `--check`, where `tools/release.sh` and `bump.yml` call it, the CI and `release.yml` checks, why `release-plz` does not touch these files and what happens if it ever opens the rtok release PR.
     8. Troubleshooting: "plugin stays old after update", "legacy install", "version mismatch in CI", each with the command to run.
   - Each section has a short command example with real output copied from a run, not invented.
   - Links: README gets a line under the plugin/agents section, `Plugin versions and updates: [docs/plugin-versions.md](docs/plugin-versions.md)`; `docs/release.md` links the Releasing section; `docs/agents.md` links it next to `agents update`; `CHANGELOG.md` mentions the page. The landing site picks the page up through the existing docs sync (owned separately; this task does not edit `sync-docs.yml`).
8. Tests (required; the full set for T279 and T279.1).
   - Unit tests on the pure decision function (no files, no processes), one case each: equal versions skip; newer available updates; older available skips with a warning; same base with different `+g<sha>` updates; same base with `.dirty` against clean updates; identical build metadata skips; source change reinstalls; legacy (no version) against any release updates; `--force` returns reinstall for every one of these inputs; invalid SemVer in a version file is an error that names the file.
   - Unit tests on the version file and receipt: parse and write round-trip; unknown `schema` rejected; receipt paths per OS; local version string from a clean and a dirty `git describe`.
   - Integration tests in `tests/plugin_versions.rs` with `Vfs` fixtures and a fake host CLI on `PATH` that logs every call:
     - update: equal versions run no CLI; a newer version runs `plugin update` and rewrites the receipt; an older one runs nothing and warns.
     - force: equal versions still run uninstall then install; a not-installed host runs install only and writes the receipt; a newer installed plugin is replaced by the older source version; the forced path never calls the comparison.
     - source change: `--force --source local` over a GitHub receipt reinstalls from the local tree and switches the receipt source; a stale marketplace (T139) triggers reinstall.
     - failure: the fake CLI fails install after a successful uninstall; the receipt is deleted, the report says `removed, reinstall failed`, the exit code is non-zero, and the next `update` installs.
     - dry-run: `--dry-run` and `--dry-run --force` spawn no CLI and change no file; their printed decisions match what a real run then does.
     - legacy: an install without a version file and with host record `0.0.1` updates once, then the second run skips.
     - outdated: only outdated rows are printed; all current prints the up-to-date line; nothing installed prints `no rtok plugins installed`; legacy rows show `legacy`; newer and same-version-with-metadata rows are hidden; `--json` matches the schema in T279.1; `--exit-code` returns 10 when something is outdated and 0 otherwise; `agents update --check` output is byte-identical to `agents outdated`.
     - offline: `agents outdated` runs with the network disabled (proxy env pointed at a closed port) and the fake CLI logs no call.
   - Release checks: `plugin-versions.sh --check` passes on the tree and fails after one manifest or version file is edited (a shell test in `tools/`); the Rust test that compares every manifest and version file with `CARGO_PKG_VERSION`.
   - All of it runs in `just check` and CI on macOS, Linux and Windows.

Check: `rtok agents update claude` on today's install (0.0.1, no file) updates once to `0.10.0` and writes the receipt; a second run prints `up to date` and runs no `claude` command; a local install from a dirty checkout shows `0.10.0+g<sha>.dirty` and a new commit triggers an update; `--dry-run` lists the decision per host; editing one version file to `0.0.2` fails the CI check and the test; `rtok agents update claude --force` reinstalls even when up to date and rewrites the receipt; `just check`.

Execution plan:
1. Worktree `_worktrees/rtok-T279`. PR 1: `.rtok-plugin-version` files and every manifest at `0.10.0`, `tools/plugin-versions.sh --set/--check` with its shell test, the `CARGO_PKG_VERSION` Rust test, `--check` in `ci.yml`, `release.yml`, `tools/release.sh`.
2. PR 2: version file and receipt types, the installed/new version lookup, and the pure decision function with its unit tests (step 8, first two groups). No behaviour change yet.
3. PR 3: `agents update` uses the decision; `--force`, `--dry-run`, `--source`, the legacy path, failure handling; integration tests in `tests/plugin_versions.rs` with a fake host CLI.
4. PR 4: `docs/plugin-versions.md` with real command output, and its links (step 7).
Progress (2026-09-28): PRs 1-4 merged (docs: #465, today's behaviour). Open against this card, for the creator: the installed copy's `.rtok-plugin-version` is never read or written (`read_installed` and `VersionFile::write` are unused), so there is no legacy line from it; a missing `claude` on `PATH` prints "already current"; the dry-run reinstall wording differs from step 5; `--source local` fails from a release install; the marketplace source does not read the catalog. Only Claude is wired; Codex, Copilot and Gemini follow.

### T279.1. `rtok agents outdated`: list only the hosts whose rtok plugin is older than the running rtok

Name: `rtok agents outdated`, the word `npm outdated`, `cargo outdated` and `brew outdated` use for exactly this list, next to the `agents list` / `info` / `update` it belongs with. `rtok agents update --check` is an alias that prints the same thing (for people who look under `update`). `versions` was rejected: it reads as "show every version", while this command hides everything that needs no action.

Behaviour:
- Walks every host rtok supports (the host registry `agents list` uses), not only the ones in the receipt, so a plugin installed by hand or by an older rtok is found too.
- For each host it reads the installed plugin's version with T279 step 2's lookup: `.rtok-plugin-version` in the installed copy, then the receipt, then the host's own record (Claude `installed_plugins.json`). The source comes from the same lookup (`github`, `local`, `marketplace`).
- Target version is the running binary's `CARGO_PKG_VERSION`. The command reads local files only: no network, no host CLI call, no marketplace refresh, so it is fast and works offline.
- A host is listed only when the plugin is installed and its version is lower than the target by SemVer, ignoring build metadata (a local `0.10.0+g12c7e91` on rtok `0.10.0` is current). An install with no version file and no recorded version counts as `0.0.0` and is listed as `legacy`. Hosts without the plugin, with the same version, or with a newer one are not printed.
- Selection flags as in `update`: an optional host list (`rtok agents outdated claude,cursor`), `--cli` / `--desktop`.

Output:
- A table with columns `agent`, `installed`, `available`, `source`, one row per outdated host and variant, for example `claude  0.0.1  0.10.0  github` and `gemini  legacy  0.10.0  marketplace`. A footer names the next step: `run: rtok agents update claude,gemini`.
- Nothing to update, some plugins installed: `all rtok plugins are up to date (3 installed, rtok 0.10.0)`.
- No plugin installed anywhere: `no rtok plugins installed`.
- `--json`: `{"rtok":"0.10.0","outdated":[{"agent":"claude","variant":"cli","installed":"0.0.1","available":"0.10.0","source":"github","legacy":false}],"installed":3}`, with `outdated` empty in both "nothing to do" cases; the human messages are not printed.
- Exit code 0 by default, so scripts that only read the output keep working; `--exit-code` returns 10 when at least one host is outdated, for CI and hooks.

Implementation: one function `outdated(cfg, selection) -> Vec<Outdated>` built on T279's version lookup and comparison (the same pure function `update` uses, so both always agree on "outdated"); the table uses the existing `render` table helpers; the `demon` and web UI can call the same function later for an "updates available" badge.

Tests (`Vfs` fixtures): no plugins prints `no rtok plugins installed` and `outdated: []`; all current prints the up-to-date line; one outdated and one current prints only the outdated row; a legacy install without a version file is listed as `legacy`; a newer installed plugin is not listed; local build metadata on the same version is not listed; `--json` matches the schema above; `--exit-code` gives 10 and 0 in the matching cases; `agents update --check` output equals `agents outdated`; no host CLI is spawned (fake CLI on `PATH` logs nothing).

Documentation and tests (required): this command has its own section in `docs/plugin-versions.md` (T279 step 7, section 6) and its cases in T279 step 8 (the `outdated` and `offline` groups, the `--exit-code` and alias checks). T279.1 is not done until both are in and green.

Check: on the creator's machine today `rtok agents outdated` prints `claude 0.0.1 0.10.0 github`; after `rtok agents update claude` it prints the up-to-date line; `just check`.


### T281. Probe: tie a host session's hooks and its rtok MCP server to one agent

Creator request 2026-09-27 (T281–T290, D34): every agent working through rtok gets one rtok agent id, visible to the user in the terminal and to the agent over MCP. Hooks see the host's session id (`research.md` §26), but `rtok mcp` is started by the host with only `["mcp"]` and its cwd (`src/mcp.rs:592`, host from `[hook] host`); nothing today tells the MCP process which session it serves. Without that link an agent's MCP calls (`whoami`, `worktree_add`, `agent_send`) cannot be attributed to its agent id. No product code in this task.

Plan:
1. For each host with hooks and MCP (Claude Code CLI and desktop, Cursor, Codex, Copilot CLI, Grok, Gemini, Kimi, ZCode, CodeWhale): a throwaway MCP wrapper and hook script in the scratchpad (never committed) log, per process: pid, ppid chain up to the host process, cwd, every env var whose name contains `SESSION`, `CONVERSATION`, `THREAD` or the host name, MCP `initialize.params.clientInfo` and `_meta`, and the hook payload's session field.
2. Run one session per host by hand, on the creator's machine only; real agents are for manual debugging, never tests (T280). Include one sub-agent per host that has them.
3. Pick the link rule per host, in this order of preference: a session id env var the MCP process inherits (e.g. `GROK_SESSION_ID`, `GEMINI_SESSION_ID`); the nearest common host ancestor pid shared by hook processes and the MCP process; cwd + host + start time as the last resort, marked ambiguous when two sessions share a cwd.
4. Check whether one MCP process serves several sessions (the desktop apps may share one) and whether it outlives its session.

Check: `research.md` §26 gains a dated table host → host version → the MCP process's link to the session (env var name / ppid rule / cwd only / none) → sub-agent behaviour → one MCP process per session yes/no, each row with its source (the probe log or the vendor docs URL). The T283 card is updated with the rule per host.

Execution (2026-09-27): agent sessions may not read their own process environment (blocked by the host's policy), so the live runs are the creator's. (1) Worktree `_worktrees/rtok-t281`. (2) Vendor docs first, per host: which env vars an MCP server process inherits, the hook payload's session field, whether one MCP process serves one session; each fact cited with URL and date. (3) A probe kit in the scratchpad, never committed: an MCP stdio wrapper that logs pid, ppid chain, cwd, the filtered env var names and values, `initialize` params, then execs `rtok mcp`; a hook script that logs the same next to the payload's session field; a one-page run sheet for the creator. (4) `research.md` §26 gets the table from the docs now, with a "probe" column left `pending` until the creator's logs arrive; T283's card gets the rule per host once they do.

### T283. An agent learns its own rtok agent id

Depends on T282 and T281 (the MCP link rule per host) and on T275 (every host's MCP entry is rewritten there; do not collide). An agent must know its id to report it, to claim worktrees and to message others.

Plan:
1. Worktree `_worktrees/rtok-T283`.
2. SessionStart (and SubagentStart where the host has it, T262.2) adds one line to the injected context: `rtok agent id: <first 8 hex> (full: <uuid>). Use it with rtok's agent_* and worktree_* MCP tools.` Fixed wording, byte-stable apart from the id, counted in the injection budget; no line when `[agents] enabled = false`.
3. Env: where the host lets a SessionStart hook export variables to the agent's shell (Claude Code `CLAUDE_ENV_FILE`: confirm in the hooks docs first and cite it), export `RTOK_AGENT_ID`, so `rtok` run from the agent's Bash tool knows its caller. Other hosts: `rtok` resolves the caller by the T281 rule.
4. MCP: `rtok mcp` resolves its agent at `initialize` with the T281 rule for its host; for hosts without hooks (Zed, Antigravity, Cline, pi, omp, opencode, …) the MCP process registers its own agent row (host from a new `--host <id>` arg that every host's MCP entry passes; update each `register_mcp` call and the host tests after T275 lands). New MCP tool `whoami` → `{id, short, host, host_session, cwd, worktrees: [...]}`.
5. CLI `rtok agents whoami [--json]` → the same, from `RTOK_AGENT_ID` or the T281 rule; exit 1 with "not inside an agent session" otherwise.

Check: hook fixture test: SessionStart output carries the line and it is identical across two runs but for the id; MCP e2e with a fake client: `initialize` then `tools/call whoami` returns the registered id; a hook-less fake host registers through MCP alone; trycmd for `agents whoami`; `surface_parity`, `config_coverage`, man page; `just check`.

Execution (2026-09-27): two PRs. PR 1, cut on top of T282's branch until #439 merges: the SessionStart line (step 2) inside the injection budget; `RTOK_AGENT_ID` through `CLAUDE_ENV_FILE` (step 3, cited from the Claude Code hooks docs); `rtok agents whoami [--json]` from `RTOK_AGENT_ID` (step 5); tests: hook fixture byte-stable but for the id, `enabled = false` prints nothing, trycmd, `surface_parity`, `config_coverage`, man page. PR 2, after T281's rules and T275's per-host PRs land: `rtok mcp` resolves its agent at `initialize`, `--host <id>` in every host's MCP entry, hook-less hosts register through MCP, MCP tool `whoami`; MCP e2e with a fake client.
Progress (2026-09-28): PR 1 merged (#449): SessionStart line, `RTOK_AGENT_ID`, `rtok agents whoami` (host session id only in `--json`). Left: PR 2, the MCP link at `initialize` after T281's probe.

### T284. See what every agent is doing: ids, worktree and activity in `rtok agents sessions`, `rtok agents show`

Depends on T282, T283. `rtok agents sessions` (`src/cli.rs:579`) already lists sessions with host, model and tokens; it gains the agent id, where the agent works and what it is doing, instead of a second listing command (no duplicated logic).

Plan:
1. Worktree `_worktrees/rtok-T284`.
2. `rtok agents sessions`: new columns `agent` (8-hex short id), `worktree` (claimed worktree name from T285 when present, else the cwd relative to the project), `activity`, `seen` (e.g. `12s ago`), `state` (`live`, `idle`, `ended`); sub-agents indented under their parent; `--json` carries the full id and paths. Default shows live and idle; `--all` adds ended.
3. `rtok agents show <id-prefix> [--json]`: host, model, full id, host session id, parent and sub-agents, cwd, claimed worktrees with branch and state (T285), task id (from the worktree lock), last activity, the agent's own status text, unread message count (T287), started / last seen.
4. `rtok agents status "<text>"` and MCP tool `agent_status_set {text}`: the agent says what it is busy with (≤ 120 chars, plain text); shown in `sessions` and `show`.
5. MCP tools `agents_list {all?}` and `agent_show {id}` returning the same JSON as the CLI (one model function behind both, like `web::model` for `rtok web`/`rtok tui`).
6. `rtok web`/`rtok tui` session views read the same model; showing the new fields there is out of scope unless free.

Check: model unit tests over a seeded store (live, idle, ended, sub-agent nesting, prefix lookup); trycmd for `sessions`, `show`, `status`; MCP e2e with a fake client; `surface_parity`, `config_coverage`; `just check`.

Execution (2026-09-27): two PRs. PR 1, cut on top of T283 PR 1 (#449): one model function over the store; `agents sessions` gains `agent`, `worktree` (cwd relative to the project until T285 lands), `activity`, `seen`, `state`, sub-agent nesting and `--all`; `agents show <id-prefix> [--json]`; `agents status "<text>"`; model unit tests on a seeded store and trycmd. PR 2, after T283 PR 2 and T285: MCP `agents_list` / `agent_show` / `agent_status_set`, the claimed-worktree and unread-message fields.
Progress (2026-09-28): PR 1 is #470 (`agents sessions` columns, `agents show`, `agents status`, web model). Left: PR 2, the MCP tools (after T283 PR 2 and T285).

### T285. Worktree claims: `rtok worktree add` hands the worktree to the calling agent; MCP `worktree_add`

Depends on T282, T283. `rtok worktree add <task> [slug] --owner` exists (T158, `src/worktree/add.rs`) and prints the path, but the owner is free text and nothing links the worktree to an agent; `list` guesses the session from the last cwd seen (T154, `src/worktree/list.rs:174`).

Done means: an agent asks rtok for a worktree (MCP or CLI) and gets back the path to work in; the worktree is bound to its agent id in the git lock and in the store; `rtok worktree list` shows the agent id next to every worktree, on every host alike.

Plan:
1. Worktree `_worktrees/rtok-T285`.
2. Lock reason v2: `<owner> | <task-id> | <date> | agent <uuid>`. `Owner::parse` (`src/worktree/mod.rs:82`) reads both the 3-field and the 4-field form; an old lock stays valid and shows `agent -`. The lock stays the source of truth (it survives a lost store); the store keeps `worktree_claims (path, agent_id, task, claimed_at, released_at)` for fast joins.
3. `rtok worktree add` binds to the caller: `--agent <id-prefix>`, else `RTOK_AGENT_ID`, else the T281 rule, else no agent (current behaviour). `--owner` defaults to `<host> / <model>` of that agent when known.
4. MCP tool `worktree_add {task, slug?, base?}` → `{path, branch, task, agent}` plus the instruction text "work only inside `path`; remove it with `worktree_remove` when merged". Same code path as the CLI; the agent comes from the MCP session.
5. `rtok worktree list`: column `agent` (short id + host, e.g. `0193ab12 claude`) and `agent state` (`live`, `idle`, `ended`) from the claim; T154's inferred session stays as a fallback shown as `seen <host> <id8>`; `--json` carries full ids. MCP tool `worktree_list` returns the same rows.
6. `rtok worktree claim <path> [--agent]` for a worktree created before this task (writes the v2 lock only when the caller owns the lock or it has none; never takes a worktree locked by another owner).
7. `gc` (`src/worktree/gc.rs`): a worktree whose agent is live is never removed, even when merged; ended or unknown agents keep today's rules.

Check: unit tests for v2 parse/format and old-format compatibility; `add` e2e in a scratch repo with a fake agent row (bound lock, claim row, printed path under the T158 root); MCP e2e `worktree_add` → path exists, lock names the agent; `list` table and JSON snapshots with live / ended / unclaimed / old-format rows; gc keeps a live agent's merged worktree; trycmd and gates; `just check`.

Execution (2026-09-27): two PRs. PR 1, cut on top of T283 PR 1 (#449): lock reason v2 parse/format, migration `0025_worktree_claims`, `worktree add --agent` / `RTOK_AGENT_ID` binding, `worktree claim`, the `agent` column in `worktree list`, gc keeps a live agent's worktree; tests on a scratch repo with fake agent rows. PR 2, after T283 PR 2: MCP `worktree_add` / `worktree_list` and their e2e.
Progress (2026-09-28): PR 1 is #471 (lock v2, `worktree claim`, list columns, gc keeps a live agent's worktree). Left: PR 2, MCP `worktree_add` / `worktree_list` (after T283 PR 2).

### T286. `rtok worktree remove` and MCP `worktree_remove`: an agent removes its own worktree

Depends on T285. Removal today exists only in bulk (`rtok worktree gc`) and in the external `wt.sh done` script; an agent that finished its task has no single-worktree command.

Plan:
1. Worktree `_worktrees/rtok-T286`.
2. `rtok worktree remove <path|task-id> [--agent] [--json]` and MCP `worktree_remove {path|task}`: refuses (exit 1, reason on stderr / in the tool result) when the worktree has uncommitted or untracked files, is locked by another owner or another agent, or is the caller's cwd; never `--force`. Otherwise: unlock, `git worktree remove`, delete the local branch only when merged (squash-aware `is_merged`, `src/worktree/git.rs:57`), release the claim, print what happened and the one-line hint to delete the remote branch.
3. An unmerged clean worktree: removed only with `--keep-branch` (the branch survives, nothing is lost); without it, refused with that hint.
4. Extract the single-worktree removal that `gc` already does (`src/worktree/gc.rs:76`, lock restored on failure) into one function that `gc` and `remove` both call; no second copy.
5. `skills/worktrees/SKILL.md` finish step switches from `gc` to `remove` for "my task is merged".

Check: e2e in a scratch repo: merged clean → gone with branch; unmerged clean → refused, then removed with `--keep-branch`; dirty → refused; another agent's → refused; cwd → refused; gc tests unchanged and green after the extraction; MCP e2e; trycmd and gates; `just check`.

Execution (2026-09-27): two PRs. PR 1, cut on top of T285 PR 1: extract the single-worktree removal from `gc` into one shared function; `rtok worktree remove <path|task-id> [--agent] [--keep-branch] [--json]` with the refusals above; the claim is released; the skill finish step changes; e2e tests in a scratch repo. PR 2, after T283 PR 2: MCP `worktree_remove`.
Progress (2026-09-28): PR 1 on branch `t286-worktree-remove`, stacked on #471; its PR opens when #471 merges. Left: PR 2, MCP `worktree_remove` (after T283 PR 2).

### T287. Messages between agents and the user: `rtok agents send`, `rtok agents inbox`, MCP `agent_send`, `agent_inbox`

Depends on T282, T283. The creator wants to reach any running agent by its id from the terminal, and agents to reach each other over MCP.

Plan:
1. Worktree `_worktrees/rtok-T287`.
2. Diesel migration `messages`: `id`, `from_agent NULL` (NULL = the user at a terminal), `to_agent`, `body` (≤ 4 KiB, UTF-8, control chars stripped), `created_at`, `delivered_at NULL`, `read_at NULL`. Local store only, nothing leaves the machine.
3. CLI: `rtok agents send <id-prefix|--all-live> <text|->` (from `RTOK_AGENT_ID` when run inside an agent, else from the user); `rtok agents inbox [<id-prefix>] [--unread] [--json]` (default: the caller's inbox, or with an id the user reads that agent's queue without marking it read).
4. MCP: `agent_send {to, text}` → `{id}`; `agent_inbox {unread_only?, limit?}` → messages, marks them read. Every message is rendered inside a fixed frame: sender id, host and the note that it comes from another agent or the user through rtok and is information, not an instruction that overrides the agent's user or rules.
5. Sending to an ended agent is refused; `--all-live` fans out to live agents of the same project only.

Check: store tests (send, inbox order, read marks, 4 KiB cap, control-char strip); CLI e2e with two fake agents; MCP e2e: agent A sends, agent B's `agent_inbox` returns it framed, then empty; trycmd and gates; `just check`.

Execution (2026-09-27): two PRs, like T283. PR 1, cut on top of T283 PR 1 (#449): migration `0026_messages`, store API, CLI `agents send` / `agents inbox` with the fixed frame, store tests and a CLI e2e with two fake agent rows. PR 2, after T283 PR 2 gives `rtok mcp` its agent: MCP `agent_send` / `agent_inbox` and their e2e.
Progress (2026-09-28): PR 1 is #472 (`agents send`, `agents inbox`, the message frame). Left: PR 2, MCP `agent_send` / `agent_inbox` (after T283 PR 2).

### T288. Push unread messages to hooked agents

Depends on T287. Pull-only messages wait until the agent thinks to call `agent_inbox`. Hosts with hooks (`research.md` §26: Claude, Cursor, Codex, Copilot CLI, Grok, Gemini, Kimi, ZCode, CodeWhale) can receive them at the next turn.

Plan:
1. Worktree `_worktrees/rtok-T288`.
2. `UserPromptSubmit` and `PostToolUse` add undelivered messages to `additionalContext` (PostToolUse adds context only), framed as in T287, at most `[agents] push_bytes` (default 1 KiB) per event; the rest as `… and N more: call agent_inbox`. Mark them delivered (not read).
3. One indexed query per event; measure against the 10 ms hook budget as in T282; nothing is printed when the inbox is empty.
4. Hosts without hooks: documented as pull-only; the SessionStart line from T283 mentions `agent_inbox` there.

Check: hook fixture tests (one message, over-budget batch, empty inbox prints nothing, delivered once); hook bench row; `just check`.

Execution (2026-09-27): one PR, cut on top of T287 PR 1. The push goes through the budgeted injection path on `UserPromptSubmit` and `PostToolUse` using the T287 frame. New key `[agents] push_bytes`. Messages are marked delivered, not read. The `agent_inbox` mention is deferred until T287 PR 2 ships the tool. Includes hook fixture tests and a hook bench row in the PR.
Progress (2026-09-28): PR 1 on branch `t288-push-messages`, stacked on #472; its PR opens when #472 merges. The hook latency bench must be rerun on a quiet machine before the PR claims its row.

### T289. Worktrees the host creates join rtok: `rtok worktree adopt` and the post-create hooks

Depends on T285, T286. Only Claude Code can redirect worktree creation (T159). Cursor (`.cursor/worktrees.json` `setup-worktree*`), Kilo (`.kilo/setup-script`) and Devin/Windsurf (`post_setup_worktree`) only run a script after they create a worktree in their own pool (`research.md` §26). For worktrees to behave the same on every host, those must still get an owner, an agent id and rtok's remove / gc / clean.

Plan:
1. Worktree `_worktrees/rtok-T289`.
2. `rtok worktree adopt [<path>] [--task <id>] [--agent]` and MCP `worktree_adopt`: lock a host-made worktree with the v2 reason (T285), record the claim and `source: <host>`; the directory stays where the host put it. First confirm on each host whether a locked worktree breaks the host's own eviction (Cursor's cap of 25, Devin/Windsurf LRU); where it does, adopt records the claim in the store only, with no git lock, and the card says so.
3. Wire the post-create scripts: `rtok agents install <host> --project` writes rtok's entry into the project file (`.cursor/worktrees.json`, `.kilo/setup-script`, Devin/Windsurf's hook config), our entry only, the rest byte-for-byte (host-config rule); removal takes it out.
4. Hosts with native worktrees and no hook (Codex, Grok Build, MiMo, omp, Antigravity): the skill tells the agent to call `worktree_adopt` when it finds itself in a host-made worktree.
5. `rtok worktree list` already shows every registered worktree of the repo (git knows them wherever they are); add `source` (`rtok`, `claude`, `cursor`, …) from the path pool.

Check: adopt e2e in a scratch repo with a worktree under a fake `~/.cursor/worktrees/`; install/remove e2e per host writing only our entry; list shows `source`; `just check`.

### T290. Docs, skill and one cross-host test for agents and worktrees

Depends on T282–T289 (lands last; T159 may land after it and adds its own rows).

Plan:
1. Worktree `_worktrees/rtok-T290`.
2. `docs/agents-and-worktrees.md`: agent ids (D34), how an agent learns its id, `rtok agents sessions/show/status/send/inbox`, `rtok worktree add/list/remove/adopt/claim/gc/clean`, the MCP tools, per-host table (hooks, push vs pull messages, native worktrees: redirected / adopted / skill only), security note on messages. Links from `README.md` and `docs/config.md` (`[agents]` keys).
3. `skills/worktrees/SKILL.md` and `skills/rtok`: use `worktree_add` / `worktree_remove` / `worktree_adopt`, report the agent id, check `agent_inbox`; keep under the skill budget.
4. `tests/agents_worktrees.rs`: table-driven over every host in `src/agents/*` with fakes only (`RTOK_HOST_SANDBOX`, T280): fake session start (hook payload or MCP initialize, per the host's surface) → agent registered → `worktree_add` → `worktree list` shows the id → `agent_send` from a second fake agent → inbox / push → `worktree_remove`. Every host must produce the same worktree path rule, lock format and list row.

Check: `tests/host_docs.rs`, `tests/agents_doc.rs` regenerated where host tables change; the new test green on macOS, Linux and Windows CI; `just check`.

### T310. React SPA replaces the Slint web UI (epic)

`rtok web` draws its admin with Slint compiled to WASM on one `<canvas>` (`crates/rtok-webui`, D20). The creator chose to replace it with a React SPA in `web/`. Stack (creator's picks): React 19, TanStack (Router, Query, Table, Virtual, Form where a page needs it), Vite 8, Vitest 5, Tailwind CSS v4, Storybook 10, Playwright e2e. Data stays the `/ws` snapshot of D23; its TypeScript types are generated from the Rust types (`schemars` → JSON Schema → TS), so Rust stays the one source of truth. The visual reference is `design/html/` (all 13 pages, tokens, icons, fonts) and the `web/` prototype. When the SPA covers every page, `crates/rtok-webui`, the WASM build in CI/release, `design/html/` and the HTML prototype are deleted. One subtask = one PR (≤300 LOC hand-written, ≤10 files; lockfiles and generated files excepted).

Done when: `rtok web` serves the SPA from the binary, every page of `model::pages()` renders on it, Playwright drives the real binary, and no Slint code is left.

Check: `rtok web` from a release build shows every page of `model::pages()` from the embedded SPA; no `slint`/`rtok-webui` left in the tree; `just check` and the SPA CI job green.

### T310.3. Data layer: WebSocket client + TanStack Query

A typed `/ws` client (same-origin `ws`/`wss`, backoff reconnect, connection state) that pushes each snapshot into the TanStack Query cache; mutations for `set` and `expand`; a fixture source (`?sample`) with a snapshot fixture for Storybook, Vitest and offline e2e. Vitest covers reconnect and frame handling.

Check: Vitest covers frame parsing, reconnect with backoff, `set`/`expand` mutations and the `?sample` source; the app renders on sample data with no server.

### T310.4. App shell: router, layout, theme, states

TanStack Router (code-based route tree built from one page list), sidebar/top bar from `design/html`, theme toggle (`rtok-theme` in localStorage, system default), the orb background, reduced motion, and shared loading/empty/error/offline states.

Check: Vitest covers the route tree built from the page list, theme persistence across reload and the four shared states; every route reachable by keyboard.

### T310.5. UI kit + Storybook

Storybook 10 (`@storybook/react-vite`, addon-vitest, addon-a11y): Panel, Kpi, Pill, Switch, Search, Chip, Sparkline, DataTable (TanStack Table + Virtual) with stories for every state; stories run as Vitest browser tests.

Check: `storybook build` succeeds; stories run as Vitest browser tests with no a11y violations.

### T310.6. Pages: overview, plugins (toggle), calls (expand)

Check: each page matches `design/html/admin/<page>.html` in dark and light at 375 and 1280 px on sample data; toggle and expand round-trip against `rtok web`; stories and Vitest for page logic.

### T310.7. Pages: sessions, doctor, logs

Check: each page matches `design/html/admin/<page>.html` in dark and light at 375 and 1280 px on sample data; stories and Vitest for page logic.

### T310.8. Pages: skills, stats, graph, hosts, config, services, worktrees

Check: each page matches `design/html/admin/<page>.html` in dark and light at 375 and 1280 px on sample data; stories and Vitest for page logic.

### T310.9. Serve the SPA from `rtok web`

Embed `web/dist` in the binary (hashed assets, precompressed, SPA fallback, CSP), keep `RTOK_WEB_PKG`-style dev override for a local `dist`, build the SPA in CI and release before cargo. Rewrite `tests/web.rs`, `tests/web_e2e.rs`, `tests/release_bundle.rs` and `tests/surface_parity.rs` for the SPA (parity reads the SPA's page list).

Check: `cargo nextest run --test web --test web_e2e --test release_bundle --test surface_parity`; a release build serves the SPA with no `dist` on disk.

### T310.10. Playwright e2e against the real binary

Playwright drives `rtok web` on a fixture store (no real agents): every page renders, plugin toggle round-trips through `/ws`, expand works, offline/reconnect state shows. Runs in CI on Linux; Storybook tests run in the same job.

Check: `npx playwright test` green locally and in CI; breaking the toggle round-trip on purpose fails it.

### T310.11. CI job for the SPA

One CI job: `npm ci`, typecheck, oxlint/oxfmt, Vitest, Storybook tests, Playwright, `vite build`; cache npm and Playwright browsers.

Check: the job is green on a PR and goes red when a Vitest, Storybook or Playwright test is broken on purpose.

### T310.12. Delete Slint, the WASM build and the HTML design

Remove `crates/rtok-webui`, `tools/webui-bundle.sh`, `just web-bundle`/`webui-check`, the wasm steps in CI/release, `tests/web_wasm.rs`, `design/html/` and the rest of the prototype; update D20, `architecture.md`, `toolchain.md` and `rust.md`.

Check: `just check` green; `git grep -i slint` finds only history docs; the release workflow dry-run builds.

### T314. Fuzz testing with cargo-fuzz / libFuzzer

Ivan, 2026-09-30: the same fuzz setup as ketch (ketch plan R4). Already in progress. Branch `test/cargo-fuzz` (worktree `_worktrees/rtok-cargo-fuzz`) has uncommitted work and is not yet pushed to `pyrlyn/rtok`; no PR exists yet. Link the PR here when it opens.

Plan:
1. `fuzz/`: a standalone cargo-fuzz workspace excluded from the root one (`cargo-fuzz = true`, `libfuzzer-sys` 0.4, `arbitrary`, `rtok = { path = ".." }`), so `cargo build`, `just check` and CI never compile it.
2. Entry points in `src/fuzzing.rs`, compiled only under `--cfg fuzzing` (set by `cargo fuzz`), reach crate-private parsers without disk, env or log side effects.
3. Targets: `cli-argv` (`rtok::cli::Cli::try_parse_from` plus the help/error rendering), and the crash-prone parsers: `config-toml` (`config validate` + the file-free config stack), `cmd-filter` (user `rules` TOML + the stdout compactor), `hook-io` (the `HookInput` JSON and each host adapter), `jsonc-edit` (the JSONC editor that writes host settings), `proxy-wire` (Anthropic / OpenAI / Gemini request, response and SSE parsing), `transcript-jsonl` (`measure::jsonl`), `outline` (`read` outlines, tree-sitter queries) and `text-ops` (terse compression, `rtok expand` ranges, cuts). Seed corpora come from `tests/fixtures`.
4. Verify: `cargo +nightly fuzz build` for every target, then a short run of each (`cargo +nightly fuzz run <target> -- -max_total_time=60`). Every crash becomes a minimized regression test with its fix in its own PR.
5. Optional: a non-required nightly CI job (build plus a short run) on Linux. Do not touch `dependabot.yml` or `sync-docs.yml`.
6. Deliver as a PR; do not merge it.

Dependencies: nightly toolchain (`rustup toolchain install nightly`); `mise.toml` keeps stable 1.98.1 as the build toolchain. `cargo-fuzz` via `cargo install cargo-fuzz` or `"cargo:cargo-fuzz"` in `mise.toml`, recorded in `toolchain.md`. libFuzzer runs on macOS and Linux only. Independent of T315.

Check: `cargo +nightly fuzz build` succeeds for every target; each target runs 60 s with no crash (or the crash is filed with a repro test); `just check` on stable does not compile `fuzz/`.

### T315. Emoji and colour on by default for human-facing output

Ivan, 2026-09-30: emoji and colour on by default, with a config toggle, for human-facing output only. Agent-facing and JSON output stay clean. Colour is off outside a TTY and with `NO_COLOR`. Already in progress. Branch `feat/emoji-color-output` (worktree `_worktrees/rtok-emoji-color`) has uncommitted work and is not yet pushed to `pyrlyn/rtok`; no PR exists yet. Link the PR here when it opens.

Plan:
1. One style table (`src/ui/style.rs`): each line kind (success, info, warning, error) picks its emoji and colour together, and clap's `--help` / error styles live in the same place.
2. Config: `[ui] emoji = true` and `[ui] color = true` in `config/default.toml` and `docs/config.md`. `color = false` turns colour off process-wide (owo-colors override), so `render.rs` diffs, state words and log levels follow the same key. Re-bless the trycmd config snapshots.
3. Where it applies: human-facing lines only (status, summaries, `doctor`, `agents` output, `--help`). It never applies to what agents read: `rtok hook` stdout, MCP responses, the proxy, filtered command output from `rtok run` / `rtok filter`, `--json`, or anything written to a pipe or a file.
4. Gates: an emoji needs `[ui] emoji` and a terminal on that stream. Colour needs `[ui] color` and owo-colors' `if_supports_color` answer for that stream: a TTY, `NO_COLOR` unset, `TERM` not `dumb`, or `CLICOLOR_FORCE` / `FORCE_COLOR` set.
5. Deliver as a PR; do not merge it.

Dependencies: `owo-colors` 4 (`supports-colors`) is already a dependency (T20.2); no new crate. Independent of T314.

Check: `tests/ui_style.rs` covers each line kind with emoji and colour on and off; `rtok hook`, MCP and `--json` output contain no ANSI escapes or emoji with both keys on and `CLICOLOR_FORCE=1`; piped output and `NO_COLOR=1` output have no colour; `just check`.

### T317. `rtok agents junk list` and `clear`: per-agent junk with folders, sizes and space freed

Ivan, 2026-10-01: one command group to see and clean junk for every agent: `rtok agents junk list` to view and `rtok agents junk clear` to remove. For each agent, show its folders (as links), the size of each folder in KB/MB/GB, and how much space a clear would free. `clear` stays a dry run by default and deletes only with `--yes`, as T182's `rtok agents junk clear` does today.

Today `rtok agents junk clear` (T182, #286) only clears junk rtok itself owns under its home (rotated `rtok.log.<N>` siblings past `[log] files`, archive payloads past `core.retain_calls_days`). The per-host junk map is research only (`research.md` §22); T182.1 (wire the host folders after review) never landed. There is no `list`, no per-agent view and no sizes.

#### Terms

- **Agent:** each host in `agents::HOSTS` (Claude Code, Cursor, Codex, Gemini, Kimi, ...), plus **rtok** itself as its own row. Where rtok knows agent ids (T282, D34), sessions and worktrees are attributed to the agent id under its host.
- **Agent folder:** a directory an agent writes to: its config/data home (for example `~/.claude`, `~/.cursor`, `~/.codex`), its cache dir (`~/Library/Caches/<app>`, `$XDG_CACHE_HOME/<app>`), its log dir, and per-project folders it creates (for example `<repo>/.claude/`), plus worktrees rtok created for it (T285).
- **Junk:** files that can be deleted without losing user data, settings, credentials or history the user wants, because they are regenerated, re-downloaded or were only temporary. Each junk kind below has a **class**: `safe` (always regenerated, cleared by default), `review` (usually junk, cleared only when named or with `--include review`), or `never` (shown for size only, never deleted).

#### Junk kinds per agent

| Kind | Examples | Class | Notes |
| --- | --- | --- | --- |
| `cache` | HTTP/model/response caches, `Cache/`, `CachedData/`, `GPUCache/`, `Code Cache/` | safe | Regenerated on next run. |
| `temp` | temp files and temp directories the agent created (`tmp/`, `*.tmp`, `$TMPDIR/<agent>-*`) | safe | Only entries older than 24 h and not open by a running process. |
| `logs` | agent log files that are not tied to a session (`logs/*.log`, rotated logs) | review | Kept for the last `[agents.junk] keep_sessions_days` (default 30). Session transcripts are the `sessions` kind below. |
| `build` | build artifacts in agent worktrees and scratch dirs (`target/`, `dist/`, `build/`, `.next/`, `__pycache__/`) | safe | Only under agent-owned worktrees or scratch dirs, never in the user's main checkout. |
| `deps` | reinstallable dependencies (`node_modules/`, `.venv/`, `vendor/` with a lockfile, `.gradle/`, Pods) | review | Only in agent-owned worktrees/scratch dirs; requires a lockfile or manifest next to it so it can be reinstalled. |
| `locks` | stale lock files (`*.lock` for agent state, `LOCK`, `.lock` dirs) | safe | Only when no process holds them (checked with the OS); package-manager lockfiles (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`) are never junk. |
| `backups` | backup files (`*.bak`, `*.bak-<ts>`, `*~`, `_backup/` generations past the cap, T249) | review | The newest backup of each file is always kept. |
| `swap` | editor/agent swap files (`*.swp`, `*.swo`, `.#*`, `*.crswap`) | safe | Only when the owning process is gone. |
| `index` | rebuildable indexes (rtok graph/tags index of a removed project, LSP caches like `.rust-analyzer/`, agent codebase indexes, SQLite `-wal`/`-shm` of closed DBs) | review | Rebuilt on next use; the active project's index is never cleared while rtok is running on it. |
| `rtok-own` | today's T182 junk: rotated logs past cap, archives past retention | safe | Existing behaviour, unchanged. |

Paths for each host come from `research.md` §22 (official docs or source only). A cell §22 marks "not documented" is not scanned; `list` says "not documented" for that kind rather than guessing.

#### Never touched

Settings, credentials and tokens, MCP and hook config, installed plugins and extensions, user-written files (rules, memories, prompts, skills), the latest session of each project, `rtok.db`, any archive a call still references, the user's main checkouts, package-manager lockfiles, and anything outside the paths listed per host. Symlinks are never followed out of an agent folder.

#### Cache: rtok's own and each agent's

Cache is junk for rtok itself and for every agent, listed with its size and clearable like any other kind.

**rtok's cache (row `rtok`):**

- The per-project LSP state rtok confines language servers to (`<root>/.rtok-lsp-xdg/{cache,data,state,pub-cache}`, `src/plugins/graph/lsp.rs`): `cache` and `pub-cache` are `cache` kind (safe); `data` and `state` are `index` kind (review).
- Graph/tags index rows and files for projects no longer in the registry (T316) or whose root is gone: `index` kind (review).
- Any other directory under rtok's home or `$XDG_CACHE_HOME/rtok` (`~/Library/Caches/rtok` on macOS, `%LOCALAPPDATA%\rtok\cache` on Windows) that rtok writes as a cache, and every directory under rtok-owned paths carrying a valid `CACHEDIR.TAG` (the same test `worktree::list::is_cache_dir` uses).
- Plugin download/staging caches (version-numbered plugin copies a host no longer points to, T279): `cache` kind (safe) once no host config references them.

**Each agent's cache:**

- Detected from, in order: (1) the host's entry in `research.md` §22 (documented cache dirs only); (2) the platform cache root for that app (`~/Library/Caches/<bundle id or name>`, `$XDG_CACHE_HOME/<app>`, `%LOCALAPPDATA%\<app>\Cache`); (3) well-known Electron/Chromium cache subfolders inside the app's data dir (`Cache`, `Code Cache`, `GPUCache`, `CachedData`, `DawnCache`, `Service Worker/CacheStorage`), only when §22 confirms the host is Electron-based; (4) any directory under the agent's folders carrying a valid `CACHEDIR.TAG`; (5) `[agents.junk] extra` entries with `kind = "cache"`.
- Not cache even if the name says so: anything §22 marks as settings or state, extension/plugin install dirs, and model weights the user downloaded on purpose (listed as `never`, size only).
- Environment overrides are honoured (`XDG_CACHE_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and the like listed in §22), so a relocated cache is still found.

**Sizes.** Disk usage per cache directory (allocated blocks), hard links and APFS clones counted once across the whole report, symlinks not followed. Shown per cache folder and summed per agent; the same numbers feed "Freed by `clear`".

**Clearing.** `cache` is `safe`, so `rtok agents junk clear` (dry run) lists every cache folder and its size in the plan, and `--yes` deletes their contents, keeping the top folder (so apps that expect it to exist don't fail) and keeping any `CACHEDIR.TAG`. `--kind cache` clears only caches; `--agent rtok --kind cache` clears only rtok's. A running agent's cache is skipped unless §22 says that host tolerates it (reason "agent running").

**Cache edge cases:** a cache dir shared between two agents (for example a common Electron runtime cache) is shown under both with a "shared with" note, counted once, and cleared once; a cache dir that is a symlink to another volume is listed by its target with a note and cleared only inside the target; a cache that refills during the clear (app running despite detection) is reported as "freed X, now Y"; a `CACHEDIR.TAG` with the wrong signature does not make a folder cache.

#### More junk kinds

| Kind | What | Class | Detection | Kept |
| --- | --- | --- | --- | --- |
| `sessions` | old sessions and their logs: transcripts (`*.jsonl`), per-session log files, per-session attachments and tool output dirs | review | §22 session paths per host; rtok's own session rows and logs keyed by session id (T284) | the newest session per project, anything used within `keep_sessions_days` (default 30), and sessions of a running agent |
| `stale-worktrees` | temporary worktrees an agent created and did not finish: rtok-tagged worktrees (T285) or host-created ones (T289) whose agent session ended, idle longer than `stale_worktree_days` (default 14) | review | `worktree::inventory` (T150) plus agent attribution | worktrees with uncommitted changes or unpushed commits are listed but **never** removed here; locked worktrees are never removed |
| `stale-tokens` | auth/token cache files that hold only expired or revoked tokens (OAuth caches, `auth.json`-style files that contain only a token and its expiry) | review, explicit only | file is a token-only cache per §22 and the stored expiry is in the past (or the refresh token is marked invalid by the host's own format); rtok never calls a network to check | configs that mix tokens with settings are **never** edited or deleted: listed as "stale token inside config, sign in again in <host> to refresh" with size 0 |
| `crash-dumps` | dumps and tracebacks from crashed runs: `*.dmp`, `*.crash`, `*.ips` for the agent's binary, Crashpad/Breakpad `Crashpad/completed`, `pending` folders, panic logs, core files named for the agent | safe after 7 days, review before | §22 crash paths; macOS `~/Library/Logs/DiagnosticReports/<app>*`; Linux `$XDG_STATE_HOME`/app crash dirs (not system `/var/crash`, which needs root) | dumps newer than 7 days stay unless named with `--kind crash-dumps` |
| `snapshots` | old agent state snapshots if the host keeps them: checkpoint/undo snapshots, conversation state backups, shadow git repos for checkpoints | review | §22 snapshot paths per host; not scanned for hosts §22 marks "not documented" | the newest snapshot per project and anything newer than `keep_snapshots_days` (default 14) |

Every kind in this table follows the same rules as the rest: listed per agent with folder links and sizes, included in "Freed with `--include review`" (or "Freed by `clear`" for safe ones), deleted only by `clear --yes`.

Extra edge cases for these kinds:

- `sessions`: a session file still being appended to (modified in the last 10 minutes, or open by a process) is never removed; a session whose project folder is gone is still listed under "(project missing)" with its path.
- `stale-worktrees`: removal uses `git worktree remove` (never `rm -rf` on a checkout), then `git worktree prune`; the branch is kept (branch deletion stays with `rtok worktree gc`, T153); a worktree whose main repo is missing is listed as "orphaned" and removed only by deleting its folder with `--include review`, after showing its path.
- `stale-tokens`: removing a token file signs the user out of that host; `list` says so on the row, and `clear` prints it again before the confirmation line. It is excluded from `--include review` and needs `--kind stale-tokens` by name.
- `crash-dumps`: dumps can contain memory with secrets; they are never uploaded, copied or printed, only sized and deleted.
- `snapshots`: hosts that use snapshots for "undo" lose undo for cleared sessions; the row says so.

#### Item-level breakdown in `list`

`list` shows not just totals but every concrete item that `clear` would delete, so the user can verify the exact set before running `clear --yes`.

- Under each agent and each kind, the items are listed one per line: a link to the item (OSC 8 `file://` hyperlink in a capable terminal, plain path otherwise), its size, and, for anything time-stamped, **last used** (relative and absolute, for example `3 weeks ago (2026-09-09 14:02)`).
- "Last used" means, in order of preference: the host's own last-activity time when its format records one (session `updated_at`, last message timestamp); otherwise the newest modification time of any file inside the item; atime is not used (often disabled). For worktrees: the last commit time or the newest file change, whichever is later, and the owning agent id.
- Each item also shows why it is junk (for example "older than 30 days", "agent session ended 16 days ago", "token expired 2026-08-30", "crash dump 12 days old") and, if it is not going to be cleared, why ("newest session for this project", "uncommitted changes", "agent running", "review kind: add --include review").
- Items are sorted by size, largest first (`--sort size|last-used|path`). To keep the default output readable, each kind shows its 10 largest items and a "+N more (X)" line; `--items all` shows every item, `--items 0` shows totals only.
- `--json` always includes every item: `{ path, size_bytes, last_used, reason, will_clear, skip_reason? }`.
- `clear` (dry run) prints the same item list as `list` restricted to what it would delete, so what the user reviewed is exactly what `--yes` removes. `clear --yes` re-scans and refuses to delete an item that changed since the dry run in a way that would change the decision (became recent, gained uncommitted changes, became open by a process) and reports it as skipped.
- `rtok agents junk list --agent claude --kind sessions --items all` is the way to see every session with its last-used time.

Breakdown edge cases: a kind with thousands of tiny items (temp files) is grouped by parent folder in text output ("1,240 files in /…/tmp, 380 MB, last used 2 days ago") while JSON keeps every file; an item whose last-used time cannot be read shows "unknown" and is treated as recent (not cleared) unless `--older-than` was given and the folder mtime satisfies it; times are shown in the local time zone; paths under the home directory are shown with `~`.

#### `rtok agents junk list`

- Scans every agent (or `--agent <host|id>`, repeatable) and prints, per agent: the agent name, each agent folder with a clickable link (OSC 8 hyperlink `file://` in a terminal that supports it, the plain path otherwise), the folder's total size, and under it each junk kind with its size and item count, then a line "Freed by `clear`: X" (safe kinds only) and "Freed with `--include review`: Y".
- Sizes are shown in human units with one decimal (B, KB, MB, GB, TB; 1 KB = 1024 B, labelled as such in `--help`) and right-aligned; `--bytes` prints exact bytes. Size means disk usage (allocated blocks), not apparent size, so sparse files and APFS clones are not over-counted; hard links are counted once.
- Ends with a total across agents: folder sizes, junk by kind, space freed by default and with review kinds.
- Sorting: by space freed, largest first (`--sort name|size|freed`). `--kind <kind>` (repeatable) filters kinds. `--min-size 10MB` hides smaller rows.
- `--json` prints the same data: `agents[] { name, id?, folders[] { path, size_bytes, kinds[] { kind, class, size_bytes, items, paths_sample[] } }, freed_default_bytes, freed_review_bytes }`, `totals`.
- Agents that are not installed are skipped; `--all` lists them with "not installed".
- Read-only: `list` never deletes, moves or touches files (no atime updates where the OS allows avoiding them).

#### `rtok agents junk clear`

- Same scan as `list`, then removes. Default is a **dry run**: it prints exactly what would be deleted (per agent, per kind, sizes, the space that would be freed) and changes nothing. `--yes` deletes. `--json` works for both.
- By default only `safe` kinds are cleared. `--include review` adds review kinds; `--kind <kind>` limits to named kinds (a named review kind is included without `--include review`). `never` kinds are never deleted.
- `--agent <host|id>` limits to one or more agents; `--older-than 7d` applies an age floor to every kind.
- Before deleting each item, rtok re-checks it (still exists, still matches the kind, not open by a process, not a symlink pointing outside, not modified in the last minute). Anything that fails the check is skipped and reported.
- Deletion goes to the OS trash when `--trash` is given (macOS Trash, freedesktop trash on Linux, Recycle Bin on Windows); otherwise it is a direct delete.
- Output after `--yes`: per agent and kind, items removed, bytes freed, items skipped with reasons, and a total "Freed X of Y planned".
- Exit codes: 0 when everything planned was removed (or on a dry run), 1 when some items could not be removed (as T182 does today: "some junk could not be removed"), 2 on usage errors.
- Backwards compatible: `rtok agents junk clear` with no new flags still clears T182's `rtok-own` junk, and now also the safe kinds for every agent; `rtok agents junk clear --agent rtok` reproduces T182 exactly.

#### Running agents

- An agent whose process is running is detected (process list per host binary, plus live rtok sessions from T284). For a running agent, `clear` skips `temp`, `locks`, `swap`, `index` and the current session's logs, and says so; caches are still cleared only if the host's §22 entry says it tolerates that while running, otherwise skipped with "agent running".
- `--force-running` is not offered; the user closes the agent and runs `clear` again.

#### Config

`[agents.junk] keep_sessions_days = 30`, `keep_snapshots_days = 14`, `stale_worktree_days = 14`, `crash_dump_min_age_days = 7`, `temp_min_age_hours = 24`, `exclude = []` (glob paths never touched), `extra = []` (extra paths per host to treat as a kind, for example `{ host = "cursor", kind = "cache", path = "~/Library/Application Support/Cursor/CachedData" }`), documented in `docs/config.md`.

#### Edge cases and expected results

- A folder that does not exist: not listed. A folder without read permission: listed with "permission denied" and no size; `clear` skips it.
- A path on another volume or network share: listed; scanning is bounded (`--timeout`, default 30 s per agent) and a timeout is reported, not hung on.
- Very large trees (a 2 GB `node_modules`): scanning is parallel and streams progress in a TTY; `--json` waits and prints once.
- A file deleted between scan and delete: counted as skipped "already gone", not an error.
- The same folder reached from two agents (shared cache): listed under each with a "shared with" note and counted once in the totals; `clear` removes it once.
- Windows: paths use `%LOCALAPPDATA%`/`%APPDATA%`; files locked by a process are skipped with the reason.
- macOS: `~/Library/Caches` entries are treated as `cache`; nothing under `~/Library/Application Support/<app>` is cleared unless §22 lists that subfolder as junk.
- A worktree rtok created for an agent that still has unmerged commits: its `build` and `deps` can be cleared, the worktree itself is never removed here (that is `rtok worktree gc`, T153).
- `--yes` with nothing to clear: prints "Nothing to clear" and exits 0.

#### Also

- `rtok doctor` adds one line with total reclaimable space and a hint to run `rtok agents junk list` when it exceeds 1 GB.
- The web UI gets the same data on a `junk` card on the hosts page (read-only list with sizes and a "clear safe junk" button that runs the dry run, shows it and asks for confirmation), after T310.8.
- Docs: `docs/agents.md` (new "Junk" section), `docs/config.md`, and `research.md` §22 updated with any new paths, with `docs/ru/` and `docs/uk/` updated in the same change.
- Deliver as a PR; do not merge it.

Dependencies: T182 (existing `clear` and §22 map), T249 (backup generations), T282/T284 (agent ids and live sessions), T285/T153 (agent worktrees).

Check (fixture home under a temp dir, `HOME`/`XDG_*`/`LOCALAPPDATA` pointed at it, no real agent folders touched):

- A fixture with Claude Code, Cursor and Codex folders containing every junk kind: `list` shows each agent, its folders as links, each kind with the exact sizes created (checked in bytes with `--bytes` and in human units), "Freed by `clear`" equal to the sum of safe kinds, and totals equal to the per-agent sums; `--json` matches the table.
- `clear` without `--yes` changes no file (tree hash before equals after) and prints the same plan as `list`'s freed lines.
- `clear --yes` removes exactly the safe items; settings, credentials, plugins, the latest session per project, package-manager lockfiles and `rtok.db` are untouched; freed bytes match the plan.
- `--include review` also removes old session logs (older than `keep_sessions_days`), `node_modules` with a lockfile, extra backups (newest kept) and rebuildable indexes; `node_modules` without a lockfile is not removed.
- A lock file held by a test process and a swap file of a live process are skipped with reasons; a symlink inside a cache dir pointing to `$HOME/important` is not followed.
- A shared folder is counted once in totals and removed once.
- A simulated running agent skips temp, locks, swap, index and its current session.
- Cache: rtok's `.rtok-lsp-xdg/cache` in a fixture project, a `$XDG_CACHE_HOME/rtok` folder and a `CACHEDIR.TAG` dir appear under `rtok` with exact sizes; a host cache in `~/Library/Caches/<app>` (or `$XDG_CACHE_HOME/<app>`) and an Electron `Code Cache` appear under that agent; `clear --kind cache` dry run lists them, `--yes` empties them and keeps the top folders and `CACHEDIR.TAG`; a folder with a bad `CACHEDIR.TAG` signature is not treated as cache.
- New kinds: a 40-day-old session is listed with its last-used time and removed with `--include review`, the newest session per project and a session modified 5 minutes ago are kept; a finished-session worktree idle 20 days is removed with `git worktree remove` and its branch kept, while one with uncommitted changes and one with unpushed commits are listed and never removed; a token-only file with a past expiry is listed with the sign-out warning and removed only with `--kind stale-tokens`, a mixed config with an expired token is never touched; a 10-day-old crash dump is cleared by default, a 2-day-old one only with `--kind crash-dumps`; an old snapshot is removed and the newest kept.
- Breakdown: every planned item appears in `list` with path link, size, last used and reason; `--items all` and `--json` list every item; the default shows 10 per kind plus "+N more"; `clear` dry run prints the same items; touching an item between the dry run and `--yes` makes `--yes` skip it with "changed since plan".
- `rtok agents junk clear --agent rtok --yes` behaves exactly as T182's tests expect (existing tests stay green unchanged).
- Permission-denied and timeout folders are reported, not fatal; exit code 1 when anything planned was not removed.
- `--trash` moves items to the platform trash (tested on macOS and Linux CI).
- `just check`.

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
| D33 | **rtok's MCP lives in each agent's own config, not in its plugins (T275, amends D21 for MCP).** Install and update always write the config entry `rtok`; only `remove` takes it out, and a plugin no longer suppresses or strips it. Where an agent would show a plugin server next to the config entry (Claude Code and Desktop, Cursor, Copilot, Codex, VS Code, ZCode, Kimi, Grok; `research.md` §25), the rtok plugin ships no MCP server and keeps its hooks, skills and agents. Gemini keeps both, since settings.json wins over an extension's same-name server. Same-name entries across one agent's files are left to the agent to merge. Hooks keep D21 unchanged. |
| D34 | **rtok gives every agent session its own id and owns its worktrees the same way on every host (T281–T290, creator request 2026-09-27).** The agent id is a random UUIDv4 issued by rtok per host session (sub-agents get their own, with a parent), shown as its first 8 hex chars; any unique prefix of 4+ chars is accepted. Not UUIDv7: its leading hex is a timestamp, so agents started within the same minute would share the short id (found 2026-09-27; `started_at` keeps the order). The host's session id is kept alongside but never used as the identity: it collides across hosts and is missing on several (`research.md` §26). A worktree is bound to one agent by the git lock reason `<owner> \| <task-id> \| <date> \| agent <uuid>` (the old 3-field form stays valid) and a store row; the lock is the source of truth. Every host gets the same root, naming, lock, list, remove and gc: Claude Code redirects its own worktrees through `WorktreeCreate`/`WorktreeRemove` (T159), hosts with a post-create script adopt theirs (T289), all others use the skill and the MCP tools. Messages between agents and from the user are local, capped, framed as information from another agent and never as instructions. |

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

