// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! T284: who is running and what each agent is doing — the one model behind `rtok agents
//! sessions`, `rtok agents show` and `rtok agents status` (and, later, their MCP tools).
//! Sessions come from the Sessions page's own read ([`Model::sessions`]); each gets its
//! `agents` row with the sub-agents nested under it.

use anyhow::{Result, bail};
use serde::Serialize;
use std::path::Path;

use super::Model;
use crate::config::Config;
use crate::store::{AgentDetail, SessionTotals, Store, idle_secs};

/// `live`: seen within `[agents] idle`; `idle`: not ended, but quiet longer; `ended`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Live,
    Idle,
    Ended,
}

impl AgentState {
    pub fn of(ended_at: Option<i64>, last_seen: i64, now: i64, idle: i64) -> Self {
        match ended_at {
            Some(_) => Self::Ended,
            None if now - last_seen > idle => Self::Idle,
            None => Self::Live,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Idle => "idle",
            Self::Ended => "ended",
        }
    }
}

/// One agent with its sub-agents nested (`--json` of `agents show`, and of each
/// `agents sessions` row's `agent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentView {
    #[serde(flatten)]
    pub detail: AgentDetail,
    /// The session's newest model; `None` for a sub-agent or when no call recorded one.
    pub model: Option<String>,
    /// `cwd` relative to its checkout, named by the checkout's directory (T285 swaps in
    /// the claimed worktree).
    pub worktree: Option<String>,
    pub state: AgentState,
    pub sub_agents: Vec<AgentView>,
}

/// One `rtok agents sessions` row: the session's totals plus its agent, if registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionView {
    #[serde(flatten)]
    pub session: SessionTotals,
    pub state: AgentState,
    pub agent: Option<AgentView>,
}

/// `<checkout dir>[/<path inside it>]` for `cwd`; `cwd` itself outside any checkout.
pub fn worktree_label(cwd: &str) -> String {
    let path = Path::new(cwd);
    let Some(root) = path.ancestors().find(|a| a.join(".git").exists()) else {
        return cwd.to_string();
    };
    let name = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => format!("{name}/{}", rel.display()),
        _ => name,
    }
}

/// Nest `rows` under their parents: a row whose parent is absent from `rows` is a root.
/// Ended agents (and everything under them) are dropped unless `all`.
fn nest(rows: &[AgentDetail], now: i64, idle: i64, all: bool) -> Vec<AgentView> {
    rows.iter()
        .filter(|r| {
            r.parent_id
                .as_deref()
                .is_none_or(|p| !rows.iter().any(|o| o.id == p))
        })
        .filter_map(|r| tree(r, rows, now, idle, all))
        .collect()
}

fn tree(
    r: &AgentDetail,
    rows: &[AgentDetail],
    now: i64,
    idle: i64,
    all: bool,
) -> Option<AgentView> {
    let state = AgentState::of(r.ended_at, r.last_seen, now, idle);
    (all || state != AgentState::Ended).then(|| AgentView {
        worktree: r.cwd.as_deref().map(worktree_label),
        state,
        model: None,
        sub_agents: rows
            .iter()
            .filter(|c| c.parent_id.as_deref() == Some(r.id.as_str()))
            .filter_map(|c| tree(c, rows, now, idle, all))
            .collect(),
        detail: r.clone(),
    })
}

/// Pair each session with its main agent (sub-agents nested); `all` keeps ended rows.
pub fn session_views(
    sessions: Vec<SessionTotals>,
    agents: &[AgentDetail],
    now: i64,
    idle: i64,
    all: bool,
) -> Vec<SessionView> {
    let trees = nest(agents, now, idle, true);
    sessions
        .into_iter()
        .filter_map(|s| {
            let agent = trees
                .iter()
                .find(|a| {
                    a.detail.host_session_id == s.id
                        && s.host.as_ref().is_none_or(|h| *h == a.detail.host)
                })
                .map(|a| AgentView {
                    model: s.model.clone(),
                    sub_agents: a
                        .sub_agents
                        .iter()
                        .filter(|c| all || c.state != AgentState::Ended)
                        .cloned()
                        .collect(),
                    ..a.clone()
                });
            let state = match (&agent, s.ended_at) {
                (_, Some(_)) => AgentState::Ended,
                (Some(a), None) => a.state,
                (None, None) => AgentState::of(None, s.last_activity, now, idle),
            };
            (all || state != AgentState::Ended).then_some(SessionView {
                session: s,
                state,
                agent,
            })
        })
        .collect()
}

