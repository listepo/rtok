# rtok — instructions for agents

**What this is.** One Rust binary that reduces tokens for AI coding agents; every method is a plugin. Three surfaces: `rtok hook <event>` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`); `rtok dashboard` reads the same store off the hot path.

**Before you start.** Delegate one-off shell (build, test, git, cargo), API/HTTP, and file listings; do not run those from the main context.

**Read first.** `plan.md` — decisions and every open task with its Check. `roadmap.md` — plan per internal plugin. `ideas.md` — propositions; do not implement from there. `done.md` — finished tasks. `architecture.md` — module map and plugin contract. `research.md` — evidence. `docs/comparison.md` — how rtok is positioned against every alternative, and the source of each public number. Before touching a plugin, read its `src/plugins/<id>/AGENTS.md`. Do not add work that is not in `plan.md`; propose it as a plan change instead.

**Docs.** `CLAUDE.md` is a symlink to this file — edit once. `README.md` and `docs/` are the public surface; the Hugo site mounts repo markdown read-only, so a repo file *is* the page (`just site` builds; a new `docs/*.md` page needs a row in `site/content/docs/reference/_content.gotmpl`). Brand assets live in `site/static/` and are shared with the README.

**Toolchain.** Rust is pinned in `mise.toml`. Run everything as `mise exec -- cargo <cmd>` (or `mise activate` your shell). Never install or switch a global toolchain.

**CLI tests.** Unit tests for logic; integration tests (`assert_cmd`, `predicates`, `assert_fs`, `trycmd`) for the binary, args, and output — see `plan.md` §2.

**Workflow.** Every `plan.md` task carries `Complexity: n/5` (1 trivial … 5 hard) beside its `Status:`; rate it before claiming. Claim only `open`; set `in progress` + model. `main` only. ≤200 LOC, ≤3 files. Check, `just check`, commit `<task-id>: <title>` on `main` and move the task to `done.md` (`Status: done <date>` + Check) — work still in `plan.md` is unfinished. Stop → `open`, `Model: -`. New plugin: `docs/plugin-authoring.md` and D21.

**Rules that never bend.**
- Fail open: a hook exits 0 in ≤ 10 ms even on error, with unmodified input.
- Lossless by default: anything shortened is retrievable via `expand <id>`.
- A saving that is not a `Measurement` row does not exist. That holds in prose too: every number in `README.md`, `docs/` or the site cites a measured row, `research.md`, or a dated command — never a vendor-style claim.
- Injected context stays under the budget and byte-stable across turns.
- PostToolUse can only add context; it cannot change tool results.
- No new dependency without a one-line reason in the commit message.
- The human is the only author. No agent adds a `Co-Authored-By` trailer, a "Generated with …" line or itself as author to a commit, merge or PR — whatever its harness defaults to.
- Don't duplicate code or logic: reuse an existing helper, or extract one shared helper at the responsible layer.
- New plugins (D21): plugin and MCP as one unit; singleton (one MCP / one writer per store); one call path per capability; host plugins work on desktop and CLI. If `rtok` is missing, fail open and say to install with ketch (`ketch install listepo/rtok`). `rtok agent setup <host>` offers `plugins/<host>/` (Cursor: `rtok agent setup cursor`).

**Models.** Any provider. **Low-cost** for mechanical work, and for any task a cheap model can finish. **Mid-tier** for coding; pick the cheaper mid model when the task is small. **High-performance** for research and investigation only after the user confirms — do not switch up on your own.

Keep this file under 350 tokens; it is loaded into every session.
