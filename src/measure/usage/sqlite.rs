// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! OpenCode and Kilo (T358.3). Both keep every session in one SQLite file and every
//! message as a JSON `message.data` document; an assistant message carries its own
//! `tokens`, `modelID` and `time.created`. Kilo is a fork of OpenCode and shares the
//! schema (anomalyco/opencode `packages/core/src/session/sql.ts` and `.../v1/session.ts` at
//! c42ae0d; Kilo-Org/kilocode `packages/core/src/session/sql.ts` at 76bcfd4, see `research.md`).
//!
//! The file belongs to the host, not to rtok: it is opened read-only on its own
//! connection and only the columns read here are declared, so a migration that adds
//! others cannot break the read.

use crate::store::UsageSlice;
use diesel::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

diesel::table! {
    message (id) {
        id -> Text,
        session_id -> Text,
        time_created -> BigInt,
        data -> Text,
    }
}

/// Every request of `host` (`opencode` or `kilo`) in the databases under `dirs`, and the
/// first database that could not be read. A host names a database `<host>.db`, or
/// `<host>-<channel>.db` for a non-stable release channel; the backups next to them end
/// in something else.
pub(super) fn slices(
    host: &str,
    dirs: &[PathBuf],
    since: i64,
) -> (Vec<UsageSlice>, Option<PathBuf>) {
    let mut out = Vec::new();
    let mut unreadable = None;
    for path in databases(host, dirs) {
        match read(host, &path, since) {
            Ok(rows) => out.extend(rows),
            Err(_) => {
                unreadable.get_or_insert(path);
            }
        }
    }
    (out, unreadable)
}

fn databases(host: &str, dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        found.extend(
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| is_database(host, p)),
        );
    }
    found.sort();
    found
}

fn is_database(host: &str, path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = name.strip_suffix(".db").unwrap_or("");
    path.is_file()
        && stem
            .strip_prefix(host)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
}

/// SQLite takes `?`, `#` and `%` in a `file:` name as a query, a fragment and an escape, and
/// Windows needs `file:///C:/...`, so the path goes through `Url` rather than `format!`.
/// `mode=ro` means a running host is never written to, not even a journal checkpoint.
fn read_only_uri(path: &Path) -> Option<String> {
    let mut url = url::Url::from_file_path(std::path::absolute(path).ok()?).ok()?;
    url.set_query(Some("mode=ro"));
    Some(url.into())
}

fn read(host: &str, path: &Path, since: i64) -> Result<Vec<UsageSlice>, diesel::result::Error> {
    let url = read_only_uri(path)
        .ok_or_else(|| diesel::result::Error::QueryBuilderError("path is not a file URI".into()))?;
    let mut conn = SqliteConnection::establish(&url)
        .map_err(|e| diesel::result::Error::QueryBuilderError(e.to_string().into()))?;
    let rows: Vec<(String, i64, String)> = message::table
        .filter(message::time_created.ge(since.saturating_mul(1000)))
        .order(message::time_created.asc())
        .select((message::session_id, message::time_created, message::data))
        .load(&mut conn)?;
    Ok(rows
        .into_iter()
        .filter_map(|(session, created, data)| request(host, &session, created, &data, since))
        .collect())
}

