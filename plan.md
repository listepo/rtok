# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T73 | in progress | P1 | 2 | 0% | Cursor / grok 4.6 |
| T74 | todo | P2 | 1 | — | — |
| T75 | todo | P1 | 2 | — | — |
| T76 | todo | P2 | 2 | — | — |

### T73. Cycle demon surfaces around a binary replace

Creator 2026-09-19. `ketch upgrade` kills PIDs holding the binary but does not write the demon stop marker, so the supervisor can respawn mid-replace and keep SQLite (`rtok.db` WAL) locked. `rtok-update` does not stop anything. HTTP+WS are `web`; MCP is `mcp`; SQLite is released when those processes exit.

**Plan.** Hidden `rtok demon upgrade`: snapshot kernel-live services (`rows` flock), `stop` them (marker + wait), run `ketch upgrade rtok --yes` (or `rtok-update`, or `RTOK_UPDATE_CMD` in tests), `start` the same set even if replace failed. Reuse `stop`/`start`. Do not revive T40 `demon update`. Spawn the supervisor from the on-disk path when `current_exe` is gone after replace.

Check: `tests/demon.rs` — live mcp is down during the replace command (no state file), up afterwards; a failing replace still leaves mcp running; `just check`.

**Evidence (2026-09-20).** The Check's first clause is live: one full gate failed `upgrade_stops_before_replace_and_starts_after` — the replace stub's guard `[ ! -f $RTOK_HOME/demon/mcp.json ] || exit 2` fired (`tests/demon.rs:197`, `update command failed (exit status: 2)`), i.e. the update command ran while the mcp state file still existed. Six other runs the same day passed (two local full suites, PR-CI ×2, push-CI ×2, release verify ×2). Harden the stop-wait: wait for the state file to be gone and re-check immediately before invoking the update command, not just for the stop marker.

### T74. Make the two load-sensitive gate tests deterministic

Two tests fail a full `just check` under parallel CPU load and pass standalone, so a green gate still rerolls dice:
- `tui::app::tests::space_toggles_the_selected_plugin_through_config_set` — nextest `terminate-after = 3` killed it at 180 s once on 2026-09-20 (full gate on a loaded machine); the immediately following full run and every scoped run passed. The key-injection → frame-assert waits carry no internal deadline, so contention turns into a suite-level timeout.
- `otel::hooks_stay_fast_with_an_unreachable_endpoint` — latency budget; failed two gates on 2026-09-17, passed standalone every time.

(Related but different, fixed 2026-09-20: `rtok::cli_trycmd cli` "panics" after every release bump — `tests/trycmd/version.stdout` / `man.stdout` pinned the literal version. Now wildcarded `rtok [..] ([..])` / `v[..] ([..])`, so a bump can't break the gate again.)

Done when both tests bound their own waiting (deadline + tolerant retry to that deadline in the tui TestBackend loop and in the otel latency assert) so a loaded runner slows them instead of failing them — no `--test-threads` masking: the point is the wait, not the machine. Check: two full suites running concurrently on one busy machine — zero flakes across three runs.

### T75. `agents uninstall` leaves the host marked installed (green check stuck)

Creator 2026-09-21. After `rtok agents uninstall <host>` (reported with a "cloud" plugin uninstall), the host/plugin is either not actually removed or the UI still shows it as installed — the green checkmark stays on.

**Repro.** Run `rtok agents uninstall <host>` (example path: uninstall involving a "cloud" plugin / host install). Open the agents/plugins UI (TUI or web) and look at that row.

**Expected.** The host/plugin is uninstalled: hooks/MCP/proxy/plugin link gone, and the green installed checkmark is cleared.

**Actual.** The entry still looks installed — green checkmark stuck — and/or the uninstall did not take effect on disk.

**Plan.** Trace `AgentCmd::Uninstall` → `setup_host(..., SetupArgs::removing)` and whatever feeds the agents/plugins list `enabled` / installed mark. Make uninstall write the same source the UI reads (config + on-disk host files), then refresh or re-read so the checkmark clears. Add a regression test: uninstall → list/UI snapshot shows not installed.

Check: uninstall a previously installed host; UI checkmark off; `rtok agents list` / info agree; `just check`.


### T76. Offer to restart the host after `agents install` / `uninstall`

Creator 2026-09-21. After `rtok agents install <host>` or `rtok agents uninstall <host>` finishes (hooks/MCP/proxy/plugin link already written or removed), ask whether to restart that application or agent. Yes → stop it, then start it again so the new config is live. No → leave the process alone and exit.

**Timeout logic (no baked-in default wait).** Config key `restart_prompt_timeout_seconds` (nesting to match existing config style) **defaults to `0`**. **`0` means "not set"** — **no timeout** — wait **indefinitely** for the user's answer. A timeout applies **only** when the user explicitly sets a **positive** number in config; then **no response within that time = No** (do **not** restart the agent; continue without hanging forever past that limit). There is **no** 30-second or 60-second default.

**Repro / flow.** Run install or uninstall for a host that is currently running. When the config change has completed, rtok prompts (interactive stdin / TUI confirm — not a silent restart). With `restart_prompt_timeout_seconds = 0` (default), wait until yes/no. With a positive value, wait that many seconds then treat silence as no.

**Expected.**
- Prompt: e.g. "Restart <host> now so the change takes effect? [y/N]" — if a positive timeout is configured, mention auto-no in Ns; if 0, do not imply a countdown.
- Yes: stop the host/agent process, then start it again (post-config-change only — never restart before the install/uninstall writes finish).
- No: do nothing further; print that a manual restart is still needed if the host caches config.
- **No response + positive timeout = No:** do **not** restart; print that the timeout elapsed; exit successfully.
- **No response + timeout 0:** keep waiting (no auto-no).
- Changing the config key changes behavior without a rebuild; absent key behaves as `0`.

**Actual (today).** Install/uninstall edit files and return; no restart offer and no timed prompt, so a running host keeps the old hooks/MCP until the user restarts it by hand.

**Plan.** Add `restart_prompt_timeout_seconds` defaulting to `0`. At the end of `setup_host` (install and remove paths): if 0, blocking read for yes/no; if >0, timed read (select/poll or equivalent) that defaults to no on expiry. Per-host restart: prefer an existing host helper if one exists; otherwise document the stop/start command matrix (Claude Code, Cursor, …) and implement the ones we can drive safely. Skip the prompt under `--dry-run` and non-interactive CI (`!stdin.isatty()` or an explicit `--no-restart` / `--yes` policy — pick one and test it). Regression: yes path stops then starts after the config write; no path never touches the process; positive timeout + silence → no restart; `0` does not auto-no.

Check: yes/no, timeout→no (positive only), and `0` = wait-forever covered in tests (prompt/timer stubbed); dry-run never restarts; `just check`.


## Reference