/// `rtok agents sessions`: the Sessions page's rows, each with its agent tree.
pub fn agent_sessions(cfg: &Config, all: bool, now: i64) -> Result<Vec<SessionView>> {
    let idle = idle_secs(&cfg.agents.idle)?;
    let store = Store::open(&cfg.core.db_path)?;
    let sessions = Model::new(cfg, Some(&store)).sessions(0);
    let ids: Vec<String> = sessions.iter().map(|s| s.id.clone()).collect();
    let agents = store.agents_of_sessions(&ids)?;
    Ok(session_views(sessions, &agents, now, idle, all))
}

fn find(views: &[AgentView], id: &str) -> Option<AgentView> {
    views.iter().find_map(|v| {
        (v.detail.id == id)
            .then(|| v.clone())
            .or_else(|| find(&v.sub_agents, id))
    })
}

/// `rtok agents show <prefix>`: the agent as `agent_sessions(all)` shows it; an agent
/// past the Sessions page's window is built from its own row and sub-agents.
pub fn agent_show(cfg: &Config, prefix: &str, now: i64) -> Result<AgentView> {
    let store = Store::open(&cfg.core.db_path)?;
    let id = match store.resolve_agent(prefix) {
        Ok(id) => id,
        Err(e) if e.to_string() == "unknown" => bail!("no agent id starts with {prefix:?}"),
        Err(e) => bail!("agent id {prefix:?} is {e}"),
    };
    let listed = agent_sessions(cfg, true, now)?;
    let trees: Vec<AgentView> = listed.into_iter().filter_map(|s| s.agent).collect();
    if let Some(v) = find(&trees, &id) {
        return Ok(v);
    }
    let Some(me) = store.agent_detail(&id)? else {
        bail!("no agent id starts with {prefix:?}");
    };
    let mut rows = store.agent_children(&id)?;
    rows.insert(0, me);
    let idle = idle_secs(&cfg.agents.idle)?;
    find(&nest(&rows, now, idle, true), &id).ok_or_else(|| anyhow::anyhow!("agent {id} vanished"))
}

/// Longest status text an agent may set, in chars (after cleaning).
pub const STATUS_MAX: usize = 120;

/// Status text as stored: control characters stripped, trimmed; `None` when nothing is
/// left (clears it). Longer than [`STATUS_MAX`] is refused, never cut (like T287's cap).
pub fn clean_status(text: &str) -> Result<Option<String>> {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim();
    let n = clean.chars().count();
    if n > STATUS_MAX {
        bail!("status text is {n} chars; at most {STATUS_MAX} allowed");
    }
    Ok((!clean.is_empty()).then(|| clean.to_string()))
}

