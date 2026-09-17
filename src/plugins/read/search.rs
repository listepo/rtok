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

/// Canonical walk base: `dunce::canonicalize(cwd)`, or `cwd` when it does not
/// exist. `search`/`tree` compute this once per call (T59.2) — `display_rel`
/// takes the base instead of paying the syscalls per hit or row.
fn canonical_base_with(fs: &impl super::fs::ReadFs, cwd: &Path) -> PathBuf {
    fs.canonicalize(cwd).unwrap_or_else(|| cwd.to_path_buf())
}

fn canonical_base(cwd: &Path) -> PathBuf {
    canonical_base_with(&super::fs::HostFs, cwd)
}

fn display_rel(path: &Path, root: &Path, base: &Path) -> String {
    strip_prefix_ci(path, base)
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
    let base = canonical_base(&cwd);
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
        let rel = display_rel(entry.path(), &root, &base);
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
    let base = canonical_base(&cwd);
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
        let rel = display_rel(p, &root, &base);
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
    // WalkBuilder host-disk e2e — kept; Vfs twins use walk::WalkFs (T56.4).
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
        let rel = display_rel(path, root, &canonical_base(cwd));
        if cfg!(windows) {
            assert_eq!(rel.replace('/', "\\"), r"src\a.rs");
        } else {
            // On Unix Path::strip_prefix is case-sensitive; mixed-case paths do not strip.
            assert!(rel.contains("a.rs"), "{rel}");
        }
    }

    /// T55.5: files over `search_max_bytes` are never loaded (WalkBuilder e2e; pure gate in Vfs above).
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

    /// T56.4: thin wrappers over [`super::walk`] so twins share the real WalkFs adapter.
    fn search_hits_from_vfs(
        vfs: &crate::testutil::Vfs,
        pattern: &str,
        max_bytes: u64,
        max_hits: usize,
    ) -> Vec<String> {
        crate::plugins::read::walk::search_hits(vfs, "", pattern, max_bytes, max_hits)
    }

    fn tree_rows_from_vfs(
        vfs: &crate::testutil::Vfs,
        root_prefix: &str,
        max_depth: usize,
    ) -> Vec<String> {
        crate::plugins::read::walk::tree_rows(vfs, root_prefix, max_depth)
    }

    #[test]
    fn search_max_bytes_gate_uses_vfs_sizes() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("ok.txt", b"needle small\n");
        vfs.write("big.txt", {
            let mut b = b"needle large\n".to_vec();
            b.resize(200, b'x');
            b
        });
        vfs.write("other.txt", b"no match\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 64, 10);
        assert_eq!(hits, vec!["ok.txt:1: needle small".to_string()]);
    }

    /// T56.2: relative display path + hit formatting without host TempDir.
    #[test]
    fn vfs_search_hit_paths_stay_basename_relative() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("nest/hit.rs", b"fn needle() {}\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 1024, 10);
        assert_eq!(hits, vec!["nest/hit.rs:1: fn needle() {}".to_string()]);
    }

    #[test]
    fn vfs_search_respects_max_hits_across_files() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("a.txt", b"needle\nneedle\n");
        vfs.write("b.txt", b"needle\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 1024, 2);
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits.iter().all(|h| h.contains("needle")), "{hits:?}");
    }

    #[test]
    fn vfs_search_skips_non_utf8_bodies() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("bin.dat", [0xff, 0xfe, 0x00]);
        vfs.write("ok.txt", b"needle here\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 1024, 10);
        assert_eq!(hits, vec!["ok.txt:1: needle here".to_string()]);
    }

    #[test]
    fn vfs_search_empty_is_empty() {
        let vfs = crate::testutil::Vfs::new();
        assert!(search_hits_from_vfs(&vfs, "needle", 1024, 10).is_empty());
    }

    #[test]
    fn vfs_search_spaced_windows_style_keys() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(r"C:/Users/Ivan Tuhai/proj/src/a.rs", b"fn needle() {}\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 1024, 10);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].contains("Ivan Tuhai"), "{hits:?}");
        assert!(hits[0].contains("a.rs:1:"), "{hits:?}");
    }

    /// T56.2 twin of `search_and_tree_skip_git_dir` — keep disk e2e; Vfs skips `.git/` keys.
    #[test]
    fn search_and_tree_skip_git_dir_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(".git/logs/HEAD", b"needle in reflog\n");
        vfs.write("a.txt", b"needle in tree\n");
        let hits = search_hits_from_vfs(&vfs, "needle", 1024, 10);
        assert_eq!(hits, vec!["a.txt:1: needle in tree".to_string()]);
        let rows = tree_rows_from_vfs(&vfs, "", 3);
        assert!(rows.iter().any(|r| r.starts_with("a.txt ")), "{rows:?}");
        assert!(
            rows.iter().all(|r| !r.contains(".git")),
            "tree must skip .git: {rows:?}"
        );
    }

    /// T56.2 twin of `tree_paths_stay_relative_for_allow_paths_root`.
    #[test]
    fn tree_paths_stay_relative_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("nest/a/f.txt", b"x");
        let rows = tree_rows_from_vfs(&vfs, "nest", 3);
        assert!(
            rows.iter().any(|r| r.starts_with("a/f.txt ")),
            "expected path relative to walk root, got {rows:?}"
        );
        assert!(
            rows.iter().all(|r| !r.contains("nest/")),
            "must not echo the absolute/search-root prefix: {rows:?}"
        );
    }

    /// T56.2/T56.4 twin of `search_paths_stay_relative_for_allow_paths_root` via WalkFs root.
    #[test]
    fn search_paths_stay_relative_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("nest/hit.rs", b"fn needle() {}\n");
        let hits = crate::plugins::read::walk::search_hits(&vfs, "nest", "needle", 1024, 10);
        assert!(
            hits.iter().any(|h| h.starts_with("hit.rs:")),
            "expected path relative to allow_paths root, got {hits:?}"
        );
        assert!(
            hits.iter().all(|h| !h.contains("nest/")),
            "must not echo the nested root: {hits:?}"
        );
    }

    /// T56.2 twin of `search_skips_files_over_search_max_bytes` (already covered by size gate; assert twin name).
    #[test]
    fn search_skips_files_over_search_max_bytes_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("small.txt", b"needle small\n");
        vfs.write("large.txt", {
            let mut b = b"needle large\n".to_vec();
            b.resize(128, b'x');
            b
        });
        let hits = search_hits_from_vfs(&vfs, "needle", 64, 10);
        assert_eq!(hits, vec!["small.txt:1: needle small".to_string()]);
    }

    /// Pure path: display_rel with spaced profile (T55/T56 edge).
    #[test]
    fn display_rel_keeps_spaced_segment_when_prefix_matches() {
        let path = Path::new(r"C:\Users\Ivan Tuhai\proj\src\a.rs");
        let root = Path::new(r"C:\Users\Ivan Tuhai\proj");
        let cwd = Path::new(r"C:\Users\Ivan Tuhai\proj");
        let rel = display_rel(path, root, &canonical_base(cwd));
        assert!(rel.contains("a.rs"), "{rel}");
        // When strip works (Windows case fold or exact match), stay relative.
        if cfg!(windows) || path.starts_with(root) {
            assert!(
                !rel.contains("Ivan Tuhai") || rel.starts_with("src"),
                "{rel}"
            );
        }
    }

    /// T59.2: one canonicalization per call — `search`/`tree` resolve the base once
    /// on the adapter and per-row `display_rel` never touches the FS.
    #[test]
    fn display_rel_canonicalizes_base_once_per_call_from_vfs() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::plugins::read::fs::ReadFs;

        struct CountingFs<'a> {
            fs: &'a crate::testutil::Vfs,
            calls: &'a AtomicUsize,
        }
        impl ReadFs for CountingFs<'_> {
            fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
                ReadFs::read_to_string(self.fs, path)
            }
            fn canonicalize(&self, path: &Path) -> Option<PathBuf> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                ReadFs::canonicalize(self.fs, path)
            }
        }

        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("src/a.rs", b"fn main() {}\n");
        vfs.write("src/b.rs", b"fn other() {}\n");
        let calls = AtomicUsize::new(0);
        let cfs = CountingFs {
            fs: &vfs,
            calls: &calls,
        };

        // The call: the base is canonicalized exactly once…
        let base = canonical_base_with(&cfs, Path::new("src"));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(base, PathBuf::from("src"));

        // …then any number of rows display without another adapter call.
        assert_eq!(
            display_rel(Path::new("src/a.rs"), Path::new("src"), &base),
            "a.rs"
        );
        let _ = display_rel(Path::new("src/b.rs"), Path::new("src"), &base);
        let _ = display_rel(Path::new("elsewhere/x.rs"), Path::new("src"), &base);
        assert_eq!(
            calls.load(Ordering::Relaxed),
            1,
            "per-row display must not canonicalize"
        );
    }
}
