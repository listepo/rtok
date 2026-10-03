// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T283.1 (D34): which rtok agent an `rtok mcp` process serves.
//!
//! The host starts `rtok mcp` with its cwd and nothing that names the session, while the
//! agent row is written by that session's hooks. The rule order is **doc-derived**, from the
//! vendor docs and spawn code recorded in `research.md` §26; the T281 live probe only
//! confirms it per host:
//!
//! 1. the host's own session-id env var in this process ([`SESSION_ENV`]);
//! 2. the nearest common host ancestor pid: a hook records the pids above its own process on
//!    the agent row (T283.3), and this process's own parent chain is matched against them.
//!    Hooks and MCP children both descend from the host, usually the hook through a shell
//!    (`research.md` §26), so the first [`ANCESTORS`] pids of each are compared and the
//!    smallest index in this process's chain wins. One live agent at it is the link, two are
//!    ambiguous. A host's process that serves several sessions (one editor window) gives every
//!    session the same ancestor, so those sessions stay ambiguous and bind nothing;
//! 3. the host's live agents in this cwd that were seen since this process started: one is
//!    the link, two or more are ambiguous and bind nothing (a wrong link would route another
//!    agent's messages here). A row not touched since the process started cannot be this
//!    session's, so an ended session's row is never linked to its successor. This rule is
//!    re-run on every call, never cached (see [`Rule::is_stable`]).
//!
//! A host without hooks never writes a row, so its MCP process registers its own.

use anyhow::Result;

use crate::agents::Support;
use crate::store::{AgentRow, Store};

/// Hosts whose MCP children inherit a session id in the environment, matching the `session_id`
/// their hooks send (`research.md` §26: only Grok Build confirms one by first-party code).
const SESSION_ENV: &[(&str, &str)] = &[("grok", "GROK_SESSION_ID")];

/// How many ancestors a hook records and an MCP process compares. Three reaches the host past
/// a shell wrapper and stops short of a shared terminal, `tmux` server or `launchd`.
pub const ANCESTORS: usize = 3;

/// Which rule produced a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Env,
    Ancestor,
    Cwd,
    /// The MCP process registered its own row (a host without hooks).
    Own,
}

impl Rule {
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::Env => "env",
            Rule::Ancestor => "ancestor",
            Rule::Cwd => "cwd",
            Rule::Own => "own",
        }
    }

    /// Whether a link from this rule may be remembered for the life of the process. `Env` names
    /// the session itself and `Own` is this process's own row; `Cwd` and `Ancestor` are guesses
    /// from who else is in the directory or behind the host process, and a second session that
    /// starts there later makes them ambiguous, so they are re-run on every call and never
    /// stick to the first match.
    pub fn is_stable(self) -> bool {
        !matches!(self, Rule::Cwd | Rule::Ancestor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Linked {
        id: String,
        rule: Rule,
    },
    /// Several live agents of the host share this cwd; none is picked.
    Ambiguous(Vec<String>),
    None,
}

/// What [`resolve`] knows about the calling `rtok mcp` process. `host_id` is the id the hooks
/// register under, so a host the `hosts` table does not know yet (Grok, Zed, …) shares the
/// `other` row and its cwd candidates include every such host.
pub struct Caller<'a> {
    pub host: &'a str,
    pub host_id: i32,
    pub cwd: Option<&'a str>,
    /// The MCP process's own session key, used only for a host without hooks.
    pub own_session: &'a str,
    /// `[agents] idle`: how recently a row must have been seen to count as live.
    pub idle: &'a str,
    /// Unix seconds when this MCP process started. A cwd candidate must have been seen at or
    /// after it: the session this process serves has fired a hook since the host launched it,
    /// while a row nobody touched since belongs to an earlier or other session in that cwd.
    pub since: i64,
    /// This process's own ancestors, nearest first (`rtok_sys::ancestors`); empty when they
    /// cannot be read, which skips the ancestor rule.
    pub ancestors: &'a [i32],
}

