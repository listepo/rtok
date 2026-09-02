//! `rtok stats --cache` (plan T5.5): per-session prompt-cache health from the proxy's
//! `usage` rows.
//!
//! A turn is one `usage` row (one API request). A *bust* is a turn whose
//! `cache_creation_input_tokens` exceeds [`BUST_CREATE_TOKENS`] while
//! `cache_read_input_tokens` fell below the previous turn's — the frozen prefix was
//! re-written. The cause comes from the request bodies the proxy recorded (`call_io`): a
//! changed `tools` array → `tools`, a changed `system` prompt → `system`, otherwise
//! `unknown` (no body on record, or the change was inside `messages`).

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::store::{Store, UsageRow};

/// `cache_creation_input_tokens` above this, with a `cache_read` drop, is a bust.
pub const BUST_CREATE_TOKENS: i64 = 20_000;

/// One API request as the cache saw it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Turn {
    pub call_id: Option<i64>,
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    /// `tools` | `system` | `unknown` when this turn busted the cache.
    pub bust: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionHealth {
    pub session: String,
    pub turns: Vec<Turn>,
    pub busts: usize,
    pub cache_read: i64,
    pub cache_create: i64,
}

/// Fingerprints of the request parts whose change busts the cache.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Shape {
    tools: Option<String>,
    system: Option<String>,
}

fn shape(body: &[u8]) -> Shape {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        return Shape::default();
    };
    let fp = |key: &str| {
        v.get(key).map(|part| {
            let mut h = Sha256::new();
            h.update(part.to_string());
            format!("{:x}", h.finalize())
        })
    };
    Shape {
        tools: fp("tools"),
        system: fp("system"),
    }
}

fn cause(prev: Option<&Shape>, cur: Option<&Shape>) -> &'static str {
    match (prev, cur) {
        (Some(p), Some(c)) if p.tools != c.tools => "tools",
        (Some(p), Some(c)) if p.system != c.system => "system",
        _ => "unknown",
    }
}

/// Turns in request order with busts attributed. `body(call_id)` supplies the recorded
/// request; it is only consulted for the two requests around a bust.
pub fn analyse(rows: &[UsageRow], body: impl Fn(i64) -> Option<Vec<u8>>) -> Vec<Turn> {
    let load = |id: Option<i64>| id.and_then(&body).map(|b| shape(&b));
    let mut turns = Vec::with_capacity(rows.len());
    let mut prev: Option<&UsageRow> = None;
    for r in rows {
        let bust = prev
            .filter(|p| r.cache_create > BUST_CREATE_TOKENS && r.cache_read < p.cache_read)
            .map(|p| cause(load(p.call_id).as_ref(), load(r.call_id).as_ref()).to_string());
        turns.push(Turn {
            call_id: r.call_id,
            input: r.input,
            cache_create: r.cache_create,
            cache_read: r.cache_read,
            bust,
        });
        prev = Some(r);
    }
    turns
}

/// Every session with usage rows, oldest first.
pub fn report(store: &Store) -> Result<Vec<SessionHealth>> {
    let mut out = Vec::new();
    for session in store.usage_sessions()? {
        let mut rows = store.usage_rows(&session)?;
        rows.reverse(); // newest-first → request order
        let turns = analyse(&rows, |id| {
            i32::try_from(id)
                .ok()
                .and_then(|id| store.call_io_request(id).ok().flatten())
        });
        out.push(SessionHealth {
            session,
            busts: turns.iter().filter(|t| t.bust.is_some()).count(),
            cache_read: turns.iter().map(|t| t.cache_read).sum(),
            cache_create: turns.iter().map(|t| t.cache_create).sum(),
            turns,
        });
    }
    Ok(out)
}

/// `rtok stats --cache`: table, or JSON when `stats.format = "json"`.
pub fn run(cfg: &Config) -> Result<String> {
    let store = Store::open(&cfg.core.db_path)?;
    let report = report(&store)?;
    if cfg.stats.format == "json" {
        Ok(serde_json::to_string_pretty(&report)?)
    } else {
        Ok(table(&report))
    }
}

