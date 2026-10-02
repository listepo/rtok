// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T329.3: `project_links` — directed "A sees into B" links between projects (T329.1) and
//! the graph scope they define: the selected project plus everything reachable through links.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use diesel::prelude::*;
use serde::Serialize;

use super::Store;
use super::projects::Project;
use super::schema::project_links as links;

/// Chains longer than this stop; a guard for a hand-built registry, since the visited set
/// already makes cycles safe.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Manual,
    Auto,
}

impl LinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
        }
    }

    // The column has a CHECK for the two values.
    fn parse(s: &str) -> Self {
        if s == "auto" {
            Self::Auto
        } else {
            Self::Manual
        }
    }
}

/// An active link; a remembered (`unlinked`) auto link is not one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub from: i32,
    pub to: i32,
    pub kind: LinkKind,
    pub reason: Option<String>,
}

type Row = (i32, i32, String, Option<String>);

impl Store {
    /// Link `from` to `to`. True when the registry changed. Linking twice is a no-op; a manual
    /// link over an auto one takes it over (so a reference that goes away cannot drop it); an
    /// auto link the user removed stays removed, only a manual link brings it back.
    pub fn link_projects(
        &self,
        from: i32,
        to: i32,
        kind: LinkKind,
        reason: Option<&str>,
    ) -> Result<bool> {
        if from == to {
            bail!("a project cannot link to itself");
        }
        let mut conn = self.lock()?;
        Ok(
            conn.immediate_transaction::<_, diesel::result::Error, _>(|conn| {
                let cur: Option<(String, i32)> = links::table
                    .find((from, to))
                    .select((links::kind, links::unlinked))
                    .first(conn)
                    .optional()?;
                let cur = cur.map(|(k, unlinked)| (LinkKind::parse(&k), unlinked == 1));
                match (cur, kind) {
                    (None, _) => {
                        diesel::insert_into(links::table)
                            .values((
                                links::from_id.eq(from),
                                links::to_id.eq(to),
                                links::kind.eq(kind.as_str()),
                                links::reason.eq(reason),
                            ))
                            .execute(conn)?;
                    }
                    (Some((_, true)), LinkKind::Auto) => return Ok(false),
                    (Some((_, true)), LinkKind::Manual)
                    | (Some((LinkKind::Auto, false)), LinkKind::Manual) => {
                        diesel::update(links::table.find((from, to)))
                            .set((links::kind.eq("manual"), links::unlinked.eq(0)))
                            .execute(conn)?;
                        if let Some(r) = reason {
                            diesel::update(links::table.find((from, to)))
                                .set(links::reason.eq(r))
                                .execute(conn)?;
                        }
                    }
                    _ => return Ok(false),
                }
                Ok(true)
            })?,
        )
    }

    /// Remove the link; true when there was an active one. A manual link is deleted, an auto
    /// link is kept as a remembered removal so the next index does not re-create it.
    pub fn unlink_projects(&self, from: i32, to: i32) -> Result<bool> {
        let mut conn = self.lock()?;
        Ok(
            conn.immediate_transaction::<_, diesel::result::Error, _>(|conn| {
                let cur: Option<(String, i32)> = links::table
                    .find((from, to))
                    .select((links::kind, links::unlinked))
                    .first(conn)
                    .optional()?;
                match cur {
                    Some((kind, 0)) if LinkKind::parse(&kind) == LinkKind::Manual => {
                        diesel::delete(links::table.find((from, to))).execute(conn)?;
                    }
                    Some((_, 0)) => {
                        diesel::update(links::table.find((from, to)))
                            .set(links::unlinked.eq(1))
                            .execute(conn)?;
                    }
                    _ => return Ok(false),
                }
                Ok(true)
            })?,
        )
    }

