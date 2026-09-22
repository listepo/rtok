//! Git worktree inventory (T150): every worktree of a repository, who holds it and what
//! state it is in. Off the hot path — a CLI concern like `doctor`, never a hook.
//! Parsing and classification are pure; [`git`] is the only module that spawns git.

pub mod add;
pub mod clean;
pub mod gc;
pub mod git;
pub mod list;

/// A `render::table` with one free-text note appended per line (after a `note` header),
/// so the table itself still ends right-aligned. `gc` and `clean` print their verdicts this way.
pub(crate) fn noted_table<'a>(
    cols: &[crate::render::Col],
    lines: &[Vec<String>],
    notes: impl Iterator<Item = &'a str>,
) -> String {
    let body = crate::render::table(cols, lines);
    let notes = std::iter::once("note").chain(notes);
    body.lines()
        .zip(notes)
        .map(|(row, note)| format!("{row}  {note}\n"))
        .collect()
}

use std::path::{Path, PathBuf};

/// One record of `git worktree list --porcelain -z`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub path: PathBuf,
    pub head: Option<String>,
    /// Short branch name; `None` when detached or bare.
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    /// The lock reason; `Some("")` is a lock without one.
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

/// Parses `git worktree list --porcelain -z`: NUL-terminated attributes, an empty
/// attribute between records. `-z` keeps paths and reasons raw, so nothing is C-quoted.
pub fn parse_porcelain(out: &[u8]) -> Vec<Record> {
    let mut records: Vec<Record> = Vec::new();
    for attr in out.split(|b| *b == 0) {
        let attr = String::from_utf8_lossy(attr);
        let (key, value) = attr.split_once(' ').unwrap_or((&attr, ""));
        if key == "worktree" {
            records.push(Record {
                path: value.into(),
                ..Record::default()
            });
            continue;
        }
        let Some(record) = records.last_mut() else {
            continue;
        };
        match key {
            "HEAD" => record.head = Some(value.into()),
            "branch" => record.branch = Some(value.trim_start_matches("refs/heads/").into()),
            "bare" => record.bare = true,
            "detached" => record.detached = true,
            "locked" => record.locked = Some(value.into()),
            "prunable" => record.prunable = Some(value.into()),
            _ => {}
        }
    }
    records
}

/// Who holds a worktree, from a lock reason `<owner> | <task-id> | <date>` — ASCII on
/// purpose: plain `--porcelain` C-quotes anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub owner: String,
    pub task: String,
    pub date: String,
}

impl Owner {
    pub fn parse(reason: &str) -> Option<Self> {
        let mut parts = reason.split(" | ").map(str::trim);
        let (owner, task, date) = (parts.next()?, parts.next()?, parts.next()?);
        let filled = parts.next().is_none() && ![owner, task, date].contains(&"");
        filled.then(|| Self {
            owner: owner.into(),
            task: task.into(),
            date: date.into(),
        })
    }
}

impl Record {
    pub fn owner(&self) -> Option<Owner> {
        Owner::parse(self.locked.as_deref()?)
    }

