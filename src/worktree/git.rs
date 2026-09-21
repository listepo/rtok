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
        Some(1) => return Ok(false), // conflicts: merging would change something
        _ => bail!(
            "git merge-tree: {}",
            String::from_utf8_lossy(&merge.stderr).trim()
        ),
    }
    let merged_tree = String::from_utf8_lossy(&merge.stdout);
    let base_tree = stdout(repo, &["rev-parse", &format!("{base}^{{tree}}")])?;
    Ok(merged_tree.lines().next() == Some(base_tree.as_str()))
}
