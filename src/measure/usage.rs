// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The agents' own session files as `rtok agents usage --source logs` rows (T358.2): one
//! `UsageSlice` per request, read on the fly and never written to the store, so a re-read is
//! idempotent (the T49.2 rule). Claude Code and Codex sit on the parsers `rtok stats` already
//! uses (`jsonl::parse_path`, `codex::requests`); this module only adds the host, session and
//! window cut. OpenCode, Kilo, Copilot CLI and Gemini CLI have a reader each in `usage/`
//! (T358.3). A request with no usable timestamp is left out: it has no day to land on.

use super::{codex, jsonl, subagents};
use crate::config::Config;
use crate::store::UsageSlice;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

mod sqlite;

/// A host whose session files exist but could not be read: named once, counted nowhere.
#[derive(Debug, Clone, Serialize)]
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

/// Why a host's files are named without being counted: they exist but no request came out.
const UNKNOWN: &str = "unknown format";

/// Droid keeps token counts in `<session>.settings.json`, but Factory documents only that the
/// file holds them, not under which keys, so a reader would be a guess (`research.md`).
const DROID_UNSUPPORTED: &str = "unsupported: the session settings fields are not documented";

/// Every host's requests stamped at or after `since`: Claude Code and Codex from the `[stats]`
/// directories, the others from `[agents.usage.dirs]`.
pub fn read(cfg: &Config, since: i64) -> Logs {
    let dirs = &cfg.agents.usage.dirs;
    let mut logs = Logs::default();
    let mut add =
        |host: &str, reason: &'static str, (slices, bad): (Vec<UsageSlice>, Option<PathBuf>)| {
            logs.slices.extend(slices);
            logs.skipped.extend(bad.map(|path| Skipped {
                host: host.into(),
                reason,
                path,
            }));
        };
    add(
        "claude",
        UNKNOWN,
        claude_slices(&cfg.stats.transcripts_dir, since),
    );
    add("codex", UNKNOWN, codex_slices(&cfg.stats.codex_dir, since));
    add(
        "opencode",
        UNKNOWN,
        sqlite::slices("opencode", &dirs.opencode, since),
    );
    add("kilo", UNKNOWN, sqlite::slices("kilo", &dirs.kilo, since));
    let droid = dirs
        .droid
        .iter()
        .find(|d| std::fs::read_dir(d).is_ok_and(|mut rd| rd.next().is_some()));
    add("droid", DROID_UNSUPPORTED, (Vec::new(), droid.cloned()));
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

    #[test]
    fn read_names_unreadable_hosts_and_lists_droid_as_unsupported() {
        let dir = tmp("read");
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.agents.usage.dirs.opencode = vec![dir.join("oc")];
        cfg.agents.usage.dirs.copilot = vec![dir.join("cop")];
        cfg.agents.usage.dirs.droid = vec![dir.join("droid"), dir.join("none")];
        fs::create_dir_all(dir.join("oc")).unwrap();
        fs::write(dir.join("oc/opencode.db"), "not sqlite").unwrap();
        // A readable log with no shutdown counts nothing and is not a skip.
        fs::create_dir_all(dir.join("cop/s")).unwrap();
        fs::write(
            dir.join("cop/s/events.jsonl"),
            "{\"type\":\"session.start\"}\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join("droid/-proj")).unwrap();
        fs::write(dir.join("droid/-proj/a.settings.json"), "{}").unwrap();
        let logs = read(&cfg, 0);
        assert!(logs.slices.is_empty());
        let named: Vec<_> = logs
            .skipped
            .iter()
            .map(|s| (s.host.as_str(), s.reason))
            .collect();
        assert_eq!(named, [("opencode", UNKNOWN), ("droid", DROID_UNSUPPORTED)]);
        // Nothing to name when the directories are absent.
        cfg.agents.usage.dirs.opencode = vec![dir.join("absent")];
        cfg.agents.usage.dirs.droid = vec![dir.join("absent")];
        assert!(read(&cfg, 0).skipped.is_empty());
        fs::remove_dir_all(&dir).ok();
    }
}