    /// Locked by someone other than `me`. A lock without a parsable reason has an
    /// unknown owner, so it holds against everyone — never read it as "unlocked".
    pub fn held_against(&self, me: Option<&str>) -> bool {
        self.locked.is_some() && self.owner().is_none_or(|o| Some(o.owner.as_str()) != me)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The main checkout (or a bare repository): never a removal candidate.
    Main,
    /// The record exists, the directory does not. Git prunes it after 3 months when
    /// unlocked and never when locked; its branch stays "checked out" meanwhile.
    Stale,
    Dirty,
    /// Merging the branch into the base would change nothing — squash merges included.
    Merged,
    Unmerged,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Main => "main",
            State::Stale => "stale",
            State::Dirty => "dirty",
            State::Merged => "merged",
            State::Unmerged => "unmerged",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub exists: bool,
    pub dirty: bool,
    pub merged: bool,
}

/// A missing directory wins over everything, uncommitted work wins over "merged".
pub fn classify(facts: Facts) -> State {
    match facts {
        Facts { exists: false, .. } => State::Stale,
        Facts { dirty: true, .. } => State::Dirty,
        Facts { merged: true, .. } => State::Merged,
        _ => State::Unmerged,
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub record: Record,
    pub state: State,
    /// Kept beside `state`: a worktree whose directory is gone still needs to know
    /// whether its branch may go.
    pub merged: bool,
}

/// Every worktree of the repository at `repo`, classified against its default base.
/// A worktree git cannot read counts as dirty and an unanswerable merge check as
/// unmerged: both are the side on which nothing gets removed.
pub fn inventory(repo: &Path) -> anyhow::Result<Vec<Entry>> {
    let base = git::default_base(repo);
    let entries = git::list(repo)?.into_iter().enumerate();
    Ok(entries
        .map(|(i, record)| {
            let main = i == 0 || record.bare;
            let rev = record.branch.as_deref().or(record.head.as_deref());
            let merged =
                !main && rev.is_some_and(|r| git::is_merged(repo, &base, r).unwrap_or(false));
            let state = if main {
                State::Main
            } else {
                let exists = record.path.is_dir();
                classify(Facts {
                    exists,
                    dirty: exists && git::is_dirty(&record.path).unwrap_or(true),
                    merged,
                })
            };
            Entry {
                record,
                state,
                merged,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn porcelain_z_keeps_raw_paths_reasons_and_every_flag() {
        let out = b"worktree /r\0HEAD aa\0branch refs/heads/main\0\0\
            worktree /w/a b\nc\0HEAD bb\0branch refs/heads/t1-x\0locked Cursor / grok | t1 | 2026-09-22\0\0\
            worktree /w/gone\0HEAD cc\0detached\0locked\0prunable gitdir file points to non-existent location\0\0\
            worktree /bare\0bare\0";
        let r = parse_porcelain(out);
        assert_eq!(r.len(), 4);
        assert_eq!(
            (r[0].branch.as_deref(), &r[0].locked),
            (Some("main"), &None)
        );
        assert_eq!(r[1].path, PathBuf::from("/w/a b\nc"));
        assert_eq!(
            r[1].owner().map(|o| (o.owner, o.task)),
            Some(("Cursor / grok".into(), "t1".into()))
        );
        assert!(r[2].detached && r[2].branch.is_none() && r[2].prunable.is_some());
        assert_eq!(r[2].locked.as_deref(), Some(""));
        assert!(r[3].bare && r[3].head.is_none());
        assert!(parse_porcelain(b"").is_empty());
    }

    #[rstest]
    #[case(None, Some("me"), false)]
    #[case(Some("me | t1 | 2026-09-22"), Some("me"), false)]
    #[case(Some("me | t1 | 2026-09-22"), Some("you"), true)]
    #[case(Some("me | t1 | 2026-09-22"), None, true)]
    #[case(Some(""), Some("me"), true)] // no reason: owner unknown
    #[case(Some("me | t1"), Some("me"), true)] // not the convention: owner unknown
    #[case(Some("me |  | 2026-09-22"), Some("me"), true)]
    fn a_lock_holds_against_everyone_but_its_named_owner(
        #[case] locked: Option<&str>,
        #[case] me: Option<&str>,
        #[case] held: bool,
    ) {
        let record = Record {
            locked: locked.map(Into::into),
            ..Record::default()
        };
        assert_eq!(record.held_against(me), held);
    }

    #[rstest]
    #[case(false, true, true, State::Stale)]
    #[case(false, false, false, State::Stale)]
    #[case(true, true, true, State::Dirty)]
    #[case(true, true, false, State::Dirty)]
    #[case(true, false, true, State::Merged)]
    #[case(true, false, false, State::Unmerged)]
    fn classification(
        #[case] exists: bool,
        #[case] dirty: bool,
        #[case] merged: bool,
        #[case] state: State,
    ) {
        assert_eq!(
            classify(Facts {
                exists,
                dirty,
                merged
            }),
            state
        );
    }
}
