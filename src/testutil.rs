//! Test-only helpers shared by the unit tests: a fresh temp dir per call and a `Config` whose
//! every on-disk path lives inside it, so no test writes to `~/.rtok` or into another test's files.

use crate::config::Config;
use crate::plugin::Runtime;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh, empty directory unique to this call. The pid keeps parallel test binaries apart and
/// the counter keeps parallel tests of one binary apart, even when they pass the same `tag`.
pub fn tmp_dir(tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rtok-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Default config with the DB, the archive and the log under a fresh [`tmp_dir`].
pub fn config(tag: &str) -> (Config, PathBuf) {
    let dir = tmp_dir(tag);
    (config_in(&dir), dir)
}

/// Default config with every on-disk path under `dir`. A test that sets only `db_path` left
/// `log.path` at its unexpanded default and wrote a literal `./~/.rtok/logs/` into the repo.
pub fn config_in(dir: &Path) -> Config {
    let mut c = Config::default();
    c.core.db_path = dir.join("rtok.db");
    c.core.archive_dir = dir.join("archive");
    c.log.path = dir.join("rtok.log");
    c
}

/// A [`Runtime`] over [`config`]; `tag` doubles as the session id.
pub fn runtime(tag: &str) -> (Runtime, PathBuf) {
    let (c, dir) = config(tag);
    (Runtime::open(c, tag).unwrap(), dir)
}

/// In-memory path → bytes map for unit tests that must not touch the host disk (D29 / T56).
/// Prefer this over `tmp_dir` when the code under test only needs path/content/size.
#[derive(Default, Clone, Debug)]
pub struct Vfs {
    files: std::collections::BTreeMap<String, Vec<u8>>,
}

impl Vfs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, path: impl Into<String>, bytes: impl AsRef<[u8]>) {
        self.files.insert(path.into(), bytes.as_ref().to_vec());
    }

    pub fn read(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    pub fn len(&self, path: &str) -> Option<u64> {
        self.files.get(path).map(|b| b.len() as u64)
    }

    pub fn paths(&self) -> impl Iterator<Item = String> + '_ {
        self.files.keys().cloned()
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// UTF-8 body, or `None` if missing / not UTF-8.
    pub fn read_str(&self, path: &str) -> Option<&str> {
        self.read(path).and_then(|b| std::str::from_utf8(b).ok())
    }

    /// Paths under `prefix/` (or exact `prefix`), sorted — for drop-in / tree fixtures.
    pub fn paths_under(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = if prefix.is_empty() {
            self.files.keys().cloned().collect()
        } else {
            let with_sep = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            self.files
                .keys()
                .filter(|p| *p == prefix || p.starts_with(&with_sep))
                .cloned()
                .collect()
        };
        out.sort();
        out
    }

    /// Immediate child names under `dir` (files and inferred directories), sorted.
    /// Empty `dir` lists the Vfs root. Used by the T56.4 walk adapter.
    pub fn list_dir(&self, dir: &str) -> Vec<String> {
        let prefix = if dir.is_empty() {
            String::new()
        } else if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        let mut names = std::collections::BTreeSet::new();
        for path in self.files.keys() {
            let rest = if prefix.is_empty() {
                path.as_str()
            } else if let Some(r) = path.strip_prefix(&prefix) {
                r
            } else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            if let Some(name) = rest.split('/').next()
                && !name.is_empty()
            {
                names.insert(name.to_string());
            }
        }
        names.into_iter().collect()
    }

    /// File and/or directory metadata for `path`. Directories are inferred from prefixes.
    pub fn meta(&self, path: &str) -> Option<VfsMeta> {
        let is_file = self.files.contains_key(path);
        let is_dir = !self.list_dir(path).is_empty();
        if !is_file && !is_dir {
            return None;
        }
        Some(VfsMeta {
            len: self.files.get(path).map(|b| b.len() as u64).unwrap_or(0),
            is_file,
            is_dir,
        })
    }
}

/// Metadata returned by [`Vfs::meta`] (T56.4 walk adapter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfsMeta {
    pub len: u64,
    pub is_file: bool,
    pub is_dir: bool,
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_tag_gives_distinct_dirs() {
        let (a, b) = (super::tmp_dir("same"), super::tmp_dir("same"));
        assert_ne!(a, b);
        assert!(a.is_dir() && b.is_dir());
        let (c, dir) = super::config("paths");
        assert!(c.core.db_path.starts_with(&dir) && c.log.path.starts_with(&dir));
    }

    #[test]
    fn vfs_round_trips_bytes_and_len() {
        let mut v = super::Vfs::new();
        v.write("a.txt", b"hi");
        assert_eq!(v.read("a.txt"), Some(b"hi".as_slice()));
        assert_eq!(v.read_str("a.txt"), Some("hi"));
        assert_eq!(v.len("a.txt"), Some(2));
        assert!(v.exists("a.txt"));
        assert_eq!(v.paths().collect::<Vec<_>>(), vec!["a.txt".to_string()]);
        v.write("rules.d/b.toml", b"x");
        v.write("rules.d/a.toml", b"y");
        assert_eq!(
            v.paths_under("rules.d"),
            vec!["rules.d/a.toml".to_string(), "rules.d/b.toml".to_string()]
        );
    }

    #[test]
    fn vfs_read_str_rejects_non_utf8_and_missing() {
        let mut v = super::Vfs::new();
        v.write("bin", [0x80, 0x81]);
        assert!(v.exists("bin"));
        assert_eq!(v.read_str("bin"), None);
        assert_eq!(v.read_str("nope"), None);
        assert!(!v.exists("nope"));
    }

    #[test]
    fn vfs_paths_under_trailing_slash_and_exact() {
        let mut v = super::Vfs::new();
        v.write("rules.d/x.toml", b"1");
        v.write("rules.d", b"not-a-dir-key"); // exact prefix key
        v.write("other/y.toml", b"2");
        assert_eq!(
            v.paths_under("rules.d/"),
            vec!["rules.d/x.toml".to_string()]
        );
        let under = v.paths_under("rules.d");
        assert!(under.contains(&"rules.d".to_string()));
        assert!(under.contains(&"rules.d/x.toml".to_string()));
        assert!(!under.iter().any(|p| p.starts_with("other")));
    }

    #[test]
    fn vfs_overwrite_replaces_bytes_and_len() {
        let mut v = super::Vfs::new();
        v.write("a.txt", b"hi");
        v.write("a.txt", b"hello");
        assert_eq!(v.read_str("a.txt"), Some("hello"));
        assert_eq!(v.len("a.txt"), Some(5));
    }

    #[test]
    fn vfs_list_dir_and_meta_infer_directories() {
        let mut v = super::Vfs::new();
        v.write("nest/a/f.txt", b"x");
        v.write("nest/hit.rs", b"y");
        assert_eq!(v.list_dir(""), vec!["nest".to_string()]);
        assert_eq!(
            v.list_dir("nest"),
            vec!["a".to_string(), "hit.rs".to_string()]
        );
        assert_eq!(v.list_dir("nest/a"), vec!["f.txt".to_string()]);
        let nest = v.meta("nest").unwrap();
        assert!(nest.is_dir && !nest.is_file && nest.len == 0);
        let hit = v.meta("nest/hit.rs").unwrap();
        assert!(hit.is_file && !hit.is_dir && hit.len == 1);
        assert!(v.meta("missing").is_none());
    }
}
