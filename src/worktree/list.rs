//! `rtok worktree list` (T151): what every worktree costs on disk, split into source and
//! tagged build cache, plus the orphans git cannot see. Read-only — it deletes nothing.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::{Entry, inventory};
use crate::info::human_bytes;
use crate::render::{Col, duration, table};

/// First line of a valid `CACHEDIR.TAG` (<https://bford.info/cachedir/>); cargo writes one
/// into `target/`. The tag, not a directory name, is what makes bytes "cache".
const CACHEDIR_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

pub fn is_cache_dir(dir: &Path) -> bool {
    std::fs::read(dir.join("CACHEDIR.TAG")).is_ok_and(|tag| tag.starts_with(CACHEDIR_SIGNATURE))
}

/// Logical bytes: an APFS clone or a hard link counts in full, so the sum over worktrees
/// can exceed what deleting them would free.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Usage {
    pub source: u64,
    pub cache: u64,
    pub modified: Option<SystemTime>,
    /// Every tagged cache root under the worktree; `cache` is their sum.
    pub caches: Vec<Cache>,
}

/// One tagged cache root: the unit `rtok worktree clean` (T152) deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

pub fn usage(dir: &Path) -> Usage {
    let mut total = Usage::default();
    walk(dir, None, &mut total);
    total
}

/// `cache` indexes `total.caches` once the walk is inside a tagged root.
fn walk(dir: &Path, cache: Option<usize>, total: &mut Usage) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let cache = cache.or_else(|| {
        is_cache_dir(dir).then(|| {
            let root = Cache {
                path: dir.to_path_buf(),
                bytes: 0,
                modified: None,
            };
            total.caches.push(root);
            total.caches.len() - 1
        })
    });
    for entry in entries.flatten() {
        // `DirEntry::metadata` does not follow symlinks: a link costs its own length.
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let path = entry.path();
        if meta.is_dir() {
            // A nested checkout (another worktree, a submodule) is its own row.
            if !path.join(".git").exists() {
                walk(&path, cache, total);
            }
            continue;
        }
        let modified = meta.modified().ok();
        match cache {
            Some(i) => {
                let root = &mut total.caches[i];
                root.bytes += meta.len();
                root.modified = root.modified.max(modified);
                total.cache += meta.len();
            }
            None => total.source += meta.len(),
        }
        total.modified = total.modified.max(modified);
    }
}

/// Directories that look like a linked worktree but that git does not list: their `.git`
/// *file* names a `…/worktrees/<id>` admin directory that no longer exists — the
/// repository moved, or the record was pruned. Looked for next to the known worktrees
/// and in `<main>/.claude/worktrees`.
pub fn orphans(entries: &[Entry]) -> Vec<PathBuf> {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let known: HashSet<PathBuf> = entries.iter().map(|e| real(&e.record.path)).collect();
    let linked = entries.iter().skip(1);
    let mut parents: BTreeSet<PathBuf> = linked
        .filter_map(|e| e.record.path.parent().map(real))
        .collect();
    if let Some(main) = entries.first() {
        parents.insert(real(&main.record.path.join(".claude/worktrees")));
    }
    let children = parents
        .iter()
        .flat_map(|p| std::fs::read_dir(p).into_iter().flatten().flatten());
    children
        .map(|child| real(&child.path()))
        .filter(|dir| !known.contains(dir) && is_orphan(dir))
        .collect()
}

fn is_orphan(dir: &Path) -> bool {
    // A `.git` directory (a repository of its own) fails to read as a file.
    let Ok(dot_git) = std::fs::read_to_string(dir.join(".git")) else {
        return false;
    };
    let Some(admin) = dot_git.trim().strip_prefix("gitdir: ") else {
        return false;
    };
    // A relative `gitdir:` (`worktree.useRelativePaths`) resolves against the worktree.
    let admin = dir.join(admin);
    admin.parent().is_some_and(|p| p.ends_with("worktrees")) && !admin.exists()
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub path: PathBuf,
    pub branch: Option<String>,
    /// From the lock reason; `None` with `locked` means the owner is unknown.
    pub owner: Option<String>,
    pub locked: bool,
    /// T150's state, or `orphan`.
    pub state: &'static str,
    pub source_bytes: u64,
    pub cache_bytes: u64,
    pub modified_unix: Option<u64>,
}

impl Row {
    fn new(path: PathBuf, state: &'static str) -> Self {
        let used = usage(&path);
        let unix = |t: SystemTime| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
        Self {
            path,
            branch: None,
            owner: None,
            locked: false,
            state,
            source_bytes: used.source,
            cache_bytes: used.cache,
            modified_unix: used.modified.and_then(unix),
        }
    }
}

