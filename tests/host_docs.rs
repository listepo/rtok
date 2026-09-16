//! Every host plugin (`plugins/<host>/`) and host installer (`src/agents/<host>/`) documents
//! itself: a `README.md` with a `## Docs` list of live links to the host's config and plugin
//! documentation (AGENTS.md, D21).

use std::fs;
use std::path::PathBuf;

fn hosts(sub: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(sub);
    let mut dirs: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no hosts under {}", root.display());
    dirs
}

#[test]
fn every_host_readme_links_its_docs() {
    for dir in hosts("plugins").into_iter().chain(hosts("src/agents")) {
        let readme = dir.join("README.md");
        let text =
            fs::read_to_string(&readme).unwrap_or_else(|e| panic!("{}: {e}", readme.display()));
        let docs = text
            .split("## Docs")
            .nth(1)
            .unwrap_or_else(|| panic!("{}: no `## Docs` section", readme.display()));
        let links = docs
            .lines()
            .filter(|l| l.trim_start().starts_with("- ") && l.contains("https://"))
            .count();
        assert!(
            links >= 1,
            "{}: `## Docs` has no `- … https://` link",
            readme.display()
        );
    }
}
