// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! pi (T358.4). `~/.pi/agent/sessions/<cwd>/*.jsonl` holds one entry per line; the entries
//! that spent tokens carry a `usage` object (`input`, `output`, `cacheRead`, `cacheWrite`,
//! each a request's own count): an assistant `message` under `message.usage`, and a
//! `compaction`, `branch_summary` or `usage` entry directly (pi-mono `packages/coding-agent/
//! docs/session-format.md` at 69f0be6, see `research.md` 30.5). Reasoning tokens are already
//! inside `output`.
//!
//! A forked session copies its parent's entries, so an entry is counted once however many
//! files hold it, keyed by its id and timestamp.

use crate::measure::{codex, jsonl};
use crate::store::UsageSlice;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Every priced entry under `dirs`, and the first session file that was unreadable or held no
/// JSON line.
pub(super) fn slices(
    dirs: &[PathBuf],
    since: i64,
    cutoff: SystemTime,
) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut paths: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| codex::jsonl_paths(d, cutoff))
        .collect();
    paths.sort();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut unreadable = None;
    for p in paths {
        match std::fs::read_to_string(&p) {
            Ok(text) => match session(&p, &text, since, &mut seen) {
                Some(rows) => out.extend(rows),
                None => {
                    unreadable.get_or_insert(p);
                }
            },
            Err(_) => {
                unreadable.get_or_insert(p);
            }
        }
    }
    (out, unreadable)
}

/// The requests of one session file; `None` when it has text but no line of it was JSON.
fn session(
    path: &Path,
    text: &str,
    since: i64,
    seen: &mut HashSet<(String, String)>,
) -> Option<Vec<UsageSlice>> {
    let mut id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut parsed = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        parsed += 1;
        if v.get("type").and_then(Value::as_str) == Some("session") {
            if let Some(s) = v.get("id").and_then(Value::as_str) {
                id = s.to_owned();
            }
            continue;
        }
        out.extend(entry(&id, &v, since, seen));
    }
    (parsed > 0 || text.trim().is_empty()).then_some(out)
}

fn entry(
    session: &str,
    v: &Value,
    since: i64,
    seen: &mut HashSet<(String, String)>,
) -> Option<UsageSlice> {
    let usage = v
        .get("usage")
        .or_else(|| v.pointer("/message/usage"))
        .filter(|u| u.is_object())?;
    let stamp = v.get("timestamp").and_then(Value::as_str)?;
    if let Some(entry_id) = v.get("id").and_then(Value::as_str)
        && !seen.insert((entry_id.to_owned(), stamp.to_owned()))
    {
        return None;
    }
    let n = |k: &str| usage.get(k).and_then(Value::as_i64).unwrap_or(0).max(0);
    let text = |k: &str| {
        v.get(k)
            .or_else(|| v.get("message").and_then(|m| m.get(k)))
            .and_then(Value::as_str)
    };
    super::slice(
        "pi",
        text("provider").unwrap_or("pi"),
        session,
        text("model").map(str::to_owned),
        jsonl::line_ts(v),
        [n("input"), n("cacheWrite"), n("cacheRead"), n("output")],
        since,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SINCE: i64 = 1_700_000_000;

    fn run(text: &str) -> Vec<UsageSlice> {
        session(
            Path::new("/s/2024_abc.jsonl"),
            text,
            SINCE,
            &mut HashSet::new(),
        )
        .expect("readable")
    }

    fn assistant(id: &str, ts: &str, u: Value) -> String {
        json!({"type":"message","id":id,"timestamp":ts,"message":{
            "role":"assistant","provider":"anthropic","model":"claude-sonnet-4-5","usage":u}})
        .to_string()
    }

    #[test]
    fn an_assistant_message_and_a_summary_entry_are_counted_with_their_legs() {
        let u = json!({"input":10,"output":5,"cacheRead":100,"cacheWrite":7,"totalTokens":122});
        let text = [
            json!({"type":"session","id":"sess-1","timestamp":"2024-12-03T14:00:00.000Z"})
                .to_string(),
            json!({"type":"message","id":"u1","timestamp":"2024-12-03T14:00:01.000Z",
                "message":{"role":"user","content":"hi"}})
            .to_string(),
            assistant("a1", "2024-12-03T14:00:02.000Z", u),
            json!({"type":"compaction","id":"c1","timestamp":"2024-12-03T14:10:00.000Z",
                "provider":"anthropic","model":"claude-haiku-4-5",
                "usage":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0}})
            .to_string(),
        ]
        .join("\n");
        let rows = run(&text);
        assert_eq!(rows.len(), 2);
        let a = &rows[0];
        assert_eq!(
            (a.host.as_deref(), a.api.as_str(), a.session.as_str()),
            (Some("pi"), "anthropic", "sess-1")
        );
        assert_eq!(a.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(
            (a.input, a.cache_create, a.cache_read, a.output),
            (10, 7, 100, 5)
        );
        assert_eq!(rows[1].model.as_deref(), Some("claude-haiku-4-5"));
        assert_eq!((rows[1].input, rows[1].output), (1, 2));
    }

    #[test]
    fn an_entry_copied_into_a_forked_session_counts_once() {
        let line = assistant(
            "a1",
            "2024-12-03T14:00:02.000Z",
            json!({"input":3,"output":4,"cacheRead":0,"cacheWrite":0}),
        );
        let mut seen = HashSet::new();
        let first = session(Path::new("/s/a.jsonl"), &line, SINCE, &mut seen).unwrap();
        let fork = session(Path::new("/s/b.jsonl"), &line, SINCE, &mut seen).unwrap();
        assert_eq!((first.len(), fork.len()), (1, 0));
    }

    #[test]
    fn an_old_entry_and_an_empty_usage_are_dropped_and_a_non_json_file_is_unreadable() {
        let zero = json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0});
        let live = json!({"input":1,"output":1,"cacheRead":0,"cacheWrite":0});
        let text = [
            assistant("a1", "2001-01-01T00:00:00.000Z", live.clone()),
            assistant("a2", "2024-12-03T14:00:02.000Z", zero),
        ]
        .join("\n");
        assert!(run(&text).is_empty());
        let mut seen = HashSet::new();
        assert!(session(Path::new("/s/x.jsonl"), "not json\n", SINCE, &mut seen).is_none());
        let empty = session(Path::new("/s/x.jsonl"), "", SINCE, &mut seen);
        assert!(empty.is_some_and(|rows| rows.is_empty()));
    }
}
