//! `rtok worktree add` (T158): the one creation path — one location, one name, one
//! owner. [`plan`] decides everything from paths and strings; [`run`] is one fetch and
//! one `git worktree add`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use super::{Owner, git};

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub path: PathBuf,
    pub branch: String,
    /// The lock reason, `<owner> | <task-id> | <date>` — what [`Owner::parse`] reads back.
    pub reason: String,
}

/// Lower-cased, then `[a-z0-9][a-z0-9._-]*` minus what git refuses in a ref name.
fn ident(what: &str, raw: &str) -> Result<String> {
    let id = raw.to_ascii_lowercase();
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c);
    let ok = id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id.chars().all(allowed)
        && !id.contains("..")
        && !id.ends_with('.')
        && !id.ends_with(".lock");
    ensure!(
        ok,
        "{what} `{raw}` must match [a-z0-9][a-z0-9._-]* and be a valid branch name"
    );
    Ok(id)
}

/// `root` when configured, else the nearest ancestor of the main checkout that holds
/// `_worktrees/`, else `_worktrees/` next to it.
fn root_for(main: &Path, root: Option<&Path>) -> Result<PathBuf> {
    if let Some(root) = root {
        return Ok(root.to_path_buf());
    }
    let shared = main.ancestors().skip(1).map(|a| a.join("_worktrees"));
    if let Some(found) = shared.into_iter().find(|dir| dir.is_dir()) {
        return Ok(found);
    }
    let parent = main
        .parent()
        .context("the main checkout has no parent directory")?;
    Ok(parent.join("_worktrees"))
}

/// Symlinks resolved through the deepest ancestor that exists, the rest appended: a root
/// that does not exist yet still has to be judged by where it would land.
fn resolved(path: &Path) -> PathBuf {
    let real = |a: &Path| Some(a.canonicalize().ok()?.join(path.strip_prefix(a).ok()?));
    let landed = path.ancestors().find_map(real);
    landed.unwrap_or_else(|| path.to_path_buf())
}

/// `temp` lists the directories the OS may purge; a worktree under one of them loses
/// its files while git keeps the (locked, never pruned) record.
pub fn plan(
    main: &Path,
    root: Option<&Path>,
    temp: &[PathBuf],
    (task, slug): (&str, Option<&str>),
    owner: &str,
    date: &str,
) -> Result<Plan> {
    let task = ident("task id", task)?;
    let branch = match slug {
        Some(slug) => format!("{task}-{}", ident("slug", slug)?),
        None => task.clone(),
    };
    let reason = format!("{owner} | {task} | {date}");
    // ASCII: `git worktree list --porcelain` C-quotes anything else.
    let round_trips = Owner::parse(&reason).is_some_and(|o| o.owner == owner);
    ensure!(
        owner.is_ascii() && round_trips && !owner.contains(['\n', '\r']),
        "--owner `{owner}` must be non-empty ASCII without ` | `, e.g. \"Claude Code / sonnet\""
    );
    let root = root_for(main, root)?;
    let (landing, checkout) = (resolved(&root), resolved(main));
    // A repository that itself lives under the temp directory (a test fixture, a scratch
    // clone) is no worse off with its worktrees beside it.
    let purgeable = |t: &PathBuf| landing.starts_with(t) && !checkout.starts_with(t);
    if let Some(tmp) = temp.iter().map(|t| resolved(t)).find(purgeable) {
        bail!(
            "worktree root {} is under the temporary directory {}; set `[worktree] root`",
            root.display(),
            tmp.display()
        );
    }
    let repo = main
        .file_name()
        .context("the main checkout has no directory name")?;
    let path = root.join(format!("{}-{task}", repo.to_string_lossy()));
    ensure!(
        !path.exists(),
        "{} already exists: one worktree per task",
        path.display()
    );
    Ok(Plan {
        path,
        branch,
        reason,
    })
}

