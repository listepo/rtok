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

/// Hosts that receive `skills/rtok/` on install must link their skill-root docs (T71.3).
const SKILL_HOSTS: &[(&str, &str)] = &[
    ("claude", "code.claude.com/docs/en/skills"),
    ("cursor", "cursor.com/docs/skills"),
    ("codex", "agentskills.io"),
    ("opencode", "opencode.ai/docs/skills"),
    ("copilot", "copilot/concepts/agents/about-agent-skills"),
];

#[test]
fn skill_hosts_link_their_skill_root_docs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/agents");
    for (id, needle) in SKILL_HOSTS {
        let readme = root.join(id).join("README.md");
        let text =
            fs::read_to_string(&readme).unwrap_or_else(|e| panic!("{}: {e}", readme.display()));
        assert!(
            text.contains(needle),
            "{}: `## Docs` must link the host skill root ({needle})",
            readme.display()
        );
    }
}

/// T217: `AGENTS.md` is loaded into every agent session and promises to stay under
/// 350 tokens. Words × 4/3 is the usual English token rate; bytes / 4 catches a file
/// that stays short in words but grows in tables, paths and code spans.
#[test]
fn agents_md_stays_under_its_350_token_budget() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    let by_words = text.split_whitespace().count() * 4 / 3;
    let by_bytes = text.len() / 4;
    assert!(
        by_words < 350 && by_bytes < 350,
        "AGENTS.md ≈ {by_words} tokens by words, {by_bytes} by bytes; the budget is 350"
    );
    assert!(
        text.contains("under 350 tokens"),
        "the budget line must stay in the file"
    );
    #[cfg(unix)]
    assert_eq!(
        fs::read_link(root.join("CLAUDE.md")).unwrap(),
        PathBuf::from("AGENTS.md"),
        "CLAUDE.md must stay a symlink to AGENTS.md"
    );
}
