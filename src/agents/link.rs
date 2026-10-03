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
//! 2. the nearest common host ancestor pid (T283.3, not wired yet);
//! 3. the host's live agents in this cwd that were seen since this process started: one is
//!    the link, two or more are ambiguous and bind nothing (a wrong link would route another
//!    agent's messages here). A row not touched since the process started cannot be this
//!    session's, so an ended session's row is never linked to its successor. This rule is
//!    re-run on every call, never cached (see [`Rule::is_stable`]).
//!
//! A host without hooks never writes a row, so its MCP process registers its own.

use anyhow::Result;

use crate::agents::Support;
use crate::store::Store;

/// Hosts whose MCP children inherit a session id in the environment, matching the `session_id`
/// their hooks send (`research.md` §26: only Grok Build confirms one by first-party code).
const SESSION_ENV: &[(&str, &str)] = &[("grok", "GROK_SESSION_ID")];

/// Which rule produced a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Env,
    Cwd,
    /// The MCP process registered its own row (a host without hooks).
    Own,
}

impl Rule {
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::Env => "env",
            Rule::Cwd => "cwd",
            Rule::Own => "own",
        }
    }

    /// Whether a link from this rule may be remembered for the life of the process. `Env` names
    /// the session itself and `Own` is this process's own row; `Cwd` is a guess from who else
    /// is in the directory, and a second session that starts there later makes it ambiguous,
    /// so it is re-run on every call and never sticks to the first match.
    pub fn is_stable(self) -> bool {
        !matches!(self, Rule::Cwd)
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
    if let Some(cwd) = who.cwd {
        // One directory has many spellings (`/var` vs `/private/var`, `RUNNER~1`, `\\?\`); a
        // path that no longer resolves still matches only itself.
        let canon = |p: &str| dunce::canonicalize(p).ok();
        let here = canon(cwd);
        let same = |p: &str| p == cwd || (here.is_some() && canon(p) == here);
        let mut ids: Vec<String> = store
            .live_agents(who.idle)?
            .into_iter()
            .filter(|a| a.host_id == who.host_id && a.parent_key.is_empty())
            .filter(|a| a.cwd.as_deref().is_some_and(same))
            .filter(|a| a.last_seen >= who.since)
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
}
