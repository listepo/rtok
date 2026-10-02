// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Gemini CLI (T358.3). `~/.gemini/tmp/<project>/chats/session-*.jsonl` is an append-only
//! log whose records are chat messages, patches and rewinds; a `gemini` message carries
//! `tokens` (`input` is the prompt count including `cached`, `thoughts` and `tool` are
//! counted apart) and its `model` (google-gemini/gemini-cli `packages/core/src/services/
//! chatRecordingService.ts` and `chatRecordingTypes.ts` at fb972b2, see `research.md`).
//! A session an older release wrote is one `session-*.json` document with a `messages`
//! array; the CLI migrates it only when it is opened again, so both are read.
//!
//! A message is keyed by its id and the last record wins, as the CLI's own loader does.
//! A rewound message was still billed, so rewinds are ignored.

use crate::measure::{codex, jsonl};
use crate::store::UsageSlice;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// Every `gemini` message with token counts under `dirs`, and the first chat file that held
/// no JSON at all.
pub(super) fn slices(
    dirs: &[PathBuf],
    since: i64,
    cutoff: SystemTime,
) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut paths: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| {
            ["jsonl", "json"]
                .into_iter()
                .flat_map(|e| codex::paths_with_ext(d, cutoff, e))
        })
        .filter(|p| is_chat(p))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    let mut unreadable = None;
    for p in paths {
        match std::fs::read_to_string(&p) {
            Ok(text) => match session(&p, &text, since) {
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

/// A recorded chat, not the project's `logs.json` or a checkpoint: `session-*` under `chats/`
/// (a sub-agent's chat sits one level deeper, in `chats/<parent session>/`).
fn is_chat(p: &Path) -> bool {
    let named = p
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("session-"));
    named
        && p.components()
            .any(|c| c == Component::Normal("chats".as_ref()))
}

/// The requests of one chat file; `None` when no line of it was JSON.
fn session(path: &Path, text: &str, since: i64) -> Option<Vec<UsageSlice>> {
    let mut id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut messages: BTreeMap<String, Value> = BTreeMap::new();
    let mut parsed = 0;
    let mut take = |v: &Value, id: &mut String| {
        if let Some(s) = v.get("sessionId").and_then(Value::as_str) {
            *id = s.to_owned();
        }
        let list = v.get("messages").and_then(Value::as_array);
        for m in list.into_iter().flatten().chain(std::iter::once(v)) {
            if let Some(mid) = m.get("id").and_then(Value::as_str) {
                messages.insert(mid.to_owned(), m.clone());
            }
        }
    };
    if path.extension().is_some_and(|e| e == "json") {
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            parsed += 1;
            take(&v, &mut id);
        }
    } else {
        for v in text
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        {
            parsed += 1;
            take(&v, &mut id);
        }
    }
    if parsed == 0 && !text.trim().is_empty() {
        return None;
    }
    Some(
        messages
            .values()
            .filter_map(|m| request(&id, m, since))
            .collect(),
    )
}

fn request(session: &str, m: &Value, since: i64) -> Option<UsageSlice> {
    if m.get("type")?.as_str()? != "gemini" {
        return None;
    }
    let t = m.get("tokens")?;
    let n = |k: &str| t.get(k).and_then(Value::as_i64).unwrap_or(0).max(0);
    let cached = n("cached");
    super::slice(
        "gemini",
        "google",
        session,
        m.get("model").and_then(Value::as_str).map(str::to_owned),
        jsonl::line_ts(m),
        [
            (n("input") - cached).max(0) + n("tool"),
            0,
            cached,
            n("output") + n("thoughts"),
        ],
        since,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(id: &str, ts: &str, tokens: Value) -> String {
        json!({"id":id,"timestamp":ts,"type":"gemini","content":"","model":"gemini-x",
            "tokens":tokens})
        .to_string()
    }

    fn tokens(input: i64, output: i64, cached: i64, thoughts: i64, tool: i64) -> Value {
        json!({"input":input,"output":output,"cached":cached,"thoughts":thoughts,
            "tool":tool,"total":input+output+thoughts+tool})
    }

    #[test]
    fn jsonl_and_legacy_json_chats_count_each_message_once() {
        let d = std::env::temp_dir().join(format!("rtok-usage-gem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let chats = d.join("proj/chats");
        std::fs::create_dir_all(chats.join("parent")).unwrap();
        let log = [
            json!({"sessionId":"sess-a","projectHash":"h","messages":[]}).to_string(),
            msg("g1", "2026-09-30T23:30:00Z", tokens(100, 7, 40, 3, 2)),
            // The same message re-recorded (a rewrite) is still one request.
            msg("g1", "2026-09-30T23:30:00Z", tokens(100, 7, 40, 3, 2)),
            json!({"id":"u1","timestamp":"2026-09-30T23:29:00Z","type":"user","content":"hi"})
                .to_string(),
            json!({"$patch":{"id":"g1","content":"x"}}).to_string(),
            json!({"$rewindTo":"u1"}).to_string(),
        ]
        .join("\n");
        std::fs::write(chats.join("session-1-aaaa.jsonl"), log).unwrap();
        std::fs::write(
            chats.join("parent/session-2-bbbb.jsonl"),
            msg("g2", "2026-10-01T00:00:00Z", tokens(10, 1, 0, 0, 0)),
        )
        .unwrap();
        let legacy = json!({"sessionId":"sess-c","messages":[
            serde_json::from_str::<Value>(&msg("g3","2026-10-02T00:00:00Z",tokens(5,5,5,0,0))).unwrap()]});
        std::fs::write(chats.join("session-3-cccc.json"), legacy.to_string()).unwrap();
        // Not chats: the project log and a stray file under `chats/`.
        std::fs::write(d.join("proj/logs.json"), "[]").unwrap();
        std::fs::write(
            chats.join("notes.jsonl"),
            msg("x", "2026-10-02T00:00:00Z", tokens(1, 1, 0, 0, 0)),
        )
        .unwrap();

        let (mut rows, bad) = slices(std::slice::from_ref(&d), 0, SystemTime::UNIX_EPOCH);
        assert!(bad.is_none());
        rows.sort_by_key(|r| r.ts);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (
                rows[0].session.as_str(),
                rows[0].model.as_deref(),
                rows[0].host.as_deref()
            ),
            ("sess-a", Some("gemini-x"), Some("gemini"))
        );
        // `input` includes `cached`; thoughts join output, tool-use prompt joins input.
        assert_eq!(
            (rows[0].input, rows[0].cache_read, rows[0].output),
            (62, 40, 10)
        );
        assert_eq!(rows[1].session, "session-2-bbbb");
        assert_eq!(
            (rows[2].session.as_str(), rows[2].input, rows[2].cache_read),
            ("sess-c", 0, 5)
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_chat_file_with_no_json_is_named() {
        let d = std::env::temp_dir().join(format!("rtok-usage-gem-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("p/chats")).unwrap();
        std::fs::write(d.join("p/chats/session-1-x.jsonl"), "garbage\n").unwrap();
        let (rows, bad) = slices(std::slice::from_ref(&d), 0, SystemTime::UNIX_EPOCH);
        assert!(rows.is_empty());
        assert!(bad.unwrap().ends_with("session-1-x.jsonl"));
        std::fs::remove_dir_all(&d).ok();
    }
}
