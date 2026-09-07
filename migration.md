# Replacing lean-ctx with rtok — migration plan

**Status: plan only, nothing executed.** Written 2026-09-07 from an inventory of the lean-ctx
install on this machine and of every rtok plugin (`architecture.md`, `plan.md`, `done.md`,
`src/plugins/*/AGENTS.md`). Nothing here is a task: a gap becomes work only when it is promoted
into `plan.md` with a Check (`CLAUDE.md`). The migration itself is Gate P9's "config B"
(`bench/configs/rtok.json`), so its evidence is the P9 A/B bench, not this file.

The rule that shapes every row below: rtok replaces a lean-ctx behaviour only where an rtok
plugin already **owns** that behaviour (D6, D10 in `plan.md` §0). Where lean-ctx does something
that is not a token method — a shell allowlist, a file editor, secret redaction, code-smell
scans — rtok does not grow a copy; the native Claude Code feature or nothing takes over.

---

## 0. What lean-ctx is doing here today (2026-09-07)

| Surface | Detail | Source |
|---|---|---|
| Binary | lean-ctx 3.10.0, Homebrew, daemon running, dashboard :3333 | `lean-ctx doctor` |
| MCP | tool profile `lean`: 12 tools allowed in `~/.claude/settings.json` (`ctx_read`, `ctx_search`, `ctx_tree`, `ctx_overview`, `ctx_plan`, `ctx_metrics`, `ctx_compress`, `ctx_session`, `ctx_knowledge`, `ctx_graph`, `ctx_retrieve`, `ctx_provider`); the server also serves `ctx_shell` / `shell`, `ctx_glob`, `ctx_patch`, `ctx_compose`, `ctx_expand`, `ctx_callgraph`, `ctx_call` | settings.json, ToolSearch |
| Hooks (9) | `PreToolUse(Bash)` → `hook rewrite` (output compression + the shell allowlist, 247 commands + `shell_allowlist_extra`); `PreToolUse(Read\|Grep\|Glob\|…)` → `hook redirect` (refuses native Grep/Glob, reroutes `cat` / `sed -n` to `ctx_read`); `PostToolUse(Read)` → `hook read-dedup`; `hook observe` on `PostToolUse(.*)`, `PreCompact`, `SessionEnd`, `SessionStart`, `Stop`, `UserPromptSubmit` (metering plus the policy banner) | settings.json lines 109–811 |
| Banner | `SessionStart`: the "lean-ctx active: ALWAYS use ctx_*" list; every `UserPromptSubmit`: the "lean-ctx policy (mechanically enforced)" line; `~/.claude/CLAUDE.md`: the "lean-ctx — Replace Mode" section, 196 words, loaded into every session of every project | this session's hook context |
| Proxy | `ANTHROPIC_BASE_URL=http://127.0.0.1:8788` is set in the shell environment while no lean-ctx proxy runs — `lean-ctx doctor` flags it as the cause of 401s | `lean-ctx doctor` |
| State | `~/.local/share/lean-ctx`: 841 session dirs, `knowledge/`, `read_cache/`, archives 30.9 MB, `evidence_ledger.jsonl` 2.8 MB, `metering.jsonl` 1.4 MB | `ls` |
| Claim | 35.2 M tokens saved all-time (66.6 %) by its own meter | `lean-ctx gain` |

Two numbers in `research.md` describe different lean-ctx configurations: "78 tools, 3.1 K per
turn" (§4, default profile when surveyed) and "12 tools, ~704 description tokens" (`rtok doctor`,
P8 gate, `lean` profile). Step 1 below re-measures on the profile actually installed.

The same `PreToolUse(Bash)` event carries three rewriters today: `rtk hook claude`, `rtok hook
PreToolUse` (the `rtok` binary is not on `PATH`, so this entry does nothing yet) and
`lean-ctx hook rewrite`. Claude Code runs matching hooks together; which `updatedInput` wins is
not defined. One rewriter per event is a precondition of any measurement.

---

## 1. Mapping: each lean-ctx behaviour → the rtok owner

Legend: **covered** = rtok already does it, no code; **partial** = owner exists, one gap named in
§2; **drop** = deliberately not replicated; **native** = Claude Code's own feature takes over.

