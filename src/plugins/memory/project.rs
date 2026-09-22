//! Project identity for notes (T133): the checkout `cwd` belongs to, resolved so that a
//! linked git worktree and its main checkout share one name.
//!
//! Lexical only — `.git` files, `commondir` and `config` are read, git is never spawned, so
//! the hook budget (≤ 10 ms) holds. Runs over [`ReadFs`] so unit tests use `Vfs`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::plugins::read::fs::{HostFs, ReadFs};
use crate::plugins::read::normalize;

/// Name of the project `cwd` lies in, if it is inside a git checkout: the `origin` repo name
/// (`git@host:owner/repo.git`, `https://…/repo`, `file:///…/repo.git` → `repo`), else the
/// basename of the main checkout. A linked worktree resolves to its main checkout's name.
pub fn project_name(cwd: &Path) -> Option<String> {
    project_name_with(&HostFs, cwd)
}

pub(crate) fn project_name_with(fs: &impl ReadFs, cwd: &Path) -> Option<String> {
    let mut root = cwd.to_path_buf();
    let common = loop {
        let dot = root.join(".git");
        match fs.read_to_string(&dot) {
            // A linked worktree: `.git` is a file naming its admin dir, whose `commondir`
            // names the main checkout's `.git`. Anything else with a `.git` file (a
            // submodule) keeps the old answer: this directory is the project.
            Ok(text) => break common_dir(fs, &root, &text).unwrap_or(dot),
            // Missing on the host (Vfs also answers NotFound for a directory key, so the
            // existence check is `canonicalize`): keep climbing.
            Err(e) if e.kind() == ErrorKind::NotFound && fs.canonicalize(&dot).is_none() => {}
            // Exists but does not read as a file: the `.git` directory of a main checkout.
            Err(_) => break dot,
        }
        if !root.pop() {
            return None;
        }
    };
    let main = common.parent()?;
    origin_name(fs, &common).or_else(|| main.file_name().map(|s| s.to_string_lossy().into_owned()))
}

/// `<main>/.git` for a worktree whose `.git` file reads `gitdir: <admin dir>`; both that path
/// and the admin dir's `commondir` may be relative (`worktree.useRelativePaths`).
fn common_dir(fs: &impl ReadFs, root: &Path, dot_git: &str) -> Option<PathBuf> {
    let gitdir = dot_git
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:"))?
        .trim();
    let gitdir = normalize(root, Path::new(gitdir));
    let common = fs.read_to_string(&gitdir.join("commondir")).ok()?;
    Some(normalize(&gitdir, Path::new(common.trim())))
}

/// Last path segment of `[remote "origin"] url` in `<common>/config`, without `.git`.
fn origin_name(fs: &impl ReadFs, common: &Path) -> Option<String> {
    let config = fs.read_to_string(&common.join("config")).ok()?;
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = line == r#"[remote "origin"]"#;
            continue;
        }
        let Some((key, url)) = line.split_once('=') else {
            continue;
        };
        if !in_origin || key.trim() != "url" {
            continue;
        }
        let name = url
            .trim()
            .trim_end_matches('/')
            .rsplit(['/', ':'])
            .next()?
            .trim_end_matches(".git");
        return (!name.is_empty()).then(|| name.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Vfs;
    use rstest::rstest;

    /// A main checkout with an `origin` and one linked worktree whose paths are relative.
    fn fixture(url: &str) -> Vfs {
        let mut vfs = Vfs::new();
        vfs.write(
            "/home/me/rtok/.git/config",
            format!("[core]\n\tbare = false\n[remote \"origin\"]\n\turl = {url}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n"),
        );
        vfs.write("/home/me/rtok/.git/HEAD", "ref: refs/heads/main\n");
        vfs.write(
            "/home/me/rtok/.git/worktrees/rtok-t133/commondir",
            "../..\n",
        );
        vfs.write(
            "/home/me/rtok/.git/worktrees/rtok-t133/gitdir",
            "/home/me/_worktrees/rtok-t133/.git\n",
        );
        vfs.write(
            "/home/me/_worktrees/rtok-t133/.git",
            "gitdir: ../../rtok/.git/worktrees/rtok-t133\n",
        );
        vfs.write("/home/me/_worktrees/rtok-t133/src/lib.rs", "");
        vfs
    }

    #[test]
    fn main_checkout_and_linked_worktree_resolve_to_one_project() {
        let vfs = fixture("git@github.com:listepo/rtok-notes.git");
        let main = project_name_with(&vfs, Path::new("/home/me/rtok/src"));
        let linked = project_name_with(&vfs, Path::new("/home/me/_worktrees/rtok-t133/src"));
        assert_eq!(main.as_deref(), Some("rtok-notes"));
        assert_eq!(linked, main);
    }

    #[test]
    fn plain_directory_is_no_project() {
        let mut vfs = Vfs::new();
        vfs.write("/home/me/scratch/notes.txt", "");
        assert_eq!(project_name_with(&vfs, Path::new("/home/me/scratch")), None);
    }

    #[test]
    fn without_origin_the_main_checkout_basename_names_the_project() {
        let mut vfs = Vfs::new();
        vfs.write("/home/me/local-only/.git/HEAD", "ref: refs/heads/main\n");
        vfs.write("/home/me/local-only/.git/worktrees/wt/commondir", "../..\n");
        vfs.write(
            "/home/me/wt/.git",
            "gitdir: /home/me/local-only/.git/worktrees/wt\n",
        );
        assert_eq!(
            project_name_with(&vfs, Path::new("/home/me/local-only")).as_deref(),
            Some("local-only")
        );
        assert_eq!(
            project_name_with(&vfs, Path::new("/home/me/wt")).as_deref(),
            Some("local-only")
        );
    }

    #[rstest]
    #[case("https://github.com/listepo/rtok", "rtok")]
    #[case("https://github.com/listepo/rtok.git/", "rtok")]
    #[case("ssh://git@github.com/listepo/rtok.git", "rtok")]
    #[case("file:///srv/git/rtok.git", "rtok")]
    fn origin_url_shapes_give_the_repo_name(#[case] url: &str, #[case] name: &str) {
        let vfs = fixture(url);
        assert_eq!(
            project_name_with(&vfs, Path::new("/home/me/rtok")).as_deref(),
            Some(name)
        );
    }

    /// A submodule's `.git` file points at `modules/<name>` (no `commondir`): the submodule
    /// directory stays its own project, as before T133.
    #[test]
    fn submodule_keeps_its_own_name() {
        let mut vfs = fixture("https://github.com/listepo/rtok");
        vfs.write(
            "/home/me/rtok/vendor/dep/.git",
            "gitdir: ../../.git/modules/dep\n",
        );
        vfs.write(
            "/home/me/rtok/.git/modules/dep/HEAD",
            "ref: refs/heads/main\n",
        );
        assert_eq!(
            project_name_with(&vfs, Path::new("/home/me/rtok/vendor/dep/src")).as_deref(),
            Some("dep")
        );
    }
}
