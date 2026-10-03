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
| T281 | in progress | P1 | 3 | 70% | Claude Code / claude-opus-5-5 |
| T283 | in progress | P1 | 3 | 60% | Claude Code / sonnet-5 |
| T283.2 | todo | P1 | 2 | 0% | |
| T283.3 | todo | P1 | 3 | 0% | |
| T284 | in progress | P1 | 3 | 50% | Claude Code / claude-opus-5-5 |
| T286 | in progress | P1 | 3 | 40% | Claude Code / claude-opus-5-5 |
| T287 | in progress | P1 | 4 | 50% | Claude Code / claude-opus-5-5 |
| T288 | in progress | P2 | 3 | 40% | Claude Code / claude-opus-5-5 |
| T289 | in progress | P2 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T290 | todo | P1 | 3 | 0% | |
| T310 | todo | P1 | 5 | 0% | |
| T310.7 | todo | P2 | 3 | 0% | |
| T310.9 | todo | P1 | 4 | 0% | |
| T310.10 | todo | P1 | 3 | 0% | |
| T310.11 | todo | P2 | 3 | 0% | |
| T310.12 | todo | P2 | 3 | 0% | |
| T329 | todo | P2 | 5 | 0% | |
| T329.4 | todo | P2 | 4 | 0% | |
| T329.5 | todo | P2 | 3 | 0% | |
| T329.6 | todo | P2 | 3 | 0% | |
| T329.7 | todo | P2 | 4 | 0% | |
| T329.8 | todo | P2 | 3 | 0% | |
| T329.9 | todo | P2 | 4 | 0% | |
| T329.10 | todo | P3 | 3 | 0% | |
| T329.11 | todo | P2 | 3 | 0% | |
| T329.13 | todo | P2 | 4 | 0% | |
| T329.14 | todo | P2 | 4 | 0% | |
| T329.15 | todo | P3 | 5 | 0% | |
| T329.16 | todo | P3 | 3 | 0% | |
| T329.17 | todo | P3 | 3 | 0% | |
| T329.18 | todo | P3 | 4 | 0% | |
| T329.19 | todo | P3 | 3 | 0% | |
| T329.21 | todo | P3 | 2 | 0% | |
| T330 | todo | P2 | 4 | 0% | |
| T330.2 | todo | P2 | 3 | 0% | |
| T330.3 | todo | P2 | 3 | 0% | |
| T330.4 | todo | P2 | 4 | 0% | |
| T330.5 | todo | P2 | 4 | 0% | |
| T330.6 | todo | P3 | 3 | 0% | |
| T331 | todo | P1 | 4 | 0% | |
| T332 | todo | research | 1 | 0% | |
| T333 | todo | research | 1 | 0% | |
| T334 | todo | research | 1 | 0% | |
| T335 | todo | research | 1 | 0% | |
| T336 | todo | research | 1 | 0% | |
| T337 | todo | research | 1 | 0% | |
| T338 | todo | research | 1 | 0% | |
| T339 | todo | research | 1 | 0% | |
| T340 | todo | research | 1 | 0% | |
| T341 | todo | research | 1 | 0% | |
| T342 | todo | research | 1 | 0% | |
| T343 | todo | research | 1 | 0% | |
| T344 | todo | research | 1 | 0% | |
| T345 | todo | research | 1 | 0% | |
| T346 | todo | research | 1 | 0% | |
| T359 | todo | P1 | 2 | 0% | |
| T361 | todo | P2 | 1 | 0% | |
| T366 | todo | P3 | 1 | 0% | |
| T367 | todo | P3 | 1 | 0% | |
| T347 | todo | research | 1 | 0% | |
| T348 | todo | research | 1 | 0% | |
| T356 | in progress | P1 | 2 | 5% | Claude Code / claude-opus-5-5 |
| T358 | todo | P2 | 4 | 0% | |
| T358.3 | todo | P2 | 4 | 0% | |
| T358.4 | todo | P2 | 3 | 0% | |
| T358.5 | todo | P2 | 3 | 0% | |



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
Progress (2026-10-03): PR 2 is split into T283.1 (resolve the link, MCP `whoami`, `rtok mcp --host`), T283.2 (`--host` in every host's MCP entry) and T283.3 (the ancestor-pid rule, which needs the hook wire request to carry a pid). The link rule is derived from `research.md` §26's vendor docs and spawn code; the T281 live probe only confirms it.

### T283.2. `--host <id>` in every host's MCP entry

PR 2 of T283, part 2 (after T283.1 and T275's per-host entries). Every host's `register_mcp` passes `--host <id>` to `rtok mcp`, so the process knows its host without `[hook] host`; the host tests and fixtures change with it. Only our entry changes, the rest of the host's file stays byte-for-byte.

Check: each host's install/remove test shows `mcp --host <id>` in the written entry and removal leaves the file as before; `tests/host_docs.rs`, `tests/agents_doc.rs` regenerated where tables change; `just check`.

### T283.3. MCP link rule (b): the nearest common host ancestor pid

PR 2 of T283, part 3. The hook wire request (`crates/rtok-hook`, `src/hooks/resident.rs`: `version, fingerprint, event, host, cwd, stdin`) carries no pid, and the resident hook process is not the host's child, so a hook cannot record its own ancestry today. Add the client's parent pid to the request (protocol version bump), store it on the agent row (migration), record it on registration, and let `link.rs` match it against the `rtok mcp` process's ancestor chain (nearest first; two agents behind one ancestor are ambiguous). Doc-derived like the rest of the rule order; the T281 probe confirms it per host.

Check: wire round-trip test; store test; `link.rs` test with a seeded agent row and a fake ancestor chain; hook latency stays inside the 10 ms budget; `just check`.

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

### T310.7. Pages: sessions, doctor, logs

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

### T329. Graph page: project selector, auto-added projects and linked projects (epic)

Ivan, 2026-10-01: in the web UI's graph tab, the graph is built for a project the user picks. The page always shows which project is selected. Projects the user needs are added automatically. Other projects can be linked to the selected one, and the graph then traverses into them as if everything were one project. If the selected project references other projects, those are added, indexed and linked automatically, so an agent working in the current project can follow the graph across them right away.

Today the graph plugin (`src/plugins/graph/`) always works on one root: the process's current directory. The index is keyed by that root (`index::canon(root)` in `src/store/symbols.rs`), and the MCP tools `symbol`, `callers`, `impact`, `outline` and `explore`, plus `dead` and `affected`, only see that root. The graph page shows the same single root (`root .`). There is no way to pick another project and no way to follow a call into a dependency's source.

Split (2026-10-03, complexity 5): one subtask = one PR, T329.1 to T329.21 in dependency order (T329.1 to T329.3, T329.12 and T329.20 are already in `done.md`). This card stays the specification; each subtask reads the section it names and updates `docs/` (en, ru, uk) for its own part. Open questions T334, T336 and T337 gate T329.9, T329.4 and T329.11/T329.17.

#### Terms

- **Project**: a directory rtok indexes as one unit, identified by its canonical root path. Display name defaults to the directory name (or the package name from the manifest when there is one); the user can rename it.
- **Selected project**: the project the graph page (and, by default, the CLI and MCP tools) answers for.
- **Link**: a directed edge "project A sees into project B". A link is either **manual** (the user made it) or **auto** (rtok made it from a reference, see 4).
- **Graph scope**: the selected project plus every project reachable through its links (transitively). All graph queries run over the scope.

#### 1. Project registry

- A `projects` table in the rtok store holds id, canonical root, display name, origin (`manual`, `session`, `worktree`, `mcp`, `reference`), created and last-used times, and per-project index status (rows, files, pending, `indexed_at`, watch state, last error).
- Each project keeps its own symbol index, keyed by its canonical root as today, so switching projects never re-indexes the others and never mixes their rows.
- Two paths that canonicalize to the same directory (symlinks, `..`, case on macOS) are the same project; registering one twice is a no-op that only updates last-used.
- A project whose root no longer exists stays in the registry marked **missing**: it is greyed out in the selector, excluded from the scope, and its links are kept so they come back if the directory returns. The user can remove it.
- Removing a project drops rtok's index rows, its links in both directions and its registry row. It never touches the project's files.
- The registry and links migrate forward with the store schema; an existing store starts with one project, the root it already indexed, selected.

#### 2. Project selector on the graph page

- The page header has a project selector listing every known project: name, root path, index status and a link count. It is searchable when there are more than about ten projects.
- Picking a project switches the whole page to that project's scope: summary counts, dead symbols, the symbol/callers/impact views and the graph drawing.
- The selection is stored in the rtok store, so it survives page reloads, other browser tabs (they update over `/ws`) and `rtok web` restarts.
- When `rtok web` starts in a directory that is a known project and nothing is selected yet, that project is selected. A stored selection that is now missing falls back to the current directory's project, with a notice.

#### 3. Current-project indicator

- The selected project's name and root path are always visible in the page header, together with its index status: rows, files, pending files, last indexed time and watch state.
- When the scope includes linked projects, the header says so ("+ 3 linked") and expands to list them, each with its own status.
- States the page must show clearly: **not indexed yet** (empty state with an "Index now" action), **indexing** (progress, the page stays usable on the old data), **stale** (pending files, same banner the tools already use), **failed** (the error and a retry action), **missing** (root gone).

#### 4. Automatic adding

Projects are added to the registry, without the user asking, in two ways.

**4a. Projects rtok sees in use.** The working directory of a hooked agent session, a worktree created or adopted through `rtok worktree` (T285, T289), and the root of any graph MCP call are registered when first seen. A worktree is registered as its own project (its files differ from the main checkout) with its display name showing the branch.

**4b. Projects the selected project references.** When a project is indexed, rtok reads its manifests and collects references to code that lives outside its root but on this machine. Each referenced directory is registered as a project (origin `reference`), indexed, and auto-linked from the referencing project. Reference sources, in this order:

- Cargo: `path = "..."` dependencies and `[patch]` entries, and workspace members outside the root.
- npm/pnpm/yarn: `file:`, `link:` and `workspace:` dependencies that resolve outside the root.
- Go: `replace` directives with a local path in `go.mod`, and `go.work` `use` entries.
- Python: path dependencies in `pyproject.toml` (`{ path = "..." }`, editable installs).
- Git submodules (`.gitmodules`) whose checkout is present.
- Anything else the indexer finds while resolving imports: an import that resolves to a file outside the root (through an LSP server or the language's resolver) adds that file's project root (nearest directory with a manifest or `.git`).

Rules for 4b:

- References are followed transitively: if B (referenced by A) references C, C is added, indexed and linked from B, so A's scope includes C. A depth limit (`[plugins.graph] reference_depth`, default 3) and a project cap (`max_auto_projects`, default 20) stop runaway chains; hitting either is shown on the page and logged, never silent.
- Registry dependencies that are not local source (crates.io, npm registry, PyPI, Go module cache) are not followed by default, so the scope stays the user's own code. An opt-in setting (`include_registry_deps = false`) can add them later; it is out of scope for the first PR.
- A reference to a path that does not exist is recorded on the referencing project as a warning ("references ../foo, not found") and nothing is added.
- Auto-indexing runs in the background with the existing index code; the selected project is usable while its references are still indexing, and results from a reference that is not indexed yet are marked incomplete rather than missing.
- Re-indexing a project re-reads its manifests: a new reference adds and links a project; a removed reference removes the auto link (the project stays in the registry until the user removes it). Manual links are never removed automatically.
- If the user unlinks an auto link, rtok remembers that and does not re-create it on the next index.

**Turning it off.** `[plugins.graph] auto_add_projects = true` controls 4a and `auto_link_references = true` controls 4b, both on by default, documented in `docs/config.md`. With both off, the registry changes only through the page and the CLI.

#### 5. Manual links

- From the selected project the user can link any known project and unlink any linked one; the page lists links with their kind (manual or auto) and the reason for auto links (for example "Cargo path dependency `../ketch-core`").
- Linking a project that is not indexed yet starts indexing it.
- Links are directional: linking B into A puts B in A's scope, not A in B's. The page offers "link both ways" as a shortcut that creates two links.
- Cycles are allowed (A to B to A). Scope building visits each project once, so cycles never loop or duplicate rows.
- A project cannot link to itself, and linking an already-linked project is a no-op.

#### 6. Cross-project traversal

- Every graph query runs over the scope as one graph: `symbol`, `callers`, `impact` (with `depth` and `to`), `explore`, `affected` and `dead`.
- A reference from a call site in one project to a definition in a linked project resolves and is followed, in both directions: `callers` of a function in B include call sites in A when A links B, and `impact` from a change in B walks up into A.
- Each result row shows which project it belongs to (project name badge on the page, a `project` field in JSON, a `[name]` prefix in text output).
- Ambiguity: when the same symbol name is defined in several projects in the scope, results are grouped by project and marked ambiguous, using the same banner the single-project path already uses. A definition in the selected project ranks first.
- `dead` is computed over the scope: a symbol in B used only from A is not dead while A links B. Dead symbols are still reported per project.
- `affected` with git changes reads `git diff` in every project in the scope that is a git repo, and maps test commands per project.
- Output caps and token budgets apply to the whole scoped answer, not per project, so linking projects does not multiply the size of an MCP reply.
- Watching: when `watch` is on, file changes in any project in the scope update its index and refresh the page over `/ws`.

#### 6a. Backends: LSP by default, then tree-sitter, then plain text search

Today `[plugins.graph] backend` defaults to `tags` (tree-sitter tags index), and `backend = "lsp"` errors out when the server is missing (`docs/lsp.md`, "Without the server"). T329 changes the default to an ordered fallback chain, `backend = "auto"`: LSP first, tree-sitter second, plain text search last. Setting `backend = "lsp"`, `"tags"` or `"text"` pins one mode with no fallback (today's strict behaviour, kept for tests and for users who want it).

Backends are chosen per project and per language, not once per process: in a scope where A is Rust with rust-analyzer installed and B is Go with no server, A's rows come from LSP and B's from tree-sitter, in the same answer.

**Mode 1: LSP (default).**

- Used when a server for the project's language is configured and works: the marker file is found (`Cargo.toml`, `compile_commands.json`, `tsconfig.json`, `pubspec.yaml`, plus any added later) and the server binary is on `PATH` (or resolved through `rustup which rust-analyzer`, as today), spawns over stdio, and answers `initialize`.
- Gives the most precise answers: type-position references, trait/interface implementations, re-exports and macro-expanded calls that tags miss.
- Cross-project traversal uses each project's own server; a definition location the server returns inside a linked project's root is mapped to that project and labelled with it.
- A server that starts but crashes or times out mid-session (default per-request timeout 10 s) marks that project's LSP as failed, answers the current request from the next mode and says so in the result (see "Which mode answered" below). It is not respawned on every request (see 6b).
- Indexing on large projects: while the server is still indexing (`$/progress` not finished), requests wait up to the timeout; on timeout they fall back to tree-sitter for that request only and keep LSP as the working mode.

**Mode 2: tree-sitter (fallback).**

- Used when LSP is not configured or not working for that project, and a tree-sitter grammar for the project's languages is compiled into rtok (the existing tags index in SQLite).
- Answers from the existing per-project tags index: definitions, call sites by name, outlines. It indexes the project on first use if needed, with the usual stale banner for pending files.
- Known limits, stated in the result when they matter: name-based resolution (several definitions with the same name are reported as ambiguous), no type-position references, no macro expansion.
- Cross-project traversal joins the tags indexes of every project in the scope by symbol name, preferring a definition in the selected project, then in directly linked projects, then transitively linked ones.

**Mode 3: plain text search (last resort).**

- Used when neither LSP nor a tree-sitter grammar is available for the project (for example a language rtok has no grammar for).
- Runs plain text search through the shell: `rg` (ripgrep) when present, `grep -rn` otherwise, with word-boundary patterns built from the symbol name and simple per-language definition patterns (`fn name`, `def name`, `function name`, `class name`, `func name`). For a project whose root is on another machine (registered as `ssh://host/path`), the same commands run over `ssh host` with the same arguments; the SSH host must already be reachable without a prompt (key or agent), otherwise the mode is reported as not working.
- Answers are best effort: `symbol` returns matching definition lines, `callers` returns lines that mention the name outside its definition, `outline` returns definition-pattern matches in the file, `impact` is limited to one level, and `dead` is not offered (the page and the tool say "not available in text mode" instead of guessing).
- Every text-mode result says it came from text search and may include false positives (comments, strings, same-named symbols). Output is capped the same way as other modes.
- Respects `.gitignore` and the project's ignore settings; never searches outside the project roots in the scope.

**When no mode works.** If all three fail for a project (no server, no grammar, no `rg`/`grep`, or SSH unreachable), that project is dropped from the answer with one clear line ("project B: no graph backend available: ...") and the other projects still answer. If it is the only project, the tool returns that error.

**Which mode answered.** Every result says which mode answered for each project (page: a small LSP / tree-sitter / text tag next to the project badge; JSON: `backend` per project; text output: one header line). `Measurement` rows keep `kind = "lsp.*"` for LSP and gain `tags.*` and `text.*` kinds, so `rtok stats` shows how often each mode is used.

**Config.** `[plugins.graph] backend = "auto" | "lsp" | "tags" | "text"` (default `auto`), `lsp_timeout_ms = 10000`, and per-language overrides (`[plugins.graph.backend_by_language] go = "tags"`), documented in `docs/config.md` and `docs/lsp.md` (whose "Without the server" section changes to describe the fallback).

#### 6b. Capability cache: check once, reuse until the MCP server restarts

- The first graph request for a project (and language) runs the capability check: find the LSP marker and server binary and try to start it; check for a tree-sitter grammar; check for `rg`/`grep` (and SSH reachability for remote roots). The result is a per-project record such as "LSP works", or "LSP: rust-analyzer not on PATH; tree-sitter works", or "LSP and tree-sitter unavailable; text works".
- Later requests use that record directly: they go straight to the working mode and do not re-probe the modes that failed. No `PATH` lookup, no server spawn attempt and no grammar check runs again on each request.
- The cache lives in memory in the rtok MCP server process (and in the `rtok web` process for the page). It is kept until that process restarts; restarting the MCP server is the way to re-check after installing a language server. It is not written to disk, so a new process always checks fresh.
- A working mode that later breaks (server crash, repeated timeouts) is downgraded in the cache once, and the next mode becomes the cached choice for that project for the rest of the process; it is not re-probed per request.
- Changing `[plugins.graph] backend` or the per-language overrides in config clears the cached record for the affected projects (the config watcher already reloads settings); nothing else invalidates it.
- Adding a new project (manually, by session or by reference) runs the check once for that project only; existing records are untouched.
- `rtok graph projects --json` and the page show each project's cached capability record and when it was checked, so the user can see why a mode was chosen.
- Concurrent first requests for the same project share one check (single-flight); they do not spawn several servers.

#### 7. CLI and MCP

- `rtok graph projects` lists projects, `rtok graph projects add <path>`, `remove <id|path>`, `select <id|path>`, `link <id|path>`, `unlink <id|path>`; all support `--json`.
- Every graph command and every graph MCP tool takes an optional `project` (id or path). Without it, the project is the caller's current directory (agents keep today's behaviour) and the scope includes that project's links, so an agent working in A automatically sees into the projects A references.
- MCP results carry the same `project` field per row as the CLI's JSON.

#### 8. Web UI

- Lands on the React SPA graph page (T310.8): selector, indicator, links panel, project badges in every list and in the graph drawing (one colour per project, with a legend).
- New `/ws` messages: project list, selection changed, links changed, per-project index progress.
- If T310.8 has not landed when the backend is ready, ship the registry, links, references, traversal, CLI, MCP and `/ws` first, and the page with T310.8.

#### 8a. Visual graph: projects overview and drill-down into one project

The graph page draws two levels of graph, both interactive (pan, zoom, drag, click), rendered from data sent over `/ws`.

**Level 1: projects overview (the page's landing view).**

- Header counters: total known projects, projects in the current scope, linked pairs, and projects with problems (missing, failed, no backend).
- A node per project, labelled with its name, sized by indexed symbol count, coloured per project (the same colour used for project badges everywhere), with a small backend tag (LSP / tree-sitter / text) and a state marker (indexing, stale, failed, missing).
- An edge per link, drawn as an arrow from the linking project to the linked one. Manual and auto links look different (solid vs dashed); hovering an auto link shows its reason (for example "Cargo path dependency `../ketch-core`"). Edge thickness reflects the number of cross-project references actually found between the two projects; a link with zero references found is drawn thin and grey with a tooltip saying so.
- The selected project is highlighted and its scope (everything reachable through links) is emphasised; projects outside the scope are dimmed but still shown.
- Interactions: click a node to select it as the current project; double-click (or an "Open" button) to drill into it; right-click or a node menu to link, unlink, re-index or remove; a filter box hides projects by name; a toggle shows only the current scope.
- Edge cases: one project only shows a single node and a hint about linking; cycles are drawn normally (no infinite layout); more than about 50 projects switches to a clustered layout grouped by origin, with a list view fallback; missing projects are drawn hollow and cannot be opened.

**Level 2: inside one project (drill-down).**

- Opening a project shows the relationships inside it as a graph: files, modules, types and functions/methods as nodes; "contains", "calls", "implements" and "imports" as edges. A breadcrumb (`All projects / rtok / src/plugins/graph`) leads back up, and the browser back button works (the drill-down state is in the URL).
- It starts at file/module level (files grouped by directory, edges are aggregated call/import counts between files) so a large project stays readable. Clicking a file expands it into its functions, methods and types; clicking a function focuses on it and shows its callers and callees (depth 1 by default, adjustable up to the same limit `impact` uses).
- Calls that leave the project into a linked project end at a node for that project (in its colour); clicking that node opens the target symbol inside the linked project, so the user can follow a call chain across projects visually, matching what cross-project traversal (6) returns.
- A side panel shows the selected node's details: path and line, signature, callers and callees lists, and "open in editor" (the `vscode://` / `file://` link rtok already uses where available).
- Search: typing a symbol name finds it in the project (and the scope) and focuses it on the graph.
- What the graph shows depends on the backend answering for that project (6a): LSP and tree-sitter give full call and containment edges; text mode shows files and definition-pattern matches only, with a banner saying call edges are not available in text mode.
- Large graphs: nodes beyond a cap (default 500 visible) are collapsed into "+N more" groups that expand on click; layout runs in a web worker so the page never freezes; the page shows a spinner while the graph data streams in.
- Live updates: when `watch` is on and files change, the affected nodes and edges update in place over `/ws` without resetting the layout or the user's zoom.
- Edge cases: an unindexed project shows the "Index now" empty state instead of an empty canvas; a project still indexing shows what is indexed so far, marked partial; dead symbols (when available) can be highlighted with a toggle; a file with parse errors is shown with a warning marker and its known nodes.

**Rendering: 3D with Three.js.**

- Both levels are drawn as a 3D graph in WebGL with Three.js. The preferred stack is `3d-force-graph` / `react-force-graph-3d` (Three.js plus a d3-force-3d layout) or `@react-three/fiber` with `@react-three/drei` if more control is needed; pick one in the first PR and record the choice and bundle size in `toolchain.md`. Library versions are pinned like other SPA dependencies.
- Camera: orbit (rotate, pan, zoom) with mouse, trackpad and touch; double-click a node flies the camera to it; a "reset view" button and a "fit all" button; the camera position is kept when data updates live.
- Nodes are spheres (projects) or smaller shapes per kind inside a project (file: cube, type: octahedron, function/method: sphere), coloured per project, with text labels as sprites that face the camera and hide past a zoom distance so the scene stays readable. Edges are lines with arrowheads (or directional particles for calls) and the same solid/dashed and thickness rules as above.
- Layout runs as a 3D force simulation in a web worker; it settles and then stops (no constant CPU use when idle). Expanding a file or project adds nodes near their parent instead of re-laying out the whole scene.
- Selection, hover tooltips, the side panel, search-to-focus and the right-click menu work the same as described above, using Three.js raycasting for picking.
- A 2D toggle shows the same graph flat (same library in 2D mode, or a 2D canvas renderer) for users who prefer it; the choice is remembered.
- Performance targets: 60 fps orbiting with 500 visible nodes and 2,000 edges on a 2020 laptop's integrated GPU; above the visible cap nodes are grouped (as above). Instanced meshes are used for nodes when counts are high.
- Fallbacks and edge cases: no WebGL (blocked, old browser, headless without GPU) switches to the 2D renderer with a notice; a lost WebGL context is restored automatically or falls back to 2D; `prefers-reduced-motion` disables camera fly-to and particle animation; the keyboard list view stays available in 3D mode; the 3D scene is disposed (geometries, materials, renderer) when leaving the page so memory does not grow when switching tabs.
- Tests: Vitest for the data-to-scene mapping (nodes, edges, colours, grouping) without WebGL; Playwright with software WebGL (SwiftShader) checks the canvas renders, a node click selects it, and the no-WebGL path shows the 2D fallback; Storybook stories for both levels with fixture data.

**Accessibility and themes.** Both levels work in dark and light themes at 375 and 1280 px; every graph has a keyboard-navigable list view with the same data (nodes, edges, counts) for screen readers and small screens; colours are not the only signal (shapes and labels carry the same meaning).

#### 8b. Two-part graph UI: interactive explorer and read-only live graph

The graph page is split into two parts that show the same graph data side by side (stacked on narrow screens).

**Part 1: interactive explorer.** Everything described in 8a: the user clicks, selects, drills down, expands, searches, links and unlinks, and moves the camera. Nothing happening in the background moves this view; it changes only when the user acts (or when indexed data changes under `watch`).

**Part 2: live graph (read-only).** The same graph, rendered with the same layout, colours and shapes, but purely for watching:

- No interaction at all: no click, hover menus, selection, drag, expand, search or link actions; no tooltips that need hovering. Pointer and keyboard events on the canvas are ignored, and the cursor stays the default arrow so it never looks clickable.
- The camera is driven automatically: it frames whatever is being queried right now and eases back to an overview when activity stops. The user cannot move it.
- It follows the level shown in part 1 (projects overview, or the project the user drilled into) so both parts show the same part of the graph; the live graph does not drill down by itself. Queries on symbols outside what part 1 shows light up the nearest visible ancestor with a counter.
- It shows only what the graph is being asked right now and how the data changes: every `symbol`, `callers`, `impact`, `explore`, `outline`, `affected` and `dead` call from MCP, the CLI or part 1 itself.

**Layout controls (outside the canvases).** A splitter between the parts (drag to resize, double-click to reset to 50/50), buttons to maximise either part, and a "Hide live graph" toggle; the choice is remembered. On screens narrower than 900 px the parts stack, live graph below, collapsed to its metrics strip until expanded. These controls are the only things the user operates for part 2; the live canvas itself stays read-only.

**What the live graph shows on the canvas.**

- When a call starts, the queried symbol's node (and its project node on the overview) pulses in an "in progress" colour; when it ends, the nodes in the answer (callers, callees, impact chain, explore hits) flash and the traversed edges animate along the path the query took, including edges crossing into linked projects.
- Each running call gets a small floating label next to its node with its tool name and a live counter of symbols returned so far; the label fades a few seconds after the call ends.
- Nodes queried often build up a heat glow that decays over a window (default 5 minutes), so hot spots are visible at a glance.
- Several concurrent calls are shown at once, each in its own accent so their paths can be told apart; more than 8 concurrent calls are merged into one "busy" pulse with a count.

**Live metric displays (read-only, updating in real time).** Arranged as a strip above the live canvas and a feed beside it; every number updates as events arrive, with a short count-up animation (disabled under `prefers-reduced-motion`).

- **Now running:** count of calls in progress, and for each: tool, symbol or query, caller (agent id and host from T283/T284, or "web" / "cli"), project, backend answering (LSP / tree-sitter / text), elapsed time ticking up.
- **Symbols requested:** for the current call, the size of its target set (for example the symbols an `impact` at depth 3 expands to); for the window, the running total. Shown as a number with a sparkline of the last 60 s.
- **Symbols returned:** same layout; the per-call value counts up while the answer streams; the ratio returned/requested is shown as a small bar.
- **Tokens sent / tokens without rtok / saved:** for the last call and for the window: answer size, the size of the unreduced answer (what a plain read or grep of the same data would have returned), and the saving in tokens and percent, shown as a large number with a sparkline. Values come from the same `Measurement` rows `rtok stats` uses, so they match `rtok stats` exactly.
- **Latency:** last call, p50 and p95 for the window, as numbers with a sparkline.
- **Files touched and projects crossed:** per call and window totals.
- **Per-tool breakdown:** a live bar per tool (`callers`, `impact`, ...) with call counts and tokens saved in the window.
- **Backend use:** live shares of LSP / tree-sitter / text answers and the number of fallbacks in the window.
- **Cache and caps:** how many answers were cut by a cap and how many fell back, as live counters.
- **Call feed:** newest first, one row per finished call with tool, symbol, caller, project, backend, requested, returned, tokens saved and latency; failed calls in red with the error; interrupted calls marked as such. The feed scrolls by itself and keeps the last 200 rows; it is read-only like the canvas (no click to replay), but it can be filtered by agent, tool and project with controls above it.
- **Window selector:** totals cover the last 1, 5 or 15 minutes, or "since `rtok web` started"; changing it recomputes from the store, not from what the browser happened to receive.
- **Freeze button:** stops the live canvas and the displays updating so a moment can be read; events keep arriving in the background and the view catches up on unfreeze. Totals never drop events.

**Data path.**

- The graph plugin emits a start event and an end event per call (with the numbers above, and a progress event while a long answer streams) on the existing `/ws` stream. The page subscribes only while part 2 is visible, so a hidden or collapsed live graph costs nothing.
- Events from the MCP server process, CLI runs and `rtok web` all reach the page through the store (or the daemon channel the web UI already uses), so an agent's call in another process shows up within one second.
- Payloads carry ids, symbol names, paths and numbers, never source text.
- Rendering is batched per animation frame; a burst (for example 200 calls per second) is coalesced for display, while counters and totals still count every call.

**Edge cases.**

- No activity yet: the live graph shows the static graph dimmed and "Waiting for graph calls"; the displays show zeros, not blanks.
- A failing call (no backend, timeout) pulses red on its node and appears red in the feed with the error.
- A call still running when its process exits is marked "interrupted" after a timeout and stops counting as running.
- Calls on a project outside the current scope are counted in the totals and listed in the feed (marked "outside scope") but do not light up the canvas.
- `/ws` drops: the live part shows "reconnecting", then resumes; missed events are shown as a count and the totals are refreshed from the store.
- Part 1 drills into a project while calls are running: the live graph switches level with it and re-attaches running calls to the new view.
- Several browser tabs: each live graph receives the stream; closing or hiding them stops the subscription.
- WebGL unavailable: the live graph uses the same 2D fallback as part 1; the metric displays do not depend on WebGL.
- Live events stay local to `rtok web` (localhost by default); nothing leaves the machine.

**Config.** `[plugins.graph] live_heat_window_s = 300`, `live_max_events_per_s = 50` (rendering cap only), `live_feed_rows = 200`, documented in `docs/config.md`.

#### 8c. Export: graph as an image or JSON

- **What can be exported:** the projects overview, the current drill-down view, or a focused subgraph (a symbol with its callers/callees/impact at the chosen depth), from part 1. The live graph (part 2) can export a snapshot of its current frame as an image only.
- **Formats:**
  - PNG at 1x/2x/4x, transparent or theme background, with a legend (project colours, node shapes, edge styles) and a footer (project names, scope, backend per project, `indexed_at`, rtok version, export time).
  - SVG for the 2D rendering (vector, editable); in 3D mode SVG exports the current camera projection flattened to 2D.
  - JSON, versioned schema (`"schema": "rtok.graph.v1"`): `projects` (id, name, root as a path relative to the user's home or redacted, origin, backend, health), `links` (from, to, kind, reason, reference count), `nodes` (id, project, kind, name, path, line), `edges` (from, to, kind), and `meta` (scope, level, focus, depth, filters, export time, rtok version). The schema is documented in `docs/plugins.md` and checked by a JSON Schema file in the repo.
- **Where:** an "Export" menu on the page; `rtok graph export --format png|svg|json [--project ID] [--focus SYMBOL --depth N] [-o FILE]`; and an MCP tool `graph_export` (JSON only) so an agent can hand a graph to another agent or attach it to a PR.
- **Sharing safety:** absolute paths, the home directory and the user name are redacted by default (`--no-redact` to keep them); source text is never included; file names and symbol names are, and the export dialog says so.
- **Edge cases:** an export larger than the visible cap includes every node in JSON but only the visible ones in images, and the image footer says "N nodes hidden"; exporting while indexing marks the export partial in `meta` and the footer; text-mode projects export without call edges and say so; images render offscreen at the requested size, not a screenshot of the window, so the result does not depend on window size; no WebGL means PNG comes from the 2D renderer.
- **Import (read-only):** the page can open an exported JSON to view it (no live data, banner "viewing export from ..."), which is also how diffs against a saved export work (8e).

#### 8d. Alerts: linked project down or unreachable

- **What raises an alert:** a project in the current scope (including auto-linked references) becomes **missing** (root deleted or moved), **unreachable** (a network or SSH root stops answering, an external disk is unmounted), **backend down** (its working backend from 6b fails and no fallback works), **index failing** (re-index errors three times in a row), or **link broken** (a manifest reference now points to a path that does not exist).
- **Detection:** the `watch` loop and every graph query update project state; a light background check runs every 60 s (`[plugins.graph] health_check_interval_s`) only for projects in an open scope, using the cached capability record (6b) rather than re-probing everything. A state must persist for two checks before it alerts, to avoid flapping on a brief unmount.
- **Where alerts show:** a red badge on the project node and link edges in both parts, a toast and an alerts list on the graph page, a line in `rtok doctor`, `rtok graph projects` output (`state` and `alert` fields in `--json`), and a short notice in graph MCP answers that touch an affected project ("project B unreachable since 14:02; results exclude B"). Agents therefore learn about it in the answer they are already reading.
- **Optional push:** if T288 (push unread messages to hooked agents) is available, an alert is delivered once to agents whose current scope includes the project; repeated failures do not repeat the message.
- **Recovery:** when the project comes back, the alert clears automatically, a "recovered" entry is logged, and the project is re-indexed if files changed while it was away.
- **Edge cases:** a project removed on purpose from the registry never alerts; unlinking a broken project clears its alert for that scope; an alert on a project that is only transitively linked names the chain ("A to B to C: C missing"); many simultaneous alerts (for example a whole disk unmounted) collapse into one grouped alert.
- **Config:** `[plugins.graph] alerts = true`, `health_check_interval_s = 60`, documented in `docs/config.md`.

#### 8e. Diff: compare the graph before and after a change

- **What can be compared:** the current graph against (a) a git ref (`HEAD~1`, a branch, a commit, the merge base of a PR branch), (b) the working tree versus `HEAD` (uncommitted changes), or (c) a saved export (8c). Diffs work over the whole scope, so a change in B that affects A's call sites shows up in A.
- **How the "before" side is built:** for git refs, rtok indexes the files at that ref from the object database into a temporary index (no checkout, no change to the user's working tree), using the same backend chain (LSP is skipped for the old side when it would need a separate checkout; tree-sitter is used instead and the diff says so).
- **What the diff reports:** symbols added, removed, renamed (same body hash, different name or path), moved between files or projects, and changed (signature or body); call edges added and removed; links added and removed; and, for each changed symbol, its callers that are affected (the `impact` set), which is the part agents need for review.
- **Where:**
  - Page: a "Compare" mode in part 1 colours nodes and edges (added green, removed red, changed amber, moved blue) and lists changes in a side panel; the live graph is unaffected.
  - CLI: `rtok graph diff [--from REF|--from-export FILE] [--to REF|working] [--project ID] [--json]`.
  - MCP: `graph_diff` returning a capped summary (counts, top changed symbols with affected callers) and an id to page through details, so an agent reviewing a PR gets a short answer by default.
- **Edge cases:** a ref that does not exist returns a clear error; a diff spanning projects at different git states diffs each project against its own ref (a `--from` per project is allowed); generated or vendored files follow the project's ignore rules; very large diffs are capped like other answers with a "more" id; renames are detected only when unambiguous, otherwise shown as remove plus add; binary or unparsed files are listed as "changed, not analysed".

#### 8f. Health score per project

- **Score:** 0 to 100 per project, shown as a coloured ring on the project node (green 80+, amber 50 to 79, red below 50) with the breakdown on hover in part 1 and in the project list, and as `health` in `rtok graph projects --json` and MCP answers.
- **Components (weights in brackets, each 0 to 1):**
  - **Index freshness [40%]:** 1 when no files are pending and the last index is newer than the last file change; drops with the share of pending files and with age (0 when more than 20% of files are pending or the index is older than 24 hours with changes since).
  - **Backend alive [30%]:** 1 when the preferred backend (LSP under `auto`) works; 0.6 when running on tree-sitter fallback; 0.3 on text fallback; 0 when no backend works. Reads the cached capability record (6b) plus recent query failures.
  - **Links not broken [30%]:** the share of the project's links whose target is present, reachable and indexed; a project with no links scores 1 here.
- **Explained, not just a number:** each score comes with the reasons that lowered it ("12 files pending", "rust-analyzer not on PATH, using tree-sitter", "link to ../foo broken"), and a suggested fix for each (re-index, install the server and restart the MCP server, fix or remove the link).
- **Scope score:** the selected project's scope shows its lowest project score (the weakest link decides), not an average.
- **Agents:** graph MCP answers include a one-line health note when the scope's score is below 80, so an agent knows when results may be incomplete; `rtok doctor` lists every project under 80 with its reasons.
- **Edge cases:** a project being indexed for the first time shows "indexing" instead of a score; a missing project scores 0 and shows "missing"; text-only languages are not penalised beyond the backend component; scores update live as state changes and are recomputed at most once per second per project.

#### 9. Docs

`docs/plugins.md` (graph section: projects, links, references, scope, backends), `docs/lsp.md` (fallback chain and capability cache) and `docs/config.md` (the new `[plugins.graph]` keys), with `docs/ru/` and `docs/uk/` updated in the same change.

#### 10. Delivery

As PRs, backend first; do not merge them.

Dependencies: T310.8 for the page; T285 and T289 for worktree-based adding; the existing graph index and LSP integration.

Check: fixture repos under `tests/fixtures`, no network:

- Repo A has a Cargo path dependency on B; B has one on C; D is unrelated.
- Indexing A registers B and C (origin `reference`), indexes them and creates auto links A to B and B to C; D is not added.
- Selecting A shows A in the header with "+ 2 linked"; `callers` of a function defined in C returns call sites in A and B, each labelled with its project; `impact` from that function walks up into A.
- `dead` over A's scope does not report B's function that only A calls; selecting B alone does.
- Unlinking B from A removes B and C from A's scope, survives a re-index (no re-link), and `callers` no longer crosses projects.
- A manual link A to D adds D to the scope; a cycle (D links A) does not loop or duplicate rows.
- Removing the path dependency from A's manifest and re-indexing removes the auto link but keeps B in the registry.
- A reference to a missing path shows a warning and adds nothing; a deleted project root shows as missing and drops out of the scope.
- `reference_depth = 1` stops at B; `max_auto_projects` limits are reported on the page and in logs.
- A new agent session in a new directory registers it when `auto_add_projects` is on and not when it is off; `auto_link_references = false` adds no reference projects.
- The selection survives an `rtok web` restart and syncs between two browser tabs.
- MCP `callers` without `project` from A's directory crosses into B and C; with `project` set to D it does not.
- Backends, with `backend = "auto"`: with rust-analyzer on `PATH`, A answers from LSP (result tagged LSP) and finds a type-position reference tags would miss; with it removed from `PATH` and the MCP server restarted, A answers from tree-sitter (tagged tree-sitter); a fixture project in a language with no grammar answers from text search (tagged text, `dead` reported as not available); a scope mixing all three labels each project with its own mode.
- `backend = "lsp"` with no server still errors as today (no fallback when pinned).
- A server that crashes mid-session: the current request is answered from tree-sitter with a notice, and later requests go straight to tree-sitter without respawning the server.
- Capability cache: a test counts probes; 100 requests to the same project after the first run zero further `PATH` lookups or spawn attempts; installing the server without restarting changes nothing; restarting the MCP server picks it up; changing `backend` in config re-checks only the affected projects; two concurrent first requests run one check.
- Remote text mode: a project registered as `ssh://localhost/<path>` (test runs only when passwordless SSH to localhost works, otherwise skipped) answers `symbol` over SSH; an unreachable host is reported as no backend available without hanging past the timeout.
- Visual graph, level 1: with A, B, C, D the page shows 4 projects, 3 in A's scope and 3 linked pairs (A to B, B to C, A to D); the A-to-B edge is dashed with the Cargo reason on hover, A-to-D is solid; clicking B selects it; a missing project is drawn hollow and cannot be opened.
- Visual graph, level 2: opening A shows its files with aggregated edges; expanding a file shows its functions; focusing the function that calls into C shows the edge ending at a C node, and clicking it opens the target symbol inside C; the breadcrumb and browser back return to the overview; a text-mode project shows the "call edges not available" banner; editing a file with `watch` on updates the node without resetting zoom; a fixture with more than 500 nodes shows "+N more" groups and the page stays responsive.
- 3D: both levels render in Three.js (Playwright with SwiftShader sees a non-empty canvas and can select a node by click); disabling WebGL shows the 2D fallback with a notice; the 2D/3D toggle is remembered across reloads; orbiting the 500-node fixture stays smooth and the layout stops when settled; leaving the page releases the WebGL context.
- Two-part UI: an MCP `callers` call from a separate process lights up the target node in the live graph within one second, animates the path into a linked project and adds a feed row whose symbols requested/returned, tokens and saving equal the matching `Measurement` row and `rtok stats`; part 1's camera and selection do not move; clicking, dragging, hovering and keyboard input on the live canvas change nothing (Playwright asserts no selection or camera change); drilling into a project in part 1 switches the live graph to it; freeze then unfreeze catches up without losing totals; a burst of 500 calls in 5 s keeps both parts responsive and the totals exact; a failing call shows red with its error; dropping and restoring `/ws` shows "reconnecting" and refreshes totals from the store; with the live part hidden, no live events are serialised; on a 375 px screen the live part stacks below as a metrics strip.
- Export: PNG, SVG and JSON exports of A's scope open correctly; the JSON validates against the schema; absolute paths and the user name are redacted by default; a 2,000-node scope exports every node to JSON and the PNG footer notes hidden nodes; `rtok graph export` and MCP `graph_export` produce the same JSON; importing the JSON shows it read-only.
- Alerts: unmounting (or renaming) B's directory raises "B missing" after two checks on the page, in `rtok doctor`, in `rtok graph projects --json` and as a notice in an MCP `callers` answer from A; restoring it clears the alert and re-indexes; a broken manifest path raises "link broken"; unmounting several projects at once shows one grouped alert; a removed project never alerts.
- Diff: changing a function signature in B and running `rtok graph diff --from HEAD` from A reports the change and lists A's affected call sites; the working tree is untouched by building the old side; a rename is reported as a rename; an unknown ref errors clearly; MCP `graph_diff` returns a capped summary with a paging id.
- Health: a fully indexed A with LSP and intact links scores 100; with 30% of files pending it drops below 80 with the reason shown; on tree-sitter fallback the backend component reads 0.6; a broken link lowers the links component; the scope shows the lowest score; an MCP answer from a scope under 80 includes the health note.
- Playwright covers the selector, the indicator and its states, link/unlink, project badges, backend tags, both graph levels, export, alerts, compare mode, health rings, 3D and 2D modes, the two-part layout with the read-only live graph and its metric displays, and the list-view fallback; `just check`.

### T329.4. `project` argument and scoped traversal for symbol, callers, impact, explore and outline (CLI and MCP)

T329 §6 (first half), §7 and the tags-backend half of §6a mode 2. Every graph command and graph MCP tool takes `project` (id or path); without it the project is the caller's cwd and its links are in scope (T336 decides whether the selected project replaces the cwd, so settle it before claiming). Queries run over the scope as one graph, rows carry `project` (JSON field, `[name]` text prefix), same-named symbols across projects are grouped and flagged ambiguous with the selected project first. Depends on T329.3.

Check: fixture repos from the T329 Check list; `callers` of a function in C returns call sites in A and B labelled by project; `impact` walks up into A; a same-named symbol is grouped and flagged; MCP `project` set to D does not cross; `just check`.

### T329.5. Scoped `dead` and `affected`, whole-answer caps, watch across the scope

T329 §6 (second half): `dead` over the scope (a symbol in B used only from A is not dead while A links B, still reported per project), `affected` reading `git diff` in every git project of the scope, caps and token budgets applied to the whole answer, `watch` updating every project in the scope. Depends on T329.4.

Check: `dead` over A's scope spares B's function only A calls, selecting B alone reports it; `affected` maps per project; an MCP reply stays under the cap with three linked projects; an edit in C updates its index under `watch`; `just check`.

### T329.6. Auto-adding projects rtok sees in use (sessions, worktrees, graph MCP calls) and its config keys

T329 §4a: register the cwd of a hooked agent session, a worktree created or adopted through `rtok worktree` (T285, T289; display name shows the branch) and the root of any graph MCP call, origin `session|worktree|mcp`. Adds `[plugins.graph] auto_add_projects = true` to the config schema and `docs/config.md`. The hook path stays within its 10 ms budget (the registration is a deferred write). T289 is owned by another agent; stay out of `adopt`. Depends on T329.1.

Check: a session in a new directory registers it with `auto_add_projects` on and does not with it off; a worktree shows its branch as the name; the hook still exits within 10 ms; `just check`.

### T329.7. Reference discovery from manifests (Cargo, npm, Go, Python, submodules)

T329 §4b sources, in the card's order: Cargo `path`/`[patch]`/out-of-root workspace members, npm/pnpm/yarn `file:`/`link:`/`workspace:`, Go `replace` and `go.work`, Python path dependencies, `.gitmodules`. A pure function from a project root to a list of `(directory, reason)`, plus warnings for paths that do not exist. Import-resolver references (the last source in the card) are left to a later sub-id once T329.9 lands. Fixture repos only. Depends on nothing but T329.1; no registry writes here.

Check: fixture manifests for each source return the expected directories and reasons; a missing path is a warning; registry dependencies are not returned; `just check`.

### T329.8. Following references: transitive, depth and project caps, auto-link lifecycle, remembered unlinks

T329 §4b rules: register each referenced directory (origin `reference`), index it in the background, auto-link it, follow references transitively with `reference_depth` (default 3) and `max_auto_projects` (default 20) both reported, drop an auto link when its reference disappears on re-index while keeping the project, never re-create a link the user removed, never remove a manual link. Adds `auto_link_references`, `reference_depth` and `max_auto_projects` to the config schema and `docs/config.md`. Depends on T329.3, T329.7.

Check: indexing A registers and links B and C, not D; `reference_depth = 1` stops at B and says so; removing the dependency drops the auto link but keeps B; an unlinked auto link is not re-created; `just check`.

### T329.9. Graph backend `auto`: LSP first, tree-sitter second, chosen per project and language

T329 §6a modes 1 and 2 and the config (`backend = "auto"|"lsp"|"tags"|"text"`, `lsp_timeout_ms`, `backend_by_language`); pinned values keep today's strict behaviour. Each answer says which mode answered per project (`Measurement` kinds `lsp.*`/`tags.*`). T334 (default backend decision) must be answered first. Depends on T329.4.

Check: with the server on `PATH` the answer is tagged LSP, without it (MCP restarted) tree-sitter, a scope mixing both labels each project; `backend = "lsp"` with no server still errors; a crash mid-session falls back with a notice; `just check`.

### T329.10. Graph text-search backend (rg/grep) including `ssh://` roots

T329 §6a mode 3 and "when no mode works": word-boundary definition and mention searches through `rg` or `grep -rn`, the same over `ssh host` for `ssh://host/path` roots (passwordless only, test skipped without it), `dead` reported as not available, `text.*` Measurement kinds. Depends on T329.9.

Check: a project in a language with no grammar answers from text search, tagged text, with `dead` not available; `ssh://localhost/<path>` answers `symbol` when passwordless SSH works (skipped otherwise); an unreachable host reports no backend within the timeout; `just check`.

### T329.11. Graph capability cache: one probe per project until the process restarts

T329 §6b: an in-memory per-project (and language) record of which mode works, single-flight first probes, downgrade once on failure, cleared for the affected projects when `backend` config changes, shown by `rtok graph projects --json` and the page. T337 (never re-probe vs alerts and health) must be answered first. Depends on T329.9.

Check: a test counts probes, 100 requests after the first run zero lookups or spawns; restarting picks up a newly installed server; a `backend` change re-checks only affected projects; concurrent first requests run one check; `just check`.

### T329.13. Graph page level 1: 3D projects overview (Three.js, 2D fallback)

T329 §8a level 1 and the rendering section: node per project, edges per link (dashed auto, solid manual, thickness by cross-project references), scope emphasis, node menu, filter, clustered layout above about 50 projects, 3D with Three.js (pick the library, record it and its bundle size in `toolchain.md`) with the 2D fallback and toggle, layout in a web worker, disposal on leaving the page, list view for accessibility. Depends on T329.12.

Check: Vitest for the data-to-scene mapping without WebGL; Playwright with software WebGL sees a non-empty canvas, selects a node by click and shows the 2D fallback and notice with WebGL off; the 2D/3D choice survives a reload; `just check`.

### T329.14. Graph page level 2: drill-down into one project

T329 §8a level 2: files, modules, types and functions with contains/calls/implements/imports edges, URL-carried drill-down state and breadcrumb, expand and focus, calls into linked projects ending at that project's node, side panel, search-to-focus, the 500-node cap with "+N more", live updates under `watch`. Depends on T329.13, T329.5.

Check: opening A shows files with aggregated edges, expanding a file shows its functions, a call into C ends at a C node that opens the target symbol, breadcrumb and browser back return; the 500-node fixture shows "+N more"; `just check`.

### T329.15. Graph page: two-part UI with the read-only live graph and live metrics

T329 §8b: the explorer and the read-only live graph side by side with a splitter, the start/end/progress call events on `/ws` from every process through the store, the live canvas and its metric displays (values from the same `Measurement` rows as `rtok stats`), freeze, window selector and the call feed. Over the size budget on its own; split into data path and display when claimed. Depends on T329.13, T329.14.

Check: an MCP `callers` call from another process lights the node within one second and adds a feed row equal to its `Measurement` row and `rtok stats`; the live canvas ignores input; freeze and unfreeze keep exact totals; a 500-call burst keeps the page responsive; `just check`.

### T329.16. Graph export: PNG, SVG, JSON, `rtok graph export`, MCP `graph_export`

T329 §8c, including the `rtok.graph.v1` JSON schema file, redaction by default and read-only import. Depends on T329.14.

Check: PNG, SVG and JSON exports of A's scope open; the JSON validates against the schema file; paths and the user name are redacted by default; `rtok graph export` and `graph_export` give the same JSON; import is read-only; `just check`.

### T329.17. Graph alerts: linked project down or unreachable

T329 §8d: alert states, the two-check rule, the 60 s background check, the page badges and toasts, `rtok doctor`, notices in MCP answers, optional T288 push. Adds `alerts` and `health_check_interval_s`. T337 shapes the probing, so settle it first. Depends on T329.11, T329.12.

Check: renaming B's directory raises "B missing" after two checks on the page, in `rtok doctor`, in `--json` and in an MCP `callers` notice; restoring clears it and re-indexes; a broken manifest path raises "link broken"; several at once group into one alert; `just check`.

### T329.18. Graph diff: compare before and after a change

T329 §8e: `rtok graph diff`, MCP `graph_diff`, the Compare mode on the page; the old side is indexed from the git object database into a temporary index. Depends on T329.5, T329.14.

Check: a signature change in B shows in `rtok graph diff --from HEAD` from A with A's affected call sites; the working tree is untouched; a rename is a rename; an unknown ref errors; `graph_diff` returns a capped summary with a paging id; `just check`.

### T329.19. Graph health score per project

T329 §8f: the 0 to 100 score with freshness, backend and link components, reasons and fixes, the scope's lowest score, the MCP health note and the `rtok doctor` list. Depends on T329.11.

Check: a fully indexed A with LSP and intact links scores 100; 30% of files pending drops it below 80 with the reason; tree-sitter fallback reads 0.6 on the backend component; a broken link lowers the links component; the scope shows the lowest score; `just check`.

### T329.21. Project badges in the graph page lists

The last piece of the original T329.12 (split out of T329.20 on 2026-10-03): a project badge on every row of the graph page lists (dead symbols, pending files), shown once those lists are scoped to the selected project and its linked projects, so a row says which project it came from. Depends on T329.4, T329.5, T329.20.

Check: Vitest and a story for a scoped list with badges from two projects; `just check`.

### T330. `rtok agents junk list` and `clear`: per-agent junk with folders, sizes and space freed

Split into T330.1 to T330.6 (one PR each); this card stays the spec and the epic.

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
| `logs` | agent log files that are not tied to a session (`logs/*.log`, rotated logs) | review | Kept for the last `[agents.junk] keep_logs_days` (default 30). Session transcripts are the `sessions` kind below. |
| `build` | build artifacts in agent worktrees and scratch dirs (`target/`, `dist/`, `build/`, `.next/`, `__pycache__/`) | safe | Only under agent-owned worktrees or scratch dirs, never in the user's main checkout. |
| `deps` | reinstallable dependencies (`node_modules/`, `.venv/`, `vendor/` with a lockfile, `.gradle/`, Pods) | review | Only in agent-owned worktrees/scratch dirs; requires a lockfile or manifest next to it so it can be reinstalled. |
| `locks` | stale lock files (`*.lock` for agent state, `LOCK`, `.lock` dirs) | safe | Only when no process holds them (checked with the OS); package-manager lockfiles (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`) are never junk. |
| `backups` | backup files (`*.bak`, `*.bak-<ts>`, `*~`, `_backup/` generations past the cap, T249) | review | The newest backup of each file is always kept. |
| `swap` | editor/agent swap files (`*.swp`, `*.swo`, `.#*`, `*.crswap`) | safe | Only when the owning process is gone. |
| `index` | rebuildable indexes (rtok graph/tags index of a removed project, LSP caches like `.rust-analyzer/`, agent codebase indexes, SQLite `-wal`/`-shm` of closed DBs) | review | Rebuilt on next use; the active project's index is never cleared while rtok is running on it. |
| `rtok-own` | today's T182 junk: rotated logs past cap, archives past retention | safe | Existing behaviour, unchanged. |

Paths for each host come from `research.md` §22 (official docs or source only). A cell §22 marks "not documented" is not scanned; `list` says "not documented" for that kind rather than guessing.

#### Never touched

Settings, credentials and tokens, MCP and hook config, installed plugins and extensions, user-written files (rules, memories, prompts, skills), `rtok.db`, any archive a call still references, the user's main checkouts, package-manager lockfiles, and anything outside the paths listed per host. Symlinks are never followed out of an agent folder.

#### Cache: rtok's own and each agent's

Cache is junk for rtok itself and for every agent, listed with its size and clearable like any other kind.

**rtok's cache (row `rtok`):**

- The per-project LSP state rtok confines language servers to (`<root>/.rtok-lsp-xdg/{cache,data,state,pub-cache}`, `src/plugins/graph/lsp.rs`): `cache` and `pub-cache` are `cache` kind (safe); `data` and `state` are `index` kind (review).
- Graph/tags index rows and files for projects no longer in the registry (T329) or whose root is gone: `index` kind (review).
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
| `sessions` | old sessions and their logs: transcripts (`*.jsonl`), per-session log files, per-session attachments and tool output dirs | review | §22 session paths per host; rtok's own session rows and logs keyed by session id (T284) | anything touched within `stale_session_days` (default 3); see "Old sessions: time only" below |
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

#### Old sessions: time only

- **The only criterion is time.** A session is old (junk) when it has not been touched for longer than the threshold. Nothing else classifies a session as old for now: not its status (finished, errored, abandoned), size, number of messages, project, whether it is the newest session of its project, or whether it was ever resumed. Those criteria are out of scope for T330 and may come later as separate options.
- **Setting:** `stale_session_days` in the `[agents.junk]` table of rtok's config. Default `3`. It lives in `config/default.toml` (shipped default), can be overridden in the user config (`~/.config/rtok/config.toml`), per project in `<git root>/.rtok.toml`, and per run through the environment (`RTOK_AGENTS_JUNK_STALE_SESSION_DAYS=7`), with the usual precedence (flags over env over project over user over default). `rtok config get agents.junk.stale_session_days` prints the effective value and `rtok config show --sources` shows where it came from. Documented in `docs/config.md` (and ru/uk).
- **Value rules:** a whole number of days, `0` to `3650`. `0` means every session not currently open counts as old (useful for a full wipe, and `list` warns "threshold 0: every closed session is junk"). Fractions, negatives and non-numbers are rejected by `rtok config validate` and at load with an error naming the key; rtok then falls back to the default for that run and says so, rather than deleting with a wrong threshold.
- **One-off override:** `rtok agents junk list|clear --session-days N` overrides the setting for that run only, and the output header shows the threshold in use ("old sessions: not touched for more than 3 days").
- **"Touched" means** the session's last-used time as defined in the item breakdown: the host's own last-activity timestamp when its format records one, otherwise the newest modification time of any file belonging to the session (transcript, its log, its attachments). Reading a session (for example `list` itself, or `rtok stats` ingesting it) does not count as touching it.
- **Threshold comparison:** a session is old when `now - last_used > stale_session_days * 24 h`, measured in UTC so time zone and daylight-saving changes don't shift it; exactly at the threshold it is not old yet.
- **Safety guards (not classification):** a session that is open by a process or was modified in the last 10 minutes is never removed even with threshold `0`, because deleting a session being written can corrupt the agent's state. `list` shows such a session as old-by-time but "skipped: in use".
- **All parts of an old session go together:** transcript, its per-session log, attachments and tool-output dirs are one item with one size; clearing removes all of them or none.
- **Edge cases:** a session whose files have different mtimes uses the newest one; a session with a future timestamp (clock skew) counts as just touched and is shown with a "future timestamp" note; a host that records no timestamps and whose files rtok cannot stat is listed as "last used unknown" and not cleared; changing the setting takes effect on the next `list`/`clear` with no restart; lowering it from 3 to 1 makes more sessions old in the next `list`, and nothing is deleted until `clear --yes`.
- **Expected results:** with the default, a session last touched 4 days ago is listed as old and removed by `clear --include review --yes` (or `--kind sessions --yes`); one touched 2 days ago is listed with "under the 3-day threshold" and kept; setting `stale_session_days = 1` and listing again marks the 2-day-old session old; `--session-days 7` on one run keeps both and the header says 7.

#### Item-level breakdown in `list`

`list` shows not just totals but every concrete item that `clear` would delete, so the user can verify the exact set before running `clear --yes`.

- Under each agent and each kind, the items are listed one per line: a link to the item (OSC 8 `file://` hyperlink in a capable terminal, plain path otherwise), its size, and, for anything time-stamped, **last used** (relative and absolute, for example `3 weeks ago (2026-09-09 14:02)`).
- "Last used" means, in order of preference: the host's own last-activity time when its format records one (session `updated_at`, last message timestamp); otherwise the newest modification time of any file inside the item; atime is not used (often disabled). For worktrees: the last commit time or the newest file change, whichever is later, and the owning agent id.
- Each item also shows why it is junk (for example "not touched for 5 days (threshold 3)", "agent session ended 16 days ago", "token expired 2026-08-30", "crash dump 12 days old") and, if it is not going to be cleared, why ("touched 2 days ago, under the 3-day threshold", "uncommitted changes", "agent running", "review kind: add --include review").
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

`[agents.junk] stale_session_days = 3`, `keep_logs_days = 30`, `keep_snapshots_days = 14`, `stale_worktree_days = 14`, `crash_dump_min_age_days = 7`, `temp_min_age_hours = 24`, `exclude = []` (glob paths never touched), `extra = []` (extra paths per host to treat as a kind, for example `{ host = "cursor", kind = "cache", path = "~/Library/Application Support/Cursor/CachedData" }`), documented in `docs/config.md`.

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

Check: fixture home under a temp dir, `HOME`/`XDG_*`/`LOCALAPPDATA` pointed at it, no real agent folders touched:

- A fixture with Claude Code, Cursor and Codex folders containing every junk kind: `list` shows each agent, its folders as links, each kind with the exact sizes created (checked in bytes with `--bytes` and in human units), "Freed by `clear`" equal to the sum of safe kinds, and totals equal to the per-agent sums; `--json` matches the table.
- `clear` without `--yes` changes no file (tree hash before equals after) and prints the same plan as `list`'s freed lines.
- `clear --yes` removes exactly the safe items; settings, credentials, plugins, package-manager lockfiles and `rtok.db` are untouched; freed bytes match the plan.
- `--include review` also removes sessions not touched for longer than `stale_session_days`, logs older than `keep_logs_days`, `node_modules` with a lockfile, extra backups (newest kept) and rebuildable indexes; `node_modules` without a lockfile is not removed.
- A lock file held by a test process and a swap file of a live process are skipped with reasons; a symlink inside a cache dir pointing to `$HOME/important` is not followed.
- A shared folder is counted once in totals and removed once.
- A simulated running agent skips temp, locks, swap, index and its current session.
- Cache: rtok's `.rtok-lsp-xdg/cache` in a fixture project, a `$XDG_CACHE_HOME/rtok` folder and a `CACHEDIR.TAG` dir appear under `rtok` with exact sizes; a host cache in `~/Library/Caches/<app>` (or `$XDG_CACHE_HOME/<app>`) and an Electron `Code Cache` appear under that agent; `clear --kind cache` dry run lists them, `--yes` empties them and keeps the top folders and `CACHEDIR.TAG`; a folder with a bad `CACHEDIR.TAG` signature is not treated as cache.
- New kinds: a session last touched 4 days ago is listed with its last-used time and removed with `--include review`, while one touched 2 days ago and one modified 5 minutes ago are kept; a finished-session worktree idle 20 days is removed with `git worktree remove` and its branch kept, while one with uncommitted changes and one with unpushed commits are listed and never removed; a token-only file with a past expiry is listed with the sign-out warning and removed only with `--kind stale-tokens`, a mixed config with an expired token is never touched; a 10-day-old crash dump is cleared by default, a 2-day-old one only with `--kind crash-dumps`; an old snapshot is removed and the newest kept.
- Session threshold: `rtok config get agents.junk.stale_session_days` prints 3 by default; `.rtok.toml` and `RTOK_AGENTS_JUNK_STALE_SESSION_DAYS` override it; an invalid value (`-1`, `2.5`, `abc`) is rejected with an error naming the key and the run uses 3; a session exactly 72 h old is not old, at 72 h and 1 min it is; status, size and "newest in project" do not change the result; an open session with threshold 0 is skipped as in use.
- Breakdown: every planned item appears in `list` with path link, size, last used and reason; `--items all` and `--json` list every item; the default shows 10 per kind plus "+N more"; `clear` dry run prints the same items; touching an item between the dry run and `--yes` makes `--yes` skip it with "changed since plan".
- `rtok agents junk clear --agent rtok --yes` behaves exactly as T182's tests expect (existing tests stay green unchanged).
- Permission-denied and timeout folders are reported, not fatal; exit code 1 when anything planned was not removed.
- `--trash` moves items to the platform trash (tested on macOS and Linux CI).
- `just check`.

### T330.2. Junk: every host as an agent row, folders from `research.md` §22

Part of T330. One row per host in `agents::HOSTS` (not installed hosts skipped, `--all` lists them), its config/data/cache/log folders from the §22 map with `file://` links (OSC 8) and sizes, environment overrides honoured (`XDG_CACHE_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`), a folder shared by two agents counted once with a "shared with" note, permission-denied and per-agent timeout reported. Depends on T330.1 and on the open investigations T338 and T339 (what §22 allows to touch, §22 paths vs heuristic cache detection).

Check: fixture HOME with Claude Code, Cursor and Codex folders: each agent, folder and size appears with exact `--bytes`; a symlink out of a folder is not followed; `just check`.

### T330.3. Junk: `cache`, `temp`, `build`, `locks`, `swap` kinds and rtok's own caches

Part of T330. The safe kinds of the T330 table plus rtok's cache (`.rtok-lsp-xdg/{cache,pub-cache}`, `$XDG_CACHE_HOME/rtok`, `CACHEDIR.TAG` dirs, T329 registry roots) and each agent's cache (§22 dirs, platform cache roots, Electron cache folders only where §22 confirms Electron). A bad `CACHEDIR.TAG` signature is not cache; settings, extensions and downloaded models are `never`. Depends on T330.2 and the investigations T339/T342.

Check: the T330 "Cache" fixtures (exact sizes, kinds under the right agent, tag handling); `just check`.

### T330.4. Junk `clear`: `--agent`, `--kind`, `--include review`, `--older-than`, `--trash`, re-check, exit codes

Part of T330. `clear` takes the same scan as `list`, filters it (`--agent` repeatable, `--kind`, `--include review`, `--older-than`), re-checks every item before deleting (still there, not open, not a symlink out, not modified in the last minute), skips a running agent's temp/locks/swap/index, supports `--trash`, prints the planned and freed bytes per agent and kind, and exits 1 when something planned was not removed. `agents junk clear` with no flags and `--agent rtok` keep T182's behaviour (the existing tests stay green). Depends on T330.3 and the investigations T340/T344.

Check: dry run changes no file (tree hash), `--yes` removes exactly the planned safe items, lockfiles/settings/`rtok.db` untouched, a changed item is skipped; `just check`.

### T330.5. Junk: review kinds (`sessions`, `stale-worktrees`, `stale-tokens`, `crash-dumps`, `snapshots`, `logs`, `deps`, `backups`, `index`) and `[agents.junk]` config

Part of T330. The review-class kinds with their keeps (`stale_session_days` default 3, time only; worktrees through `git worktree remove`; token files only by name), the `[agents.junk]` table (`stale_session_days`, `keep_logs_days`, `keep_snapshots_days`, `stale_worktree_days`, `crash_dump_min_age_days`, `temp_min_age_hours`, `exclude`, `extra`), `--session-days`, docs in `docs/config.md` (en, ru, uk). Depends on T330.4 and the investigations T338/T341.

Check: the T330 "New kinds" and "Session threshold" fixtures; invalid values rejected naming the key; `just check`.

### T330.6. Junk: item breakdown, `doctor` line, web card

Part of T330. `list` and the `clear` dry run print every planned item (link, size, last used, reason, skip reason; 10 per kind plus "+N more", `--items`, `--sort`, `--min-size`), `--json` carries every item, `rtok doctor` adds one reclaimable-space line (hint when over 1 GB), the web hosts page gets the "clear safe junk" button (after T310.8), `docs/agents.md` gets a Junk section (en, ru, uk). Depends on T330.5 and the investigation T343 (the two `--sort` value sets).

Check: the T330 "Breakdown" fixtures; `just check`.

### T331. `rtok doctor`: broken hooks, duplicate hooks and duplicate MCP entries, with a selective fix

Ivan, 2026-10-01: `rtok doctor` finds hooks in agent configs that point to files that no longer exist (broken hooks), hooks that are loaded twice for the same agent (duplicate hooks), and MCP servers that are loaded twice (duplicate MCP entries). It lists them, explains why each is a problem, and lets the user choose to clean them up: broken hooks are removed, and for duplicates one copy is kept and the extra ones removed. Nothing else in any config changes. Covered by many tests.

Today `rtok doctor` (flags `--instructions`, `--json`) reports rtok's own install state and per-host status, and `rtok agents info` reports MCP per surface (T278), including `stale` rtok entries. Install/update rewrites stale **rtok** hook entries in place (T242.x). Nothing checks hooks or MCP entries that are not rtok's, nothing detects a hook whose script was deleted, and nothing detects the same hook or MCP server loaded from two config files (T271 covers one specific case: rtok's MCP seen twice in the Claude desktop Code tab).

#### Terms

- **Agent:** a host in `agents::HOSTS`, per surface where the host has several (for example Claude Code CLI, Claude desktop Code tab, Cursor, Codex, Gemini CLI, Kimi).
- **Config source:** every file that agent reads hooks or MCP servers from, with its scope and load order, taken from `research.md` (per-host config map) and the existing host adapters: user/global files (for example `~/.claude/settings.json`, `~/.claude.json`, `~/.cursor/mcp.json`, `~/.codex/config.toml`, `~/.gemini/settings.json`), project files (`<repo>/.claude/settings.json`, `<repo>/.claude/settings.local.json`, `<repo>/.mcp.json`, `<repo>/.cursor/mcp.json`, `<repo>/.gemini/settings.json`), enterprise/managed files where the host has them (read-only, see below), and installed plugins that contribute hooks or MCP servers (for example Claude Code plugins' `hooks/hooks.json` and `.mcp.json`).
- **Effective set:** what the agent would actually load at the same time for the current directory: every source that applies to that directory, merged the way the host merges them (concatenated hook lists, MCP maps merged by name with the host's precedence).
- **Hook entry:** one hook command: event (`PreToolUse`, `SessionStart`, ...), matcher (if any), command string (or script path), and the source file and JSON/TOML path it lives at.
- **MCP entry:** one MCP server definition: name, transport (`stdio` command + args + env, or `url` for http/sse), and its source file and path.

#### 1. Broken hooks: hooks that lead nowhere

- **Detection:** for each hook entry in each source of each agent, rtok resolves what the command would run, without running it:
  - Parse the command the way the host's shell would split it (POSIX shell words on macOS/Linux, `cmd`/PowerShell rules on Windows where the host uses them). Expand `~`, `$HOME`, `${VAR}` and the host's own variables (`$CLAUDE_PROJECT_DIR`, `${CLAUDE_PLUGIN_ROOT}`, `$CURSOR_...` per `research.md`), using the project directory the check runs for.
  - The **target** is: the first word if it is a path (absolute, `./`, `../`, `~/` or contains a `/`); or the script argument when the first word is a known interpreter (`bash`, `sh`, `zsh`, `node`, `python`, `python3`, `deno`, `bun`, `ruby`, `pwsh`, `npx tsx`, `uv run`); or the program name looked up on `PATH` otherwise.
  - The hook is **broken** when its target does not exist: the path is missing, a dangling symlink, or the program is not on `PATH` (and not a shell builtin).
  - The hook is **suspect, not broken** when the target exists but is not executable while invoked directly (shown with the fix `chmod +x`, not offered for deletion), or when the command cannot be resolved statically (contains `$(…)`, backticks, `eval`, pipes into an interpreter, or an unknown variable): listed as "cannot verify" and never deleted.
- **Report:** a "Broken hooks" section in `rtok doctor` per agent: source file (link), event and matcher, the command as written, the resolved target, and why it leads nowhere ("file not found: ~/scripts/old-guard.sh", "dangling symlink to /Volumes/X/hook.js", "`foo` not on PATH"). It says "can be cleaned up" for each one.
- **Fix:** only hooks classified **broken** can be removed, and only those the user selects (see 4). Removing a hook removes just that entry from its source file: if its matcher group becomes empty, the empty group is removed too; if the event's list becomes empty, the event key is removed; nothing else in the file changes (comments, ordering and formatting kept, through the existing JSONC/TOML editors). The script file itself is never touched (it does not exist anyway).

#### 2. Duplicate hooks

- **What is a duplicate:** two or more hook entries in the same agent's effective set (so loaded at the same time) for the same event, with the same matcher (normalized: same regex or tool list, order-insensitive for lists) and the same command after normalization (variables expanded, paths canonicalized, redundant whitespace and quoting removed, `bash script.sh` equals `./script.sh` only when the script has a bash shebang, otherwise not). Timeouts or other options that differ are shown; entries that differ only in timeout are still duplicates and the one kept is chosen as below.
- **Where duplicates come from:** the same hook in user and project settings; `settings.json` and `settings.local.json`; a hook installed by a plugin and also pasted into settings by hand (or by an older rtok install, T242/T275); a hook repeated inside one file.
- **Not duplicates:** the same command on different events or different matchers; entries in sources that are never loaded together (two different projects, or a disabled plugin); a host that documents running each hook once per unique command (if `research.md` says so for that host, duplicates are reported as "harmless on <host>" with no fix offered).
- **Report:** a "Duplicate hooks" section per agent: the hook (event, matcher, command), how many times it would run, and each copy with its source file (link) and position. It recommends which copy to keep.
- **Which copy is kept (default, user can choose another):** (1) the copy owned by a plugin (so plugin updates keep working); otherwise (2) the copy in the most specific shared scope that is under version control (project `settings.json` over user settings, so teammates keep it); otherwise (3) the user-level copy over `settings.local.json`; otherwise (4) the first in load order. rtok's own hooks follow rtok's install rules (T275): the plugin copy is kept when the plugin serves the hook.
- **Fix:** deletes only the extra copies the user selects, never the last copy. Same minimal-edit rules as for broken hooks. Managed/enterprise files are never edited; a duplicate whose extra copy lives only in a managed file keeps the managed copy and offers to remove the user/project copy instead.

#### 3. Duplicate MCP entries

- **What is a duplicate:** two or more MCP entries in the same agent's effective set that would start the same server at the same time:
  - **same server, same name** from two sources that the host loads together (for example `~/.claude.json` and `<repo>/.mcp.json`), when the host does not de-duplicate by name (per `research.md`; where it does, the overridden one is reported as "shadowed, unused" instead, still removable);
  - **same server, different names**: identical normalized launch (same command resolved to the same binary, same args, same relevant env keys; or the same URL with the same transport), for example `rtok` and `rtok-mcp` both running `rtok mcp`, or a plugin's MCP server also added by hand (the T271 case generalised to every host and every server).
- **Normalization:** command resolved through `PATH` and symlinks (so `/opt/homebrew/bin/rtok` equals `rtok` when that is what `PATH` gives), `npx -y pkg@x` treated as the package `pkg` (versions differing are shown as a conflict, not a duplicate), URLs compared after lowercasing scheme/host and removing a trailing slash; env values are not printed (they may hold secrets), only whether they match.
- **Not duplicates:** same command with different args that change behaviour (for example different `--root` or profile), same name in two projects that are never loaded together, a server disabled in the host's own disable list.
- **Report:** a "Duplicate MCP servers" section per agent: server (name(s), command or URL), how many processes would start, each copy with its source file (link) and key path. Explains the cost (two processes, duplicate tools in the agent's context, double measurement for rtok per D21).
- **Which copy is kept:** same order as hooks: plugin-provided first, then project-shared, then user, then local; for rtok's own server, the rules of T275 (plugin serves MCP, so the separate entry goes).
- **Fix:** removes the selected extra entries only (the key under `mcpServers`, or the `[mcp_servers.<name>]` table in TOML), keeping the file otherwise unchanged; never removes the last copy; never edits managed files or a plugin's own files (a duplicate where the extra copy is inside a plugin is solved by removing the hand-written copy, or by telling the user to disable the plugin, never by editing the plugin).

#### 4. How the user cleans up in doctor

- `rtok doctor` stays read-only by default: it reports the three sections and ends with "N problems can be fixed: run `rtok doctor --fix`".
- `rtok doctor --fix` in a terminal shows a checklist of every fixable item (broken hooks, extra duplicate copies), all pre-selected except items in project files under version control (pre-unselected, since the change affects teammates), lets the user toggle items and change which duplicate copy is kept, shows the exact diff per file, and asks for confirmation before writing.
- Non-interactive: `rtok doctor --fix --yes` applies the default selection; `--fix --only broken-hooks|duplicate-hooks|duplicate-mcp` (repeatable) limits it; `--fix --dry-run` prints the diffs and writes nothing; `--json` works with all of them and lists `problems[] { kind, agent, source, path, detail, fixable, selected, keep? }`.
- Every edited file is backed up first with the existing bounded `_backup` generations (T249), and the backup path is printed. If a file changed on disk between the scan and the write (mtime or hash differs), that file is skipped with "changed since check, run doctor again".
- After writing, doctor re-runs the checks on the edited files and reports the result ("2 broken hooks removed, 1 duplicate MCP entry removed, 0 problems left"). If a write fails (permissions, read-only file), that file is reported and the rest proceed; exit code 1 if anything selected was not fixed.
- The web UI doctor page (T310.7) shows the same sections and a "Fix selected" action with the same confirmation, after T310.7 lands.

#### Edge cases and expected results

- A hook target on an unmounted volume is broken now; the report says "path is on /Volumes/X, which is not mounted" so the user can decide; it is still selectable.
- A hook using `${CLAUDE_PLUGIN_ROOT}` is resolved against that plugin's install dir; if the plugin is uninstalled but its hook entry remains in settings, the hook is broken.
- A hook whose program is a shell builtin or an alias defined only in the user's interactive shell: builtins are fine; aliases are not visible to hooks, so a hook relying on one is reported broken with that explanation.
- Relative script paths are resolved against the directory the host uses for hooks (project root for project settings, per `research.md`), not the doctor's current directory.
- A JSON/TOML file that does not parse: doctor reports "cannot read <file>: <parse error>" and offers no fix for it.
- Windows: `.cmd`/`.ps1`/`.exe` targets resolved with `PATHEXT`; case-insensitive paths.
- Two duplicates plus one broken copy of the same hook: the broken one is listed as broken; the remaining two as duplicates; fixing both leaves one working copy.
- Managed/enterprise policy files are read for the effective set but never edited; problems only fixable there are listed as "managed by your organization".
- A host doctor cannot locate (not installed) is skipped; `--agent <host>` limits the check to one host.
- Nothing to fix: the sections say "none found" and `--fix` exits 0 with "nothing to fix".

Docs: `docs/agents.md` (doctor section) and the doctor help text, with `docs/ru/` and `docs/uk/` updated in the same change. Deliver as a PR; do not merge it.

Dependencies: host adapters and config maps (`research.md`), JSONC/TOML editors, `_backup` generations (T249), T275/T278 (rtok MCP rules and per-surface state); relates to T271.

#### Tests: heavily mocked, many, and isolated from the real machine

**Rule: no test reads or writes a real file, a real agent directory or a real agent.** Every test runs against mocked data only. Concretely:

- **No real paths.** Tests never touch `~`, `~/.claude*`, `~/.cursor`, `~/.codex`, `~/.gemini`, `~/.kimi`, the real `PATH`, real plugin directories, or the repository's own `.claude/`/`.mcp.json`. The doctor code reaches the filesystem, environment and process lookup only through injected traits (`Fs`, `Env`, `Which`, `Clock`), so tests pass mocks and production passes the real implementations. A guard test fails the suite if any doctor module calls `std::fs`, `std::env` or `which` directly instead of the traits.
- **No real agents.** No host binary is launched, no MCP server is started, no hook is executed. Hosts are described by mocked host profiles (which config sources exist, their scope, load order, merge rules, whether the host de-duplicates MCP by name or hooks by command).
- **A belt on top:** integration tests that need real files on disk (golden-file edits, permissions, mtime races) use a fresh `tempfile::TempDir` as a fake `HOME` and fake project, with `HOME`, `USERPROFILE`, `XDG_*`, `APPDATA` and `PATH` overridden for the process, and assert at the end that nothing outside the temp dir was created or changed.

**What is mocked:**

- **File system** (`MockFs`, in memory): files with contents, permissions (executable bit, read-only), mtimes and hashes; directories; symlinks, including dangling ones and symlink loops; unmounted volumes (a path prefix that reports "not mounted"); case-insensitive mode for Windows cases; injected failures (permission denied on read or write, disk full, file changed between read and write).
- **Environment and PATH** (`MockEnv`, `MockWhich`): `HOME`, host variables (`CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, ...), a fake `PATH` with chosen programs present or absent, `PATHEXT` for Windows, shell builtins list.
- **Agent configs:** mocked settings and MCP files per host and scope (user, project, local, managed/enterprise, plugin), in JSON, JSONC (with comments and trailing commas) and TOML, including deliberately unparsable files.
- **Hook definitions:** built with a small builder (`hook(event).matcher(..).command(..).timeout(..)`) and rendered into each host's real config shape, so the same logical hook can be placed in several sources.
- **MCP server entries:** builder (`mcp(name).command(..).args(..).env(..)` / `.url(..)`) rendered into `mcpServers` JSON or `[mcp_servers.<name>]` TOML.
- **Plugins:** mocked installed and disabled plugins with their own `hooks.json` and `.mcp.json`.
- **Doctor output:** the result model (`problems[]`) is asserted structurally; the human table and `--json` output are checked with snapshot tests (`insta`) over the mocked world, with temp paths normalized.
- **User interaction:** the `--fix` checklist reads from an injected `Prompt` trait; tests script the answers (toggle item 2, keep copy B, confirm, or cancel). One pseudo-terminal test drives the real terminal UI against a mocked world.

**How fixtures are structured:**

- `tests/fixtures/doctor/<scenario>/` holds a declarative `world.toml`: host profiles, files (path, contents or a pointer to a file beside it, mode, symlink target), env, PATH programs, plugins; plus `expected.json` (the problems doctor must report) and, for fix scenarios, `after/` golden files and `selection.toml` (what the user picks).
- A `World` builder loads a scenario into `MockFs`/`MockEnv` (unit level) or materializes it in a temp dir (integration level), so the same scenario runs at both levels.
- Shared building blocks: one "clean" baseline world per host with valid hooks and MCP entries; scenarios add or remove a small delta so each test reads as "baseline plus this problem".
- Property tests (`proptest`) generate random hook and MCP sets across sources and check invariants: fix never removes the last copy of anything, never removes a valid non-duplicate entry, and running doctor after fix reports no fixable problems.

**Scenarios that must be covered (each with its expected result):**

- **Broken hook references:** missing script; deleted script that a symlink still points to; program not on PATH; interpreter + missing script for each listed interpreter; `${CLAUDE_PLUGIN_ROOT}` of an uninstalled plugin; relative path resolved against the host's hook directory, not the test's working directory; unmounted volume; quoted paths with spaces; Windows `PATHEXT` and `cmd /c`. Expected: each listed as broken with the right reason and marked fixable.
- **Hooks that must not be called broken:** valid script; builtin; program on PATH; existing but non-executable script (suspect, `chmod +x` hint, not fixable); command with `$(…)`, backticks, pipes or unknown variables (cannot verify, not fixable). Expected: not in the broken list, never offered for deletion.
- **Duplicate hooks across configs:** user + project; `settings.json` + `settings.local.json`; plugin + hand-written copy; repeat inside one file; three copies; copies differing only in whitespace, quoting, matcher-list order or timeout. Expected: one group per duplicate with every copy, the recommended keep matching the keep rules.
- **Not duplicates:** different event; different matcher; sources never loaded together (another project, disabled plugin); host that de-duplicates by command. Expected: no duplicate reported (or "harmless on <host>").
- **Duplicate MCP entries:** same name in two loaded sources; different names with the same launch; plugin MCP plus hand-written entry (the T271 case on every host); URL servers differing only by case or trailing slash. Expected: one group per server, process count, keep recommendation; env values never appear in output.
- **MCP that must not be called duplicate:** same command with behaviour-changing args; `npx pkg@1` vs `pkg@2` (conflict instead); disabled server; host that resolves by name (reported "shadowed").
- **User selecting cleanup:** default selection with `--yes`; toggling items off; choosing a different copy to keep; `--only` per kind; `--dry-run`; cancel at the confirmation. Expected: only the selected entries disappear, golden files match byte for byte (comments, ordering and formatting kept), empty groups and events removed, backups created, cancel and dry run change nothing.
- **Refusing to delete valid hooks and entries:** a selection file or JSON input that names a valid hook, a suspect hook, a "cannot verify" hook, the last copy of a duplicate, an entry in a managed file or in a plugin's own files. Expected: doctor refuses each with a clear message, writes nothing for it, and exits with code 1 if it was explicitly requested.
- **Failure and race cases:** unparsable config (reported, untouched); read-only file (reported, others fixed, exit 1); file changed between scan and write (skipped with "changed since check"); write failure midway (the file is left as it was, the backup is intact).
- **Combined case:** a world with broken hooks, duplicate hooks and duplicate MCP entries across several hosts at once; after `--fix --yes`, re-running doctor reports zero fixable problems and every valid entry is still present.

Check: all of the above pass on Linux, macOS and Windows CI; the "no real paths" guard test passes; `just check`.

### T332. Investigate: rtok's own MCP duplicate: T331 keep rule vs D33/T275

In the plan, T331 (plan.md on branch `docs/plan-doctor-hooks-mcp`, ~line 711, from PR #542 (T331), not merged yet) says "for rtok's own server, the rules of T275 (plugin serves MCP, so the separate entry goes)" and keeps the "plugin-provided first" copy by default, reporting a host that de-duplicates by name as "shadowed, unused, still removable" (~line 706). D33 (plan.md@966f067 line 709) and T275 (plan.md@966f067 lines 171-175) say "Install and update always write the config entry `rtok`; only `remove` takes it out, and a plugin no longer suppresses or strips it", "the rtok plugin ships no MCP server", and "Gemini keeps both, since settings.json wins over an extension's same-name server". These contradict each other because `rtok doctor --fix --yes` would delete exactly the config entry D33 requires (on Gemini, and on any host where an old plugin still serves MCP), and the next `rtok agents install|update` would write it back, so the two features undo each other.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T333. Investigate: T271 desktop-entry sweep vs D33/T275

In the plan, T271 (plan.md@966f067 lines 138-139, table row line 17, P1 todo) says doctor must warn on "a desktop `mcpServers.rtok` entry while `code_serves_mcp` is true" and that every install/update path "also runs the desktop removal" (`unregister_mcp_ours`), with the fix `rtok agents install claude --desktop`; done task T171 (done.md:6548) ships the same check as a `duplicate:` line. T275 (plan.md@966f067 lines 171-172, "replaces T271's 'sweep' item") and D33 (plan.md@966f067 line 709) say "Install and update always write rtok's MCP entry into each agent's own config" and the plugin ships no MCP server. These contradict each other because T271 still asks rtok to remove and flag as an error the very entry D33 requires (the one Claude Desktop chat needs), while T271 stays an open P1 task.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T334. Investigate: graph default backend: T329 `auto` (LSP first, fallback) vs graph PLAN.md P30 decisions

In the plan, T329 §6a (plan.md on branch `docs/plan-graph-projects`, ~line 752, from PR #540 (T329), not merged yet) says "T329 changes the default to an ordered fallback chain, `backend = "auto"`: LSP first, tree-sitter second, plain text search last", with per-request fallback on timeout. `src/plugins/graph/PLAN.md` (P30 survey) says "**C** rejected for gate honesty" (line 229, alternative C = tags-first with LSP fallback), lists under Rejected "**Default-on LSP** — tags stay default" (line 278) and "Hybrid tags+LSP per call without a mode flag (alternative C)" (line 279), and requires that the default answers stay byte-identical to tags (lines 246, 272); `roadmap.md:407` says "tags index remains default". These contradict each other because T329 makes the rejected design the default without revisiting the measured reasons (cold LSP start vs 23-26 ms warm tags, Gate P30 byte identity).

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T335. Investigate: graph text mode spawns `rg`/`grep`/`ssh` vs D6/D18

In the plan, T329 §6a Mode 3 (branch `docs/plan-graph-projects`, ~line 774, from PR #540 (T329), not merged yet) says the text backend "Runs plain text search through the shell: `rg` (ripgrep) when present, `grep -rn` otherwise", and runs "the same commands ... over `ssh host`" for `ssh://` roots. D6 (plan.md@966f067 line 684) says "A plugin never spawns, links, imports, or reads the data of another tool", D18 (plan.md@966f067 line 695) says "D6 holds: no spawned graph tool", and the Working agreement (plan.md@966f067 line 753) says "No plugin shells out to ... a third-party tool (D6)". These contradict each other because the graph plugin would shell out to third-party tools (and to a remote host), which D6/D18 forbid; the existing LSP spawn was justified separately in the P30 survey, text search was not.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T336. Investigate: T329: default project for CLI/MCP is the selected project or the cwd

In the plan, T329 Terms (branch `docs/plan-graph-projects`, ~line 679, from PR #540 (T329), not merged yet) says "**Selected project**: the project the graph page (and, by default, the CLI and MCP tools) answers for", and T329 §7 (~line 799) says "Without it, the project is the caller's current directory (agents keep today's behaviour)". These contradict each other because the selection is stored globally in the store (§2), so one rule makes an agent's MCP call follow whatever project the user last picked in the web UI and the other makes it follow the agent's cwd; the two give different answers whenever they differ.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T337. Investigate: T329: capability cache never re-probes vs alerts/health that need re-probing

In the plan, T329 §6b (branch `docs/plan-graph-projects`, ~lines 787-791, from PR #540 (T329), not merged yet) says later requests "do not re-probe the modes that failed", the cache "is kept until that process restarts" and "nothing else invalidates it". T329 §8d (~lines 917-923) says a background check every 60 s detects **unreachable** (SSH root stops answering) and **backend down**, and "when the project comes back, the alert clears automatically"; §8f (~line 943) scores "Backend alive" from the same record. These contradict each other because detecting an unreachable SSH host or a recovered backend requires probing again, which §6b forbids; under §6b a backend-down alert can never clear without a restart.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T338. Investigate: T330 deletes sessions, tokens and snapshots vs research.md §22 "never junk"

In the plan, T330 (branch `docs/plan-agents-junk`, ~lines 730-734, from PR #541 (T330), not merged yet) adds junk kinds `sessions` ("transcripts (`*.jsonl`), per-session log files"), `stale-tokens` ("auth/token cache files") and `snapshots` ("checkpoint/undo snapshots, conversation state backups"). `research.md` §22 (line 1772), the map T182 (done.md:6879) built and T330 says it relies on, says "Never junk, on any host: settings/config files, credentials and auth tokens, session or conversation history", and names Gemini's `~/.gemini/tmp/<hash>/` checkpoints as "session history, not junk" (line 1791). These contradict each other because T330 deletes three categories that the junk map classifies as never junk.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T339. Investigate: T330 scans only §22 paths vs heuristic cache detection

In the plan, T330 (branch `docs/plan-agents-junk`, ~line 697, from PR #541 (T330), not merged yet) says "Paths for each host come from `research.md` §22 (official docs or source only). A cell §22 marks "not documented" is not scanned". The same task (~line 716) detects agent caches from "(2) the platform cache root for that app ... (3) well-known Electron/Chromium cache subfolders ... (4) any directory ... carrying a valid `CACHEDIR.TAG`", says "macOS: `~/Library/Caches` entries are treated as `cache`" (~line 811), and its Check clears Cursor caches although §22 marks every Cursor cell "not documented". These contradict each other because one rule forbids scanning undocumented paths and the other scans and deletes them by heuristics (the risk §22 warns about: "a wrong row here can destroy a user's real data").

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T340. Investigate: T330 "never touch rtok.db" vs clearing rows inside it

In the plan, T330 "Never touched" (branch `docs/plan-agents-junk`, ~line 701, from PR #541 (T330), not merged yet) lists "`rtok.db`" (as T182 did: "It never touches `rtok.db`", done.md:6879). The same task clears "rtok's own session rows and logs keyed by session id (T284)" (`sessions` kind, ~line 730) and "Graph/tags index rows ... for projects no longer in the registry" (`index` kind, ~line 710); T182's result says the graph index "live[s] in the DB" (D18, line 695). These contradict each other because both row kinds live in `rtok.db`, so T330 cannot clear them without touching it.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T341. Investigate: T330 worktree removal vs T153 (prune, orphans) and its own edge case

In the plan, T330 (branch `docs/plan-agents-junk`, ~line 741, from PR #541 (T330), not merged yet) says stale-worktree removal "uses `git worktree remove` ..., then `git worktree prune`" and that an "orphaned" worktree is "removed only by deleting its folder with `--include review`". Done task T153 (done.md:5181-5187) says "per record, never a blanket `git worktree prune`, which would also drop the records of another session's worktrees on a volume that is merely unmounted", "Orphans are reported, never removed", and "`rtok worktree clean`/`gc` never delete a worktree directory themselves". T330's own edge case (~line 812) also says "the worktree itself is never removed here (that is `rtok worktree gc`, T153)". These contradict each other because T330 reintroduces the two removal actions T153 forbids and disagrees with itself about whether `agents junk clear` removes worktrees at all.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T342. Investigate: T330 build/cache clearing vs T152 tagged-cache rules

In the plan, T330 (branch `docs/plan-agents-junk`, ~line 689, from PR #541 (T330), not merged yet) makes `build` (`target/`, `dist/`, ...) in agent worktrees a `safe` kind cleared by default with no age rule, skips only "`temp`, `locks`, `swap`, `index`" for a running agent (~line 796), and clears caches by "keeping the top folder ... and keeping any `CACHEDIR.TAG`". Done task T152 (done.md:5229-5233) clears the same tagged caches only when idle ("`--idle`", default 24h), "Never the cache of the worktree the command runs from unless its path is given explicitly", and deletes "one cache root at a time with `remove_dir_all`". These contradict each other because two commands would delete the same `target/` directories under incompatible safety rules: T330 would clear a live agent's fresh build cache that T152 deliberately keeps.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T343. Investigate: T330 `--sort` takes two different value sets on `list`

In the plan, T330 item breakdown (branch `docs/plan-agents-junk`, ~line 766, from PR #541 (T330), not merged yet) says "Items are sorted by size, largest first (`--sort size|last-used|path`)", and the `rtok agents junk list` section (~line 778) says "Sorting: by space freed, largest first (`--sort name|size|freed`)". These contradict each other because one flag on one command is given two incompatible value sets and two different defaults.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T344. Investigate: T330 "backwards compatible" vs new default deletions

In the plan, T330 (branch `docs/plan-agents-junk`, ~line 792, from PR #541 (T330), not merged yet) says "Backwards compatible: `rtok agents junk clear` with no new flags still clears T182's `rtok-own` junk, and now also the safe kinds for every agent; `rtok agents junk clear --agent rtok` reproduces T182 exactly", and its Check requires T182's tests to "stay green unchanged". The same task adds to the `rtok` row the `.rtok-lsp-xdg` caches, `$XDG_CACHE_HOME/rtok`, every `CACHEDIR.TAG` directory and plugin staging caches as `safe` kinds (~lines 705-712). These contradict each other because `clear --yes` without flags now deletes agent data T182 never touched, and `--agent rtok` deletes more than T182 did, so neither claim of compatibility holds.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T345. Investigate: ProgressRunner in the rtok crate vs `crates/rtok-mcp` with no rtok dependency

In the plan, T276 (plan.md@966f067 lines 248, 266) says `src/proc/` "is the only place in rtok that calls `std::process::Command::new`", enforced by a `clippy.toml` ban "including `crates/`". T277 (plan.md@966f067 lines 289, 298) says `crates/rtok-mcp` has "no dependency on the `rtok` crate" yet takes over "the MCP probe (`spawn_mcp`, `mcp_command`, using T276's `ProgressRunner` for the spawn)" and the T275.1 ping spawn. These contradict each other because the MCP crate can neither import `ProgressRunner` from the `rtok` crate nor call `Command::new` itself under the ban.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T346. Investigate: D27 "writing commands stay CLI-only" vs web write actions

In the plan, D27 (plan.md@966f067 line 704) says "Anything a command prints, or the store keeps, is a page on `rtok web` and `rtok tui`. Writing commands stay CLI-only", and D23 (plan.md@966f067 line 700) says "A page that exists on one surface and not the other is a defect". T329 (branch `docs/plan-graph-projects`, ~lines 703, 818, from PR #540 (T329), not merged yet) adds web actions to select, link, unlink, re-index, remove and "Index now"; T330 (~line 818, from PR #541 (T330), not merged yet) adds a "clear safe junk" button that deletes files; T331 (~line 721, from PR #542 (T331), not merged yet) adds a "Fix selected" action that edits agent configs; none plans a `rtok tui` counterpart. These contradict each other because D27 keeps writes out of the web UI (and D23 demands TUI parity) while three open PRs plan write actions in the web UI only; the existing plugin toggle (T15.4, T310.6) shows the rule is already unclear.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T347. Investigate: D16 "one task = one PR" vs multi-PR execution plans

In the plan, D16 (plan.md@966f067 line 694) says "**One task = one PR.** Each task gets its own branch ... and lands through its own pull request". T275 (plan.md@966f067 lines 193-197, PR A-E), T276 (plan.md@966f067 lines 278-283, PR 1-5), T277 (plan.md@966f067 lines 310-315, PR 1-6), T279 (plan.md@966f067 lines 403-407, PR 1-4), T283-T287 (two PRs each) and T329 §10 ("As PRs, backend first", from PR #540 (T329), not merged yet) plan several PRs for one task, while T310 splits its work into subtasks to keep one PR per task. These contradict each other because the rule and the plans disagree on what may land under one task id and when the `plan.md` → `done.md` move happens.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T348. Investigate: `--agent` means an agent id, a host, or both

In the plan, D34 (plan.md@966f067 line 710) makes the agent id a UUID accepted by "any unique prefix of 4+ chars", and T285/T286/T289 (plan.md@966f067 lines 493, 510, 557) define `--agent <id-prefix>` for `rtok worktree` commands. T331 (branch `docs/plan-doctor-hooks-mcp`, ~line 733, from PR #542 (T331), not merged yet) defines "`--agent <host>` limits the check to one host", and T330 (~lines 775, 787, from PR #541 (T330), not merged yet) defines `--agent <host|id>`, including the pseudo-agent `rtok`. These contradict each other because one flag name gets three meanings, and under D12 (plan.md@966f067 line 690, "every CLI flag is a config key") it cannot map to one key; a host name that is also valid hex (for example `cafe`) would be ambiguous between the two forms.

Goal: research both approaches, compare trade-offs, recommend one, then update the conflicting tasks. Do not change either task before the decision.

Check: the recommendation and the chosen approach are recorded in this card (or as a decision row), and every task named above is updated so the plan no longer contradicts itself.

### T359. `docs/config.md`: the reference TOML fence swallows the semantic-cache section and breaks the landing build

Found 2026-10-01 (QA audit, #601; still present on `main` @ `aecab806`). The "Reference file" fence opened at `docs/config.md:67` (```` ```toml ````) is not closed before `#### [plugins.proxy.semantic_cache]` (`:338`); the ```` ```toml ```` at `:355` cannot close a fence (a closer has no info string). In any CommonMark renderer (markdown-it 14, GitHub, the landing site) the semantic-cache heading, table and example render as code, the rest of the reference file renders as loose prose, and `### [proxy.batch] / [proxy.flex] / [proxy.routing]` plus `#### [proxy.batch]` land in a stray code block. The first heading after `## Reference file` becomes `#### [proxy.flex]` (`:448`), so the `pyrlyn/landing` `check:seo` step fails with `heading skips a level: h2 → h4 ("[proxy.flex]")` and blocks Pages deploys for every synced product. Origin: `2af52d66` (T31.1) put the semantic-cache prose inside the fence; `fb214cf9` added the `[proxy.*]` sections it now swallows. The raw fence count stays even, so counting backticks misses it.

Repro: `awk '/^[ \t]*```/{print NR": "$0}' docs/config.md` (the fence at 67 is followed by ```` ```toml ```` at 355); parse the file with markdown-it and list fences and headings.

Done when: the reference block closes with a bare ```` ``` ```` before the semantic-cache section; that section (heading, table, example) sits outside the reference file under a `###` (or is dropped from the reference, since `config/default.toml`, which the reference mirrors, has no such table); the rest of the reference reopens as ```` ```toml ````; no heading level is skipped. A test parses every `docs/**/*.md` with a CommonMark parser (no Markdown parser is in `Cargo.lock` today: `pulldown-cmark` as a dev-dependency with a one-line reason and a `toolchain.md` row) and fails on unclosed/mis-nested fences and heading-level skips. English-only change: rtok has no `docs/ru` / `docs/uk` mirror.

Check: a new docs-structure test fails on `main` @ `aecab806` (h2 → h4 at `[proxy.flex]`) and passes after the fix; `config_coverage` and `public_numbers` stay green; `just check`.

### T361. `rtok memory import` reports success for a missing or unreadable file

Found 2026-10-01 (QA audit, #601; still present on `main` @ `aecab806`). A path typo, a directory, a permission error or a non-UTF-8 file all print `inserted 0  skipped 0  malformed 0` with exit 0, so scripts and agents believe the import ran. `src/plugins/memory/import.rs:57` is `std::fs::read_to_string(path).unwrap_or_default()`.

Repro: `rtok memory import /nonexistent.json; echo $?` prints the zero counts and `0`.

Done when: the read propagates its error with the path as context (`.with_context(|| path.display().to_string())?`, the pattern `src/config/validate.rs` already uses), so the command exits non-zero with `Error: /nonexistent.json: No such file or directory`.

Check: an import test with a missing path and one with a directory both return an error naming the path and insert nothing; existing `memory import` tests unchanged; `just check`.

### T366. `rtok run` / `rtok mcp -- …` report exit 1 for a child killed by a signal

Found 2026-10-01 (QA audit, #601; still present on `main` @ `aecab806`). A command killed by SIGKILL (OOM killer, timeout) or SIGTERM comes back as `1`, not `128+signal` (137 / 143), so agents and scripts cannot tell "killed" from "failed" and OOM kills in test runs hide. On Unix `ExitStatus::code()` is `None` for a signal death and both sites map `None` to `1`: `src/plugins/cmd/run.rs:298` (`out.code.unwrap_or(1)`) and `src/mcp/wrap.rs:90` (`code().unwrap_or(1)`).

Repro: `rtok run -- sh -c 'kill -TERM $$'; echo $?` and `rtok run -- sh -c 'kill -KILL $$'; echo $?` print `1`; plain `sh -c 'kill -TERM $$'; echo $?` prints `143`.

Done when: one shared helper maps a Unix signal death (`ExitStatusExt::signal()`) to `128 + sig` where `code()` is `None`, used by both sites; Windows behaviour unchanged.

Check: Unix-only tests for `rtok run` and the `rtok mcp --` wrap path get `143` for SIGTERM and `137` for SIGKILL, and a normal non-zero exit keeps its code; `just check`.

### T367. `rtok graph index <path>` exits 0 for a path that does not exist

Found 2026-10-01 (QA audit, #601; still present on `main` @ `aecab806`). A typo prints `indexed 0 files · 0 rows …` with exit 0, so a script or agent thinks the index was built. `src/cli.rs` (the `graph index` arm, `index::run_with` at `:1684`) passes `path` as-is and the walker yields nothing for a missing root. The sibling graph subcommands in `src/cli.rs` and `src/plugins/graph/status.rs` resolve `path` the same unchecked way (suspected, not reproduced).

Repro: `rtok graph index /nonexistent; echo $?` prints the zero counts and `0`.

Done when: every graph subcommand that takes a path checks it is an existing directory (or canonicalizes with the path as context) before walking, and exits non-zero with `Error: /nonexistent: No such file or directory`; the T356 home/`/` refusal stays as it is.

Check: tests for `graph index` and each sibling path-taking graph subcommand with a missing path exit non-zero naming the path; indexing a temp project is unchanged; `just check`.

### T356. Never index `$HOME` or `/` as a graph root

Found 2026-10-02 (T352 research): `symbols` holds 617,319 rows (~120 MB plus indexes) under the root `/Users/listepo` — `.config/amp/plugins`, `.cursor/extensions`, `.grok/bundled`, `go/pkg/mod`, `.motive/node_modules`. The graph root is the `rtok mcp` process cwd (`std::env::current_dir()` in `src/plugins/graph/mod.rs`), so a server launched in the home directory (no `roots/list` answer yet, or a host without roots) walks the whole home on its first `symbol`/`callers`/`explore` call, and the rows never leave: `delete_symbols_missing` runs only when that same root is re-indexed.

Done when: the graph refuses a root that is the home directory or the filesystem root (an error that says to pass a path or open a project, no walk), and existing `symbols`/`symbol_stale` rows of such roots are dropped once (store retention or a migration). Indexing of real projects is unchanged.

Check: graph tests for both refused roots and an accepted project root; a store test that drops the home-root rows; `symbols` size on a copy of the real store before and after, recorded in this card.

Execution plan (after T352 lands — it adds `drop_vanished_symbol_roots` to retention): reuse `plugins::read::walk_root_ok` (T263: refuses `/` and the home directory for the watcher) in the graph's root choice (`src/plugins/graph/mod.rs`, the `current_dir()` call sites) and return `no project: pass path or open a project (roots)`; extend T352's root drop so a root that fails `walk_root_ok` is dropped like a vanished one. Tests: graph refuses home and `/`, accepts a temp project; retention drops a home-root row.

### T358. `rtok agents usage`: tokens and estimated cost across every coding agent

Ivan, 2026-10-02: one terminal screen with token usage and estimated cost across all coding agents, modelled on `npx ccusage` (screenshot: [`docs/assets/t358-agents-usage-ccusage.png`](docs/assets/t358-agents-usage-ccusage.png)). Two data sources: what the agents themselves logged on disk, and what rtok recorded passing through it. No product code in this card: it is the spec; split when claiming (below).

Today, nothing answers "how much did all my agents spend": `rtok stats` reads Claude Code transcripts (`[stats] transcripts_dir`) and Codex sessions (`measure::codex`, T49.2) as two `api` rows over a `--since` window, and `rtok stats --price` (T49.1) prices only the proxy `usage` rows by model; there is no per-agent, per-day or per-month view, no other host's logs, and no count of unpriced models outside that table. `rtok agents sessions` (T25.2) shows tokens per live session of this project, not totals. `rtok report` (D24) renders the operator model and has no usage-by-agent section.

#### What the screen shows (default text output)

Illustrative: the numbers are the screenshot's, not a measurement.

```text
$ rtok agents usage
rtok agents usage — logs from 8 agents, through 2026-10-02 (Europe/Kyiv)

  63.76B tokens
  $52,020.90 estimated cost
  10,137 sessions
  220 daily rows

! Cost is incomplete: 15 models have no price in [stats.prices], so their tokens are not in
  the estimate. `rtok agents usage --unpriced` lists them.

Agent               Tokens   Estimated cost
Droid               26.52B       $30,571.46
Codex               24.21B       $13,249.69
Claude Code         12.69B        $7,978.15
OpenCode           155.45M          $142.04
GitHub Copilot CLI 121.49M           $63.69
Gemini CLI          20.04M            $8.97
Grok                43.43M            $6.90
ZCode                22.1K            $0.00

Monthly totals
Month               Tokens   Estimated cost
Feb 2026             2.56B          $888.02
Mar 2026             9.88B        $4,682.41
...
```

1. **Header:** the source (`logs from N agents`, `through rtok`, or both), the last day covered and the time zone used for day and month boundaries.
2. **Summary lines:** total tokens; estimated cost (USD); sessions (distinct session ids across agents); daily rows (distinct `(agent, day)` pairs with usage, the same count ccusage calls "daily rows").
3. **Unpriced warning:** printed only when at least one model has no price: the number of such models (as in the screenshot's "no pricing for 15 models"), never a guessed price. `--unpriced` prints the model ids with their agent and tokens.
4. **Per-agent table:** agent (display name), tokens, estimated cost; sorted by cost descending, then tokens. An agent whose every model is unpriced shows `-` for cost, not `$0.00`; `$0.00` means priced and free or rounded.
5. **Monthly totals:** month, tokens, estimated cost, oldest first. `--daily` swaps it for day rows (`2026-10-02`), `--monthly` is the default view; `--by agent` (default) and `--by model` change the grouping of the middle table.

Numbers use the same short units in text (`22.1K`, `155.45M`, `26.52B`, decimal SI, two decimals above 1K) and exact integers in JSON. Tokens = input + cache write + cache read + output (+ reasoning where the host logs it separately); the four legs stay separate in JSON.

#### Two modes

`--source logs|rtok|both` (config `[agents.usage] source`, default `logs`).

1. **`logs` — the agents' own records (like ccusage).** Read each host's local session files on the fly; never written to the store, so a re-read is idempotent (the T49.2 rule). One reader per host in `src/measure/usage/<host>.rs`, all returning one row type `(host, session, model, ts, input, cache_write, cache_read, output, reasoning)`. Claude Code reuses `measure::jsonl` (`message.usage`, dedup by message id + request id, as `stats::collect` already does) and Codex reuses `measure::codex` (`last_token_usage` per `token_count` line). The others start from ccusage's documented locations (https://ccusage.com/guide/, fetched 2026-10-02) and each is verified against the host's own files before it lands:

   | Host | Default location (env override) |
   | --- | --- |
   | Claude Code | `~/.claude/projects/`, `~/.config/claude/projects/` (`CLAUDE_CONFIG_DIR`) |
   | Codex | `~/.codex/sessions`, `~/.codex/archived_sessions` (`CODEX_HOME`); `sessions/` wins over an archived copy |
   | Droid | `~/.factory/sessions` (`DROID_SESSIONS_DIR`) |
   | OpenCode | `${XDG_DATA_HOME:-~/.local/share}/opencode` (`OPENCODE_DATA_DIR`) |
   | Kilo | `~/.local/share/kilo` (`KILO_DATA_DIR`) |
   | Copilot CLI | `~/.copilot/session-state/*/events.jsonl`, `~/.copilot/otel/**/*.jsonl` (`COPILOT_HOME`) |
   | Gemini CLI | `~/.gemini/tmp` (`GEMINI_DATA_DIR`) |
   | Grok | `~/.grok` (`GROK_HOME`) |
   | ZCode | `~/.zcode` (`ZCODE_HOME`) |
   | Kimi | `~/.kimi`, `~/.kimi-code` (`KIMI_DATA_DIR`) |
   | pi | `~/.pi/agent/sessions` (`PI_AGENT_DIR`) |
   | Antigravity | `~/.gemini/antigravity*`, `~/.config/antigravity` (`ANTIGRAVITY_DATA_DIR`) |

   Every path is also a config key (`[agents.usage.dirs] <host> = [...]`, D12); an env override wins over the default and loses to the config file, like every other key. A host whose files are missing is skipped silently; a host whose files exist but cannot be parsed is named once on stderr (`skipped grok: unknown format in ~/.grok/...`) and counts nowhere. The `measure::codex` header (2026-09-17) says OpenCode's `opencode.db` and Copilot CLI's `data.db` carry no token counts: the readers use the files ccusage reads (OpenCode's message storage, Copilot's `events.jsonl`/OTel export), and if a host really has no counts it is listed as `unsupported`, never estimated. Hosts outside `agents::HOSTS` (Droid) still get a reader: this is about usage on the machine, not about what rtok installs into. D6 holds: rtok reads the files itself; it never spawns or imports ccusage or any other tool.

2. **`rtok` — what passed through rtok.** Read only the store: proxy `usage` rows (input, cache write, cache read, output, model, `api`) grouped by host through `usage.session → sessions.host_id` (rows with no session host go under `unattributed (<api>)`), plus the `measurements` ledger for what rtok removed (`est_before - est_after`, `expand` counted as a cost, as `ReportSavings::saved` does, T207). Two extra columns, **saved tokens** and **saved estimate** (saved tokens priced at that host's input price, labelled as an estimate), and one extra summary line: `rtok saved 1.23B tokens (≈ $812.40)`. Uses the existing reads (`Store::usage_by_model`, `measurement_totals`); new reads go through Diesel, grouped in SQL (no raw SQL, no second recorder, D19).

3. **`both`:** the per-agent table gets `logs tokens`, `through rtok`, and `coverage` (through rtok ÷ logs), so it is visible which agents bypass the proxy. Costs come from the logs side; saved columns from the rtok side.

#### Flags (each one a config key under `[agents.usage]`, D12)

| Flag | Key | Default | Meaning |
| --- | --- | --- | --- |
| `--source logs\|rtok\|both` | `source` | `logs` | Data source, above. |
| `--host <ids>` | `hosts` | all | Comma-separated host ids (`claude,codex,droid`); unknown ids are an error listing the known ones. Named `--host` like `agents info <host>`; if T348 settles on `--agent` for hosts, this follows. |
| `--since <date\|dur>` / `--until <date>` | `since`, `until` | all time | `2026-09-01` or a duration (`30d`, `24h`, `stats::parse_since`); dates are whole days in `--tz`. |
| `--daily` / `--monthly` | `period` | `monthly` | Bottom table: day rows or month rows. |
| `--by agent\|model` | `by` | `agent` | Middle table grouping. |
| `--tz <IANA>` | `tz` | system zone | Time zone for day/month boundaries and the header. |
| `--unpriced` | — | off | List models without a price instead of the tables. |
| `--json` | — | off | One JSON document, below. |

#### Models without a price

Prices come only from `[stats.prices."<model>"]` (T49.1); nothing is fetched from the network and nothing is guessed. Model ids are normalised before lookup (provider prefix such as `anthropic/` or `openai/` stripped, a trailing date suffix such as `-20260901` dropped) and the raw id is kept in JSON. A model with no row: its tokens count in every token total, its cost is `null`, it is counted once per model id in `unpriced_models`, and the warning line names the count (the tokens left out are in `--unpriced` and JSON). Extending the shipped price table (or importing one) is a separate decision, not this task.

#### Time zone

Usage timestamps are stored and emitted in UTC (unix seconds and RFC 3339 `Z` in JSON). Day and month buckets are cut in `--tz` (default: the system zone; `TZ` respected), DST-aware, so a session that crosses midnight splits by each request's own timestamp. The header and JSON `tz` field name the zone used. Parsing IANA zones needs a time-zone crate (`jiff`, one-line reason in `Cargo.toml`: IANA zones with DST for day/month buckets); `chrono`/`time` are not in the tree today.

#### `--json`

```json
{
  "source": "logs",
  "tz": "Europe/Kyiv",
  "through": "2026-10-02",
  "totals": {"tokens": 63760000000, "input": 0, "cache_write": 0, "cache_read": 0,
             "output": 0, "cost_usd": 52020.90, "sessions": 10137, "daily_rows": 220},
  "unpriced_models": 15,
  "unpriced": [{"model": "<model id>", "host": "zcode", "tokens": 22100}],
  "agents": [{"host": "droid", "name": "Droid", "tokens": 26520000000, "cost_usd": 30571.46}],
  "periods": [{"period": "2026-02", "tokens": 2560000000, "cost_usd": 888.02}],
  "skipped": [{"host": "grok", "reason": "unknown format", "path": "~/.grok/..."}]
}
```

With `--source rtok|both` each agent and the totals also carry `saved_tokens` and `saved_usd`, and `both` adds `through_rtok_tokens` and `coverage`. Field names are stable; a golden test pins them.

#### Relationship to existing commands

- `rtok agents list` / `info` say what is installed; `rtok agents sessions` says what is running now in this project; `rtok agents usage` says what has been spent, over time, on the whole machine. No overlap in rows.
- `rtok stats` stays the measurement report (D3) and keeps its `api`/`--price` sections; `agents usage` reuses its readers (`measure::jsonl`, `measure::codex`), `row_cost` and `[stats.prices]` instead of copying them (no duplicated logic). `stats --price` keeps working unchanged.
- D23/D27: the same rows become a Usage page on `rtok web` and `rtok tui` (T358.5), built from one model function both surfaces and the CLI call.

#### Split when claiming (≤300 LOC, ≤10 files each)

- **T358.1** `--source rtok`: CLI skeleton, `[agents.usage]` config section, store reads by host/day/month, summary + both tables + `--json`, unpriced count, `--tz`.
- **T358.2** `--source logs` for Claude Code and Codex, reusing `measure::jsonl`/`measure::codex`; `--source both`.
- **T358.3** Readers for Droid, OpenCode, Kilo, Copilot CLI, Gemini CLI (one per PR if over budget), each verified on real files and pinned by a fixture.
- **T358.4** Readers for Grok, ZCode, Kimi, pi, Antigravity; `unsupported` hosts listed in `docs/agents.md`.
- **T358.5** The Usage page on `rtok web` and `rtok tui` (D23/D27).

Check: every item below passes.

- Fixture homes per host (no real agents in tests): a fixed set of session files gives exact totals, per-agent and per-month numbers in text and JSON goldens (`trycmd`); re-running gives the same output.
- Unpriced: a fixture with one priced and two unpriced models prints the warning with `2 models`, costs only the priced one, and `--unpriced` lists the two.
- Time zone: one request at 23:30 UTC on 2026-09-30 lands in October with `--tz Europe/Kyiv` and in September with `--tz UTC`; a DST change day buckets correctly.
- `--source rtok` on a store fixture matches `rtok stats --price` totals for the same rows; `both` coverage is through-rtok ÷ logs.
- `rtok config validate` accepts every new key; each has its `default.toml` row and `docs/config.md` row; `just check` green.
- The screenshot's layout (summary, warning, per-agent table, monthly totals) is what `rtok agents usage` prints for the fixture.

### T358.3. `rtok agents usage` readers: Droid, OpenCode, Kilo, Copilot CLI, Gemini CLI

Scope: the T358.3 bullet under "Split when claiming" in T358, plus the `[agents.usage.dirs]` config keys (T358.2 reads Claude Code and Codex from the existing `[stats] transcripts_dir` and `codex_dir`).

Check: one fixture per host pins its totals; each reader was run against that host's real files once; `just check`.

### T358.4. `rtok agents usage` readers: Grok, ZCode, Kimi, pi, Antigravity

Scope: the T358.4 bullet under "Split when claiming" in T358.

Check: one fixture per host pins its totals; `unsupported` hosts are listed in `docs/agents.md`; `just check`.

### T358.5. The Usage page on `rtok web` and `rtok tui`

Scope: the T358.5 bullet under "Split when claiming" in T358. T358.1 lists `agents usage` in `EXEMPT` in `tests/surface_parity.rs` with this task as the reason; this task moves it to `COMMAND_PAGES`.

Check: `surface_parity` passes with `agents usage` in `COMMAND_PAGES`; the page shows the CLI's rows on web and tui; `just check`.

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
