//! `search` + `tree` (plan T4.5): gitignore-aware regex hits and a size listing.

use std::fs;
use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;
use regex::Regex;

use crate::plugin::Ctx;

use super::resolve;

/// `path:line: snippet` rows, at most `max` (default `plugins.read.search_max`).
pub fn search(cx: &Ctx, pattern: &str, path: &str, max: Option<u32>) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let root = resolve(
        &cwd,
        Path::new(if path.is_empty() { "." } else { path }),
        &cx.config.plugins.read.allow_paths,
    )?;
    let cap = max.unwrap_or(cx.config.plugins.read.search_max).max(1) as usize;
    let re = Regex::new(pattern)?;
    let mut hits = Vec::new();
    for entry in WalkBuilder::new(&root).hidden(false).build() {
        if hits.len() >= cap {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(&cwd)
            .unwrap_or(entry.path())
            .display()
            .to_string();
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
    Ok(hits.join("\n"))
}

/// Compact listing `path size` down to `depth` (default `plugins.read.tree_depth`).
pub fn tree(cx: &Ctx, path: &str, depth: Option<u32>) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let root = resolve(
        &cwd,
        Path::new(if path.is_empty() { "." } else { path }),
        &cx.config.plugins.read.allow_paths,
    )?;
    let depth = depth.unwrap_or(cx.config.plugins.read.tree_depth).max(1) as usize;
    let mut rows = Vec::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
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
        let rel = p.strip_prefix(&cwd).unwrap_or(p);
        let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        rows.push(format!("{} {size}", rel.display()));
    }
    rows.sort();
    Ok(rows.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::plugin::Ctx;

    fn cx(name: &str) -> Ctx {
        let dir = std::env::temp_dir().join(format!("rtok-search-{name}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        Ctx::open(c, name).unwrap()
    }

    #[test]
    fn search_fn_main_finds_src_main() {
        let cx = cx("fnmain");
        let out = search(&cx, "fn main", ".", None).unwrap();
        assert!(out.contains("src/main.rs"), "{out}");
        let n = out.lines().count();
        assert!(n <= cx.config.plugins.read.search_max as usize, "{n}");
    }

    #[test]
    fn search_respects_max() {
        let cx = cx("max");
        let out = search(&cx, "the", ".", Some(3)).unwrap();
        assert!(out.lines().count() <= 3, "{out}");
    }
}
