// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! T282 (D34): the rtok agent id. The host's own session id collides across hosts and is
//! missing on several (`research.md` §26), so rtok issues its own random UUIDv4 per host session,
//! shown as its first 8 hex chars and resolved from any unique prefix of 4+ hex chars.
//! `parent_key` is `""` for the main window or the host's own sub-agent `agent_id`
//! (`HookInput::agent_id`, `research.md` §17.2); `parent_id` is the resolved rtok id of a
//! sub-agent's parent row.

use anyhow::{Result, bail};
use diesel::prelude::*;
use serde::Serialize;

use super::Store;
use super::schema::{agents, hosts};
use super::{coalesce, substr, unixepoch};

/// One `agents` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRow {
    pub id: String,
    pub host_id: i32,
    pub host_session_id: String,
    pub parent_key: String,
    pub parent_id: Option<String>,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub last_seen: i64,
    pub ended_at: Option<i64>,
    pub activity: Option<String>,
}

type AgentTuple = (
    String,
    i32,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    i64,
    Option<i64>,
    Option<String>,
);

fn row_from(t: AgentTuple) -> AgentRow {
    AgentRow {
        id: t.0,
        host_id: t.1,
        host_session_id: t.2,
        parent_key: t.3,
        parent_id: t.4,
        cwd: t.5,
        started_at: t.6,
        last_seen: t.7,
        ended_at: t.8,
        activity: t.9,
    }
}

/// Column list `live_agents`/`agent_row` select — kept in one place so the tuple order in
/// [`AgentTuple`] and [`row_from`] only has to match here.
fn agent_cols() -> (
    agents::id,
    agents::host_id,
    agents::host_session_id,
    agents::parent_key,
    agents::parent_id,
    agents::cwd,
    agents::started_at,
    agents::last_seen,
    agents::ended_at,
    agents::activity,
) {
    (
        agents::id,
        agents::host_id,
        agents::host_session_id,
        agents::parent_key,
        agents::parent_id,
        agents::cwd,
        agents::started_at,
        agents::last_seen,
        agents::ended_at,
        agents::activity,
    )
}

/// One agent joined to its host's slug — [`Store::agent_detail`], the shape `rtok agents
/// whoami` (T283) prints and serializes for `--json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentDetail {
    pub id: String,
    pub short: String,
    pub host: String,
    pub host_session_id: String,
    pub parent_id: Option<String>,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub last_seen: i64,
    pub ended_at: Option<i64>,
    pub activity: Option<String>,
    /// What the agent says it is busy with (`rtok agents status`, T284).
    pub status_text: Option<String>,
}

type AgentDetailTuple = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    i64,
    Option<i64>,
    Option<String>,
    Option<String>,
);

fn agent_detail_from(t: AgentDetailTuple) -> AgentDetail {
    let short = t.0.chars().take(8).collect();
    AgentDetail {
        id: t.0,
        short,
        host: t.1,
        host_session_id: t.2,
        parent_id: t.3,
        cwd: t.4,
        started_at: t.5,
        last_seen: t.6,
        ended_at: t.7,
        activity: t.8,
        status_text: t.9,
    }
}

/// `agents ⋈ hosts` rows in the [`AgentDetailTuple`] order — the one select every
/// [`AgentDetail`] reader shares.
macro_rules! agent_details {
    () => {
        agents::table.inner_join(hosts::table).select((
            agents::id,
            hosts::slug,
            agents::host_session_id,
            agents::parent_id,
            agents::cwd,
            agents::started_at,
            agents::last_seen,
            agents::ended_at,
            agents::activity,
            agents::status_text,
        ))
    };
}

/// `[agents] idle` (e.g. `"30m"`) in whole seconds, the one parser of that key.
pub fn idle_secs(idle: &str) -> Result<i64> {
    humantime::parse_duration(idle)
        .map_err(|e| anyhow::anyhow!("bad [agents] idle {idle:?}: {e}"))?
        .as_secs()
        .try_into()
        .map_err(|_| anyhow::anyhow!("[agents] idle {idle:?} is out of range"))
}

