// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! Narrow FS surface for `read()` resolve + content (T56.5 / post-T56.4) and for the
//! project resolver (T133): read a file as text, canonicalize, and a lexical path join.
//!
//! Production uses [`HostFs`]. Unit tests drive the same path over [`crate::testutil::Vfs`]
//! without deleting disk e2e twins. Search/tree keep `ignore::WalkBuilder` on the host.
//! Crate level, not inside the `read` plugin: `plugin.rs` attributes every session through
//! [`crate::project`] whatever features are compiled in (T154).

use std::path::{Component, Path, PathBuf};

/// Read bytes as UTF-8 text and canonicalize (follow symlinks when the backend supports them).
pub trait ReadFs {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;
    /// Canonical path when the entry exists; `None` if missing (caller keeps the lexical abs).
    fn canonicalize(&self, path: &Path) -> Option<PathBuf>;
}

/// Host disk backend for production `read` / `resolve`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostFs;

impl ReadFs for HostFs {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn canonicalize(&self, path: &Path) -> Option<PathBuf> {
        dunce::canonicalize(path).ok()
    }
}

/// Lexical join of `path` onto `root` (`..` pops, `.` drops) — no disk access, so a missing
/// path still normalises. `read` resolves with it; `project` follows `gitdir:` / `commondir`.
///
/// Deliberately NOT clamped: `project` legitimately climbs above `root` (a linked
/// worktree's `gitdir: ../../main/.git/...`). Confinement lives at the boundary —
/// `read::resolve_with` rejects anything outside the allow-roots via `under()`.
pub(crate) fn normalize(root: &Path, path: &Path) -> PathBuf {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        root.to_path_buf()
    };
    for c in path.components() {
        match c {
            // Push, do not replace: on Windows `C:\foo` is Prefix("C:") then
            // RootDir — replacing wiped the drive and confined to `\foo`.
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::Prefix(p) => out = PathBuf::from(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

/// T263/T356: whether `root` is the filesystem root or `home`, which no index or walk may use as
/// a project root. Lives here, not in `read`, so the store can apply it without the `read`
/// feature; `home` is a parameter so tests need not touch `$HOME`.
pub(crate) fn is_unwalkable_root(root: &Path, home: Option<&Path>) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let is_home = home
        .and_then(|h| h.canonicalize().ok())
        .is_some_and(|h| h == root);
    root.parent().is_none() || is_home
}

/// Forward-slash Vfs key from a [`Path`] (Windows separators normalized).
pub(crate) fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl ReadFs for crate::testutil::Vfs {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        let key = self.resolve_key(&path_key(path));
        match self.read_str(&key) {
            Some(s) => Ok(s.to_string()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("vfs missing: {key}"),
            )),
        }
    }

    fn canonicalize(&self, path: &Path) -> Option<PathBuf> {
        let key = path_key(path);
        if let Some(target) = self.symlink_target(&key) {
            // One hop, then treat the target as the real path (host canonicalize follows).
            let target_key = self.resolve_key(target);
            return Some(PathBuf::from(target_key));
        }
        if self.meta(&key).is_some() {
            return Some(PathBuf::from(key));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T356: `/` and the given home are unwalkable (also through a trailing `.`); a project is not.
    #[test]
    fn unwalkable_root_is_the_filesystem_root_or_home_only() {
        let home = crate::testutil::tmp_dir("t356-fs-home");
        let project = crate::testutil::tmp_dir("t356-fs-project");
        assert!(is_unwalkable_root(Path::new("/"), None));
        assert!(is_unwalkable_root(&home, Some(&home)));
        assert!(is_unwalkable_root(&home.join("."), Some(&home)));
        assert!(!is_unwalkable_root(&project, Some(&home)));
        assert!(!is_unwalkable_root(&project, None));
    }
}
