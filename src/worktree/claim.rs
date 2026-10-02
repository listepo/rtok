// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T285: bind a worktree to the calling rtok agent (T282) — in the git lock reason (v2, the
//! source of truth) and in the store's `worktree_claims` (the fast join).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::{Owner, git};
use crate::store::{AgentDetail, Store};

/// The calling agent: `--agent <prefix>` (must resolve), else `RTOK_AGENT_ID` (T283; an id
/// the store does not know is ignored), else none.
pub fn caller(store: Option<&Store>, flag: Option<&str>) -> Result<Option<AgentDetail>> {
    let env = std::env::var("RTOK_AGENT_ID")
        .ok()
        .filter(|s| !s.is_empty());
    let (raw, explicit) = match (flag, env) {
        (Some(flag), _) => (flag.to_string(), true),
        (None, Some(env)) => (env, false),
        (None, None) => return Ok(None),
    };
    let found = store
        .context("no store")
        .and_then(|s| s.agent_detail(&s.resolve_agent(&raw)?));
    match found {
        Ok(Some(detail)) => Ok(Some(detail)),
        Ok(None) if !explicit => Ok(None),
        Err(_) if !explicit => {
            eprintln!("warning: RTOK_AGENT_ID {raw} is not in the store; no agent bound");
            Ok(None)
        }
        Ok(None) => bail!("--agent {raw}: unknown"),
        Err(e) => bail!("--agent {raw}: {e:#}"),
    }
}

/// `--owner`, else `<host> / <model>` of the agent (`<host>` alone while no call has named
/// a model), else an error: without an agent the owner is the only name a lock has.
pub fn owner(
    flag: Option<String>,
    agent: Option<&AgentDetail>,
    store: Option<&Store>,
) -> Result<String> {
    if let Some(owner) = flag {
        return Ok(owner);
    }
    let Some(agent) = agent else {
        bail!("--owner is required when no agent is known (--agent or RTOK_AGENT_ID)");
    };
    let model = store.and_then(|s| s.session_model(&agent.host_session_id).ok().flatten());
    Ok(match model {
        Some(model) => format!("{} / {model}", agent.host),
        None => agent.host.clone(),
    })
}

/// `rtok worktree add` and the `WorktreeCreate` hook (T159) share this: create the worktree
/// for `id` under the configured root, locked for `owner` (else the agent's `<host> / <model>`)
/// and bound to `agent`.
pub fn add(
    cfg: &crate::config::Config,
    store: Option<&Store>,
    cwd: &Path,
    id: (&str, Option<&str>),
    (owner, agent): (Option<String>, Option<&AgentDetail>),
) -> Result<super::add::Plan> {
    let root = Some(cfg.worktree.root.as_path()).filter(|r| !r.as_os_str().is_empty());
    let owner = self::owner(owner, agent, store)?;
    let agent_id = agent.map(|a| a.id.as_str());
    let plan = super::add::run(cwd, root, id, (&owner, agent_id))?;
    if let Some(agent) = agent_id {
        remember(store, &plan.path, agent, &plan.task);
    }
    Ok(plan)
}

/// Record the claim in the store; a store error only warns — the lock already holds it.
pub fn remember(store: Option<&Store>, path: &Path, agent: &str, task: &str) {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let saved = store.map(|s| s.claim_worktree(&path.to_string_lossy(), agent, task));
    if let Some(Err(e)) = saved {
        eprintln!("warning: claim not stored: {e:#}");
    }
}

/// `rtok worktree claim`: rewrite `path`'s lock as `owner`'s, bound to `agent`, when it has
/// no lock or the lock is already theirs. Returns the worktree path and its task.
pub fn run(path: &Path, owner: &str, agent: &str) -> Result<(PathBuf, String)> {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let target = real(path);
    let worktrees = git::list(&target)?;
    let main = &worktrees.first().context("git lists no worktree")?.path;
    let Some(record) = worktrees.iter().skip(1).find(|r| real(&r.path) == target) else {
        bail!("{} is not a linked worktree", path.display());
    };
    if !record.claimable_by(owner, agent) {
        let held = record
            .owner()
            .map_or("an unknown owner".into(), |o| o.reason());
        bail!("{} is locked by {held}; not taken", path.display());
    }
    let old = record.owner();
    let task = match (&old, record.branch.as_deref()) {
        (Some(o), _) => o.task.clone(),
        (None, Some(branch)) => branch.split('-').next().unwrap_or(branch).to_string(),
        (None, None) => bail!("{}: no lock and no branch to name its task", path.display()),
    };
    let reason = Owner {
        owner: owner.into(),
        task: task.clone(),
        date: super::add::today()?,
        agent: Some(agent.into()),
    }
    .reason();
    anyhow::ensure!(
        Owner::parse(&reason).is_some_and(|o| o.owner == owner) && owner.is_ascii(),
        "owner `{owner}` must be non-empty ASCII without ` | `"
    );
    if let Some(old) = &record.locked {
        git::unlock(main, &record.path)?;
        if let Err(e) = git::lock(main, &record.path, &reason) {
            git::lock(main, &record.path, old)?;
            return Err(e);
        }
    } else {
        git::lock(main, &record.path, &reason)?;
    }
    Ok((record.path.clone(), task))
}