    /// Delete the active auto links of `from` whose target is not in `keep`: the references its
    /// manifests no longer name. A manual link and a remembered removal are never touched.
    /// Returns how many were dropped.
    pub fn drop_auto_links(&self, from: i32, keep: &[i32]) -> Result<usize> {
        let mut conn = self.lock()?;
        Ok(diesel::delete(
            links::table
                .filter(links::from_id.eq(from))
                .filter(links::kind.eq("auto"))
                .filter(links::unlinked.eq(0))
                .filter(links::to_id.ne_all(keep)),
        )
        .execute(&mut *conn)?)
    }

    /// Every active link, ordered by `(from, to)`.
    pub fn project_links(&self) -> Result<Vec<Link>> {
        let mut conn = self.lock()?;
        let rows: Vec<Row> = links::table
            .filter(links::unlinked.eq(0))
            .select((links::from_id, links::to_id, links::kind, links::reason))
            .order((links::from_id, links::to_id))
            .load(&mut *conn)?;
        Ok(rows
            .into_iter()
            .map(|(from, to, kind, reason)| Link {
                from,
                to,
                kind: LinkKind::parse(&kind),
                reason,
            })
            .collect())
    }

    /// The graph scope of `root`: itself first, then what it links to, level by level, each
    /// project once. A missing project is left out and not walked through; its links stay in
    /// the registry for when the directory returns.
    pub fn project_scope(&self, root: i32) -> Result<Vec<Project>> {
        Ok(scope_of(&self.projects()?, &self.project_links()?, root))
    }
}