| # | lean-ctx | rtok owner | Verdict | Notes |
|---|---|---|---|---|
| 1 | `ctx_read` `full` / `lines:N-M` / `map` / `signatures`, cached re-reads (~13 tokens), output cap | `read` MCP `read` (`mode`, `range`); per-session `read_cache` + archive sha256 → `unchanged since {id}`; `max_chars` head/tail cap with archive id (`src/plugins/read/{mod,cache}.rs`) | covered | Same four modes, same dedup outcome; rtok records the dedup as a `Measurement`, lean-ctx as a claim. |
| 2 | `ctx_read` `anchored` / `raw` / `diff` / `auto` / `reference` / `task` / `aggressiveness`, batch `paths`, `fresh` | — | drop | `raw` is `full`; `diff` is `git diff` through `cmd`; `anchored` only feeds `ctx_patch` (row 3); `auto` / `task` / `reference` are heuristics that change bytes between turns (against the byte-stable rule). Batch `paths` is I-07 territory: add only with a Check on the measured Read tail. |
| 3 | `ctx_patch` (line + hash edits, `replace_unique`, `replace_symbol`) | — | native | Claude Code's `Edit` / `Write` do this. rtok has no edit surface and `PostToolUse` cannot change tool results (`CLAUDE.md`). If `rtok stats` ever shows `Edit` inputs dominating, that is an idea, not a port. |
| 4 | `ctx_search` regex / `symbol` | `read` `search` (regex, `search_max`), `graph` `symbol` / `outline` | covered | |
| 5 | `ctx_search` semantic (BM25 / dense, ONNX embeddings) | — | drop | I-22 (v0.2+): no measured need; FTS5 is enough for `memory`. |
| 6 | `ctx_glob`, `ctx_tree` | `read` `tree` (`tree_depth`); native `Glob` | covered | rtok does not deny native `Glob` / `Grep` (I-08 stays unpromoted), so no glob tool is needed. |
| 7 | `ctx_compose` (task-ranked files with inline source) | — | drop | Two or three calls (`graph` `impact` / `symbol`, `read` `search`) instead of one ranked bundle. Fewer tools is the P4 goal ("5 tools, 0 banner"); a ranked map is I-28 and needs a P7-style A/B first. |
| 8 | `ctx_callgraph` `callers` / `callees` / `trace` / `risk`, `ctx_call` | `graph` `callers`, `impact(name, depth)` (BFS over stored call edges) | partial | `callees` and `trace from→to` are the same edges read the other way; see G4 — add only if a session is shown to need them. `risk` is a score, not a method. |
| 9 | `ctx_shell` / `hook rewrite`: ~95 output patterns, `[exit:N]` kept, `[Archived:ID]` above `inline_max_bytes`, `raw=true` | `cmd`: `PreToolUse(Bash)` rewrites to `rtok run -- <cmd>`, archives raw stdout+stderr first, keeps the exit code, prints the family formatter's output plus `[rtok {id} · … · expand: rtok expand {id}]` (`src/plugins/cmd/{hook,run,formatters}.rs`, `rules/default.toml`) | partial | Same shape, lossless in the same way. Coverage is ~18 families against lean-ctx's ~95 (I-05) and — the gap that matters in this repo — the formatter keys on `argv[0..1]`, so `mise exec -- cargo test`, `just check` and `cd x && …` fall through to raw output. G1 and G2. |
| 10 | `ctx_shell` `run_in_background`, job status / cancel | — | native | Claude Code's `Bash` has `run_in_background`. |
| 11 | Shell allowlist (`lean-ctx allow`, `[BLOCKED]`, `shell_security`) | — | native | Not a token method. Claude Code `permissions.allow` / `permissions.deny` with `Bash(docker:*)` patterns is the platform feature; `cmd/AGENTS.md` forbids a shell parser in rtok. The user decides whether to carry `shell_allowlist_extra` over or drop it (step 4). |
| 12 | `ctx_expand` (retrieve archived output; `head` / `tail` / `search` / `json_path`; `list` / `search_all`) | `rtok expand <id> [--lines a-b] [--grep]` and the MCP `expand` tool (`src/expand.rs`, `src/mcp.rs`), fed by `cmd`, `guard`, `read`, `archive` | covered | `json_path` and archive listing are not needed; `expand` records a `Measurement`. |
| 13 | `hook read-dedup` (`PostToolUse(Read)`) | `guard`: `PreToolUse` denies a duplicate `Read` / `Bash` inside `window_turns` (8) with `duplicate; rtok expand {id}`, `Measurement{kind:"guard"}`; `read` cache for MCP reads | covered | rtok denies before the bytes are produced; lean-ctx dedups after. |
| 14 | `hook redirect` (deny native `Grep` / `Glob`, reroute `cat` / `sed` to `ctx_read`) | `read` `PreToolUse` advice only for files over `native_max_bytes` (32 KB) | drop | I-08 says: promote only if `doctor` shows Grep/Glob dominating Read-class tokens. A second, independent Grep/Glob gate (`~/.claude/hooks/cbm-code-discovery-gate`, codebase-memory) is not lean-ctx's and is left to the user. |
| 15 | Policy banner (SessionStart list, UserPromptSubmit line, CLAUDE.md section) | `inject`: one budget (800 tokens), byte-stable, modes as data, nothing on `UserPromptSubmit` | drop | The text is not ported (I-18: nudges only with an A/B; D5: injections are re-read every turn). Deleting the CLAUDE.md section alone saves ~260 tokens per session in every project. |
| 16 | `ctx_session` (task / finding / decision / handoff, snapshots) | `inject` `PreCompact` checkpoint + restore (T2.5) | covered | Same purpose — survive compaction — with no per-turn text. lean-ctx's 841 session dirs are not imported. |
| 17 | `ctx_knowledge` (`remember` / `recall` / `export` / `import`) | `memory`: `mem_save` / `mem_search` / `mem_get`, SessionStart recall of titles ≤ 200 tokens, `rtok memory import <jsonl>` (dedupe by body sha256) | covered | Step 3: `lean-ctx knowledge export` → `rtok memory import`; G6 checks the export shape. |
| 18 | Metering: `gain`, `evidence_ledger.jsonl`, CEP, `value-report`, dashboard | `measure`: `Measurement` rows, `rtok stats` (`--since`, `--plugin`, `--compare`), `rtok tui` (P15) | covered | Definitions differ (rtok counts only what a `Measurement` row proves), so the lean-ctx ledger is not imported; the before/after numbers come from step 1 and step 7. |
| 19 | `lean-ctx proxy` (`ANTHROPIC_BASE_URL` :8788) | `proxy`: `rtok proxy` (passthrough / compress, `archive` and `toon` filters, one `usage` row per request, T5.2 lifecycle) | covered | Today's env var points at nothing (§0). Step 5. |
| 20 | Path jail, secret redaction | `read` root guard (`cwd` + `allow_paths`); **no redaction anywhere** | covered / drop | Redaction is dropped on purpose: `cmd` is lossless and its T3.3 Check requires a fake AWS key to pass unchanged. State this to the user as a behaviour change, not a regression. |
| 21 | Code analysis and workflow extras: `health`, `smells`, `visualize`, `deps`, `overview`, `explore`, `snapshot`, `pack --pr`, `plan`, `compile`, `ghost`, `buddy`, `compliance` | — | drop | Not token methods (§1 of `plan.md`, D6). |
| 22 | Multi-host hooks (Cursor, Copilot, Codex, Pi) | `setup`: claude, cursor, codex (T10.1–T10.3) | covered / I-17 | Copilot and others are I-17. |

