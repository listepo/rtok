// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T289: which tool created a worktree, read from where it lives. The pools are the ones the
//! hosts document (`research.md` §18.3, §26); a path under none of them is `other`.

use std::ffi::OsStr;
use std::path::{Component, Path};

/// `(parent, child)` directory names, consecutive in the path, and the origin they name.
const POOLS: &[(&str, &str, &str)] = &[
    (".cursor", "worktrees", "cursor"),
    (".windsurf", "worktrees", "windsurf"),
    (".codex", "worktrees", "codex"),
    (".claude", "worktrees", "claude"),
    (".kilo", "worktrees", "kilo"),
    ("conductor", "workspaces", "conductor"),
];

/// Hosts that delete their own worktrees to stay under a cap (Cursor 25, Codex 15, Windsurf and
/// Devin about 20). A git lock there is untested against that eviction, so a claim stays in the
/// store only (T289 Execution).
pub fn evicts(origin: &str) -> bool {
    matches!(origin, "cursor" | "windsurf" | "codex")
}

pub fn of(path: &Path) -> &'static str {
    of_with(
        path,
        std::env::var_os("CODEX_HOME").as_deref().map(Path::new),
    )
}

/// `codex_home` is `$CODEX_HOME`, which moves Codex's pool away from `~/.codex`.
pub fn of_with(path: &Path, codex_home: Option<&Path>) -> &'static str {
    if codex_home
        .is_some_and(|h| !h.as_os_str().is_empty() && path.starts_with(h.join("worktrees")))
    {
        return "codex";
    }
    let names: Vec<&OsStr> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n),
            _ => None,
        })
        .collect();
    names
        .windows(2)
        .find_map(|w| {
            POOLS
                .iter()
                .find(|(a, b, _)| w[0] == OsStr::new(a) && w[1] == OsStr::new(b))
                .map(|(_, _, origin)| *origin)
        })
        .unwrap_or("other")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_documented_pool_names_its_host() {
        for (path, want) in [
            ("/h/.cursor/worktrees/app/abc", "cursor"),
            ("/h/.windsurf/worktrees/app/abc", "windsurf"),
            ("/h/.codex/worktrees/1a2b/app", "codex"),
            ("/r/app/.claude/worktrees/x", "claude"),
            ("/r/app/.kilo/worktrees/x", "kilo"),
            ("/h/conductor/workspaces/app/x", "conductor"),
            ("/h/_worktrees/app-t1", "other"),
            ("/h/.cursor/other/app", "other"),
        ] {
            assert_eq!(of_with(Path::new(path), None), want, "{path}");
        }
    }

    #[test]
    fn codex_home_moves_the_codex_pool() {
        let home = Path::new("/data/codex");
        assert_eq!(
            of_with(Path::new("/data/codex/worktrees/a/app"), Some(home)),
            "codex"
        );
        assert_eq!(
            of_with(Path::new("/data/codex/elsewhere/app"), Some(home)),
            "other"
        );
    }

    #[test]
    fn only_the_capped_pools_evict() {
        assert!(evicts("cursor") && evicts("windsurf") && evicts("codex"));
        assert!(!evicts("kilo") && !evicts("claude") && !evicts("other"));
    }
}