/// An assistant message as one request. `input` and `output` already exclude the cache
/// legs and the reasoning tokens (the host's `getUsage`), so the four legs and the
/// reasoning sum to the total; reasoning is billed as output.
fn request(
    host: &str,
    session: &str,
    created_ms: i64,
    data: &str,
    since: i64,
) -> Option<UsageSlice> {
    let v: Value = serde_json::from_str(data).ok()?;
    if v.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let t = v.get("tokens")?;
    let n = |x: Option<&Value>| x.and_then(Value::as_i64).unwrap_or(0).max(0);
    let cache = t.get("cache");
    let ms = v
        .pointer("/time/created")
        .and_then(Value::as_i64)
        .unwrap_or(created_ms);
    super::slice(
        host,
        v.get("providerID").and_then(Value::as_str).unwrap_or(host),
        session,
        v.get("modelID").and_then(Value::as_str).map(str::to_owned),
        ms / 1000,
        [
            n(t.get("input")),
            n(cache.and_then(|c| c.get("write"))),
            n(cache.and_then(|c| c.get("read"))),
            n(t.get("output")) + n(t.get("reasoning")),
        ],
        since,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;
    use serde_json::json;

    /// A database with the one table this reader declares. Diesel has no DDL builder and
    /// the host's schema is not rtok's to migrate, so the fixture creates it with one
    /// `CREATE TABLE`; every row goes in through the query builder.
    fn fixture(path: &Path, rows: &[(&str, i64, Value)]) {
        let mut c = SqliteConnection::establish(&path.display().to_string()).unwrap();
        c.batch_execute(
            "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, \
             time_created INTEGER NOT NULL, data TEXT NOT NULL)",
        )
        .unwrap();
        for (i, (session, created, data)) in rows.iter().enumerate() {
            diesel::insert_into(message::table)
                .values((
                    message::id.eq(format!("m{i}")),
                    message::session_id.eq(*session),
                    message::time_created.eq(*created),
                    message::data.eq(data.to_string()),
                ))
                .execute(&mut c)
                .unwrap();
        }
    }

    fn assistant(model: &str, tokens: [i64; 5]) -> Value {
        json!({"role":"assistant","modelID":model,"providerID":"anthropic",
            "time":{"created":1_790_811_000_500_i64},
            "tokens":{"input":tokens[0],"output":tokens[1],"reasoning":tokens[2],
                "cache":{"read":tokens[3],"write":tokens[4]}}})
    }

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rtok-usage-sql-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn assistant_messages_become_requests_and_the_rest_is_ignored() {
        let d = dir("ok");
        fixture(
            &d.join("opencode.db"),
            &[
                (
                    "s1",
                    1_790_811_000_500,
                    assistant("claude-x", [10, 4, 3, 5, 2]),
                ),
                (
                    "s1",
                    1_790_811_001_000,
                    json!({"role":"user","time":{"created":1}}),
                ),
                (
                    "s2",
                    1_790_811_002_000,
                    json!({"role":"assistant","modelID":"m"}),
                ),
                ("s2", 1_790_000_000_000, assistant("old", [1, 1, 1, 1, 1])),
            ],
        );
        // A backup and another host's database are not this host's file.
        std::fs::write(d.join("opencode.db.bak"), "x").unwrap();
        std::fs::write(d.join("opencodex.db"), "x").unwrap();
        let (rows, bad) = slices("opencode", std::slice::from_ref(&d), 1_790_800_000);
        assert!(bad.is_none());
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(
            (r.session.as_str(), r.model.as_deref(), r.api.as_str(), r.ts),
            ("s1", Some("claude-x"), "anthropic", 1_790_811_000)
        );
        // reasoning joins output; the cache legs stay apart.
        assert_eq!(
            (r.input, r.cache_create, r.cache_read, r.output),
            (10, 2, 5, 7)
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_path_with_uri_delimiters_is_read_where_it_is() {
        // Windows forbids `?` in a file name; `#` and `%` still exercise the URI escaping there.
        let d = dir(if cfg!(windows) {
            "odd #1 %41"
        } else {
            "odd #1 %41 ?x"
        });
        fixture(
            &d.join("opencode.db"),
            &[("s", 1_790_811_000_500, assistant("m", [3, 0, 0, 0, 0]))],
        );
        let (rows, bad) = slices("opencode", std::slice::from_ref(&d), 0);
        assert!(bad.is_none());
        assert_eq!(rows.len(), 1);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_file_that_is_not_a_database_is_named_and_the_channel_database_is_read() {
        let d = dir("bad");
        std::fs::write(d.join("kilo.db"), "this is not sqlite").unwrap();
        fixture(
            &d.join("kilo-beta.db"),
            &[("s", 1_790_811_000_500, assistant("m", [1, 0, 0, 0, 0]))],
        );
        let (rows, bad) = slices("kilo", std::slice::from_ref(&d), 0);
        assert_eq!(rows.len(), 1);
        assert!(bad.unwrap().ends_with("kilo.db"));
        assert!(slices("kilo", &[d.join("missing")], 0).0.is_empty());
        std::fs::remove_dir_all(&d).ok();
    }
}
