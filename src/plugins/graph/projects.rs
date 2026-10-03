// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok graph projects` — the project registry (T329.1) from the command line, with each
//! project's index status (T329.2) and the links between projects (T329.3). The `project`
//! argument on the other graph commands comes later.

use std::path::Path;

use anyhow::{Result, bail};
use serde::Serialize;

use rtok_plugin_sdk::Ctx;

use super::{index, status};
use crate::plugin::Runtime;
use crate::render::{Col, table};
use crate::store::{LinkKind, Origin, Project, Store};

/// `graph status` numbers for one project; absent for a missing root, which has nothing
/// readable to count.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProjectIndex {
    rows: i64,
    files: i64,
    pending: usize,
    watch: String,
    indexed_at: Option<i64>,
}

/// One outgoing link of a project.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProjectLink {
    to: i32,
    name: String,
    kind: LinkKind,
    reason: Option<String>,
}

/// One registry row as `graph projects` prints it and the `/ws` snapshot carries it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProjectRow {
    id: i32,
    name: String,
    root: String,
    origin: Origin,
    selected: bool,
    missing: bool,
    state: &'static str,
    created_at: i64,
    last_used_at: i64,
    index: Option<ProjectIndex>,
    links: Vec<ProjectLink>,
}

fn link_rows(store: &Store, from: i32) -> Result<Vec<ProjectLink>> {
    let mut out = Vec::new();
    for l in store
        .project_links()?
        .into_iter()
        .filter(|l| l.from == from)
    {
        let name = store
            .project(l.to)?
            .map_or_else(|| l.to.to_string(), |p| p.display_name().to_string());
        out.push(ProjectLink {
            to: l.to,
            name,
            kind: l.kind,
            reason: l.reason,
        });
    }
    Ok(out)
}

fn row(rt: &Runtime, p: Project) -> Result<ProjectRow> {
    let cx = &Ctx::new(rt);
    let missing = p.missing();
    let index = if missing {
        None
    } else {
        let s = status::collect(cx, Path::new(&p.root))?;
        Some(ProjectIndex {
            rows: s.rows,
            files: s.files,
            pending: s.pending.len(),
            watch: s.watch,
            indexed_at: s.indexed_at,
        })
    };
    let state = match &index {
        None => "missing",
        Some(i) if i.indexed_at.is_none() && i.rows == 0 => "not indexed",
        Some(i) if i.pending > 0 => "stale",
        Some(_) => "ok",
    };
    Ok(ProjectRow {
        id: p.id,
        name: p.display_name().to_string(),
        root: p.root,
        origin: p.origin,
        selected: p.selected,
        missing,
        state,
        created_at: p.created_at,
        last_used_at: p.last_used_at,
        index,
        links: link_rows(&rt.store, p.id)?,
    })
}

fn render(rows: &[ProjectRow], json: bool) -> Result<String> {
    if json {
        return Ok(serde_json::to_string_pretty(rows)? + "\n");
    }
    if rows.is_empty() {
        return Ok("no projects; `rtok graph projects add <path>`\n".into());
    }
    let cols = [
        Col::left(1),
        Col::right(2),
        Col::left(4),
        Col::left(6),
        Col::left(5),
        Col::right(4),
        Col::right(5),
        Col::right(7),
        Col::right(5),
        Col::left(4),
    ];
    let head = [
        "", "id", "name", "origin", "state", "rows", "files", "pending", "links", "root",
    ];
    let mut cells = vec![head.map(String::from).to_vec()];
    for r in rows {
        let n = |f: fn(&ProjectIndex) -> String| r.index.as_ref().map_or("-".into(), f);
        cells.push(vec![
            if r.selected { "*" } else { "" }.into(),
            r.id.to_string(),
            r.name.clone(),
            r.origin.as_str().into(),
            r.state.into(),
            n(|i| i.rows.to_string()),
            n(|i| i.files.to_string()),
            n(|i| i.pending.to_string()),
            r.links.len().to_string(),
            r.root.clone(),
        ]);
    }
    // `table` pads every column, so the free-text root would end each line in spaces.
    let text = table(&cols, &cells);
    Ok(text
        .lines()
        .map(|l| format!("{}\n", l.trim_end()))
        .collect())
}

pub fn rows(rt: &Runtime) -> Result<Vec<ProjectRow>> {
    rt.store
        .projects()?
        .into_iter()
        .map(|p| row(rt, p))
        .collect()
}

fn list(rt: &Runtime, json: bool) -> Result<String> {
    render(&rows(rt)?, json)
}

/// `<id|path>`: a number naming a known project is its id, anything else is a directory.
fn resolve(store: &Store, target: &str) -> Result<Project> {
    let by_id = target
        .parse::<i32>()
        .ok()
        .and_then(|id| store.project(id).transpose());
    match by_id {
        Some(p) => Ok(p?),
        None => match store.project_by_root(Path::new(target))? {
            Some(p) => Ok(p),
            None => bail!("no project `{target}`; see `rtok graph projects`"),
        },
    }
}