/// Creates the worktree and returns its path. A failed fetch is an error: branching
/// from a stale base is how a finished task gets rebuilt on old code.
pub fn run(
    cwd: &Path,
    root: Option<&Path>,
    id: (&str, Option<&str>),
    owner: &str,
) -> Result<PathBuf> {
    let worktrees = git::list(cwd)?;
    let main = &worktrees.first().context("git lists no worktree")?.path;
    let temp = [std::env::temp_dir(), "/tmp".into()];
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    let date = &crate::log::stamp(now.as_secs())[..10];
    let plan = plan(main, root, &temp, id, owner, date)?;
    let base = git::default_base(main);
    git::fetch(main, &base)?;
    if let Some(root) = plan.path.parent() {
        std::fs::create_dir_all(root)
            .with_context(|| format!("cannot create {}", root.display()))?;
    }
    git::add_locked(main, &plan.path, &plan.branch, &plan.reason, &base)?;
    Ok(plan.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tmp_dir;
    use rstest::rstest;

    const OWNER: &str = "Claude Code / sonnet";

    fn plan_in(
        main: &Path,
        root: Option<&Path>,
        id: (&str, Option<&str>),
        owner: &str,
    ) -> Result<Plan> {
        plan(main, root, &["/purged".into()], id, owner, "2026-09-22")
    }

    #[test]
    fn the_nearest_shared_root_wins_and_the_names_follow_the_task() {
        let dir = tmp_dir("wt-add");
        let main = dir.join("apps/rtok");
        std::fs::create_dir_all(&main).unwrap();
        let next_to_it = plan_in(&main, None, ("T158", Some("Worktree-Add")), OWNER).unwrap();
        assert_eq!(next_to_it.path, dir.join("apps/_worktrees/rtok-t158"));
        assert_eq!(next_to_it.branch, "t158-worktree-add");
        assert_eq!(
            next_to_it.reason,
            "Claude Code / sonnet | t158 | 2026-09-22"
        );
        let parsed = Owner::parse(&next_to_it.reason).unwrap();
        assert_eq!(
            (parsed.owner.as_str(), parsed.task.as_str()),
            (OWNER, "t158")
        );

        std::fs::create_dir_all(dir.join("_worktrees")).unwrap();
        let shared = plan_in(&main, None, ("t1", None), OWNER).unwrap();
        assert_eq!(
            (shared.path, shared.branch.as_str()),
            (dir.join("_worktrees/rtok-t1"), "t1")
        );

        let configured = plan_in(&main, Some(&dir.join("elsewhere")), ("t1", None), OWNER).unwrap();
        assert_eq!(configured.path, dir.join("elsewhere/rtok-t1"));

        std::fs::create_dir_all(dir.join("_worktrees/rtok-t1")).unwrap();
        let taken = plan_in(&main, None, ("t1", None), OWNER).unwrap_err();
        assert!(
            taken.to_string().contains("one worktree per task"),
            "{taken}"
        );
    }

    #[rstest]
    #[case::empty_id(("", None), OWNER, "task id")]
    #[case::path_in_id(("../t1", None), OWNER, "task id")]
    #[case::leading_dash(("-t1", None), OWNER, "task id")]
    #[case::ref_rule(("t1.lock", None), OWNER, "task id")]
    #[case::bad_slug(("t1", Some("two words")), OWNER, "slug")]
    #[case::no_owner(("t1", None), "", "--owner")]
    #[case::separator_in_owner(("t1", None), "a | b", "--owner")]
    #[case::non_ascii_owner(("t1", None), "Клод / sonnet", "--owner")]
    fn bad_input_is_refused_before_git_runs(
        #[case] id: (&str, Option<&str>),
        #[case] owner: &str,
        #[case] names: &str,
    ) {
        let err = plan_in(Path::new("/r/rtok"), None, id, owner).unwrap_err();
        assert!(err.to_string().contains(names), "{err}");
    }

    #[test]
    fn a_root_under_a_temporary_directory_is_refused_unless_the_repository_is_there_too() {
        let purged = Path::new("/purged/x");
        let err = plan_in(Path::new("/r/rtok"), Some(purged), ("t1", None), OWNER).unwrap_err();
        assert!(err.to_string().contains("temporary directory"), "{err}");
        let scratch = plan_in(Path::new("/purged/session/rtok"), None, ("t1", None), OWNER);
        assert_eq!(
            scratch.unwrap().path,
            Path::new("/purged/session/_worktrees/rtok-t1")
        );
    }
}