pub fn table(report: &[SessionHealth]) -> String {
    let mut s = format!(
        "{:<40} {:>6} {:>12} {:>12} {:>6}\n",
        "session", "turns", "cache_read", "cache_create", "busts"
    );
    for h in report {
        s.push_str(&format!(
            "{:<40} {:>6} {:>12} {:>12} {:>6}\n",
            h.session,
            h.turns.len(),
            h.cache_read,
            h.cache_create,
            h.busts
        ));
        for (i, t) in h.turns.iter().enumerate() {
            if let Some(c) = &t.bust {
                s.push_str(&format!(
                    "  bust turn {} cause={c} cache_create={} cache_read={}\n",
                    i + 1,
                    t.cache_create,
                    t.cache_read
                ));
            }
        }
    }
    if report.is_empty() {
        s.push_str("no usage rows (run `rtok proxy` first)\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = r#"{"system":"s","tools":[{"name":"A"}],"messages":[]}"#;
    const A_MORE_TOOLS: &str =
        r#"{"system":"s","tools":[{"name":"A"},{"name":"B"}],"messages":[]}"#;
    const A_NEW_SYSTEM: &str = r#"{"system":"s2","tools":[{"name":"A"}],"messages":[]}"#;

    fn row(call_id: i64, cache_create: i64, cache_read: i64) -> UsageRow {
        UsageRow {
            session: "sess".into(),
            model: Some("m".into()),
            input: 100,
            cache_create,
            cache_read,
            output: 5,
            call_id: Some(call_id),
        }
    }

    fn call(store: &Store, session: &str, body: &str) -> i32 {
        store
            .upsert_session(session, None, None, None, Some("proxy"))
            .unwrap();
        let id = store
            .insert_call(
                session,
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        store
            .insert_call_io(id, Some(body.as_bytes()), None, 1 << 20, None)
            .unwrap();
        id
    }

    /// T5.5 Check: an injected tools-array change → exactly one bust, cause `tools`.
    #[test]
    fn tools_change_is_one_bust_with_cause_tools() {
        let store = Store::open_in_memory().unwrap();
        let s = "sess";
        for (body, create, read) in [
            (A, 30_000, 0),
            (A, 500, 30_000),
            (A_MORE_TOOLS, 31_000, 200),
            (A_MORE_TOOLS, 400, 31_000),
        ] {
            let id = call(&store, s, body);
            store
                .insert_usage(s, Some("m"), 100, create, read, 5, id)
                .unwrap();
        }
        let r = report(&store).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].busts, r[0].turns.len()), (1, 4));
        let busts: Vec<(usize, &str)> = r[0]
            .turns
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.bust.as_deref().map(|c| (i + 1, c)))
            .collect();
        assert_eq!(busts, [(3, "tools")]);
        assert!(table(&r).contains("bust turn 3 cause=tools cache_create=31000"));
    }

    #[test]
    fn system_change_unknown_and_no_drop() {
        let rows = [row(1, 0, 0), row(2, 500, 40_000), row(3, 25_000, 1_000)];
        let bodies = [A, A, A_NEW_SYSTEM];
        let turns = analyse(&rows, |id| {
            bodies.get(id as usize - 1).map(|b| b.as_bytes().to_vec())
        });
        assert_eq!(turns[2].bust.as_deref(), Some("system"));
        assert!(turns[0].bust.is_none() && turns[1].bust.is_none());
        // No recorded body → unknown.
        let turns = analyse(&rows, |_| None);
        assert_eq!(turns[2].bust.as_deref(), Some("unknown"));
        // Big cache_create without a cache_read drop is a growing prefix, not a bust.
        let rows = [row(1, 0, 0), row(2, 500, 40_000), row(3, 25_000, 45_000)];
        assert!(analyse(&rows, |_| None).iter().all(|t| t.bust.is_none()));
        assert!(table(&[]).contains("no usage rows"));
    }
}
