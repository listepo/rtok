//! Tree-sitter-tags symbol index (plan T8.1).

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;

use crate::plugin::Ctx;
use crate::plugins::read::outline;
use crate::store;

#[derive(Debug, Default)]
pub struct Report {
    pub indexed: u32,
    pub inserted: usize,
    pub skipped: u32,
}

/// Canonical absolute path as one string: the index key of a root, and the match key of a
/// file for `mark_symbols_stale` (T8.3). Rows are scoped to it so one store holds many repos.
pub fn canon(p: &Path) -> String {
    p.canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

/// Incremental index of `root`. Returns how many rows were newly written.
pub fn run(cx: &Ctx, root: &Path) -> Result<Report> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let rk = canon(&root);
    let mut report = Report::default();
    let mut keep = HashSet::new();
    for entry in WalkBuilder::new(&root).hidden(false).build() {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !outline::supported(path) {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        keep.insert(rel.clone());
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let sha = store::hex_sha256(src.as_bytes());
        if cx.store.symbol_sha(&rk, &rel)?.as_deref() == Some(sha.as_str()) {
            report.skipped += 1;
            continue;
        }
        let hits = match outline::tags(path, &src) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let rows: Vec<(String, String, i32, bool)> = hits
            .into_iter()
            .map(|h| (h.name, h.kind, h.line as i32, h.is_def))
            .collect();
        let n = cx.store.replace_symbols(&rk, &rel, &sha, &rows)?;
        report.indexed += 1;
        report.inserted += n;
    }
    let _ = cx.store.delete_symbols_missing(&rk, &keep);
    Ok(report)
}

/// Index `root` only when it has no rows yet (first tool call in that repo).
pub fn ensure(cx: &Ctx, root: &Path) -> Result<Report> {
    if cx.store.symbol_count(&canon(root))? > 0 {
        return Ok(Report::default());
    }
    run(cx, root)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use std::path::PathBuf;

    /// Fresh DB + archive dir under the temp dir; shared with the `mod.rs` tool tests.
    pub(crate) fn cx(name: &str) -> (Ctx, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-graph-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        (Ctx::open(c, name).unwrap(), dir)
    }

    #[test]
    fn index_crate_has_main_def_and_registry_ref() {
        let (cx, dir) = cx("crate");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        run(&cx, &root).unwrap();
        let k = canon(&root);
        assert!(cx.store.has_symbol_def(&k, "main").unwrap(), "main def");
        assert!(
            cx.store.symbol_ref_count(&k, "Registry").unwrap() >= 1,
            "Registry ref"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.3: two repos in one store. Indexing B must not evict A, answer for A, or lose
    /// B's `src/main.rs` when A's same-named file is marked stale.
    #[test]
    fn two_roots_do_not_evict_each_other() {
        let (cx, dir) = cx("roots");
        let (a, b) = (dir.join("a"), dir.join("b"));
        for (root, extra) in [(&a, "alpha"), (&b, "beta")] {
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(
                root.join("src/main.rs"),
                format!("fn main() {{}}\nfn {extra}() {{}}\n"),
            )
            .unwrap();
        }
        run(&cx, &a).unwrap();
        let (ka, kb) = (canon(&a), canon(&b));
        let a_rows = cx.store.symbol_count(&ka).unwrap();
        run(&cx, &b).unwrap();
        assert_eq!(
            cx.store.symbol_count(&ka).unwrap(),
            a_rows,
            "indexing b evicted a"
        );
        assert_eq!(
            cx.store.symbol_defs(&ka, "main").unwrap().len(),
            1,
            "symbol(main) under a must not list b's file"
        );
        assert!(cx.store.has_symbol_def(&ka, "alpha").unwrap());
        assert!(
            !cx.store.has_symbol_def(&ka, "beta").unwrap(),
            "b answered for a"
        );
        cx.store
            .mark_symbols_stale(&canon(&a.join("src/main.rs")))
            .unwrap();
        assert!(
            !cx.store.has_symbol_def(&ka, "alpha").unwrap(),
            "a not stale"
        );
        assert!(
            cx.store.has_symbol_def(&kb, "beta").unwrap(),
            "a's stale mark dropped b's src/main.rs"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn second_run_inserts_zero() {
        let (cx, dir) = cx("twice");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        run(&cx, &root).unwrap();
        let r = run(&cx, &root).unwrap();
        assert_eq!(r.inserted, 0, "second run must insert 0 rows");
        assert_eq!(r.indexed, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn edit_fixture_reindexes_only_that_file() {
        let (cx, dir) = cx("edit");
        let a = dir.join("a.rs");
        let b = dir.join("b.rs");
        fs::write(&a, "fn alpha() {}\n").unwrap();
        fs::write(&b, "fn beta() {}\n").unwrap();
        run(&cx, &dir).unwrap();
        fs::write(&a, "fn alpha() {}\nfn gamma() {}\n").unwrap();
        let r = run(&cx, &dir).unwrap();
        assert_eq!(r.indexed, 1, "only a.rs changed");
        let k = canon(&dir);
        assert!(cx.store.has_symbol_def(&k, "gamma").unwrap());
        assert!(cx.store.has_symbol_def(&k, "beta").unwrap());
        let _ = fs::remove_dir_all(dir);
    }
}
