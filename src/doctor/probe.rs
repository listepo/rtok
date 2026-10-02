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
    /// The one name of a file reached through a symlinked folder (`/tmp` and `/private/tmp`),
    /// so a file named two ways is not read twice. `path` itself when it cannot be resolved.
    fn canonical(&self, path: &Path) -> PathBuf;
}

pub trait Env {
    fn var(&self, name: &str) -> Option<String>;
    fn home(&self) -> Option<PathBuf>;
    /// The directory the check runs for: the project whose settings apply.
    fn cwd(&self) -> Option<PathBuf>;
    /// Whether hook commands follow Windows rules (`cmd`/PowerShell words, `PATHEXT`). A mock
    /// sets it to test those rules on any OS.
    fn windows(&self) -> bool {
        cfg!(windows)
    }
}

pub trait Which {
    fn find(&self, program: &str) -> Option<PathBuf>;
}

pub struct RealFs;
pub struct RealEnv;
pub struct RealWhich;

impl Fs for RealFs {
    fn canonical(&self, path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

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

/// The only way the doctor changes a file: a copy to `_backup/` first, then an atomic swap.
/// Both are the helpers `agents install` already writes host configs with, so a doctor fix
/// and an install leave the same undo trail.
pub trait Writer {
    /// Copy `path` into its `_backup/` folder, keeping `keep` generations; `None` when an
    /// identical copy is already there.
    fn backup(&self, path: &Path, keep: usize) -> io::Result<Option<PathBuf>>;
    /// Replace `path` with `body` so a reader sees the old or the new file, never half of it.
    fn write(&self, path: &Path, body: &str) -> io::Result<()>;
}

pub struct RealWriter;

impl Writer for RealWriter {
    fn backup(&self, path: &Path, keep: usize) -> io::Result<Option<PathBuf>> {
        rtok_agent_sdk::backup(path, keep).map_err(io::Error::other)
    }

    fn write(&self, path: &Path, body: &str) -> io::Result<()> {
        rtok_agent_sdk::write_atomic(path, body).map_err(io::Error::other)
    }
}