Net: 13 covered, 2 partial (`cmd`, `graph`), 4 native, the rest dropped by an existing decision.
Nothing in lean-ctx needs a **new plugin**; the two partial rows land in `cmd` and `graph`.

---

## 2. Gaps to fill in rtok before the switch (proposed `plan.md` changes)

Each is a candidate task in the CLAUDE.md shape (≤ 200 LOC, ≤ 3 files, a Check). None exists in
`plan.md` yet; promote the ones the user wants, in this order.

**G1 `cmd` — runner prefixes (this repo's blind spot).** `formatters::format` matches on
`(bin, sub)` = `argv[0..1]` (`src/plugins/cmd/formatters.rs:25-60`). Every cargo command in this
repo is `mise exec -- cargo <cmd>` (`CLAUDE.md`), the gate is `just check`, and agents prefix
`cd <dir> &&`; all three reach the formatter as `mise` / `just` / `cd` and get raw output, so the
biggest Bash payloads here are the ones `cmd` never shortens. Do: strip runner prefixes before
matching (`mise exec --`, `mise x --`, `env K=V`, `time`, `nice`, `sudo`, a leading `cd … &&`),
and for `just <recipe>` / pipelines fall back to a content-keyed match (cargo's `Compiling` /
`test result:` / `error[` lines identify the family without argv). Files: `formatters.rs`,
`rules/default.toml`, `tests/cmd_golden/`. Check: `rtok filter --cmd "mise exec -- cargo test"`
on the golden cargo output is byte-identical to `--cmd "cargo test"`; `just check` output is
formatted as its inner cargo steps; `rtok stats --plugin cmd` over one session of this repo shows
`cmd` rows on ≥ 90 % of Bash calls (today: the `mise` / `just` share is 0 %).

**G2 `cmd` — families used here that are raw today** (I-05, promoted for the commands this
machine actually runs): `cargo fmt --check` (a diff: keep file names and hunk counts),
`docker ps` / `logs` / `images`, `gh run list` / `view`, `curl -s` returning JSON (keep keys,
cap arrays), `brew`, `git commit` / `git show --stat`. Data rows in `rules/default.toml` where
`max_lines` / `keep` / `drop` suffice, a formatter only for `cargo fmt`. Check: one golden file per
family; `research.md` §2 Bash table re-run shows the family's share of Bash tokens falling.

**G3 `read` — batch reads.** Skip unless `rtok stats` shows the Read-class call count, not
bytes, dominating; one call for N files is a round-trip saving the estimator cannot see.

**G4 `graph` — `callees` and `trace`.** One query each over the existing call edges. Skip
unless a session in this repo is shown asking for a callee list; `impact` covers the review
case.

**G5 `doctor` — cutover checks.** Two lines in `rtok doctor`: (a) more than one command
rewriter on `PreToolUse(Bash)` (today: rtk, rtok, lean-ctx) — name them; (b)
`ANTHROPIC_BASE_URL` set to a port nothing answers. Both are what breaks a migration silently.
Check: `instructions_lists_four_injectors`-style unit test with a settings fixture holding the
three rewriters; `doctor` names all three and exits 0.

**G6 `memory` — import the knowledge base.** Run `lean-ctx knowledge export` and compare its
shape with what `rtok memory import` reads (T6.3: generic JSONL, `title` + `body`, dedupe by
sha256). If it differs, a ≤ 20-line adapter under `tools/`, not a plugin change. Check: import
twice, the second run inserts 0 rows; `mem_search` finds a known lean-ctx fact.

Explicitly **not** filled, by prior decision: the shell allowlist (row 11, native permissions),
`ctx_patch` (row 3, native Edit), semantic search (I-22), `ctx_compose` (I-28), the banner (I-18),
Grep/Glob denial (I-08), secret redaction (row 20).

---

## 3. Cutover, in order (each step reversible on its own)

0. **Prerequisite.** A released `rtok` on `PATH` (Gate P18) — or `cargo install --path .` for a
   dry run — and `rtok doctor` showing its 7 hooks, `mcpServers.rtok`, and the proxy chain. Until
   then the `rtok hook …` entries already in `settings.json` are dead entries.
1. **Measure before.** `rtok stats --save-baseline` on the last two weeks of JSONL; `rtok doctor`
   for per-server MCP description tokens (lean-ctx `lean` profile) and the hook count;
   `lean-ctx gain --json` as its own claim. Numbers into `research.md` §2 with the date.
2. **Fill G1, G2, G5** (G6 only if step 3 needs it) as `plan.md` tasks, committed one by one.
3. **Knowledge.** `lean-ctx knowledge export` → `rtok memory import`; verify with `mem_search`.
   Sessions and snapshots are not migrated (`inject`'s checkpoint replaces them).
4. **Settings** (`~/.claude/settings.json`, `~/.claude/CLAUDE.md`, `~/.claude.json`):
   remove the 9 lean-ctx hook entries and the `rtk hook claude` entry (one Bash rewriter:
   `rtok`); remove `mcp__lean-ctx__*` from the allowed tools and the lean-ctx server from
   `mcpServers`; delete the "lean-ctx — Replace Mode" section from the global CLAUDE.md; lift the
   `Grep` / `Glob` deny if it was lean-ctx's (the codebase-memory gate is a separate decision);
   decide what to do with `shell_allowlist_extra` — carry the list into `permissions` or drop it.
   `rtok setup` adds rtok's entries idempotently and leaves foreign hooks alone
   (`apply_twice_then_remove_keeps_foreign`), so the removals are manual or `lean-ctx unwrap`.
   The target file is `bench/configs/rtok.json` (config B) plus the non-token hooks.
5. **Environment.** `lean-ctx proxy cleanup`; `ANTHROPIC_BASE_URL` → `rtok proxy` (T5.2 sets it)
   or unset. Check with `rtok doctor`: the chain must end at Anthropic, not at :8788.
6. **Stop lean-ctx.** `lean-ctx daemon stop`; keep `~/.local/share/lean-ctx` and the Homebrew
   install until step 8 so step 9 stays cheap.
7. **Measure after.** Same three numbers as step 1 after one week, plus the P9 bench with
   `RTOK_BENCH_LIVE=1` (six tasks × three runs × two configs, real API cost — the user's call).
   Gate P9's rule decides: adopt config B if cost per passed task is lower and the pass rate is
   equal; otherwise keep only the measured winners.
8. **Retire.** `lean-ctx uninstall`, delete `~/.local/share/lean-ctx`, remove the Homebrew
   package. Record the date and the step-7 numbers in `research.md` §2.
9. **Rollback** (any time before 8): `lean-ctx wrap` / `lean-ctx setup` restores its hooks and
   server; `rtok setup --remove` takes rtok's out. Both are idempotent.

---

## 4. What the migration does not promise

- rtok will not block commands, redact secrets, rank files by task, or edit files. Those were
  never token methods, and `cmd` is lossless by contract.
- "95 patterns" is not the target; the target is a `Measurement` row on the Bash calls this
  machine actually runs (G1's ≥ 90 % Check), family by family.
- The per-turn banner is not replaced by a smaller banner. `inject` carries the memory recall
  and the modes inside one 800-token budget, and nothing on `UserPromptSubmit`.
