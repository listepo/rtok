//! WalkBuilder-shaped walk over a narrow FS trait (T56.4).
//!
//! Production `search` / `tree` keep `ignore::WalkBuilder` on the host disk (gitignore).
//! Unit paths that only need dir metadata, list, and read use [`WalkFs`] + [`walk`] so they
//! do not touch TempDir. Prefer extending [`crate::testutil::Vfs`] over a new crate.
//! [`super::fs::HostFs`] also implements [`WalkFs`] as a stub for a later prod swap (T56.5).

use regex::Regex;

/// File / directory metadata for one walk entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryMeta {
    pub len: u64,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Narrow FS surface: metadata + read bytes + list directory (plan T56.4).
pub trait WalkFs {
    fn meta(&self, path: &str) -> Option<EntryMeta>;
    fn read_bytes(&self, path: &str) -> Option<Vec<u8>>;
    /// Immediate child names under `dir` (empty = root), sorted.
    fn list_dir(&self, path: &str) -> Vec<String>;
}

/// One visited path relative to the Vfs key space (includes `root` prefix when non-empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalkEntry {
    pub path: String,
    pub meta: EntryMeta,
}

impl WalkFs for crate::testutil::Vfs {
    fn meta(&self, path: &str) -> Option<EntryMeta> {
        crate::testutil::Vfs::meta(self, path).map(|m| EntryMeta {
            len: m.len,
            is_file: m.is_file,
            is_dir: m.is_dir,
        })
    }

    fn read_bytes(&self, path: &str) -> Option<Vec<u8>> {
        crate::testutil::Vfs::read(self, path).map(|b| b.to_vec())
    }

    fn list_dir(&self, path: &str) -> Vec<String> {
        crate::testutil::Vfs::list_dir(self, path)
    }
}

/// Host disk [`WalkFs`] stub — production search/tree still use `WalkBuilder` (gitignore).
impl WalkFs for super::fs::HostFs {
    fn meta(&self, path: &str) -> Option<EntryMeta> {
        let meta = std::fs::metadata(path).ok()?;
        Some(EntryMeta {
            len: meta.len(),
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
        })
    }

    fn read_bytes(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn list_dir(&self, path: &str) -> Vec<String> {
        let dir = if path.is_empty() { "." } else { path };
        let mut names = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return names;
        };
        for ent in rd.flatten() {
            if let Some(name) = ent.file_name().to_str() {
                names.push(name.to_string());
            }
        }
        names.sort();
        names
    }
}

