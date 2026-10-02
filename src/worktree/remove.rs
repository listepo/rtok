// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok worktree remove` (T286): an agent removes its own finished worktree. [`detach`] is
//! the single-worktree removal `gc` applies too; [`run`] refuses before it touches anything.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{Record, State, git, inventory};

/// Unlock, remove without `--force`, then delete the branch when `drop_branch`. A failed
/// removal puts the lock back, so a half-done run never strips another run's protection.
pub fn detach(repo: &Path, record: &Record, drop_branch: bool) -> Result<String> {
    if record.locked.is_some() {
        git::unlock(repo, &record.path)?;
    }
    if let Err(e) = git::remove(repo, &record.path) {
        if let Some(reason) = &record.locked {
            git::lock(repo, &record.path, reason)?;
        }
        return Err(e);
    }
    let Some(branch) = record.branch.as_deref().filter(|_| drop_branch) else {
        return Ok("removed; branch kept".into());
    };
    git::delete_branch(repo, branch)?;
    Ok(match git::has_remote_branch(repo, branch) {
        true => format!("removed with its branch; remote left: git push origin --delete {branch}"),
        false => "removed with its branch".into(),
    })
}

/// Who asks: the rtok agent id and the owner name an old lock carries — either may be unknown.
pub struct Caller<'a> {
    pub agent: Option<&'a str>,
    pub owner: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct Removed {
    /// Canonical, as the claim row stores it.
    pub path: String,
    pub branch: Option<String>,
    pub note: String,
}

/// Remove the worktree at `target` (a path, else a task id) for `who`. Refuses a dirty
/// worktree, anyone else's lock, the one holding `cwd`, and an unmerged branch unless
/// `keep_branch`.
pub fn run(cwd: &Path, target: &str, who: &Caller, keep_branch: bool) -> Result<Removed> {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let as_path = cwd.join(target);
    let named = as_path.exists();
    let from = if named { as_path.as_path() } else { cwd };
    let base = git::default_base(from);
    if let Err(e) = git::fetch(from, &base) {
        eprintln!("warning: {e:#}; merged is judged against the local {base}");
    }
    let entries = inventory(from)?;
    let main = &entries
        .first()
        .context("git lists no worktree")?
        .record
        .path;
    let found: Vec<_> = match named {
        true => entries
            .iter()
            .filter(|e| real(&e.record.path) == real(&as_path))
            .collect(),
        false => entries
            .iter()
            .filter(|e| is_task(&e.record, target))
            .collect(),
    };
    let entry = match found[..] {
        [one] => one,
        [] => bail!("{target}: no worktree with that path or task"),
        _ => bail!("{target}: {} worktrees match; name the path", found.len()),
    };
    let (record, path) = (&entry.record, real(&entry.record.path));
    let shown = path.display();
    if entry.state == State::Main {
        bail!("{shown} is the main checkout");
    }
    if real(cwd).starts_with(&path) {
        bail!("{shown} holds the current directory; run from the main checkout");
    }
    // An unknown caller holds no lock: "" is never a parsed owner or agent.
    if !record.claimable_by(who.owner.unwrap_or(""), who.agent.unwrap_or("")) {
        let held = record
            .owner()
            .map_or("an unknown owner".into(), |o| o.reason());
        bail!("{shown} is locked by {held}; not removed");
    }
    match (entry.state, &record.branch) {
        (State::Stale, _) => bail!("{shown} is gone; `rtok worktree gc --yes` drops its record"),
        (State::Dirty, _) => bail!("{shown} has uncommitted or untracked files"),
        (State::Unmerged, Some(_)) if keep_branch => {}
        (State::Unmerged, Some(b)) => bail!(
            "{shown}: {b} is not merged into {base}; check `gh pr view {b}`, or pass \
             --keep-branch to remove the worktree and keep the branch"
        ),
        (State::Unmerged, None) => {
            bail!("{shown}: detached HEAD not merged into {base}; its commits would be lost")
        }
        _ => {}
    }
    let note = detach(main, record, entry.merged && !keep_branch)?;
    Ok(Removed {
        path: path.display().to_string(),
        branch: record.branch.clone(),
        note,
    })
}

/// A task id names the lock's task or the branch `<task>[-<slug>]`, case aside.
fn is_task(record: &Record, task: &str) -> bool {
    let task = task.to_ascii_lowercase();
    let locked = record
        .owner()
        .is_some_and(|o| o.task.to_ascii_lowercase() == task);
    let branch = record.branch.as_deref().map(str::to_ascii_lowercase);
    locked || branch.is_some_and(|b| b == task || b.starts_with(&format!("{task}-")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Some("t286-worktree-remove"), None, true)]
    #[case(Some("T286"), None, true)]
    #[case(Some("t2860"), None, false)]
    #[case(None, Some("me | T286 | 2026-09-27"), true)]
    #[case(Some("main"), None, false)]
    fn a_task_id_names_the_branch_or_the_lock_task(
        #[case] branch: Option<&str>,
        #[case] locked: Option<&str>,
        #[case] hit: bool,
    ) {
        let record = Record {
            branch: branch.map(Into::into),
            locked: locked.map(Into::into),
            ..Record::default()
        };
        assert_eq!(is_task(&record, "t286"), hit);
    }
}