impl Store {
    /// Ensure the row for this host session (or, when `parent_key` names one, its sub-agent)
    /// exists and is fresh; returns its rtok id (existing on repeat, else a freshly minted
    /// UUIDv4). One indexed upsert on the hot path (`parent_key: None`); a sub-agent first
    /// resolves its parent's id with one extra indexed read, so `parent_id` is set from the
    /// row's very first insert. `activity` follows [`Store::touch_agent`]'s rule: `None`
    /// leaves whatever is already stored untouched (`register_agent` never blanks a hook
    /// event's activity when a later event, e.g. `SessionEnd`, only needs the id back).
    /// A repeat also clears `ended_at`, so a resumed session is live again (T324).
    pub fn register_agent(
        &self,
        host_id: i32,
        host_session: &str,
        parent_key: Option<&str>,
        cwd: Option<&str>,
        activity: Option<&str>,
    ) -> Result<String> {
        let mut conn = self.lock()?;
        let key = parent_key.unwrap_or("");
        let parent_id: Option<String> = if key.is_empty() {
            None
        } else {
            agents::table
                .filter(agents::host_id.eq(host_id))
                .filter(agents::host_session_id.eq(host_session))
                .filter(agents::parent_key.eq(""))
                .select(agents::id)
                .first(&mut *conn)
                .optional()?
        };
        let id = uuid::Uuid::new_v4().to_string();
        let row_id: String = diesel::insert_into(agents::table)
            .values((
                agents::id.eq(&id),
                agents::host_id.eq(host_id),
                agents::host_session_id.eq(host_session),
                agents::parent_key.eq(key),
                agents::parent_id.eq(parent_id.as_deref()),
                agents::cwd.eq(cwd),
                agents::activity.eq(activity),
            ))
            .on_conflict((agents::host_id, agents::host_session_id, agents::parent_key))
            .do_update()
            .set((
                agents::last_seen.eq(unixepoch().assume_not_null()),
                // A registering event is proof of life: a resumed session revives its row.
                agents::ended_at.eq(None::<i64>),
                agents::cwd.eq(coalesce(diesel::upsert::excluded(agents::cwd), agents::cwd)),
                agents::activity.eq(coalesce(
                    diesel::upsert::excluded(agents::activity),
                    agents::activity,
                )),
            ))
            .returning(agents::id)
            .get_result(&mut *conn)?;
        Ok(row_id)
    }

