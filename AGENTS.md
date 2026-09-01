# rtok — instructions for agents

**What this is.** One Rust binary that reduces tokens for AI coding agents; every method is a plugin. Three surfaces: `rtok hook <event>` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`).

**Before you start.** Delegate one-off shell (build, test, git, cargo), API/HTTP, and file listings; do not run those from the main context.

**Read first.** `plan.md` — decisions and every open task with its Check. `roadmap.md` — plan per internal plugin. `ideas.md` — propositions; do not implement from there. `done.md` — finished tasks. `architecture.md` — module map and plugin contract. `research.md` — evidence. Before touching a plugin, read its `src/plugins/<id>/AGENTS.md`. Do not add work that is not in `plan.md`; propose it as a plan change instead.

**Toolchain.** Rust is pinned in `mise.toml`. Run everything as `mise exec -- cargo <cmd>` (or `mise activate` your shell). Never install or switch a global toolchain.

**Workflow.** Take the next unblocked task in `plan.md`. Branch `rtok/<task-id>`. Stay within ≤ 200 LOC and ≤ 3 files. Run the task's Check verbatim, then `make check`. Commit as `<task-id>: <title>`. Move the task from `plan.md` to `done.md` with `Status: done <date>` and the Check result. New plugin: follow `docs/plugin-authoring.md`.

**Rules that never bend.**
- Fail open: a hook exits 0 in ≤ 10 ms even on error, with unmodified input.
- Lossless by default: anything shortened is retrievable via `expand <id>`.
- A saving that is not a `Measurement` row does not exist.
- Injected context stays under the budget and byte-stable across turns.
- PostToolUse can only add context; it cannot change tool results.
- No new dependency without a one-line reason in the commit message.

**Models.** Any provider. **Low-cost** for mechanical work, and for any task a cheap model can finish. **Mid-tier** for coding; pick the cheaper mid model when the task is small. **High-performance** for research and investigation only after the user confirms — do not switch up on your own.

Keep this file under 350 tokens; it is loaded into every session.
