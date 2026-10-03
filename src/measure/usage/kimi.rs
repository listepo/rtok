// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Kimi Code (T358.4). A session lives at `$KIMI_CODE_HOME/sessions/<work dir key>/<session>/`,
//! and every agent in it (the main one and its sub-agents) appends to
//! `agents/<agent>/wire.jsonl`. Each LLM request ends in a `usage.record` line with its own
//! `usage` (`inputOther`, `inputCacheCreation`, `inputCacheRead`, `output`), `model` and a
//! millisecond `time` (MoonshotAI/kimi-code `packages/` at 21406fb, see `research.md` 30.6).
//! `usageScope` only says whether a turn issued the request; every record is one request,
//! so all are summed. The legacy `~/.kimi` of the archived kimi-cli is a different format
//! and is not read.

use crate::measure::codex;
use crate::store::UsageSlice;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// Every `usage.record` under `dirs`, and the first wire log that could not be read.
pub(super) fn slices(
    dirs: &[PathBuf],
    since: i64,
    cutoff: SystemTime,
) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut paths: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| codex::jsonl_paths(d, cutoff))
        .filter(|p| p.file_name().is_some_and(|n| n == "wire.jsonl"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    let mut unreadable = None;
    for p in paths {
        match std::fs::read_to_string(&p) {
            Ok(text) => out.extend(wire(&p, &text, since)),
            Err(_) => {
                unreadable.get_or_insert(p);
            }
        }
    }
    (out, unreadable)
}

/// The session a wire log belongs to: the folder above `agents/`.
fn session_of(path: &Path) -> String {
    let parts: Vec<_> = path.components().collect();
    parts
        .iter()
        .rposition(|c| *c == Component::Normal("agents".as_ref()))
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| parts.get(i))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn wire(path: &Path, text: &str, since: i64) -> Vec<UsageSlice> {
    let id = session_of(path);
    // Most of a log is context and tool events: only the usage lines are parsed.
    text.lines()
        .filter(|l| l.contains("\"usage.record\""))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v.get("type").and_then(Value::as_str) == Some("usage.record"))
        .filter_map(|v| {
            let u = v.get("usage")?;
            let n = |k: &str| u.get(k).and_then(Value::as_i64).unwrap_or(0).max(0);
            super::slice(
                "kimi",
                "moonshot",
                &id,
                v.get("model").and_then(Value::as_str).map(str::to_owned),
                v.get("time").and_then(Value::as_i64).unwrap_or(0) / 1000,
                [
                    n("inputOther"),
                    n("inputCacheCreation"),
                    n("inputCacheRead"),
                    n("output"),
                ],
                since,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(model: &str, ms: i64, u: [i64; 4]) -> String {
        json!({"type":"usage.record","model":model,"usageScope":"turn","time":ms,
            "usage":{"inputOther":u[0],"inputCacheCreation":u[1],"inputCacheRead":u[2],"output":u[3]}})
        .to_string()
    }

    #[test]
    fn every_usage_record_of_a_wire_log_is_one_request_in_its_session() {
        let text = [
            json!({"type":"metadata","protocol_version":"1.1"}).to_string(),
            // The step.end event repeats the record's usage: only the record counts.
            json!({"type":"context.append_loop_event","time":1_779_256_800_300_i64,
                "event":{"type":"step.end","usage":{"inputOther":10,"output":5}}})
            .to_string(),
            record("kimi-k2", 1_779_256_800_302, [10, 2, 30, 5]),
            record("kimi-k2", 1_779_256_900_000, [1, 0, 0, 1]),
        ]
        .join("\n");
        let p = Path::new("/h/sessions/key/session_9/agents/main/wire.jsonl");
        let rows = wire(p, &text, 1_700_000_000);
        assert_eq!(rows.len(), 2);
        let a = &rows[0];
        assert_eq!(
            (a.host.as_deref(), a.api.as_str(), a.session.as_str()),
            (Some("kimi"), "moonshot", "session_9")
        );
        assert_eq!(a.model.as_deref(), Some("kimi-k2"));
        assert_eq!(
            (a.ts, a.input, a.cache_create, a.cache_read, a.output),
            (1_779_256_800, 10, 2, 30, 5)
        );
    }

    #[test]
    fn a_record_before_since_or_without_tokens_is_dropped() {
        let text = [
            record("kimi-k2", 1_000_000, [5, 0, 0, 5]),
            record("kimi-k2", 1_779_256_800_000, [0, 0, 0, 0]),
        ]
        .join("\n");
        let p = Path::new("/h/sessions/k/s/agents/main/wire.jsonl");
        assert!(wire(p, &text, 1_700_000_000).is_empty());
    }
}
