---
name: worktrees
description: Load before editing a repo: each task gets its own git worktree via rtok unless AGENTS.md or the user says otherwise.
---

# worktrees

Every task that edits a repository gets its own worktree, never the shared main checkout,
unless the project's `AGENTS.md` or the user says otherwise.

Use rtok, not raw `git worktree`: git records no owner, age or size for a worktree
and never cleans build output.

No `rtok` on PATH? Install it: `ketch install listepo/rtok`. Until then, fall back to
`git worktree add --lock --reason "<owner> | <task> | <date>" --no-track -b <task> <path> origin/main`.

## Create

`rtok worktree add <task> [slug] --owner "<provider> / <model>"` — run inside the repository;
it prints the new path. One location (`_worktrees/<repo>-<task>` beside the repository, never
`/tmp`), one name (branch `<task>[-<slug>]` off a fresh `origin/<default>` with no upstream, so
a bare `git push` cannot reach `main`), one owner (lock reason `<owner> | <task> | <date>`), one
worktree per task. If the host created the worktree, lock it first:
`git worktree lock --reason "<owner> | <task> | <date>" .`

## See

`rtok worktree list` — every worktree and orphan with owner, state, last session and
sizes; `--json` for scripts.

## Finish

After the PR is merged, from the main checkout: `rtok worktree remove <path|task>` removes
your worktree with its merged branch; it refuses uncommitted files, a foreign lock and an
unmerged branch (`--keep-branch` keeps that). Then delete the remote branch if the forge did
not, and `git fetch --prune`. `rtok worktree gc --yes` sweeps many.

## Free disk

`rtok worktree clean [paths]`, `--yes` to apply — deletes idle `CACHEDIR.TAG` caches such as
`target/` and keeps the worktree; the next build recreates them.

## Never

- Never `rm -rf` a worktree, never `--force` past a refusal, never `git worktree prune`.
- Never remove, unlock, move or clean a worktree whose lock names another owner or has no
  reason, nor one with changes you did not make — report it to the creator.
- A directory with a `.git` *file* that `git worktree list` omits is an orphan: report its
  path and size; do not delete it.
