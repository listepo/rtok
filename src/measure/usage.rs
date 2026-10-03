// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The agents' own session files as `rtok agents usage --source logs` rows (T358.2): one
//! `UsageSlice` per request, read on the fly and never written to the store, so a re-read is
//! idempotent (the T49.2 rule). Both readers sit on the parsers `rtok stats` already uses
//! (`jsonl::parse_path`, `codex::requests`); this module only adds the host, session and
//! window cut. A request with no usable timestamp is left out: it has no day to land on.

use super::{codex, jsonl, subagents};
use crate::store::UsageSlice;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A host whose session files exist but could not be read: named once, counted nowhere.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Skipped {
    pub host: String,
    pub reason: &'static str,
    pub path: PathBuf,
}

/// Everything `--source logs` read, and the hosts it had to skip.
#[derive(Debug, Default)]
pub struct Logs {
    pub slices: Vec<UsageSlice>,
    pub skipped: Vec<Skipped>,
}

/// Claude Code (`claude_dir`) and Codex (`codex_dir`) requests stamped at or after `since`.
pub fn read(claude_dir: &Path, codex_dir: &Path, since: i64) -> Logs {
    let mut logs = Logs::default();
    for (host, (slices, bad)) in [
        ("claude", claude_slices(claude_dir, since)),
        ("codex", codex_slices(codex_dir, since)),
    ] {
        logs.slices.extend(slices);
        logs.skipped.extend(bad.map(|path| Skipped {
            host: host.into(),
            reason: "unknown format",
            path,
        }));
    }
    logs
}

/// Claude Code transcripts under `dir`, requests stamped at or after `since` (unix seconds),
/// and the first transcript that was unreadable or held no JSON line. A sub-agent transcript
/// counts toward its parent's session, like `rtok stats` attributes it.
fn claude_slices(dir: &Path, since: i64) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut paths = codex::jsonl_paths(dir, mtime_cutoff(since));
    paths.sort();
    let mut out = Vec::new();
    let mut unreadable = None;
    for p in paths {
        // One unreadable transcript is skipped, not fatal (fail open).
        let Ok(parsed) = jsonl::parse_path(&p) else {
            unreadable.get_or_insert(p);
            continue;
        };
        if parsed.lines > 0 && parsed.malformed == parsed.lines {
            unreadable.get_or_insert(p.clone());
        }
        let owner = if subagents::is_subagent(&p) {
            p.parent().and_then(Path::parent)
        } else {
            Some(p.as_path())
        };
        let session = owner
            .and_then(|o| o.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        for u in parsed.usages {
            out.extend(slice(
                "claude",
                "anthropic",
                &session,
                u.model,
                u.ts,
                [
                    i64::from(u.input_tokens),
                    i64::from(u.cache_creation_input_tokens),
                    i64::from(u.cache_read_input_tokens),
                    i64::from(u.output_tokens),
                ],
                since,
            ));
        }
    }
    (out, unreadable)
}

/// Codex rollouts under `dir`, requests stamped at or after `since`, and the first rollout
/// that was unreadable or held no JSON line.
fn codex_slices(dir: &Path, since: i64) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let (requests, unreadable) = codex::requests(dir, mtime_cutoff(since));
    let slices = requests
        .into_iter()
        .filter_map(|r| {
            slice(
                "codex",
                "openai",
                &r.session,
                r.model,
                r.ts,
                [r.input, r.cache_create, r.cache_read, r.output],
                since,
            )
        })
        .collect();
    (slices, unreadable)
}

/// A file untouched since before `since` cannot hold a later request: skip it unread.
fn mtime_cutoff(since: i64) -> SystemTime {
    u64::try_from(since)
        .ok()
        .and_then(|s| SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(s)))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// `legs` is `[input, cache_create, cache_read, output]`. A request with no timestamp, before
/// `since`, or carrying no tokens (Claude Code's `<synthetic>` turns) yields nothing.
fn slice(
    host: &str,
    api: &str,
    session: &str,
    model: Option<String>,
    ts: i64,
    legs: [i64; 4],
    since: i64,
) -> Option<UsageSlice> {
    if ts <= 0 || ts < since || legs.iter().sum::<i64>() == 0 {
        return None;
    }
    Some(UsageSlice {
        host: Some(host.into()),
        api: api.into(),
        session: session.into(),
        model,
        ts,
        input: legs[0],
        cache_create: legs[1],
        cache_read: legs[2],
        output: legs[3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rtok-usage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn claude_line(id: &str, ts: &str, model: &str, usage: [u32; 4]) -> String {
        json!({"type":"assistant","timestamp":ts,"message":{"id":id,"model":model,
            "usage":{"input_tokens":usage[0],"cache_creation_input_tokens":usage[1],
            "cache_read_input_tokens":usage[2],"output_tokens":usage[3]}}})
        .to_string()
    }

    #[test]
    fn claude_requests_keep_model_time_and_the_parent_session() {
        let dir = tmp("claude");
        fs::create_dir_all(dir.join("p/s1/subagents")).unwrap();
        // A streamed message repeats its `usage` per content block: it is one request.
        let body = [
            claude_line(
                "m1",
                "2026-09-30T23:30:00Z",
                "claude-sonnet-4",
                [10, 0, 5, 2],
            ),
            claude_line(
                "m1",
                "2026-09-30T23:30:01Z",
                "claude-sonnet-4",
                [10, 0, 5, 2],
            ),
            claude_line("m2", "2026-10-01T00:10:00Z", "<synthetic>", [0, 0, 0, 0]),
            "not json".into(),
        ]
        .join("\n");
        fs::write(dir.join("p/s1.jsonl"), body).unwrap();
        fs::write(
            dir.join("p/s1/subagents/agent-a.jsonl"),
            claude_line("m3", "2026-10-01T01:00:00Z", "claude-haiku-4", [1, 2, 3, 4]),
        )
        .unwrap();
        let mut rows = claude_slices(&dir, 0).0;
        rows.sort_by_key(|r| r.ts);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].session.as_str(),
                rows[0].model.as_deref(),
                rows[0].ts
            ),
            ("s1", Some("claude-sonnet-4"), 1_790_811_000)
        );
        assert_eq!(
            (rows[0].input, rows[0].cache_read, rows[0].output),
            (10, 5, 2)
        );
        assert_eq!((rows[1].session.as_str(), rows[1].cache_create), ("s1", 2));
        assert_eq!(claude_slices(&dir, 1_790_812_000).0.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn codex_requests_take_the_model_from_the_latest_turn_context() {
        let dir = tmp("codex");
        let count = |ts: &str, input: i64, cached: i64, out: i64| {
            json!({"timestamp":ts,"type":"event_msg","payload":{"type":"token_count","info":{
                "last_token_usage":{"input_tokens":input,"cached_input_tokens":cached,
                "output_tokens":out}}}})
            .to_string()
        };
        let body = [
            json!({"type":"session_meta","payload":{"id":"sess-1"}}).to_string(),
            json!({"type":"turn_context","payload":{"model":"gpt-5"}}).to_string(),
            count("2026-09-30T23:30:00Z", 100, 40, 7),
            count("not a time", 5, 0, 1),
        ]
        .join("\n");
        fs::write(dir.join("rollout-a.jsonl"), body).unwrap();
        let rows = codex_slices(&dir, 0).0;
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(
            (r.session.as_str(), r.model.as_deref(), r.host.as_deref()),
            ("sess-1", Some("gpt-5"), Some("codex"))
        );
        assert_eq!((r.input, r.cache_read, r.output), (60, 40, 7));
        fs::remove_dir_all(&dir).ok();
    }
}
