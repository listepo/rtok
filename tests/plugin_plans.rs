//! T14.0: every `src/plugins/*/PLAN.md` has the required D15 structure.
//! Passes on a tree with no `PLAN.md` and tightens as each lands.

use std::fs;
use std::path::PathBuf;

const REQUIRED_HEADINGS: &[&str] = &[
    "## Problem",
    "## Alternatives",
    "## Mechanism",
    "## Rejected",
];

fn plugin_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugins")
}

fn plan_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(plugin_root()) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path().join("PLAN.md");
        if p.is_file() {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn alternatives_data_rows(body: &str) -> usize {
    let rest = body.split("## Alternatives").nth(1).unwrap_or("");
    let section = rest.split("\n## ").next().unwrap_or(rest);
    section
        .lines()
        .filter(|l| l.trim().starts_with('|') && !is_separator(l))
        .skip(1)
        .count()
}

fn labeled_lines(body: &str, label: &str) -> usize {
    body.lines()
        .filter(|l| {
            l.trim()
                .trim_start_matches('#')
                .trim()
                .trim_start_matches('*')
                .trim()
                .starts_with(label)
        })
        .count()
}

fn check(body: &str) -> Result<(), String> {
    for h in REQUIRED_HEADINGS {
        if !body.lines().any(|l| l.trim() == *h) {
            return Err(format!("missing heading {h}"));
        }
    }
    let rows = alternatives_data_rows(body);
    if rows < 3 {
        return Err(format!("alternatives table has {rows} data rows, need ≥ 3"));
    }
    let targets = labeled_lines(body, "Target:");
    if targets != 1 {
        return Err(format!("want exactly one Target: line, found {targets}"));
    }
    let falsified = labeled_lines(body, "Falsified by:");
    if falsified != 1 {
        return Err(format!(
            "want exactly one Falsified by: line, found {falsified}"
        ));
    }
    Ok(())
}

const COMPLETE: &str = r#"## Problem
waste

## Alternatives
| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| a | 1 | 2026-01-01 | r | w |
| b | 1 | 2026-01-01 | r | w |
| c | 1 | 2026-01-01 | r | w |

## Mechanism
m

## Rejected
- x
- y

Target: 1
Falsified by: 2
"#;

#[test]
fn plugin_plans_walks_existing() {
    for path in plan_files() {
        let body = fs::read_to_string(&path).unwrap();
        check(&body).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

#[test]
fn plugin_plans_fixture_complete_passes() {
    check(COMPLETE).unwrap();
}

#[test]
fn plugin_plans_missing_heading_fails() {
    let body = COMPLETE.replace("## Mechanism\nm\n", "");
    assert!(check(&body).unwrap_err().contains("## Mechanism"));
}

#[test]
fn plugin_plans_missing_table_row_fails() {
    let body = COMPLETE.replace("| c | 1 | 2026-01-01 | r | w |\n", "");
    assert!(check(&body).unwrap_err().contains("data rows"));
}

#[test]
fn plugin_plans_missing_target_fails() {
    let body = COMPLETE.replace("Target: 1\n", "");
    assert!(check(&body).unwrap_err().contains("Target:"));
}

#[test]
fn plugin_plans_extra_target_fails() {
    let body = COMPLETE.replace("Target: 1\n", "Target: 1\nTarget: 2\n");
    assert!(check(&body).unwrap_err().contains("Target:"));
}

#[test]
fn plugin_plans_missing_falsified_fails() {
    let body = COMPLETE.replace("Falsified by: 2\n", "");
    assert!(check(&body).unwrap_err().contains("Falsified by:"));
}

#[test]
fn every_target_matches_a_roadmap_gate() {
    let roadmap = include_str!("../roadmap.md");
    let files = plan_files();
    assert_eq!(files.len(), 10, "want 10 PLAN.md files");
    for p in &files {
        let body = fs::read_to_string(p).unwrap();
        let target = body
            .lines()
            .find(|l| l.trim().starts_with("Target:"))
            .unwrap_or_else(|| panic!("{} missing Target:", p.display()));
        let rest = target.trim().trim_start_matches("Target:").trim();
        assert!(
            roadmap.contains(rest),
            "{} Target is not a roadmap.md gate: {rest}",
            p.display()
        );
    }
}