/// Depth-first walk of `root` (empty string = Vfs root).
///
/// Mirrors `WalkBuilder` + `filter_entry(skip_git)` for the parts unit tests need:
/// skips a child named `.git` (no entry, no descent). `max_depth` matches
/// `ignore::WalkBuilder::max_depth` (root is depth 0; `Some(1)` = immediate children only).
/// Does not apply `.gitignore` (Vfs has no ignore files).
pub fn walk(fs: &impl WalkFs, root: &str, max_depth: Option<usize>) -> Vec<WalkEntry> {
    let mut out = Vec::new();
    // (dir_path, depth_of_dir). Root itself is not emitted (same as tree() skipping `p == root`).
    let mut stack = vec![(root.to_string(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if let Some(max) = max_depth
            && depth >= max
        {
            continue;
        }
        for name in fs.list_dir(&dir) {
            // Same rule as `skip_git`: never yield or descend into `.git`.
            if name == ".git" {
                continue;
            }
            let path = if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            };
            let Some(meta) = fs.meta(&path) else {
                continue;
            };
            let child_depth = depth + 1;
            out.push(WalkEntry {
                path: path.clone(),
                meta,
            });
            if meta.is_dir {
                let descend = match max_depth {
                    Some(max) => child_depth < max,
                    None => true,
                };
                if descend {
                    stack.push((path, child_depth));
                }
            }
        }
    }
    out
}

fn rel_to_root(path: &str, root: &str) -> String {
    if root.is_empty() {
        return path.to_string();
    }
    let with_sep = format!("{root}/");
    path.strip_prefix(&with_sep).unwrap_or(path).to_string()
}

/// Regex search over a [`WalkFs`] tree (size gate + `.git` skip). Paths in hits are
/// relative to `root` when `root` is non-empty.
pub fn search_hits(
    fs: &impl WalkFs,
    root: &str,
    pattern: &str,
    max_bytes: u64,
    max_hits: usize,
) -> Vec<String> {
    let re = Regex::new(pattern).expect("search pattern");
    let mut hits = Vec::new();
    for entry in walk(fs, root, None) {
        if hits.len() >= max_hits {
            break;
        }
        if !entry.meta.is_file {
            continue;
        }
        if entry.meta.len > max_bytes {
            continue;
        }
        let Some(bytes) = fs.read_bytes(&entry.path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let rel = rel_to_root(&entry.path, root);
        for (i, line) in text.lines().enumerate() {
            if hits.len() >= max_hits {
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
    hits
}

/// Compact `rel size` rows over a [`WalkFs`] tree (dirs included, like WalkBuilder tree).
pub fn tree_rows(fs: &impl WalkFs, root: &str, max_depth: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for entry in walk(fs, root, Some(max_depth.max(1))) {
        let rel = rel_to_root(&entry.path, root);
        if rel.is_empty() {
            continue;
        }
        rows.push(format!("{rel} {}", entry.meta.len));
    }
    rows.sort();
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Vfs;

    #[test]
    fn walk_lists_dirs_and_files_skips_git() {
        let mut vfs = Vfs::new();
        vfs.write(".git/logs/HEAD", b"reflog");
        vfs.write("a.txt", b"hi");
        vfs.write("sub/b.txt", b"x");
        let entries = walk(&vfs, "", None);
        let paths: Vec<_> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"a.txt"), "{paths:?}");
        assert!(paths.contains(&"sub"), "{paths:?}");
        assert!(paths.contains(&"sub/b.txt"), "{paths:?}");
        assert!(
            paths.iter().all(|p| !p.contains(".git")),
            "must skip .git: {paths:?}"
        );
    }

    #[test]
    fn walk_respects_max_depth() {
        let mut vfs = Vfs::new();
        vfs.write("nest/a/f.txt", b"x");
        let deep = walk(&vfs, "nest", Some(3));
        let paths: Vec<_> = deep.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"nest/a"), "{paths:?}");
        assert!(paths.contains(&"nest/a/f.txt"), "{paths:?}");
        let shallow = walk(&vfs, "nest", Some(1));
        let paths: Vec<_> = shallow.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["nest/a"]);
    }

    #[test]
    fn search_hits_size_gate_and_relative_root() {
        let mut vfs = Vfs::new();
        vfs.write("nest/small.txt", b"needle small\n");
        vfs.write("nest/large.txt", {
            let mut b = b"needle large\n".to_vec();
            b.resize(128, b'x');
            b
        });
        vfs.write("nest/.git/x", b"needle git\n");
        let hits = search_hits(&vfs, "nest", "needle", 64, 10);
        assert_eq!(hits, vec!["small.txt:1: needle small".to_string()]);
    }

    #[test]
    fn tree_rows_relative_and_include_dirs() {
        let mut vfs = Vfs::new();
        vfs.write("nest/a/f.txt", b"x");
        let rows = tree_rows(&vfs, "nest", 3);
        assert!(rows.iter().any(|r| r.starts_with("a ")), "{rows:?}");
        assert!(rows.iter().any(|r| r.starts_with("a/f.txt 1")), "{rows:?}");
        assert!(rows.iter().all(|r| !r.contains("nest/")), "{rows:?}");
    }

    /// HostFs WalkFs stub smoke (prod still on WalkBuilder; T56.5).
    #[test]
    fn host_fs_walk_lists_temp_file() {
        let dir = crate::testutil::tmp_dir("walk-host");
        let file = dir.join("only.txt");
        std::fs::write(&file, b"hi").unwrap();
        let root = dir.to_string_lossy().to_string();
        let entries = walk(&super::super::fs::HostFs, &root, Some(1));
        let names: Vec<_> = entries
            .iter()
            .filter_map(|e| e.path.rsplit('/').next())
            .collect();
        assert!(names.contains(&"only.txt"), "{names:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
