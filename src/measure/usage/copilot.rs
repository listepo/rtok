// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! GitHub Copilot CLI (T358.3). `session-state/<session>/events.jsonl` is the host's event
//! log. The per-request `assistant.usage` event is ephemeral (the SDK schema marks it as
//! never persisted), so the only token counts on disk are the `modelMetrics` of the
//! `session.shutdown` event: one total per model for the whole run (github/copilot-sdk
//! `nodejs/src/generated/session-events.ts` at 5b2d7cd, see `research.md`).
//!
//! That makes this reader coarser than the others: a session lands on the day it ended,
//! and a session that never shut down cleanly (still open, killed) has no totals and counts
//! nowhere rather than being estimated from the `outputTokens` of its messages.

use crate::measure::{codex, jsonl};
use crate::store::UsageSlice;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Every `session.shutdown` model total under `dirs`, and the first log that could not be
/// read.
pub(super) fn slices(
    dirs: &[PathBuf],
    since: i64,
    cutoff: SystemTime,
) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut paths: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| codex::jsonl_paths(d, cutoff))
        .filter(|p| p.file_name().is_some_and(|n| n == "events.jsonl"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    let mut unreadable = None;
    for p in paths {
        match std::fs::read_to_string(&p) {
            Ok(text) => out.extend(session(&p, &text, since)),
            Err(_) => {
                unreadable.get_or_insert(p);
            }
        }
    }
    (out, unreadable)
}

fn session(path: &Path, text: &str, since: i64) -> Vec<UsageSlice> {
    let id = path
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = Vec::new();
    // Most of a log is tool and hook events: only the shutdown line is parsed.
    for line in text.lines().filter(|l| l.contains("\"session.shutdown\"")) {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("session.shutdown") {
            continue;
        }
        let ts = jsonl::line_ts(&v);
        let Some(models) = v.pointer("/data/modelMetrics").and_then(Value::as_object) else {
            continue;
        };
        for (model, metric) in models {
            let n = |k: &str| {
                metric
                    .pointer(&format!("/usage/{k}"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .max(0)
            };
            let (cache_read, cache_write) = (n("cacheReadTokens"), n("cacheWriteTokens"));
            // `inputTokens` is taken to count the cached part too (unverified, `research.md`
            // 30.2): the one real log's cache reads stay below it.
            let input = (n("inputTokens") - cache_read - cache_write).max(0);
            out.extend(super::slice(
                "copilot",
                "github",
                &id,
                Some(model.clone()),
                ts,
                [input, cache_write, cache_read, n("outputTokens")],
                since,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shutdown(ts: &str, metrics: Value) -> String {
        json!({"type":"session.shutdown","timestamp":ts,"data":{"modelMetrics":metrics}})
            .to_string()
    }

    fn metric(i: i64, cr: i64, cw: i64, o: i64) -> Value {
        json!({"requests":{"count":3},"usage":{"inputTokens":i,"cacheReadTokens":cr,
            "cacheWriteTokens":cw,"outputTokens":o,"reasoningTokens":4}})
    }

    #[test]
    fn each_model_of_a_shutdown_is_one_slice_dated_when_the_session_ended() {
        let d = std::env::temp_dir().join(format!("rtok-usage-cop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sess-1")).unwrap();
        std::fs::create_dir_all(d.join("sess-2")).unwrap();
        let log = [
            json!({"type":"assistant.message","data":{"outputTokens":99}}).to_string(),
            shutdown(
                "2026-09-02T10:58:10.672Z",
                json!({"gpt-x": metric(100, 60, 0, 7), "claude-y": metric(50, 10, 20, 5)}),
            ),
            "not json \"session.shutdown\"".into(),
        ]
        .join("\n");
        std::fs::write(d.join("sess-1/events.jsonl"), log).unwrap();
        // No shutdown (still open, or killed): no totals, so nothing is counted.
        std::fs::write(
            d.join("sess-2/events.jsonl"),
            "{\"type\":\"session.start\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("sess-1/other.jsonl"),
            shutdown("2026-09-02T10:58:10Z", json!({})),
        )
        .unwrap();

        let (mut rows, bad) = slices(std::slice::from_ref(&d), 0, SystemTime::UNIX_EPOCH);
        assert!(bad.is_none());
        rows.sort_by(|a, b| a.model.cmp(&b.model));
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].model.as_deref(),
                rows[0].session.as_str(),
                rows[0].ts
            ),
            (Some("claude-y"), "sess-1", 1_788_346_690)
        );
        // `inputTokens` includes both cache legs; reasoning is inside `outputTokens`.
        assert_eq!(
            (
                rows[0].input,
                rows[0].cache_create,
                rows[0].cache_read,
                rows[0].output
            ),
            (20, 20, 10, 5)
        );
        assert_eq!(
            (rows[1].input, rows[1].cache_read, rows[1].output),
            (40, 60, 7)
        );
        assert!(
            slices(
                std::slice::from_ref(&d),
                1_788_346_691,
                SystemTime::UNIX_EPOCH
            )
            .0
            .is_empty()
        );
        std::fs::remove_dir_all(&d).ok();
    }
}
