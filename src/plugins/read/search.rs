//! `search` + `tree` (plan T4.5): gitignore-aware regex hits and a size listing.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;
use regex::Regex;

use super::resolve;
use rtok_plugin_sdk::Ctx;

/// Walk results are rooted at `resolve`'s path (often `canonicalize(cwd)`), but
/// display used to strip the raw `current_dir()`. Whenever those disagree —
/// macOS `/tmp` → `/private/tmp`, or an `allow_paths` root outside cwd — every
/// hit became an absolute path. Prefer relative-to-canonical-cwd (so a search
/// of `src` still yields `src/…`), then relative-to-the-walk-root, then raw.
///
/// Strip `prefix` from `path`, ASCII-case-insensitive on Windows (same rule as
/// `under` / `under_ascii_case_insensitive` in `mod.rs`).
fn strip_prefix_ci(path: &Path, prefix: &Path) -> Option<PathBuf> {
    if let Ok(rel) = path.strip_prefix(prefix) {
        return Some(rel.to_path_buf());
    }
    if !cfg!(windows) {
        return None;
    }
    use std::path::Component;
    let path_c: Vec<_> = path.components().collect();
    let pref_c: Vec<_> = prefix.components().collect();
    if pref_c.is_empty() || pref_c.len() > path_c.len() {
        return None;
    }
    let ok = path_c.iter().zip(pref_c.iter()).all(|(p, r)| match (p, r) {
        (Component::Normal(a), Component::Normal(b)) => a.eq_ignore_ascii_case(b),
        (Component::Prefix(a), Component::Prefix(b)) => {
            a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
        }
        (a, b) => a == b,
    });
    if !ok {
        return None;
    }
    let mut out = PathBuf::new();
    for c in path_c.into_iter().skip(pref_c.len()) {
        out.push(c.as_os_str());
    }
    Some(out)
}