/// True when no variant of `host` can have a hook installed, so only the MCP process can
/// register its agent.
pub fn hookless(host: &str) -> bool {
    crate::agents::host(host).is_some_and(|a| {
        a.variants()
            .iter()
            .all(|v| matches!(a.support(v.kind, "hooks"), Support::No(_)))
    })
}

pub fn resolve(store: &Store, who: &Caller, env: impl Fn(&str) -> Option<String>) -> Result<Link> {
    let session = SESSION_ENV
        .iter()
        .find(|(h, _)| *h == who.host)
        .and_then(|(_, var)| env(var))
        .filter(|v| !v.trim().is_empty());
    if let Some(session) = session {
        let id = store.register_agent(who.host_id, session.trim(), None, who.cwd, None)?;
        return Ok(Link::Linked {
            id,
            rule: Rule::Env,
        });
    }
    let live: Vec<AgentRow> = store
        .live_agents(who.idle)?
        .into_iter()
        .filter(|a| a.host_id == who.host_id && a.parent_key.is_empty())
        .filter(|a| a.last_seen >= who.since)
        .collect();
    // Index into this process's chain of the nearest ancestor an agent's hook also had.
    let depth = |a: &AgentRow| who.ancestors.iter().position(|p| a.ancestors.contains(p));
    if let Some(best) = live.iter().filter_map(depth).min() {
        let mut ids: Vec<String> = live
            .iter()
            .filter(|a| depth(a) == Some(best))
            .map(|a| a.id.clone())
            .collect();
        if ids.len() == 1 {
            return Ok(Link::Linked {
                id: ids.remove(0),
                rule: Rule::Ancestor,
            });
        }
        ids.sort();
        return Ok(Link::Ambiguous(ids));
    }
    if let Some(cwd) = who.cwd {
        // One directory has many spellings (`/var` vs `/private/var`, `RUNNER~1`, `\\?\`); a
        // path that no longer resolves still matches only itself.
        let canon = |p: &str| dunce::canonicalize(p).ok();
        let here = canon(cwd);
        let same = |p: &str| p == cwd || (here.is_some() && canon(p) == here);
        let mut ids: Vec<String> = live
            .into_iter()
            .filter(|a| a.cwd.as_deref().is_some_and(same))
            .map(|a| a.id)
            .collect();
        match ids.len() {
            0 => {}
            1 => {
                return Ok(Link::Linked {
                    id: ids.remove(0),
                    rule: Rule::Cwd,
                });
            }
            _ => {
                ids.sort();
                return Ok(Link::Ambiguous(ids));
            }
        }
    }
    if hookless(who.host) {
        let id = store.register_agent(who.host_id, who.own_session, None, who.cwd, Some("mcp"))?;
        return Ok(Link::Linked {
            id,
            rule: Rule::Own,
        });
    }
    Ok(Link::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller<'a>(host: &'a str, store: &Store, cwd: Option<&'a str>) -> Caller<'a> {
        Caller {
            host,
            host_id: store.host_id(host).unwrap().unwrap_or(6),
            cwd,
            own_session: "mcp-1",
            idle: "30m",
            since: 0,
            ancestors: &[],
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn env_rule_registers_the_hook_session_row_and_hooks_reuse_it() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("grok", &store, Some("/r"));
        let got = resolve(&store, &who, |k| {
            (k == "GROK_SESSION_ID").then(|| "s-9".into())
        });
        let Link::Linked {
            id,
            rule: Rule::Env,
        } = got.unwrap()
        else {
            panic!("expected an env link");
        };
        // The hook of the same session upserts the same row, so both surfaces name one agent.
        let hooked = store
            .register_agent(who.host_id, "s-9", None, Some("/r"), None)
            .unwrap();
        assert_eq!(id, hooked);
    }

    #[test]
    fn env_var_of_another_host_is_ignored() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("claude", &store, Some("/r"));
        let got = resolve(&store, &who, |_| Some("s-9".into())).unwrap();
        assert_eq!(got, Link::None);
    }

    #[test]
    fn one_live_agent_in_the_cwd_is_the_link() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("claude", &store, Some("/r"));
        let id = store
            .register_agent(who.host_id, "s1", None, Some("/r"), None)
            .unwrap();
        store
            .register_agent(who.host_id, "s2", None, Some("/elsewhere"), None)
            .unwrap();
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Linked {
                id,
                rule: Rule::Cwd
            }
        );
    }

    #[test]
    fn two_live_agents_in_the_cwd_are_ambiguous() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("claude", &store, Some("/r"));
        let mut want = vec![
            store
                .register_agent(who.host_id, "s1", None, Some("/r"), None)
                .unwrap(),
            store
                .register_agent(who.host_id, "s2", None, Some("/r"), None)
                .unwrap(),
        ];
        want.sort();
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Ambiguous(want)
        );
    }

    #[test]
    fn ended_sub_and_foreign_host_agents_are_not_candidates() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("claude", &store, Some("/r"));
        let ended = store
            .register_agent(who.host_id, "s1", None, Some("/r"), None)
            .unwrap();
        store.end_agent(&ended, 1).unwrap();
        store
            .register_agent(who.host_id, "s2", Some("sub"), Some("/r"), None)
            .unwrap();
        let cursor = store.host_id("cursor").unwrap().unwrap();
        store
            .register_agent(cursor, "s3", None, Some("/r"), None)
            .unwrap();
        assert_eq!(resolve(&store, &who, no_env).unwrap(), Link::None);
    }

    #[test]
    fn a_host_without_hooks_registers_its_own_row() {
        let store = Store::open_in_memory().unwrap();
        let who = caller("zed", &store, Some("/r"));
        assert!(hookless("zed"));
        let Link::Linked {
            id,
            rule: Rule::Own,
        } = resolve(&store, &who, no_env).unwrap()
        else {
            panic!("expected an own row");
        };
        let detail = store.agent_detail(&id).unwrap().unwrap();
        assert_eq!(
            (detail.host.as_str(), detail.host_session_id.as_str()),
            ("other", "mcp-1")
        );
    }

    #[test]
    fn a_host_with_hooks_registers_nothing_itself() {
        assert!(!hookless("claude"));
        assert!(!hookless("no-such-host"));
    }

    /// A row last touched `ago` seconds before now, so a test places it before or after a
    /// process start without sleeping.
    fn seen(store: &Store, who: &Caller, session: &str, ago: i64) -> String {
        let id = store
            .register_agent(who.host_id, session, None, who.cwd, None)
            .unwrap();
        store
            .set_agent_last_seen(&id, crate::log::now() as i64 - ago)
            .unwrap();
        id
    }

    fn started_100s_ago<'a>(store: &Store) -> Caller<'a> {
        Caller {
            since: crate::log::now() as i64 - 100,
            ..caller("claude", store, Some("/r"))
        }
    }

    #[test]
    fn an_agent_not_seen_since_the_process_started_does_not_link() {
        let store = Store::open_in_memory().unwrap();
        let who = started_100s_ago(&store);
        // The ended session A, still inside `[agents] idle`: not this process's session.
        seen(&store, &who, "a", 500);
        assert_eq!(resolve(&store, &who, no_env).unwrap(), Link::None);
    }

    #[test]
    fn only_the_agent_seen_since_the_start_links() {
        let store = Store::open_in_memory().unwrap();
        let who = started_100s_ago(&store);
        seen(&store, &who, "a", 500);
        let b = seen(&store, &who, "b", 10);
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Linked {
                id: b,
                rule: Rule::Cwd
            }
        );
    }

    #[test]
    fn two_agents_seen_since_the_start_are_ambiguous() {
        let store = Store::open_in_memory().unwrap();
        let who = started_100s_ago(&store);
        seen(&store, &who, "stale", 500);
        let mut want = vec![seen(&store, &who, "a", 20), seen(&store, &who, "b", 10)];
        want.sort();
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Ambiguous(want)
        );
    }

    #[test]
    fn only_a_cwd_link_is_unstable() {
        assert!(Rule::Env.is_stable() && Rule::Own.is_stable());
        assert!(!Rule::Cwd.is_stable());
    }

    /// A live agent whose hook recorded `chain` above itself, in `cwd`.
    fn hooked(store: &Store, host_id: i32, session: &str, cwd: &str, chain: &[i32]) -> String {
        let id = store
            .register_agent(host_id, session, None, Some(cwd), None)
            .unwrap();
        store.set_agent_ancestors(&id, chain).unwrap();
        id
    }

    #[test]
    fn the_nearest_common_ancestor_picks_the_agent_over_a_farther_one() {
        let store = Store::open_in_memory().unwrap();
        let mut who = caller("claude", &store, Some("/r"));
        // A's hook ran through a shell (100) under host 200; B sits under host 500 and the
        // same terminal (300) as A.
        let a = hooked(&store, who.host_id, "a", "/r", &[100, 200, 300]);
        let b = hooked(&store, who.host_id, "b", "/r", &[501, 500, 300]);
        who.ancestors = &[200, 300];
        let want = |id: &String| Link::Linked {
            id: id.clone(),
            rule: Rule::Ancestor,
        };
        assert_eq!(resolve(&store, &who, no_env).unwrap(), want(&a));
        who.ancestors = &[500, 300];
        assert_eq!(resolve(&store, &who, no_env).unwrap(), want(&b));
    }

    #[test]
    fn two_agents_behind_one_ancestor_are_ambiguous_even_with_one_in_the_cwd() {
        let store = Store::open_in_memory().unwrap();
        let mut who = caller("claude", &store, Some("/r"));
        let mut want = vec![
            hooked(&store, who.host_id, "a", "/r", &[200]),
            hooked(&store, who.host_id, "b", "/elsewhere", &[200]),
        ];
        want.sort();
        who.ancestors = &[200];
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Ambiguous(want)
        );
    }

    #[test]
    fn the_ancestor_rule_beats_the_cwd_rule_and_is_not_remembered() {
        let store = Store::open_in_memory().unwrap();
        let mut who = caller("claude", &store, Some("/r"));
        let by_pid = hooked(&store, who.host_id, "a", "/worktree", &[200]);
        hooked(&store, who.host_id, "b", "/r", &[999]);
        who.ancestors = &[200];
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Linked {
                id: by_pid,
                rule: Rule::Ancestor
            }
        );
        assert!(!Rule::Ancestor.is_stable());
    }

    #[test]
    fn a_missing_chain_or_a_foreign_ended_or_unseen_agent_skips_the_rule() {
        let store = Store::open_in_memory().unwrap();
        let mut who = started_100s_ago(&store);
        let cursor = store.host_id("cursor").unwrap().unwrap();
        hooked(&store, cursor, "other-host", "/r", &[200]);
        let ended = hooked(&store, who.host_id, "ended", "/r", &[200]);
        store.end_agent(&ended, crate::log::now() as i64).unwrap();
        let stale = hooked(&store, who.host_id, "stale", "/r", &[200]);
        store
            .set_agent_last_seen(&stale, crate::log::now() as i64 - 500)
            .unwrap();
        who.ancestors = &[200];
        assert_eq!(resolve(&store, &who, no_env).unwrap(), Link::None);
        // No pid on the hook side (an old client): the cwd rule still answers.
        let old = seen(&store, &who, "old-client", 10);
        who.ancestors = &[200, 300];
        assert_eq!(
            resolve(&store, &who, no_env).unwrap(),
            Link::Linked {
                id: old,
                rule: Rule::Cwd
            }
        );
    }
}
