//! Walk overrides and extension map for the graph index (T68.10).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use ignore::overrides::{Override, OverrideBuilder};

use crate::config::Graph;
use crate::plugins::read::outline;

/// Gitignore-style overrides plus an extension → grammar map, shared by the walker
/// and the background watcher (T68.10).
pub struct Matcher {
    root: PathBuf,
    overrides: Override,
    extensions: HashMap<String, String>,
    exclude: Vec<String>,
}

impl Matcher {
    pub fn new(root: &Path, cfg: &Graph) -> Self {
        let mut ob = OverrideBuilder::new(root);
        // `Override` inverts gitignore rules: `!pat` excludes, bare `pat` includes (T68.10).
        // Any `include` turns on whitelist mode; whitelist everything first so only
        // `exclude` and gitignore shrink the walk.
        if !cfg.include.is_empty() {
            let _ = ob.add("**");
        }
        for pat in &cfg.exclude {
            let _ = ob.add(&format!("!{pat}"));
        }
        for pat in &cfg.include {
            let glob = if pat.ends_with('/') {
                format!("{pat}**")
            } else {
                pat.clone()
            };
            let _ = ob.add(&glob);
        }
        let overrides = ob
            .build()
            .unwrap_or_else(|_| OverrideBuilder::new(root).build().unwrap());
        Self {
            root: root.to_path_buf(),
            overrides,
            extensions: cfg.extensions.clone(),
            exclude: cfg.exclude.clone(),
        }
    }

    pub fn extensions(&self) -> &HashMap<String, String> {
        &self.extensions
    }

    pub fn walk_builder(&self, root: &Path) -> WalkBuilder {
        let mut b = WalkBuilder::new(root);
        b.hidden(false)
            .filter_entry(crate::plugins::read::search::skip_git)
            .overrides(self.overrides.clone());
        b
    }

    fn is_excluded(&self, path: &Path) -> bool {
        let rel = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        self.exclude.iter().any(|pat| {
            let pat = pat.trim_end_matches('/');
            rel == pat || rel.starts_with(&format!("{pat}/"))
        })
    }

    fn has_supported_ext(&self, path: &Path) -> bool {
        outline::supported_with(path, &self.extensions)
    }

    /// Whether a file path should be indexed: `exclude`, then built-in or mapped extension.
    pub fn indexable(&self, path: &Path) -> bool {
        if self.is_excluded(path) {
            return false;
        }
        self.has_supported_ext(path)
    }

    /// True when `path` is only indexable because of `extensions` (not built-in `supported`).
    pub fn uses_extension_map(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(OsStr::to_str) else {
            return false;
        };
        self.extensions.contains_key(ext) && !outline::supported(path)
    }
}

/// Grammar names accepted in `[plugins.graph.extensions]` (validated in `config/validate.rs`).
pub const GRAMMAR_NAMES: &[&str] = &[
    "rust", "ts", "tsx", "js", "mjs", "cjs", "py", "dart", "c", "h", "go",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Graph;
    use std::fs;

    fn matcher(
        dir: &Path,
        exclude: &[&str],
        include: &[&str],
        extensions: &[(&str, &str)],
    ) -> Matcher {
        let cfg = Graph {
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            include: include.iter().map(|s| s.to_string()).collect(),
            extensions: extensions
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Default::default()
        };
        Matcher::new(dir, &cfg)
    }

    #[test]
    fn exclude_drops_a_subtree_and_include_brings_gitignored_back() {
        let dir = std::env::temp_dir().join(format!("rtok-graph-walk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("skip")).unwrap();
        fs::create_dir_all(dir.join(".gitignored")).unwrap();
        fs::write(dir.join("skip/a.rs"), "fn a() {}\n").unwrap();
        fs::write(dir.join("keep.rs"), "fn keep() {}\n").unwrap();
        fs::write(dir.join(".gitignored/hidden.rs"), "fn hidden() {}\n").unwrap();
        fs::write(dir.join(".gitignore"), ".gitignored/\n").unwrap();

        let m = matcher(&dir, &["skip/"], &[".gitignored/"], &[]);
        assert!(!m.indexable(&dir.join("skip/a.rs")));
        assert!(m.indexable(&dir.join("keep.rs")));
        assert!(m.indexable(&dir.join(".gitignored/hidden.rs")));

        let mut seen = Vec::new();
        for entry in m.walk_builder(&dir).build().flatten() {
            let p = entry.path();
            if p.is_file() && m.indexable(p) {
                seen.push(
                    p.strip_prefix(&dir)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        seen.sort();
        assert_eq!(seen, [".gitignored/hidden.rs", "keep.rs"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extension_map_indexes_a_custom_suffix() {
        let dir = std::env::temp_dir().join(format!("rtok-graph-ext-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tpl.rs"), "fn rs() {}\n").unwrap();
        fs::write(dir.join("weird.tpl"), "fn tpl() {}\n").unwrap();
        let m = matcher(&dir, &[], &[], &[("tpl", "rust")]);
        assert!(!outline::supported(Path::new("x.tpl")));
        assert!(m.indexable(&dir.join("weird.tpl")));
        assert!(m.uses_extension_map(&dir.join("weird.tpl")));
        assert!(!m.uses_extension_map(&dir.join("tpl.rs")));
        let _ = fs::remove_dir_all(&dir);
    }
}
