---
name: worktrees
description: Load before editing a repo: each task gets its own git worktree via rtok unless AGENTS.md or the user says otherwise.
---

# worktrees

Every task that edits a repository gets its own worktree, never the shared main checkout,
unless `AGENTS.md` or the user says otherwise.

Use rtok, not raw `git worktree`: git records no owner, age or size and never cleans build output.

No `rtok`? `ketch install pyrlyn/rtok`; meanwhile `git worktree add --lock --reason
"<owner> | <task> | <date>" --no-track -b <task> <path> origin/main`.

## Create

MCP `worktree_add` (task, slug) or `rtok worktree add <task> [slug] --owner "<provider> / <model>"`
— the CLI runs inside the repository; both give the path and bind your agent id, which you
quote when you report. One location (`_worktrees/<repo>-<task>`, never `/tmp`), one name (branch
`<task>[-<slug>]` off a fresh `origin/<default>`, no upstream, so a bare `git push` cannot
reach `main`), one owner (lock `<owner> | <task> | <date>`).
A host-made worktree (Cursor, Codex, Kilo, Devin, Grok, MiMo, omp, Antigravity): `worktree_adopt`
(`path`; `task` on a detached HEAD) or `rtok worktree adopt --task <task>`.

## See

`worktree_list` / `rtok worktree list`: owner, agent, origin, state, sizes, orphans; `--json`.

## Finish

After the PR is merged, `worktree_remove` (or `rtok worktree remove <path|task>` from the main
checkout) removes your worktree with its merged branch; it refuses uncommitted files, a foreign
lock and an unmerged branch (`--keep-branch` keeps that). Then delete the remote branch if the
forge did not, and `git fetch --prune`. `rtok worktree gc --yes` sweeps many.

## Free disk

`rtok worktree clean [paths]`, `--yes` to apply — deletes idle `CACHEDIR.TAG` caches such as
`target/`, keeps the worktree.

## Never

- Never `rm -rf` a worktree, never `--force` past a refusal, never `git worktree prune`.
- Never remove, unlock, move or clean a worktree locked by another owner or without a
  reason, or with changes you did not make: report it.
- A directory with a `.git` *file* that `git worktree list` omits is an orphan: report it, never delete it.
