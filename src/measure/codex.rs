//! Codex CLI session usage (plan T49.2). `~/.codex/sessions/**/*.jsonl` writes one
//! `event_msg` line of `payload.type == "token_count"` per API request; its
//! `last_token_usage` is that request's counters (`input_tokens` includes
//! `cached_input_tokens`, `output_tokens` includes reasoning). Read on the fly like the
//! Claude Code transcripts, never written to the store, so a re-read is idempotent by
//! construction. Surveyed 2026-09-17: OpenCode's `opencode.db` and Cursor's `state.vscdb`
//! carry no token counts and Copilot CLI's `data.db` only a context size, so `codex` is the
//! one host row here; the others are documented as unsupported, not estimated.

use super::stats::ApiRow;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Every `*.jsonl` under `dir` (recursive) modified at or after `cutoff`. Shared with the
/// Claude Code transcript walk in `stats::collect`.
pub(super) fn jsonl_paths(dir: &Path, cutoff: SystemTime) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if mtime >= cutoff {
                out.push(p);
            }
        }
    }
    out
}

/// Sum of `last_token_usage` over every `token_count` line, as an `ApiRow` in the
/// proxy's vocabulary: `input` is the uncached part, `cache_read` the cached part,
/// `cache_create` Codex's `cache_write_input_tokens`. `None` when no line was found.
pub fn collect(dir: &Path, cutoff: SystemTime) -> Option<ApiRow> {
    let mut row = ApiRow::default();
    let mut lines = 0u64;
    for p in jsonl_paths(dir, cutoff) {
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(u) = v.pointer("/payload/info/last_token_usage") else {
                continue;
            };
            if v.pointer("/payload/type").and_then(Value::as_str) != Some("token_count") {
                continue;
            }
            let n = |k: &str| u.get(k).and_then(Value::as_i64).unwrap_or(0);
            let cached = n("cached_input_tokens");
            row.input += n("input_tokens").saturating_sub(cached);
            row.cache_read += cached;
            row.cache_create += n("cache_write_input_tokens");
            row.output += n("output_tokens");
            lines += 1;
        }
    }
    if lines == 0 {
        return None;
    }
    let denom = row.cache_read + row.cache_create + row.input;
    row.hit = if denom == 0 {
        0.0
    } else {
        row.cache_read as f64 / denom as f64
    };
    Some(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn line(input: i64, cached: i64, write: i64, output: i64) -> String {
        json!({"timestamp":"2026-09-02T21:33:02.049Z","type":"event_msg","payload":{"type":"token_count",
            "info":{"last_token_usage":{"input_tokens":input,"cached_input_tokens":cached,
            "cache_write_input_tokens":write,"output_tokens":output,"reasoning_output_tokens":1},
            "total_token_usage":{"input_tokens":999}}}})
        .to_string()
    }

    #[test]
    fn sums_last_token_usage_and_splits_cached_input() {
        let dir = std::env::temp_dir().join(format!("rtok-codex-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("2026/09/03")).unwrap();
        let body = [
            json!({"type":"session_meta","payload":{"id":"s1","model":null}}).to_string(),
            line(20776, 20480, 0, 187),
            line(21324, 20608, 10, 46),
            // `total_token_usage` alone (no `last_token_usage`) must not count.
            json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":5}}}}).to_string(),
            "not json".into(),
        ]
        .join("\n");
        fs::write(dir.join("2026/09/03/rollout-a.jsonl"), body).unwrap();
        fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let row = collect(&dir, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            (row.input, row.cache_read, row.cache_create, row.output),
            (296 + 716, 20480 + 20608, 10, 233)
        );
        let denom = (296 + 716 + 20480 + 20608 + 10) as f64;
        assert!((row.hit - (20480.0 + 20608.0) / denom).abs() < 1e-9);
        assert!(
            collect(
                &dir,
                SystemTime::now() + std::time::Duration::from_secs(3600)
            )
            .is_none()
        );
        assert!(collect(&dir.join("absent"), SystemTime::UNIX_EPOCH).is_none());
        fs::remove_dir_all(&dir).ok();
    }
}
