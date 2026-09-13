# rtok — instructions for agents

**What this is.** One Rust binary that reduces tokens for AI coding agents; every method is a plugin. Three surfaces: `rtok hook <event>` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`); `rtok dashboard` reads the same store off the hot path.

**Before you start.** Delegate one-off shell (build, test, git, cargo), API/HTTP, and file listings; do not run those from the main context.

**Read first.** `plan.md` — decisions and every open task with its Check. `roadmap.md` — plan per internal plugin. `ideas.md` — propositions; do not implement from there. `done.md` — finished tasks. `architecture.md` — module map and plugin contract. `research.md` — evidence. `docs/comparison.md` — how rtok is positioned against every alternative, and the source of each public number. Before touching a plugin, read its `src/plugins/<id>/AGENTS.md`. Do not add work that is not in `plan.md`; propose it as a plan change instead.

**Docs.** `CLAUDE.md` is a symlink to this file — edit once. `README.md` and `docs/` are the public surface; the Hugo site mounts repo markdown read-only, so a repo file *is* the page (`just site` builds; a new `docs/*.md` page needs a row in `site/content/docs/reference/_content.gotmpl`). Brand assets live in `site/static/` and are shared with the README.

**Toolchain.** Rust is pinned in `mise.toml`. Run everything as `mise exec -- cargo <cmd>` (or `mise activate` your shell). Never install or switch a global toolchain.

**CLI tests.** Unit tests for logic; integration tests (`assert_cmd`, `predicates`, `assert_fs`, `trycmd`) for the binary, args, and output — see `plan.md` → Reference / Working agreement.

**Workflow.** Claim only a `todo` row in `plan.md`: set Статус to `in progress` and Агент to `Provider / model`. Сложность is 1–5 in the table. `main` only. ≤200 LOC, ≤3 files. Check, `just check`, commit `<task-id>: <title>` on `main` and move the task entirely to `done.md` — work still in `plan.md` is unfinished. Stop → `todo`, clear Агент. New plugin: `docs/plugin-authoring.md` and D21.

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

**Models.** Any provider. **Low-cost** for mechanical work, and for any task a cheap model can finish. **Mid-tier** for coding; pick the cheaper mid model when the task is small. **High-performance** for research and investigation only after the user confirms — do not switch up on your own. On Cursor: grok 4.6 (no fast) for planning, refactoring, bugs; composer 2.5 (no fast) for commands, tests, file moves, scans, web. No max effort or fast without permission. ≤5 agents per project unless told otherwise. Ask if unclear; write the execution plan into the `plan.md` card before claiming. Before writing code, decide whether a ready library or framework should be used. A new dependency is allowed only if it is current (not abandoned) and the creator approved it. Packages already in `toolchain.md` may be reused without asking again. Prefer the latest versions of tools and packages, but bump already-installed ones only with the creator’s permission. Rust: reuse crates already used by sibling projects in this workspace (workspace-root `rust.md`). If this repo lacks one it should use, add a `plan.md` task — do not add the dependency silently. Extract duplicated helpers into `packages/` and depend via local `{ path = "..." }`. No version bumps without permission.

**Parent rules.** If a directory above this repository contains an `AGENTS.md` or `CLAUDE.md`, follow it too. If it conflicts with this file, ask the creator.

Keep this file under 350 tokens; it is loaded into every session.