fn display_rel(path: &Path, root: &Path, cwd: &Path) -> String {
    let base = dunce::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    strip_prefix_ci(path, &base)
        .or_else(|| strip_prefix_ci(path, cwd))
        .or_else(|| strip_prefix_ci(path, root))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

/// `WalkBuilder::hidden(false)` also descends into `.git/`; no tool wants object files,
/// packed refs or reflogs as hits (`search` returned `.git/logs/HEAD`, 2026-09-16).
pub(crate) fn skip_git(e: &ignore::DirEntry) -> bool {
    e.file_name() != ".git"
}

/// `path:line: snippet` rows, at most `max` (default `plugins.read.search_max`).
pub fn search(cx: &Ctx, pattern: &str, path: &str, max: Option<u32>) -> Result<String> {
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    let cwd = std::env::current_dir()?;
    let root = resolve(
        &cwd,
        Path::new(if path.is_empty() { "." } else { path }),
        &cfg.allow_paths,
    )?;
    let cap = max.unwrap_or(cfg.search_max).max(1) as usize;
    let re = Regex::new(pattern)?;
    let mut hits = Vec::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .filter_entry(skip_git)
        .build()
    {
        if hits.len() >= cap {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        // T55.5: skip before reading so a multi-GB blob cannot spike MCP memory.
        let Ok(meta) = fs::metadata(entry.path()) else {
            continue;
        };
        if meta.len() > cfg.search_max_bytes {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = display_rel(entry.path(), &root, &cwd);
        for (i, line) in text.lines().enumerate() {
            if hits.len() >= cap {
                break;
            }
            if !re.is_match(line) {
                continue;
            }
            let mut snippet = line.trim().to_string();
            if snippet.chars().count() > 120 {
                snippet = snippet.chars().take(120).collect();
            }
            hits.push(format!("{rel}:{}: {snippet}", i + 1));
        }
    }
    super::cap(cx, hits.join("\n"))
}

/// Compact listing `path size` down to `depth` (default `plugins.read.tree_depth`).
pub fn tree(cx: &Ctx, path: &str, depth: Option<u32>) -> Result<String> {
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    let cwd = std::env::current_dir()?;
    let root = resolve(
        &cwd,
        Path::new(if path.is_empty() { "." } else { path }),
        &cfg.allow_paths,
    )?;
    let depth = depth.unwrap_or(cfg.tree_depth).max(1) as usize;
    let mut rows = Vec::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .filter_entry(skip_git)
        .max_depth(Some(depth))
        .build()
    {
        let Ok(entry) = entry else {
            continue;
        };
        let p = entry.path();
        if p == root {
            continue;
        }
        let rel = display_rel(p, &root, &cwd);
        let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        rows.push(format!("{rel} {size}"));
    }
    rows.sort();
    super::cap(cx, rows.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;
    fn cx(name: &str) -> crate::plugin::Runtime {
        crate::testutil::runtime(name).0
    }

    #[test]
    fn search_fn_main_finds_src_main() {
        let cx = cx("fnmain");
        let out = search(&Ctx::new(&cx), "fn main", ".", None).unwrap();
        assert!(out.contains("src/main.rs"), "{out}");
        let n = out.lines().count();
        assert!(n <= cx.config.plugins.read.search_max as usize, "{n}");
    }

    #[test]
    fn search_respects_max() {
        let cx = cx("max");
        let out = search(&Ctx::new(&cx), "the", ".", Some(3)).unwrap();
        assert!(out.lines().count() <= 3, "{out}");
    }

    #[test]
    fn search_paths_stay_relative_for_allow_paths_root() {
        let (rt, dir) = crate::plugins::read::tests::cx("searchrel");
        let nested = dir.join("nest");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("hit.rs"), "fn needle() {}\n").unwrap();
        let out = search(&Ctx::new(&rt), "needle", nested.to_str().unwrap(), None).unwrap();
        assert!(
            out.contains("hit.rs:"),
            "expected path relative to allow_paths root, got {out}"
        );
        let abs = nested.to_string_lossy();
        assert!(
            !out.contains(abs.as_ref()),
            "must not echo the absolute search root: {out}"
        );
    }

    /// `.git/` is never walked: no hit from a reflog, no `.git/objects` rows in a tree.
    #[test]
    fn search_and_tree_skip_git_dir() {
        let (rt, dir) = crate::plugins::read::tests::cx("skipgit");
        let git = dir.join(".git").join("logs");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "needle in reflog\n").unwrap();
        fs::write(dir.join("a.txt"), "needle in tree\n").unwrap();
        let cx = Ctx::new(&rt);
        let out = search(&cx, "needle", dir.to_str().unwrap(), None).unwrap();
        assert!(out.contains("a.txt:1:") && !out.contains(".git"), "{out}");
        let out = tree(&cx, dir.to_str().unwrap(), Some(3)).unwrap();
        assert!(out.contains("a.txt") && !out.contains(".git"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }

    /// Oversized output is archived like `read`, so `max`/`depth` cannot flood the context.
    #[test]
    fn search_output_is_capped_with_archive_id() {
        let (mut c, dir) = crate::testutil::config("searchcap");
        c.plugins.read.allow_paths = vec![dir.clone()];
        c.plugins.read.max_chars = 200;
        let rt = crate::plugin::Runtime::open(c, "searchcap").unwrap();
        let body = "needle line\n".repeat(100);
        fs::write(dir.join("big.txt"), body).unwrap();
        let out = search(&Ctx::new(&rt), "needle", dir.to_str().unwrap(), Some(100)).unwrap();
        assert!(out.contains("archived"), "{out}");
        assert!(out.chars().count() <= 200, "{}", out.chars().count());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tree_paths_stay_relative_for_allow_paths_root() {
        let (rt, dir) = crate::plugins::read::tests::cx("treerel");
        let nested = dir.join("nest");
        fs::create_dir_all(nested.join("a")).unwrap();
        fs::write(nested.join("a").join("f.txt"), "x").unwrap();
        let out = tree(&Ctx::new(&rt), nested.to_str().unwrap(), Some(3)).unwrap();
        assert!(out.contains("a"), "{out}");
        let abs = nested.to_string_lossy();
        assert!(
            !out.contains(abs.as_ref()),
            "must not echo the absolute tree root: {out}"
        );
    }

    /// T55.1: when only ASCII case differs, hits stay relative (Windows residual).
    #[test]
    fn display_rel_strips_ascii_case_insensitive_prefix() {
        // Pure path logic — no host TempDir (plan VFS / T56).
        let path = Path::new(r"C:\Users\Me\Proj\src\a.rs");
        let root = Path::new(r"c:\users\me\proj");
        let cwd = Path::new(r"C:\Users\Me\Proj");
        let rel = display_rel(path, root, cwd);
        if cfg!(windows) {
            assert_eq!(rel.replace('/', "\\"), r"src\a.rs");
        } else {
            // On Unix Path::strip_prefix is case-sensitive; mixed-case paths do not strip.
            assert!(rel.contains("a.rs"), "{rel}");
        }
    }

    /// T55.5: files over `search_max_bytes` are never loaded.
    #[test]
    fn search_skips_files_over_search_max_bytes() {
        let (mut c, dir) = crate::testutil::config("searchcapbytes");
        c.plugins.read.allow_paths = vec![dir.clone()];
        c.plugins.read.search_max_bytes = 64;
        let rt = crate::plugin::Runtime::open(c, "searchcapbytes").unwrap();
        fs::write(dir.join("small.txt"), "needle small\n").unwrap();
        let large = dir.join("large.txt");
        fs::write(&large, "needle large\n").unwrap();
        let f = fs::OpenOptions::new().write(true).open(&large).unwrap();
        f.set_len(128).unwrap();
        drop(f);
        let out = search(&Ctx::new(&rt), "needle", dir.to_str().unwrap(), None).unwrap();
        assert!(out.contains("small.txt"), "{out}");
        assert!(
            !out.contains("large.txt"),
            "oversized file must be skipped: {out}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T55.5 / T56: size gate is unit-testable against an in-memory VFS without host disk.
    #[test]
    fn search_max_bytes_gate_uses_vfs_sizes() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("ok.txt", b"needle\n");
        vfs.write("big.txt", vec![b'x'; 200]);
        let cap = 64u64;
        assert!(vfs.len("ok.txt").unwrap() <= cap);
        assert!(vfs.len("big.txt").unwrap() > cap);
        let kept: Vec<_> = vfs.paths().filter(|p| vfs.len(p).unwrap() <= cap).collect();
        assert_eq!(kept, vec!["ok.txt".to_string()]);
    }
}
