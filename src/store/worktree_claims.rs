// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! T285: `worktree_claims` — the agent (T282) each worktree is bound to. The git lock reason
//! is the source of truth; these rows are the fast join `rtok worktree list` falls back on.

use anyhow::Result;
use diesel::prelude::*;

use super::Store;
use super::schema::{calls, models, worktree_claims};
use super::unixepoch;

impl Store {
    /// Bind `path` to `agent_id`, replacing any earlier claim on the same path.
    pub fn claim_worktree(&self, path: &str, agent_id: &str, task: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let row = (
            worktree_claims::path.eq(path),
            worktree_claims::agent_id.eq(agent_id),
            worktree_claims::task.eq(task),
        );
        diesel::insert_into(worktree_claims::table)
            .values(row)
            .on_conflict(worktree_claims::path)
            .do_update()
            .set((
                worktree_claims::agent_id.eq(agent_id),
                worktree_claims::task.eq(task),
                worktree_claims::claimed_at.eq(unixepoch().assume_not_null()),
                worktree_claims::released_at.eq(None::<i64>),
            ))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// `rtok worktree remove` (T286): the open claim on `path`, if any, is released.
    pub fn release_worktree_claim(&self, path: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let open = worktree_claims::table
            .filter(worktree_claims::path.eq(path))
            .filter(worktree_claims::released_at.is_null());
        diesel::update(open)
            .set(worktree_claims::released_at.eq(unixepoch()))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// `(path, agent_id)` of every claim not yet released.
    pub fn open_worktree_claims(&self) -> Result<Vec<(String, String)>> {
        let mut conn = self.lock()?;
        Ok(worktree_claims::table
            .filter(worktree_claims::released_at.is_null())
            .select((worktree_claims::path, worktree_claims::agent_id))
            .load(&mut *conn)?)
    }

    /// The model of the newest call a host session made — `rtok worktree add`'s default
    /// `--owner` is `<host> / <model>`.
    pub fn session_model(&self, session: &str) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        Ok(calls::table
            .inner_join(models::table)
            .filter(calls::session_id.eq(session))
            .order(calls::ts.desc())
            .select(models::slug)
            .first(&mut *conn)
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_upserts_per_path() {
        let store = Store::open_in_memory().unwrap();
        let claude = store.host_id("claude").unwrap().unwrap();
        let a = store
            .register_agent(claude, "s-a", None, None, None)
            .unwrap();
        let b = store
            .register_agent(claude, "s-b", None, None, None)
            .unwrap();
        store.claim_worktree("/w/x", &a, "t1").unwrap();
        store.claim_worktree("/w/y", &a, "t2").unwrap();
        store.claim_worktree("/w/x", &b, "t1").unwrap();
        let mut claims = store.open_worktree_claims().unwrap();
        claims.sort();
        assert_eq!(claims, [("/w/x".into(), b), ("/w/y".into(), a.clone())]);
        store.release_worktree_claim("/w/x").unwrap();
        assert_eq!(store.open_worktree_claims().unwrap(), [("/w/y".into(), a)]);
        assert_eq!(store.session_model("s-a").unwrap(), None);
    }
}