fn scope_of(projects: &[Project], all: &[Link], root: i32) -> Vec<Project> {
    let by_id: HashMap<i32, &Project> = projects.iter().map(|p| (p.id, p)).collect();
    let mut seen = HashSet::from([root]);
    let mut out = Vec::new();
    let mut level = vec![root];
    for _ in 0..=MAX_DEPTH {
        let mut next = Vec::new();
        for id in level {
            let Some(p) = by_id.get(&id).filter(|p| !p.missing()) else {
                continue;
            };
            out.push((*p).clone());
            next.extend(
                all.iter()
                    .filter(|l| l.from == id && seen.insert(l.to))
                    .map(|l| l.to),
            );
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::projects::{Origin, fixture::Dir};
    use super::*;

    /// A, B, C and D as registered projects: `ids("ABCD")`.
    struct World {
        store: Store,
        dir: Dir,
        id: HashMap<char, i32>,
    }

    fn world(tag: &str) -> World {
        let dir = Dir::new(tag);
        let store = Store::open_in_memory().unwrap();
        let id = "ABCD"
            .chars()
            .map(|c| {
                let p = store
                    .register_project(&dir.sub(&c.to_string()), Origin::Manual)
                    .unwrap();
                (c, p.id)
            })
            .collect();
        World { store, dir, id }
    }

    impl World {
        fn link(&self, a: char, b: char, kind: LinkKind) -> bool {
            self.store
                .link_projects(self.id[&a], self.id[&b], kind, None)
                .unwrap()
        }

        fn scope(&self, a: char) -> String {
            let names: HashMap<i32, char> = self.id.iter().map(|(c, i)| (*i, *c)).collect();
            self.store
                .project_scope(self.id[&a])
                .unwrap()
                .iter()
                .map(|p| names[&p.id])
                .collect()
        }
    }

    #[test]
    fn scope_follows_links_transitively_once_each_and_survives_cycles() {
        let w = world("scope");
        assert_eq!(w.scope('A'), "A");
        w.link('A', 'B', LinkKind::Auto);
        w.link('B', 'C', LinkKind::Auto);
        assert_eq!(w.scope('A'), "ABC");
        assert_eq!(w.scope('B'), "BC", "links are directional");
        w.link('A', 'D', LinkKind::Manual);
        w.link('D', 'A', LinkKind::Manual);
        w.link('C', 'A', LinkKind::Auto);
        assert_eq!(w.scope('A'), "ABDC");
        assert_eq!(w.scope('D'), "DABC");
    }

    #[test]
    fn a_missing_project_drops_out_of_the_scope_and_its_links_come_back_with_it() {
        let w = world("missing");
        w.link('A', 'B', LinkKind::Auto);
        w.link('B', 'C', LinkKind::Auto);
        let b = w.dir.0.join("B");
        std::fs::remove_dir_all(&b).unwrap();
        assert_eq!(w.scope('A'), "A", "C is behind the missing B");
        assert_eq!(w.store.project_links().unwrap().len(), 2);
        std::fs::create_dir_all(&b).unwrap();
        assert_eq!(w.scope('A'), "ABC");
    }

    #[test]
    fn link_rules_self_duplicate_manual_over_auto_and_remembered_unlink() {
        let w = world("rules");
        assert!(
            w.store
                .link_projects(w.id[&'A'], w.id[&'A'], LinkKind::Manual, None)
                .is_err()
        );
        assert!(w.link('A', 'B', LinkKind::Auto));
        assert!(!w.link('A', 'B', LinkKind::Auto), "a duplicate is a no-op");
        assert!(
            w.link('A', 'B', LinkKind::Manual),
            "a manual link takes an auto one over"
        );
        assert_eq!(w.store.project_links().unwrap()[0].kind, LinkKind::Manual);
        assert!(!w.link('A', 'B', LinkKind::Manual));

        assert!(w.store.unlink_projects(w.id[&'A'], w.id[&'B']).unwrap());
        assert!(!w.store.unlink_projects(w.id[&'A'], w.id[&'B']).unwrap());
        assert!(
            w.store.project_links().unwrap().is_empty(),
            "a manual unlink deletes"
        );

        assert!(w.link('A', 'C', LinkKind::Auto));
        assert!(w.store.unlink_projects(w.id[&'A'], w.id[&'C']).unwrap());
        assert_eq!(w.scope('A'), "A");
        assert!(
            !w.link('A', 'C', LinkKind::Auto),
            "an auto link the user removed stays removed"
        );
        assert!(
            w.link('A', 'C', LinkKind::Manual),
            "a manual link brings it back"
        );
        assert_eq!(w.scope('A'), "AC");
    }

    #[test]
    fn drop_auto_links_removes_only_active_auto_links_outside_keep() {
        let w = world("drop");
        w.link('A', 'B', LinkKind::Auto);
        w.link('A', 'C', LinkKind::Auto);
        w.link('A', 'D', LinkKind::Manual);
        w.link('B', 'C', LinkKind::Auto);
        assert_eq!(
            w.store.drop_auto_links(w.id[&'A'], &[w.id[&'B']]).unwrap(),
            1
        );
        let kept: Vec<(i32, i32)> = w
            .store
            .project_links()
            .unwrap()
            .iter()
            .map(|l| (l.from, l.to))
            .collect();
        let (a, b, c, d) = (w.id[&'A'], w.id[&'B'], w.id[&'C'], w.id[&'D']);
        assert_eq!(kept, [(a, b), (a, d), (b, c)]);
        // A removal the user made stays remembered, so the link is not re-created later.
        w.store.unlink_projects(a, b).unwrap();
        assert_eq!(w.store.drop_auto_links(a, &[]).unwrap(), 0);
        assert!(!w.link('A', 'B', LinkKind::Auto));
    }

    #[test]
    fn removing_a_project_leaves_no_links_in_either_direction() {
        let w = world("remove");
        w.link('A', 'B', LinkKind::Manual);
        w.link('B', 'C', LinkKind::Auto);
        w.link('C', 'B', LinkKind::Auto);
        w.store.unlink_projects(w.id[&'C'], w.id[&'B']).unwrap();
        assert!(w.store.remove_project(w.id[&'B']).unwrap());
        let conn = &mut *w.store.lock().unwrap();
        let rows: i64 = links::table.count().get_result(conn).unwrap();
        assert_eq!(rows, 0, "active and remembered links both went with B");
    }
}
