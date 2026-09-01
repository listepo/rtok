# rtok — instructions for agents

**What this is.** One Rust binary that reduces tokens for AI coding agents; every method is a plugin. Three surfaces: `rtok hook <event>` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`).

**Read first.** `plan.md` — decisions D1–D10 and every open task with its Check. `done.md` — finished tasks. `architecture.md` — module map and plugin contract. `research.md` — evidence. Before touching a plugin, read its `src/plugins/<id>/AGENTS.md`. Do not add work that is not in `plan.md`; propose it as a plan change instead.

**Toolchain.** Rust is pinned in `mise.toml`. Run everything as `mise exec -- cargo <cmd>` (or `mise activate` your shell). Never install or switch a global toolchain.

**Workflow.** Take the next unblocked task in `plan.md`. Branch `rtok/<task-id>`. Stay within ≤ 200 LOC and ≤ 3 files. Run the task's Check verbatim, then `make check`. Commit as `<task-id>: <title>`. Move the task from `plan.md` to `done.md` with `Status: done <date>` and the Check result. New plugin: follow `docs/plugin-authoring.md`.

**Rules that never bend.**
- Fail open: a hook exits 0 in ≤ 10 ms even on error, with unmodified input.
- Lossless by default: anything shortened is retrievable via `expand <id>`.
- A saving that is not a `Measurement` row does not exist.
- Injected context stays under the budget and byte-stable across turns.
- PostToolUse can only add context; it cannot change tool results.
- No new dependency without a one-line reason in the commit message.

**Models.** Haiku implements tasks. Sonnet reviews phase gates. Opus only edits `plan.md`.

Keep this file under 350 tokens; it is loaded into every session.
