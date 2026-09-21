//! `rtok worktree gc` (T153): remove finished worktrees and their merged branches, and
//! drop the records of worktrees whose directory is already gone. [`decide`] is pure;
//! [`run`] applies one verdict at a time and never forces anything.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Serialize;

use super::list::usage;
use super::{Entry, State, git, inventory};
use crate::render::Col;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Merged, clean, idle and not held by anyone else: remove the worktree.
    Remove,
    /// The directory is gone: drop this one record (never a blanket `prune`).
    DropRecord,
    Keep(String),
}

pub struct Policy<'a> {
    /// Whose locks may be opened; every other lock is a hard stop.
    pub owner: Option<&'a str>,
    pub idle: Duration,
    pub now: SystemTime,
}

/// `modified` is the newest mtime under the worktree (T151) and `current` says the
/// command runs from inside it.
pub fn decide(entry: &Entry, modified: Option<SystemTime>, current: bool, p: &Policy) -> Verdict {
    let keep = |why: &str| Verdict::Keep(why.into());
    let record = &entry.record;
    if entry.state == State::Main {
        return keep("main checkout");
    }
    if current {
        return keep("the worktree this command runs from");
    }
    if record.held_against(p.owner) {
        let owner = record.owner().map(|o| o.owner);
        return keep(&owner.map_or("locked, owner unknown".into(), |o| format!("locked by {o}")));
    }
    let active = modified.is_some_and(|m| p.now.duration_since(m).unwrap_or_default() < p.idle);
    match entry.state {
        State::Stale => Verdict::DropRecord,
        State::Dirty => keep("uncommitted changes"),
        State::Unmerged => match &record.branch {
            Some(b) => keep(&format!("not merged into the base; check `gh pr view {b}`")),
            None => keep("detached HEAD not merged into the base"),
        },
        _ if active => keep("modified within the idle window"),
        _ => Verdict::Remove,
    }
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub path: String,
    pub branch: Option<String>,
    /// `remove`, `drop-record` or `keep`.
    pub action: &'static str,
    /// Why it was kept, what was done, or git's message when a step failed.
    pub note: String,
    pub failed: bool,
}

/// Unlock, remove without `--force`, then delete the branch when it is merged. A failed
/// removal puts the lock back, so a half-done run never strips another run's protection.
fn apply(repo: &Path, entry: &Entry) -> anyhow::Result<String> {
    let record = &entry.record;
    if record.locked.is_some() {
        git::unlock(repo, &record.path)?;
    }
    if let Err(e) = git::remove(repo, &record.path) {
        if let Some(reason) = &record.locked {
            git::lock(repo, &record.path, reason)?;
        }
        return Err(e);
    }
    let Some(branch) = record.branch.as_deref().filter(|_| entry.merged) else {
        return Ok("removed; branch kept".into());
    };
    git::delete_branch(repo, branch)?;
    Ok(match git::has_remote_branch(repo, branch) {
        true => format!("removed with its branch; remote left: git push origin --delete {branch}"),
        false => "removed with its branch".into(),
    })
}

pub fn run(cwd: &Path, policy: &Policy, yes: bool) -> anyhow::Result<Vec<Outcome>> {
    let here = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let entries = inventory(cwd)?;
    // Removing worktrees from inside one of them: git needs a directory that survives.
    let repo = entries
        .first()
        .map_or(cwd, |main| main.record.path.as_path());
    let outcomes = entries.iter().map(|entry| {
        let path = &entry.record.path;
        let current = path.canonicalize().is_ok_and(|p| here.starts_with(p));
        // The walk is the expensive part: only a removal candidate needs its mtime.
        let modified = (entry.state == State::Merged).then(|| usage(path).modified);
        let verdict = decide(entry, modified.flatten(), current, policy);
        let (action, planned) = match &verdict {
            Verdict::Remove => ("remove", "merged, clean and idle"),
            Verdict::DropRecord => ("drop-record", "directory is gone"),
            Verdict::Keep(why) => ("keep", why.as_str()),
        };
        let done = (yes && action != "keep").then(|| apply(repo, entry));
        Outcome {
            path: path.display().to_string(),
            branch: entry.record.branch.clone(),
            action,
            failed: matches!(done, Some(Err(_))),
            note: match done {
                Some(Ok(note)) => note,
                Some(Err(e)) => format!("failed, kept: {e:#}"),
                None => planned.into(),
            },
        }
    });
    Ok(outcomes.collect())
}

