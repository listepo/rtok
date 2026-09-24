//! Structural checks on `plan.md` / `todo.md` (plan T198).
//!
//! Rules enforced here:
//! 1. every `| T… |` task-table row id, and every `### T….` card heading id, is unique.
//! 2. the task table (header through the last contiguous `|` row) has no blank line
//!    inside it: no `| T… |` row is found once the table has ended.
//! 3. every card has exactly one line starting with `Check:`, except the ids in
//!    `CHECK_EXCEPTIONS` — their owning session must add/fix the Check and drop the
//!    id here. A card's body is the lines from its `### ` heading to the next
//!    `### ` or `## ` heading.
//! 4. no table row whose Agent column names a low-cost model (`haiku`) owns a card
//!    whose body mentions `src/` — AGENTS.md forbids low-cost models on code.
//! 5. every `todo.md` `- T…` id is unique. Set equality with the plan.md table is not
//!    asserted: concurrent sessions claim in their own branches, so it drifts in flight.

use regex::Regex;
use std::fs;
use std::path::Path;

/// Cards with no (or a misplaced) Check line today; owners must add/fix one and
/// drop the id here. T184 carries T183's stray second Check, which leaves with
/// T184's own card once T183 gets its own.
const CHECK_EXCEPTIONS: &[&str] = &["T184", "T235", "T246.3", "T246.4"];

fn read(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn dupes(ids: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut dup: Vec<String> = ids
        .iter()
        .filter(|id| !seen.insert(id.as_str()))
        .cloned()
        .collect();
    dup.sort();
    dup.dedup();
    dup
}

#[test]
fn plan_and_todo_ids_are_sane() {
    let plan = read("plan.md");
    let todo = read("todo.md");
    let lines: Vec<&str> = plan.lines().collect();

    let row_re = Regex::new(r"^\|\s*(T\d+(?:\.\d+)*)\s*\|").unwrap();
    let heading_re = Regex::new(r"^###\s+(T\d+(?:\.\d+)*)\.").unwrap();

    // --- task table: header, then contiguous `|` rows ---
    let header = lines
        .iter()
        .position(|l| l.starts_with("| # | Status"))
        .expect("no task table header in plan.md");
    let mut end = header + 2; // skip header + `| --- |` separator
    let mut row_ids = Vec::new();
    let mut row_agents = Vec::new();
    while end < lines.len() && lines[end].starts_with('|') {
        if let Some(c) = row_re.captures(lines[end]) {
            row_ids.push(c[1].to_string());
            row_agents.push(
                lines[end]
                    .split('|')
                    .nth(6)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            );
        }
        end += 1;
    }
    let d = dupes(&row_ids);
    assert!(d.is_empty(), "duplicate task-table row id(s): {d:?}");

    let mut split_by = Vec::new();
    for l in &lines[end..] {
        if let Some(c) = row_re.captures(l) {
            split_by.push(c[1].to_string());
        }
    }
    assert!(
        split_by.is_empty(),
        "blank line splits the task table before row id(s): {split_by:?}"
    );

    // --- cards: unique ids, one Check: line, no haiku model owning src/ ---
    let mut heading_idxs = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("### ") || l.starts_with("## ") {
            heading_idxs.push(i);
        }
    }
    let mut card_ids = Vec::new();
    let mut bad_check = Vec::new();
    let mut haiku_src = Vec::new();
    for (n, &idx) in heading_idxs.iter().enumerate() {
        let Some(c) = heading_re.captures(lines[idx]) else {
            continue;
        };
        let id = c[1].to_string();
        let body_end = heading_idxs.get(n + 1).copied().unwrap_or(lines.len());
        let body = lines[idx + 1..body_end].join("\n");
        let checks = body.lines().filter(|l| l.starts_with("Check:")).count();
        if checks != 1 && !CHECK_EXCEPTIONS.contains(&id.as_str()) {
            bad_check.push(format!("{id} ({checks} Check: lines)"));
        }
        let on_haiku_src = row_ids.iter().position(|r| r == &id).is_some_and(|pos| {
            row_agents[pos].to_lowercase().contains("haiku") && body.contains("src/")
        });
        if on_haiku_src {
            haiku_src.push(id.clone());
        }
        card_ids.push(id);
    }
    let d = dupes(&card_ids);
    assert!(d.is_empty(), "duplicate card heading id(s): {d:?}");
    assert!(
        bad_check.is_empty(),
        "card(s) without exactly one Check: line: {bad_check:?}"
    );
    assert!(
        haiku_src.is_empty(),
        "low-cost-model card(s) touching src/: {haiku_src:?}"
    );

    // --- todo.md: unique ids; report (don't fail on) drift against plan.md ---
    let todo_re = Regex::new(r"^-\s+(T\d+(?:\.\d+)*)\.").unwrap();
    let mut todo_ids = Vec::new();
    for l in todo.lines() {
        if let Some(c) = todo_re.captures(l) {
            todo_ids.push(c[1].to_string());
        }
    }
    let d = dupes(&todo_ids);
    assert!(d.is_empty(), "duplicate todo.md row id(s): {d:?}");
}
