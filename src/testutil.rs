//! Test-only helpers shared by the unit tests: a fresh temp dir per call and a `Config` whose
//! every on-disk path lives inside it, so no test writes to `~/.rtok` or into another test's files.

use crate::config::Config;
use crate::plugin::Runtime;
use std::path::PathBuf;
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
    let mut c = Config::default();
    c.core.db_path = dir.join("rtok.db");
    c.core.archive_dir = dir.join("archive");
    c.log.path = dir.join("rtok.log");
    (c, dir)
}

/// A [`Runtime`] over [`config`]; `tag` doubles as the session id.
pub fn runtime(tag: &str) -> (Runtime, PathBuf) {
    let (c, dir) = config(tag);
    (Runtime::open(c, tag).unwrap(), dir)
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
}