/// `rtok agents status <text>`: the caller's (`RTOK_AGENT_ID`) own status text.
pub fn set_status(cfg: &Config, agent_id: Option<&str>, text: &str) -> Result<Option<String>> {
    let store = Store::open(&cfg.core.db_path)?;
    let id = agent_id
        .filter(|s| !s.is_empty())
        .and_then(|raw| store.resolve_agent(raw).ok());
    let Some(id) = id else {
        bail!("not inside an agent session");
    };
    let text = clean_status(text)?;
    store.set_agent_status(&id, text.as_deref())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        id: &str,
        parent: Option<&str>,
        session: &str,
        seen: i64,
        ended: Option<i64>,
    ) -> AgentDetail {
        AgentDetail {
            id: id.into(),
            short: id.chars().take(8).collect(),
            host: "claude".into(),
            host_session_id: session.into(),
            parent_id: parent.map(Into::into),
            cwd: None,
            started_at: 0,
            last_seen: seen,
            ended_at: ended,
            activity: None,
            status_text: None,
        }
    }

    fn session(id: &str, ended: Option<i64>) -> SessionTotals {
        SessionTotals {
            id: id.into(),
            host: Some("claude".into()),
            project: None,
            provider: None,
            api: None,
            model: Some("claude-x".into()),
            input: 0,
            cache_create: 0,
            cache_read: 0,
            output: 0,
            started_at: 0,
            last_activity: 0,
            ended_at: ended,
        }
    }

    #[test]
    fn state_follows_the_idle_rule() {
        assert_eq!(AgentState::of(None, 990, 1000, 60), AgentState::Live);
        assert_eq!(AgentState::of(None, 900, 1000, 60), AgentState::Idle);
        assert_eq!(AgentState::of(Some(1), 1000, 1000, 60), AgentState::Ended);
    }

    #[test]
    fn sub_agents_nest_under_their_session_s_agent() {
        let agents = [
            row("aaaa0001", None, "s1", 1000, None),
            row("bbbb0002", Some("aaaa0001"), "s1", 900, None),
            row("cccc0003", Some("aaaa0001"), "s1", 1000, Some(5)),
            row("dddd0004", None, "s2", 1000, None),
        ];
        let sessions = vec![
            session("s1", None),
            session("s2", Some(7)),
            session("s3", None),
        ];
        let live = session_views(sessions.clone(), &agents, 1000, 60, false);
        assert_eq!(
            live.len(),
            2,
            "s2 ended: hidden; s3 has no agent row but shows"
        );
        let a = live[0].agent.as_ref().unwrap();
        assert_eq!(
            (a.detail.id.as_str(), a.state),
            ("aaaa0001", AgentState::Live)
        );
        assert_eq!(a.model.as_deref(), Some("claude-x"));
        let subs: Vec<_> = a
            .sub_agents
            .iter()
            .map(|s| (s.detail.id.as_str(), s.state))
            .collect();
        assert_eq!(
            subs,
            [("bbbb0002", AgentState::Idle)],
            "the ended sub-agent is hidden"
        );
        assert!(live[1].agent.is_none());

        let all = session_views(sessions, &agents, 1000, 60, true);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].agent.as_ref().unwrap().sub_agents.len(), 2);
        assert_eq!(all[1].state, AgentState::Ended);
    }

    #[test]
    fn show_finds_by_prefix_and_rejects_unknown_and_short() {
        let dir = std::env::temp_dir().join(format!("rtok-t284-show-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config::load_from(&dir).unwrap();
        let store = Store::open(&cfg.core.db_path).unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        store
            .upsert_session("s1", Some(claude), None, None, None)
            .unwrap();
        let main = store
            .register_agent(claude, "s1", None, Some("/nowhere/x"), Some("Bash: ls"))
            .unwrap();
        let sub = store
            .register_agent(claude, "s1", Some("k"), None, None)
            .unwrap();
        let now = crate::log::now() as i64;
        let v = agent_show(&cfg, &main, now).unwrap();
        assert_eq!(v.detail.activity.as_deref(), Some("Bash: ls"));
        assert_eq!(v.sub_agents[0].detail.id, sub);
        let s = agent_show(&cfg, &sub, now).unwrap();
        assert_eq!(s.detail.parent_id.as_deref(), Some(main.as_str()));
        let unknown = agent_show(&cfg, "ffffffff", now).unwrap_err().to_string();
        assert!(unknown.contains("no agent id"), "{unknown}");
        // Ids are random (UUIDv4), so ambiguity is pinned in the store's own test; here
        // the store's refusal must reach the caller.
        let short = agent_show(&cfg, &main[..3], now).unwrap_err().to_string();
        assert!(short.contains("at least 4"), "{short}");

        assert_eq!(
            set_status(&cfg, Some(&main), " busy\x07 on T284 ")
                .unwrap()
                .as_deref(),
            Some("busy on T284")
        );
        // An agent whose session is not on the Sessions page is built from its own row.
        let lone = store
            .register_agent(claude, "no-session-row", None, None, None)
            .unwrap();
        assert_eq!(agent_show(&cfg, &lone, now).unwrap().model, None);
        assert!(set_status(&cfg, None, "x").is_err());
        assert_eq!(clean_status(&"y".repeat(120)).unwrap().unwrap().len(), 120);
        let long = clean_status(&"y".repeat(121)).unwrap_err().to_string();
        assert!(long.contains("at most 120"), "{long}");
        assert_eq!(clean_status(" \t ").unwrap(), None, "empty clears");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