pub fn to_table(outcomes: &[Outcome], yes: bool) -> String {
    let dash = || "-".to_string();
    let mut lines = vec![["action", "path", "branch"].map(String::from).to_vec()];
    lines.extend(outcomes.iter().map(|o| {
        let branch = o.branch.clone().unwrap_or_else(dash);
        vec![o.action.into(), o.path.clone(), branch]
    }));
    let cols = [Col::left(0), Col::left(0), Col::right(0)];
    let notes = outcomes.iter().map(|o| o.note.as_str());
    let mut out = super::noted_table(&cols, &lines, notes);
    let planned = outcomes.iter().filter(|o| o.action != "keep").count();
    if !yes && planned > 0 {
        out.push_str(&format!(
            "\ndry run: {planned} to act on, nothing changed; rerun with --yes\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::Record;
    use super::*;
    use rstest::rstest;

    fn at(secs: u64) -> SystemTime {
        std::time::UNIX_EPOCH + Duration::from_secs(secs)
    }

    const ME: &str = "Claude Code / sonnet";
    const MINE: &str = "Claude Code / sonnet | t1 | 2026-09-22";
    const THEIRS: &str = "Cursor / grok | t2 | 2026-09-22";

    #[rstest]
    #[case::main(State::Main, None, false, 0, "keep:main checkout")]
    #[case::current(
        State::Merged,
        None,
        true,
        0,
        "keep:the worktree this command runs from"
    )]
    #[case::finished(State::Merged, None, false, 0, "remove")]
    #[case::my_lock(State::Merged, Some(MINE), false, 0, "remove")]
    #[case::foreign_lock(State::Merged, Some(THEIRS), false, 0, "keep:locked by Cursor / grok")]
    #[case::bare_lock(State::Merged, Some(""), false, 0, "keep:locked, owner unknown")]
    #[case::active(
        State::Merged,
        None,
        false,
        990,
        "keep:modified within the idle window"
    )]
    #[case::dirty(State::Dirty, None, false, 0, "keep:uncommitted changes")]
    #[case::unmerged(
        State::Unmerged,
        None,
        false,
        0,
        "keep:not merged into the base; check `gh pr view t1`"
    )]
    #[case::gone(State::Stale, None, false, 0, "drop-record")]
    #[case::gone_mine(State::Stale, Some(MINE), false, 0, "drop-record")]
    #[case::gone_theirs(State::Stale, Some(THEIRS), false, 0, "keep:locked by Cursor / grok")]
    #[case::gone_bare_lock(State::Stale, Some(""), false, 0, "keep:locked, owner unknown")]
    fn verdicts(
        #[case] state: State,
        #[case] locked: Option<&str>,
        #[case] current: bool,
        #[case] modified: u64,
        #[case] want: &str,
    ) {
        let record = Record {
            branch: Some("t1".into()),
            locked: locked.map(Into::into),
            ..Record::default()
        };
        let entry = Entry {
            record,
            state,
            merged: state == State::Merged,
        };
        let policy = Policy {
            owner: Some(ME),
            idle: Duration::from_secs(100),
            now: at(1_000),
        };
        let got = match decide(&entry, Some(at(modified)), current, &policy) {
            Verdict::Remove => "remove".to_string(),
            Verdict::DropRecord => "drop-record".to_string(),
            Verdict::Keep(why) => format!("keep:{why}"),
        };
        assert_eq!(got, want);
    }

    #[test]
    fn without_an_owner_every_lock_is_a_hard_stop() {
        let record = Record {
            locked: Some(MINE.into()),
            ..Record::default()
        };
        let entry = Entry {
            record,
            state: State::Merged,
            merged: true,
        };
        let policy = Policy {
            owner: None,
            idle: Duration::ZERO,
            now: at(1_000),
        };
        let verdict = decide(&entry, None, false, &policy);
        assert_eq!(verdict, Verdict::Keep(format!("locked by {ME}")));
    }
}