/// Every worktree of the repository `cwd` belongs to, then the orphans.
pub fn rows(cwd: &Path) -> anyhow::Result<Vec<Row>> {
    let entries = inventory(cwd)?;
    let listed = entries.iter().map(|e| Row {
        branch: e.record.branch.clone(),
        owner: e.record.owner().map(|o| o.owner),
        locked: e.record.locked.is_some(),
        ..Row::new(e.record.path.clone(), e.state.label())
    });
    let mut rows: Vec<Row> = listed.collect();
    rows.extend(orphans(&entries).into_iter().map(|p| Row::new(p, "orphan")));
    Ok(rows)
}

pub fn to_table(rows: &[Row], now: SystemTime) -> String {
    let now = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let dash = || "-".to_string();
    let mut lines = vec![
        [
            "path", "branch", "owner", "state", "modified", "source", "cache",
        ]
        .map(String::from)
        .to_vec(),
    ];
    lines.extend(rows.iter().map(|r| {
        let owner = match (&r.owner, r.locked) {
            (Some(owner), _) => owner.clone(),
            (None, true) => "locked, owner unknown".into(),
            (None, false) => dash(),
        };
        let age = r
            .modified_unix
            .map(|t| duration(now.saturating_sub(t) as i64));
        vec![
            r.path.display().to_string(),
            r.branch.clone().unwrap_or_else(dash),
            owner,
            r.state.into(),
            age.unwrap_or_else(dash),
            human_bytes(r.source_bytes),
            human_bytes(r.cache_bytes),
        ]
    }));
    let cols = [0, 0, 0, 0, 0].map(Col::left).into_iter();
    let cols: Vec<Col> = cols.chain([Col::right(0), Col::right(0)]).collect();
    let (source, cache) = rows
        .iter()
        .fold((0, 0), |(s, c), r| (s + r.source_bytes, c + r.cache_bytes));
    format!(
        "{}\n{} worktrees: {} source, {} build cache (logical bytes; clones and hard links count in full)\n",
        table(&cols, &lines),
        rows.len(),
        human_bytes(source),
        human_bytes(cache),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tmp_dir;
    use std::fs::{create_dir_all, write};

    #[test]
    fn only_a_tagged_directory_is_cache_and_a_nested_checkout_is_not_counted() {
        let dir = tmp_dir("wt-usage");
        for sub in ["src", "target/debug", "vendor/target", "nested/src"] {
            create_dir_all(dir.join(sub)).unwrap();
        }
        write(dir.join("src/a.rs"), [0; 10]).unwrap();
        write(dir.join("target/CACHEDIR.TAG"), CACHEDIR_SIGNATURE).unwrap();
        write(dir.join("target/debug/bin"), [0; 1000]).unwrap();
        // Same name, no tag (and a tag with the wrong signature): source, never cache.
        write(dir.join("vendor/target/lib"), [0; 100]).unwrap();
        write(dir.join("vendor/CACHEDIR.TAG"), "not a signature").unwrap();
        write(dir.join("nested/.git"), "gitdir: /elsewhere").unwrap();
        write(dir.join("nested/src/big"), [0; 5000]).unwrap();

        let used = usage(&dir);
        assert_eq!(used.cache, 1000 + CACHEDIR_SIGNATURE.len() as u64);
        assert_eq!(used.source, 10 + 100 + "not a signature".len() as u64);
        assert!(used.modified.is_some());
        let [root] = used.caches.as_slice() else {
            panic!("one cache root, got {:?}", used.caches);
        };
        assert_eq!(
            (root.path.as_path(), root.bytes),
            (dir.join("target").as_path(), used.cache)
        );
        // The root's newest file is at most as new as the worktree's newest file.
        assert!(root.modified.is_some() && root.modified <= used.modified);
        assert_eq!(usage(&dir.join("missing")), Usage::default());
    }

    #[test]
    fn the_table_names_an_unknown_owner_and_totals_both_kinds_of_bytes() {
        let row = |state, owner: Option<&str>, locked, cache| Row {
            path: "/w/x".into(),
            branch: Some("t1".into()),
            owner: owner.map(Into::into),
            locked,
            state,
            source_bytes: 1024,
            cache_bytes: cache,
            modified_unix: Some(1_000),
        };
        let rows = [
            row("merged", Some("Cursor / grok"), true, 2048),
            row("dirty", None, true, 0),
            row("orphan", None, false, 0),
        ];
        let now = UNIX_EPOCH + std::time::Duration::from_secs(1_000 + 3 * 86_400 + 4 * 3_600);
        insta::assert_snapshot!(to_table(&rows, now), @r"
        path branch owner                 state  modified source  cache
        /w/x t1     Cursor / grok         merged 3d04h    1.0 KB 2.0 KB
        /w/x t1     locked, owner unknown dirty  3d04h    1.0 KB    0 B
        /w/x t1     -                     orphan 3d04h    1.0 KB    0 B

        3 worktrees: 3.0 KB source, 2.0 KB build cache (logical bytes; clones and hard links count in full)
        ");
    }
}
