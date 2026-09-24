//! T218: every `docs/*.md` file must be reachable from the site nav, and no page under
//! `site/content/docs/` may duplicate a repo file that is (or should be) mounted onto the site.
//!
//! `site/hugo.toml` mounts `../docs` read-only into `assets/repo/docs`; the `_content.gotmpl`
//! adapters under `site/content/docs/` turn those mounted assets into pages ("Nothing is
//! copied; editing the repo file edits the page."). A repo doc that never shows up in an
//! adapter is mounted but unreachable. A `site/content/docs/*.md` whose name also exists
//! under `docs/` is a hand-authored fork of a file that should be the single source of truth.

use std::fs;
use std::path::PathBuf;

/// `docs/*.md` files that intentionally have no site page, with the reason why.
const EXEMPT: &[(&str, &str)] = &[(
    "plugin-plan-template.md",
    "contributor scaffold copied verbatim into src/plugins/<id>/PLAN.md \
     (see docs/plugin-authoring.md); not prose for site readers, same reasoning \
     that keeps CONTRIBUTING.md off the site",
)];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn docs_md_files() -> Vec<String> {
    let mut out = Vec::new();
    for e in fs::read_dir(repo_root().join("docs")).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(p.file_name().unwrap().to_str().unwrap().to_string());
        }
    }
    out.sort();
    out
}

/// Every `_content.gotmpl` under `site/content/docs/`, concatenated.
fn gotmpl_content() -> String {
    let mut out = String::new();
    let mut stack = vec![repo_root().join("site/content/docs")];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|s| s.to_str()) == Some("_content.gotmpl") {
                out.push_str(&fs::read_to_string(&p).unwrap());
                out.push('\n');
            }
        }
    }
    out
}

/// Real (non-adapter) markdown pages under `site/content/docs/`, excluding section indexes.
fn site_content_md_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root().join("site/content/docs")];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("md")
                && p.file_name().and_then(|s| s.to_str()) != Some("_index.md")
            {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn every_doc_is_mounted_or_exempt() {
    let gotmpl = gotmpl_content();
    let exempt: Vec<&str> = EXEMPT.iter().map(|(name, _)| *name).collect();
    for name in docs_md_files() {
        if exempt.contains(&name.as_str()) {
            continue;
        }
        let needle = format!("docs/{name}");
        assert!(
            gotmpl.contains(&needle),
            "docs/{name} is not referenced by any site/content/docs/**/_content.gotmpl \
             (mounted at assets/repo/docs but never published) — add a row or exempt it \
             in tests/site_pages.rs::EXEMPT with a reason"
        );
    }
}

#[test]
fn exemptions_are_real_docs_files_with_a_reason() {
    let have = docs_md_files();
    for (name, reason) in EXEMPT {
        assert!(
            !reason.trim().is_empty(),
            "{name}: exemption needs a reason"
        );
        assert!(
            have.contains(&name.to_string()),
            "{name}: exempted file does not exist under docs/ — stale exemption"
        );
    }
}

#[test]
fn no_site_page_duplicates_a_repo_doc() {
    let docs = docs_md_files();
    for path in site_content_md_files() {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            !docs.contains(&name),
            "{} duplicates docs/{name}; mount the repo file via _content.gotmpl \
             instead of hand-copying it (site/hugo.toml: \"Nothing is copied\")",
            path.strip_prefix(repo_root()).unwrap().display()
        );
    }
}