/// `<to>` and the project it is linked from: `--from`, else the selected project (or the
/// working directory's, which `resolve_project` selects when nothing is).
fn pair(rt: &Runtime, to: &str, from: Option<&str>) -> Result<(Project, Project)> {
    let from = match from {
        Some(f) => resolve(&rt.store, f)?,
        None => match rt.store.resolve_project(&std::env::current_dir()?)?.project {
            Some(p) => p,
            None => {
                bail!("no selected project; pass --from <project> or `rtok graph projects select`")
            }
        },
    };
    Ok((from, resolve(&rt.store, to)?))
}

fn link(rt: &Runtime, from: &Project, to: &Project, reason: Option<&str>) -> Result<bool> {
    rt.store
        .link_projects(from.id, to.id, LinkKind::Manual, reason)
}

fn changes(done: &[(&Project, &Project, bool)], yes: &str, no: &str, json: bool) -> Result<String> {
    if json {
        let rows: Vec<_> = done
            .iter()
            .map(|(a, b, c)| serde_json::json!({ "from": a.id, "to": b.id, "changed": c }))
            .collect();
        return Ok(serde_json::to_string_pretty(&rows)? + "\n");
    }
    Ok(done
        .iter()
        .map(|(a, b, c)| {
            let (a, b) = (a.display_name(), b.display_name());
            if *c {
                format!("{yes} {a} -> {b}\n")
            } else {
                format!("{a} -> {b} {no}\n")
            }
        })
        .collect())
}

/// One project as `list` shows it, so every action answers in the shape the user reads.
fn one(rt: &Runtime, p: Project, json: bool) -> Result<String> {
    let r = row(rt, p)?;
    if json {
        return Ok(serde_json::to_string_pretty(&r)? + "\n");
    }
    render(&[r], false)
}

pub fn run(rt: &Runtime, action: Action, json: bool) -> Result<String> {
    match action {
        Action::List => list(rt, json),
        Action::Add(path) => {
            if !path.is_dir() {
                bail!("{} is not a directory", path.display());
            }
            one(rt, rt.store.register_project(&path, Origin::Manual)?, json)
        }
        Action::Select(target) => {
            let p = resolve(&rt.store, &target)?;
            if p.missing() {
                bail!(
                    "{} no longer exists; remove it or restore the directory",
                    p.root
                );
            }
            rt.store.select_project(p.id)?;
            let p = rt.store.project(p.id)?.unwrap_or(p);
            one(rt, p, json)
        }
        Action::Link {
            to,
            from,
            both,
            reason,
        } => {
            let (a, b) = pair(rt, &to, from.as_deref())?;
            for p in [&a, &b] {
                if p.missing() {
                    bail!("{} no longer exists; it cannot be linked", p.root);
                }
            }
            let mut done = vec![(&a, &b, link(rt, &a, &b, reason.as_deref())?)];
            if both {
                done.push((&b, &a, link(rt, &b, &a, reason.as_deref())?));
            }
            // Linking a project that was never indexed starts indexing it, so the scope answers
            // from it right away.
            if row(rt, b.clone())?.state == "not indexed" {
                index::run(&Ctx::new(rt), Path::new(&b.root), false)?;
            }
            changes(&done, "linked", "already linked", json)
        }
        Action::Unlink { to, from, both } => {
            let (a, b) = pair(rt, &to, from.as_deref())?;
            let mut done = vec![(&a, &b, rt.store.unlink_projects(a.id, b.id)?)];
            if both {
                done.push((&b, &a, rt.store.unlink_projects(b.id, a.id)?));
            }
            changes(&done, "unlinked", "was not linked", json)
        }
        Action::Remove(target) => {
            let p = resolve(&rt.store, &target)?;
            rt.store.remove_project(p.id)?;
            if json {
                return Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "removed": p.id,
                    "root": p.root,
                }))? + "\n");
            }
            Ok(format!(
                "removed {} (index rows dropped, files untouched)\n",
                p.root
            ))
        }
    }
}

pub enum Action {
    Link {
        to: String,
        from: Option<String>,
        both: bool,
        reason: Option<String>,
    },
    Unlink {
        to: String,
        from: Option<String>,
        both: bool,
    },
    List,
    Add(std::path::PathBuf),
    Select(String),
    Remove(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_registered_says_how_to_add() {
        let rows: Vec<ProjectRow> = Vec::new();
        assert!(render(&rows, false).unwrap().contains("graph projects add"));
        assert_eq!(render(&rows, true).unwrap(), "[]\n");
    }

    #[test]
    fn resolve_prefers_a_known_id_then_a_path() {
        let store = Store::open_in_memory().unwrap();
        let dir = std::env::temp_dir().join(format!("rtok-gp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = store.register_project(&dir, Origin::Manual).unwrap();
        assert_eq!(resolve(&store, &p.id.to_string()).unwrap().id, p.id);
        assert_eq!(resolve(&store, dir.to_str().unwrap()).unwrap().id, p.id);
        assert!(resolve(&store, "9999").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
