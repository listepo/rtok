//! The only place the worktree inventory spawns git.

use std::path::Path;
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use super::{Record, parse_porcelain};

fn git(dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("cannot run git {}", args.join(" ")))
}

fn git_ok(dir: &Path, args: &[&str]) -> Result<Output> {
    let out = git(dir, args)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("git {}: {}", args.join(" "), err.trim());
    }
    Ok(out)
}

fn stdout(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git_ok(dir, args)?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub fn list(repo: &Path) -> Result<Vec<Record>> {
    let out = git_ok(repo, &["worktree", "list", "--porcelain", "-z"])?;
    Ok(parse_porcelain(&out.stdout))
}

/// Modified, staged or untracked files; ignored files (build output) do not count.
pub fn is_dirty(worktree: &Path) -> Result<bool> {
    Ok(!stdout(worktree, &["status", "--porcelain"])?.is_empty())
}

/// The remote's default branch, `origin/main` when `origin/HEAD` is not set.
pub fn default_base(repo: &Path) -> String {
    stdout(
        repo,
        &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"],
    )
    .ok()
    .filter(|base| !base.is_empty())
    .unwrap_or_else(|| "origin/main".into())
}

/// Squash-aware: `rev` is merged when merging it into `base` would produce exactly
/// `base`'s tree. `git branch --merged` cannot see this — a squash merge leaves the
/// branch's own commits outside the base's history.
pub fn is_merged(repo: &Path, base: &str, rev: &str) -> Result<bool> {
    let merge = git(repo, &["merge-tree", "--write-tree", base, rev])?;
    match merge.status.code() {
        Some(0) => {}
        // Conflicts: either real divergence, or the base moved on next to the same lines.
        Some(1) => return is_patch_equivalent(repo, base, rev),
        _ => bail!(
            "git merge-tree: {}",
            String::from_utf8_lossy(&merge.stderr).trim()
        ),
    }
    let merged_tree = String::from_utf8_lossy(&merge.stdout);
    let base_tree = stdout(repo, &["rev-parse", &format!("{base}^{{tree}}")])?;
    Ok(merged_tree.lines().next() == Some(base_tree.as_str()))
}

/// The second merged signal: the branch's whole diff as one commit already has a
/// patch-equivalent commit in `base` — a squash merge that `base` later edited around,
/// which [`is_merged`]'s trial merge reads as a conflict. Writes one unreferenced
/// commit object, as the trial merge writes a tree.
fn is_patch_equivalent(repo: &Path, base: &str, rev: &str) -> Result<bool> {
    let fork = stdout(repo, &["merge-base", base, rev])?;
    let tree = format!("{rev}^{{tree}}");
    // An identity of its own: `commit-tree` fails where `user.name` is not configured.
    let id = ["-c", "user.name=rtok", "-c", "user.email=rtok@localhost"];
    let squash = ["commit-tree", &tree, "-p", &fork, "-m", "squash"];
    let squashed = stdout(repo, &[&id[..], &squash[..]].concat())?;
    Ok(stdout(repo, &["cherry", base, &squashed])?.starts_with('-'))
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str().context("worktree path is not UTF-8")
}

pub fn lock(repo: &Path, worktree: &Path, reason: &str) -> Result<()> {
    let path = path_arg(worktree)?;
    git_ok(repo, &["worktree", "lock", "--reason", reason, path]).map(drop)
}

pub fn unlock(repo: &Path, worktree: &Path) -> Result<()> {
    git_ok(repo, &["worktree", "unlock", path_arg(worktree)?]).map(drop)
}

/// Never `--force`: git refuses a worktree with modified or untracked files. For a
/// record whose directory is gone this drops that one record.
pub fn remove(repo: &Path, worktree: &Path) -> Result<()> {
    git_ok(repo, &["worktree", "remove", path_arg(worktree)?]).map(drop)
}

/// `-D`: the caller has established "merged" squash-aware, which `-d` cannot see.
pub fn delete_branch(repo: &Path, branch: &str) -> Result<()> {
    git_ok(repo, &["branch", "-D", "--", branch]).map(drop)
}

/// From the local remote-tracking ref: no network on this path.
pub fn has_remote_branch(repo: &Path, branch: &str) -> bool {
    let name = format!("refs/remotes/origin/{branch}");
    git(repo, &["show-ref", "--verify", "-q", &name]).is_ok_and(|out| out.status.success())
}
