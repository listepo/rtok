// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Duplicate hooks (T331.3): one agent loads the same hook twice, so it runs twice. Entries
//! of the same host, event, matcher and normalized command are copies; one copy is recommended
//! to keep (plugin-owned, then a project's shared file, then the user's, then a `.local` file,
//! first in load order among equals). Report only: no copy is marked fixable before T331.6.

use std::collections::BTreeMap;

use super::hooks::{Problem, RANK_PLUGIN, Seen};

/// `Bash|Edit`, ` Edit | Bash ` and the same list reordered match the same tools; no matcher,
/// an empty one and `*` all match everything.
fn matcher_key(m: Option<&str>) -> String {
    let m = m.map_or("", str::trim);
    if m == "*" {
        return String::new();
    }
    let mut parts: Vec<&str> = m.split('|').map(str::trim).collect();
    parts.sort_unstable();
    parts.join("|")
}

fn why(rank: u8) -> &'static str {
    match rank {
        RANK_PLUGIN => "owned by a plugin",
        1 => "shared project file",
        2 => "user file",
        _ => "first in load order",
    }
}

/// Every group of two or more copies, as one finding per copy, groups in load order.
pub(super) fn find(seen: &[Seen]) -> Vec<Problem> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut index: BTreeMap<(&str, &str, String, &str), usize> = BTreeMap::new();
    for (i, s) in seen.iter().enumerate() {
        let key = (
            s.agent,
            s.event.as_str(),
            matcher_key(s.matcher.as_deref()),
            s.normal.as_str(),
        );
        let g = *index.entry(key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    let mut out = Vec::new();
    for (n, copies) in groups.iter().filter(|g| g.len() > 1).enumerate() {
        // `min_by_key` takes the first of equals, which is the first in load order.
        let keep = *copies
            .iter()
            .min_by_key(|i| seen[**i].rank)
            .unwrap_or(&copies[0]);
        for &i in copies {
            let s = &seen[i];
            let detail = if i == keep {
                format!(
                    "runs {} times; keep this copy ({})",
                    copies.len(),
                    why(s.rank)
                )
            } else {
                format!("runs {} times; an extra copy", copies.len())
            };
            out.push(Problem {
                kind: "duplicate-hook",
                agent: s.agent,
                source: s.source.clone(),
                path: s.path.clone(),
                event: s.event.clone(),
                matcher: s.matcher.clone(),
                command: s.command.clone(),
                detail,
                fixable: false,
                group: Some(n as u32),
                keep: i == keep,
            });
        }
    }
    out
}

/// The `duplicate hooks` lines of the doctor text: each group once, its copies below it.
pub(super) fn render(problems: &[Problem]) -> String {
    let dupes: Vec<&Problem> = problems
        .iter()
        .filter(|p| p.kind == "duplicate-hook")
        .collect();
    if dupes.is_empty() {
        return "duplicate hooks none found\n".into();
    }
    let mut out = String::from("duplicate hooks\n");
    let mut last = None;
    for p in dupes {
        if last != p.group {
            last = p.group;
            let matcher = p
                .matcher
                .as_deref()
                .map(|m| format!("[{m}]"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {} {}{matcher} `{}`: {}\n",
                p.agent,
                p.event,
                p.command,
                p.detail.split(';').next().unwrap_or_default()
            ));
        }
        let role = if p.keep { "keep " } else { "extra" };
        out.push_str(&format!("    {role} {} {}\n", p.source, p.path));
    }
    out
}
