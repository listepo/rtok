// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The only door the doctor's config checks (T331) use to reach the machine: files, the
//! environment and `PATH`. Production passes the real implementations below; a test passes an
//! in-memory one, so no check ever needs a real agent folder. The guard test in `hooks.rs`
//! fails if another file under `src/doctor/` calls `std::fs` or `std::env` itself.

use std::io;
use std::path::{Path, PathBuf};

/// What a path is, following a symlink to its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathKind {
    Missing,
    File {
        executable: bool,
    },
    Dir,
    /// A symlink whose target does not exist; the target as written.
    DanglingSymlink(PathBuf),
}

pub trait Fs {
    fn read(&self, path: &Path) -> io::Result<String>;
    fn kind(&self, path: &Path) -> PathKind;
}

pub trait Env {
    fn var(&self, name: &str) -> Option<String>;
    fn home(&self) -> Option<PathBuf>;
    /// The directory the check runs for: the project whose settings apply.
    fn cwd(&self) -> Option<PathBuf>;
}

pub trait Which {
    fn find(&self, program: &str) -> Option<PathBuf>;
}

pub struct RealFs;
pub struct RealEnv;
pub struct RealWhich;

impl Fs for RealFs {
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn kind(&self, path: &Path) -> PathKind {
        let Ok(link) = std::fs::symlink_metadata(path) else {
            return PathKind::Missing;
        };
        match std::fs::metadata(path) {
            Ok(m) if m.is_dir() => PathKind::Dir,
            Ok(m) => PathKind::File {
                executable: is_executable(&m),
            },
            Err(_) if link.file_type().is_symlink() => {
                PathKind::DanglingSymlink(std::fs::read_link(path).unwrap_or_default())
            }
            Err(_) => PathKind::Missing,
        }
    }
}

/// Windows has no exec bit: a file runs by its extension, which `Which` already resolved.
fn is_executable(m: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        true
    }
}

impl Env for RealEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn home(&self) -> Option<PathBuf> {
        Some(crate::agents::home_dir()).filter(|h| !h.as_os_str().is_empty())
    }

    fn cwd(&self) -> Option<PathBuf> {
        std::env::current_dir().ok()
    }
}

impl Which for RealWhich {
    fn find(&self, program: &str) -> Option<PathBuf> {
        crate::agents::find_on_path(program)
    }
}