    /// Set `last_seen` to now and `activity` to exactly what is passed (unlike
    /// [`Store::register_agent`], `None` here clears it) — for a caller that already holds
    /// the id and skips the host/session lookup (T283's MCP surface).
    pub fn touch_agent(&self, id: &str, activity: Option<&str>) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(agents::table.filter(agents::id.eq(id)))
            .set((
                agents::last_seen.eq(unixepoch().assume_not_null()),
                agents::activity.eq(activity),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// Stamp `ended_at`. `ts` comes from the caller (matches `end_session`'s convention) so a
    /// test can drive the clock and the hook path can share one `SystemTime::now()` read.
    pub fn end_agent(&self, id: &str, ts: i64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(agents::table.filter(agents::id.eq(id)))
            .set(agents::ended_at.eq(ts))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// The one id whose text starts with `prefix`. `Err("unknown")` for zero matches,
    /// `Err("ambiguous: <ids>")` for more than one — never guesses. D34: at least 4 chars.
    pub fn resolve_agent(&self, prefix: &str) -> Result<String> {
        if prefix.chars().count() < 4 {
            bail!("an agent id prefix needs at least 4 characters");
        }
        let mut conn = self.lock()?;
        let len = i32::try_from(prefix.len()).unwrap_or(i32::MAX);
        let matches: Vec<String> = agents::table
            .filter(substr(agents::id, 1, len).eq(prefix))
            .select(agents::id)
            .load(&mut *conn)?;
        match matches.len() {
            0 => bail!("unknown"),
            1 => Ok(matches.into_iter().next().expect("len == 1")),
            _ => bail!("ambiguous: {}", matches.join(", ")),
        }
    }

    /// One agent by its exact id, host slug instead of `hosts.id` — the shape `rtok agents
    /// whoami` (T283) prints. The caller resolves a prefix through [`Store::resolve_agent`]
    /// first, so this is always looked up by the canonical id it returned.
    pub fn agent_detail(&self, id: &str) -> Result<Option<AgentDetail>> {
        let mut conn = self.lock()?;
        agent_details!()
            .filter(agents::id.eq(id))
            .first::<AgentDetailTuple>(&mut *conn)
            .optional()
            .map(|o| o.map(agent_detail_from))
            .map_err(Into::into)
    }

    /// Every agent (main rows and sub-agents) of these host sessions, oldest first — the
    /// rows `rtok agents sessions` (T284) nests under the session they belong to.
    pub fn agents_of_sessions(&self, sessions: &[String]) -> Result<Vec<AgentDetail>> {
        let mut conn = self.lock()?;
        Ok(agent_details!()
            .filter(agents::host_session_id.eq_any(sessions))
            .order((agents::started_at, agents::id))
            .load::<AgentDetailTuple>(&mut *conn)?
            .into_iter()
            .map(agent_detail_from)
            .collect())
    }

    /// The sub-agents whose `parent_id` is `id`, oldest first (`rtok agents show`, T284).
    pub fn agent_children(&self, id: &str) -> Result<Vec<AgentDetail>> {
        let mut conn = self.lock()?;
        Ok(agent_details!()
            .filter(agents::parent_id.eq(id))
            .order((agents::started_at, agents::id))
            .load::<AgentDetailTuple>(&mut *conn)?
            .into_iter()
            .map(agent_detail_from)
            .collect())
    }

    /// Set (or, with `None`, clear) the agent's own status text (T284); `false` when no
    /// row has this id.
    pub fn set_agent_status(&self, id: &str, text: Option<&str>) -> Result<bool> {
        let mut conn = self.lock()?;
        let n = diesel::update(agents::table.filter(agents::id.eq(id)))
            .set(agents::status_text.eq(text))
            .execute(&mut *conn)?;
        Ok(n > 0)
    }

    /// Every agent with no `ended_at` and a `last_seen` within `idle` (`[agents] idle`'s raw
    /// string, e.g. `"30m"`, parsed by `humantime::parse_duration`) of now. The cutoff is
    /// computed in SQL — `unixepoch() - secs`, not `SystemTime::now()` — so it shares
    /// SQLite's own clock with the `last_seen` values it is compared against.
    pub fn live_agents(&self, idle: &str) -> Result<Vec<AgentRow>> {
        let secs = idle_secs(idle)?;
        let mut conn = self.lock()?;
        Ok(agents::table
            .filter(agents::ended_at.is_null())
            .filter(agents::last_seen.ge(unixepoch().assume_not_null() - secs))
            .select(agent_cols())
            .load::<AgentTuple>(&mut *conn)?
            .into_iter()
            .map(row_from)
            .collect())
    }

    /// Test/debug: one row by its exact id. `pub(crate)` so `hooks::mod`'s dispatch-level
    /// fixture tests can assert on what a full hook run wrote (T282).
    #[cfg(test)]
    pub(crate) fn agent_row(&self, id: &str) -> Result<Option<AgentRow>> {
        let mut conn = self.lock()?;
        agents::table
            .filter(agents::id.eq(id))
            .select(agent_cols())
            .first::<AgentTuple>(&mut *conn)
            .optional()
            .map(|o| o.map(row_from))
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_is_idempotent_and_returns_the_same_id() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let a = store
            .register_agent(claude, "sess-1", None, Some("/repo"), Some("Bash: ls"))
            .unwrap();
        let b = store
            .register_agent(claude, "sess-1", None, Some("/repo"), None)
            .unwrap();
        assert_eq!(a, b);
        let row = store.agent_row(&a).unwrap().unwrap();
        assert_eq!(row.host_session_id, "sess-1");
        assert_eq!(row.parent_key, "");
        assert_eq!(row.parent_id, None);
        // `register`'s second call passed no activity: the first call's sticks (D34: register
        // never blanks it — that is `touch_agent`'s job).
        assert_eq!(row.activity.as_deref(), Some("Bash: ls"));
    }

    #[test]
    fn a_sub_agent_gets_its_own_row_with_the_parent_s_id() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let parent = store
            .register_agent(claude, "sess-2", None, None, None)
            .unwrap();
        let child = store
            .register_agent(claude, "sess-2", Some("child-1"), None, None)
            .unwrap();
        assert_ne!(parent, child);
        let child_row = store.agent_row(&child).unwrap().unwrap();
        assert_eq!(child_row.parent_key, "child-1");
        assert_eq!(child_row.parent_id.as_deref(), Some(parent.as_str()));
        // A second sub-agent under the same session is a third, distinct row.
        let child2 = store
            .register_agent(claude, "sess-2", Some("child-2"), None, None)
            .unwrap();
        assert_ne!(child2, child);
        // Repeating the first sub-agent's key upserts the same child row, not a fourth.
        let child_again = store
            .register_agent(claude, "sess-2", Some("child-1"), None, None)
            .unwrap();
        assert_eq!(child_again, child);
    }

    #[test]
    fn touch_sets_activity_and_end_stamps_ended_at() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let id = store
            .register_agent(claude, "sess-3", None, None, None)
            .unwrap();
        store.touch_agent(&id, Some("Edit: src/a.rs")).unwrap();
        let row = store.agent_row(&id).unwrap().unwrap();
        assert_eq!(row.activity.as_deref(), Some("Edit: src/a.rs"));
        assert_eq!(row.ended_at, None);
        store.end_agent(&id, 1_800_000_000).unwrap();
        let row = store.agent_row(&id).unwrap().unwrap();
        assert_eq!(row.ended_at, Some(1_800_000_000));
    }

    #[test]
    fn agent_detail_joins_the_host_slug_and_short_id() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let id = store
            .register_agent(claude, "sess-detail", None, Some("/repo"), Some("Bash: ls"))
            .unwrap();
        let detail = store.agent_detail(&id).unwrap().unwrap();
        assert_eq!(detail.id, id);
        assert_eq!(detail.short, id[..8]);
        assert_eq!(detail.host, "claude");
        assert_eq!(detail.host_session_id, "sess-detail");
        assert_eq!(detail.cwd.as_deref(), Some("/repo"));
        assert_eq!(detail.activity.as_deref(), Some("Bash: ls"));
        assert_eq!(detail.ended_at, None);
        assert!(
            store
                .agent_detail("ffffffff-0000-7000-8000-000000000000")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn resolve_prefix_exact_ambiguous_and_unknown() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let a = store
            .register_agent(claude, "sess-a", None, None, None)
            .unwrap();
        // A hand-picked second id sharing `a`'s first 8 chars, so that prefix is ambiguous.
        let shared = format!("{}-0000-4000-8000-000000000000", &a[..8]);
        diesel::insert_into(agents::table)
            .values((
                agents::id.eq(&shared),
                agents::host_id.eq(claude),
                agents::host_session_id.eq("sess-b"),
                agents::parent_key.eq(""),
            ))
            .execute(&mut *store.lock().unwrap())
            .unwrap();

        assert_eq!(store.resolve_agent(&a).unwrap(), a); // full id: always unique
        let ambiguous = store.resolve_agent(&a[..8]).unwrap_err().to_string();
        assert!(ambiguous.starts_with("ambiguous: "), "{ambiguous}");
        assert!(ambiguous.contains(&a) && ambiguous.contains(&shared));
        let unknown = store.resolve_agent("ffffffff").unwrap_err().to_string();
        assert_eq!(unknown, "unknown");
        let short = store.resolve_agent(&a[..3]).unwrap_err().to_string();
        assert!(short.contains("at least 4"), "{short}");
    }

    #[test]
    fn live_excludes_ended_and_stale_rows() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let live = store
            .register_agent(claude, "sess-live", None, None, None)
            .unwrap();
        let ended = store
            .register_agent(claude, "sess-ended", None, None, None)
            .unwrap();
        store.end_agent(&ended, 0).unwrap();
        let stale = store
            .register_agent(claude, "sess-stale", None, None, None)
            .unwrap();
        diesel::update(agents::table.filter(agents::id.eq(&stale)))
            .set(agents::last_seen.eq(unixepoch().assume_not_null() - 10_000))
            .execute(&mut *store.lock().unwrap())
            .unwrap();

        let ids: Vec<String> = store
            .live_agents("60s")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert!(ids.contains(&live));
        assert!(!ids.contains(&ended), "ended rows are never live");
        assert!(
            !ids.contains(&stale),
            "a last_seen older than idle is never live"
        );
    }

    #[test]
    fn live_agents_parses_the_configured_idle_window() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let id = store
            .register_agent(claude, "sess-idle", None, None, None)
            .unwrap();
        let ids: Vec<String> = store
            .live_agents("30m")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert!(
            ids.contains(&id),
            "the default [agents] idle (\"30m\") must parse and match"
        );
        assert!(
            store.live_agents("not-a-duration").is_err(),
            "a bad [agents] idle must error, not panic"
        );
    }

    /// T324: a resumed session registers the same row again; it is alive again, not stuck
    /// behind the `ended_at` stamp of its earlier run.
    #[test]
    fn registering_an_ended_agent_again_makes_it_live() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let id = store
            .register_agent(claude, "sess-resume", None, None, None)
            .unwrap();
        store.end_agent(&id, 1_800_000_000).unwrap();
        assert!(store.live_agents("30m").unwrap().iter().all(|r| r.id != id));
        let again = store
            .register_agent(claude, "sess-resume", None, None, None)
            .unwrap();
        assert_eq!(again, id);
        assert!(store.live_agents("30m").unwrap().iter().any(|r| r.id == id));
    }
}
