//! Notes API: `mem_save` / `mem_search` / `mem_get` (plan T6.1).

pub mod export;
pub mod import;
pub mod status;

use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Injection, Manifest, Measurement, Plugin, PromptSubmit,
    SessionStart, Surface, ToolDef,
};
use serde_json::json;

pub struct Memory;

impl Plugin for Memory {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "memory",
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Memory",
            "Recall notes and titles without an LLM extraction step.",
            true,
        )
    }

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "mem_save",
                description: "Save a note (kind, title, body); same project+kind+title updates it.",
                input_schema: json!({"type":"object","properties":{"kind":{"type":"string"},"title":{"type":"string"},"body":{"type":"string"},"project":{"type":"string"}},"required":["kind","title","body"]}),
            },
            ToolDef {
                name: "mem_search",
                description: "Search notes by FTS5; ids, titles, snippets.",
                input_schema: json!({"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}},"required":["query"]}),
            },
            ToolDef {
                name: "mem_get",
                description: "Return one note body by id.",
                input_schema: json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}),
            },
            ToolDef {
                name: "mem_update",
                description: "Retire (tombstone, never delete) or pin a note by id.",
                input_schema: json!({"type":"object","properties":{"id":{"type":"integer"},"retire":{"type":"boolean"},"superseded_by":{"type":"integer"},"pinned":{"type":"boolean"}},"required":["id"]}),
            },
        ]
    }

    fn session_start(&self, _ev: &SessionStart, cx: &Ctx) -> Option<Injection> {
        recall(cx)
    }

    fn prompt_submit(&self, ev: &PromptSubmit, cx: &Ctx) -> Option<Injection> {
        if let Some(inj) = remember_save(ev, cx) {
            return Some(inj);
        }
        prompt_recall(ev, cx)
    }
}

/// Git directory name of `cwd`, if any.
pub fn project_name(cwd: &std::path::Path) -> Option<String> {
    let mut p = cwd.to_path_buf();
    loop {
        if p.join(".git").exists() {
            return p.file_name().map(|s| s.to_string_lossy().into_owned());
        }
        if !p.pop() {
            return None;
        }
    }
}

fn remember_save(ev: &PromptSubmit, cx: &Ctx) -> Option<Injection> {
    let first = ev.prompt.lines().next()?.trim();
    let rest = first.strip_prefix("remember:")?.trim();
    if rest.is_empty() {
        return None;
    }
    let title: String = rest.chars().take(80).collect();
    let project = cx
        .cwd()
        .map(std::path::Path::new)
        .and_then(project_name);
    let id = cx.upsert_note(project.as_deref(), "user", &title, rest).ok()?;
    Some(Injection {
        plugin: "memory",
        text: format!("saved note {id}"),
        priority: 12,
    })
}

fn prompt_recall(ev: &PromptSubmit, cx: &Ctx) -> Option<Injection> {
    let cfg = cx.plugin_config::<crate::config::Memory>("memory");
    let n = cfg.prompt_recall;
    if n == 0 {
        return None;
    }
    let query = ev.prompt.split_whitespace().take(24).collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        return None;
    }
    let hits = cx.search_notes(&query, n).ok()?;
    if hits.is_empty() {
        return None;
    }
    let mut lines = vec!["notes".to_string()];
    for h in &hits {
        lines.push(format!("{} {}", h.id, h.title));
    }
    let text = lines.join("\n");
    let sha = crate::store::hex_sha256(text.as_bytes());
    if cx.last_measurement_ref("memory", "prompt_recall").ok()? == Some(sha.clone()) {
        return None;
    }
    let mut before_bytes = 0u64;
    let mut est_before = 0u32;
    for h in &hits {
        if let Ok(Some(body)) = cx.get_note_body(h.id) {
            before_bytes += body.len() as u64;
            est_before = est_before.saturating_add(cx.estimate(&body, Class::Prose));
        }
    }
    let after_bytes = text.len() as u64;
    let est_after = cx.estimate(&text, Class::Prose);
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "prompt_recall",
        before_bytes,
        after_bytes,
        est_before,
        est_after,
        ref_id: Some(sha),
        call_id: None,
    });
    Some(Injection {
        plugin: "memory",
        text,
        priority: 11,
    })
}


fn recall(cx: &Ctx) -> Option<Injection> {
    let cfg = cx.plugin_config::<crate::config::Memory>("memory");
    let n = cfg.recall_titles.max(1);
    let cap = cfg.recall_tokens.max(1);
    let project = cx
        .cwd()
        .map(std::path::Path::new)
        .and_then(project_name)
        .or_else(|| std::env::current_dir().ok().and_then(|d| project_name(&d)));
    let rows = cx.list_note_titles(project.as_deref(), n).ok()?;
    if rows.is_empty() {
        return None;
    }
    let mut entries = rows;
    let mut lines = vec!["notes".to_string()];
    for (id, title) in &entries {
        lines.push(format!("{id} {title}"));
    }
    let mut text = lines.join("\n");
    while cx.estimate(&text, Class::Prose) > cap && entries.len() > 1 {
        entries.pop();
        lines.pop();
        text = lines.join("\n");
    }
    let mut before_bytes = 0u64;
    let mut est_before = 0u32;
    for (id, _) in &entries {
        if let Ok(Some(body)) = cx.get_note_body(*id) {
            before_bytes += body.len() as u64;
            est_before = est_before.saturating_add(cx.estimate(&body, Class::Prose));
        }
    }
    let after_bytes = text.len() as u64;
    let est_after = cx.estimate(&text, Class::Prose);
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "recall",
        before_bytes,
        after_bytes,
        est_before,
        est_after,
        ref_id: Some(cx.session().to_string()),
        call_id: None,
    });
    Some(Injection {
        plugin: "memory",
        text,
        priority: 10,
    })
}

pub fn mem_save(
    rt: &crate::plugin::Runtime,
    kind: &str,
    title: &str,
    body: &str,
    project: Option<&str>,
) -> anyhow::Result<(i32, bool)> {
    let proj = project
        .map(str::to_string)
        .or_else(|| std::env::current_dir().ok().and_then(|d| project_name(&d)));
    // The title is the topic key (T66.1): a re-save updates the row, recall never shows
    // a stale twin next to the new one.
    let (id, updated) = rt.store.upsert_note(proj.as_deref(), kind, title, body)?;
    rt.store
        .upsert_note_embedding(id, title, body, &rt.config.plugins.memory.embed)?;
    Ok((id, updated))
}

pub fn mem_search(
    rt: &crate::plugin::Runtime,
    query: &str,
    limit: u32,
) -> anyhow::Result<Vec<crate::store::NoteHit>> {
    let lim = limit.max(1);
    let embed = &rt.config.plugins.memory.embed;
    if !embed.enabled {
        return rt.store.search_notes(query, lim);
    }
    if embed.hybrid {
        rt.store.search_notes_hybrid(query, lim, embed)
    } else {
        rt.store.search_notes_embed(query, lim, embed)
    }
}

pub fn mem_get(rt: &crate::plugin::Runtime, id: i32) -> anyhow::Result<Option<String>> {
    let Some(row) = rt.store.note_row(id)? else {
        return Ok(None);
    };
    Ok(Some(match row.retired {
        None => row.body,
        Some(ts) => {
            // Nothing is lost (D4): a retired note still reads whole, one line saying why.
            let mut head = format!("retired {ts}");
            if let Some(s) = row.superseded_by {
                head.push_str(&format!(", superseded by {s}"));
            }
            format!("{head}\n{}", row.body)
        }
    }))
}

/// T69.1 lifecycle, one call path for MCP `mem_update` and `rtok memory retire|pin|unpin`:
/// `retire` tombstones the id (never deletes), `pinned` pins or unpins. Retiring can name
/// the replacement with `superseded_by`.
pub fn mem_update(
    rt: &crate::plugin::Runtime,
    id: i32,
    retire: bool,
    superseded_by: Option<i32>,
    pinned: Option<bool>,
) -> anyhow::Result<String> {
    if superseded_by.is_some() && !retire {
        anyhow::bail!("superseded_by needs retire: true (it names what the retired note replaces)");
    }
    let mut parts = Vec::new();
    if retire {
        if !rt.store.retire_note(id, superseded_by)? {
            anyhow::bail!("unknown note id: {id}");
        }
        let mut line = format!("retired note {id}");
        if let Some(s) = superseded_by {
            line.push_str(&format!(", superseded by {s}"));
        }
        parts.push(line);
    }
    if let Some(p) = pinned {
        if !rt.store.set_note_pinned(id, p)? {
            anyhow::bail!("unknown note id: {id}");
        }
        parts.push(format!(
            "{} note {id}",
            if p { "pinned" } else { "unpinned" }
        ));
    }
    if parts.is_empty() {
        anyhow::bail!("nothing to do: pass retire or pinned");
    }
    Ok(parts.join("; "))
}

/// T69.1 revise: save the replacement through the in-place `mem_save` path, then retire
/// the old id naming the replacement. Same title → the upsert already updated the row, so
/// there is nothing to retire. Returns `(replacement id, retired old id)`.
pub fn mem_revise(
    rt: &crate::plugin::Runtime,
    id: i32,
    title: &str,
    body: &str,
) -> anyhow::Result<(i32, Option<i32>)> {
    let Some(old) = rt.store.note_row(id)? else {
        anyhow::bail!("unknown note id: {id}");
    };
    let (new, _) = mem_save(rt, &old.kind, title, body, old.project.as_deref())?;
    if new == id {
        return Ok((new, None));
    }
    rt.store.retire_note(id, Some(new))?;
    Ok((new, Some(id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remember_prefix_saves_a_note_and_repeats_same_id() {
        use rtok_plugin_sdk::PromptSubmit;
        let cx = crate::plugin::Runtime::in_memory("t695-remember").unwrap();
        let ctx = Ctx::new(&cx);
        let ev = PromptSubmit {
            prompt: "remember: hooks fail open under 10 ms",
        };
        let first = Memory.prompt_submit(&ev, &ctx).unwrap();
        assert_eq!(first.text, "saved note 1");
        let second = Memory.prompt_submit(&ev, &ctx).unwrap();
        assert_eq!(second.text, "saved note 1");
        let plain = PromptSubmit { prompt: "just a question" };
        assert!(Memory.prompt_submit(&plain, &ctx).is_none());
    }

    #[test]
    fn prompt_recall_is_off_by_default() {
        use rtok_plugin_sdk::PromptSubmit;
        let cx = crate::plugin::Runtime::in_memory("t695-recall-off").unwrap();
        mem_save(&cx, "note", "alpha", "hooks fail open", None).unwrap();
        let ctx = Ctx::new(&cx);
        let ev = PromptSubmit {
            prompt: "tell me about hooks fail open",
        };
        assert!(Memory.prompt_submit(&ev, &ctx).is_none());
    }

    #[test]
    fn save_three_search_hits_first_get_full_body() {
        let cx = crate::plugin::Runtime::in_memory("t61").unwrap();
        let a = mem_save(
            &cx,
            "decision",
            "walrus",
            "the walrus journal lives here",
            Some("rtok"),
        )
        .unwrap()
        .0;
        let _b = mem_save(
            &cx,
            "decision",
            "banana",
            "yellow fruit unrelated",
            Some("rtok"),
        )
        .unwrap();
        let _c = mem_save(
            &cx,
            "decision",
            "other",
            "nothing matching the unique token",
            Some("rtok"),
        )
        .unwrap();
        let hits = mem_search(&cx, "walrus", 5).unwrap();
        assert!(!hits.is_empty(), "{hits:?}");
        assert_eq!(hits[0].title, "walrus");
        assert_eq!(hits[0].id, a);
        assert!(hits[0].snippet.len() <= 120);
        let body = mem_get(&cx, a).unwrap().unwrap();
        assert_eq!(body, "the walrus journal lives here");
    }

    /// T69.1: revise saves the replacement through `mem_save`, then retires the old row
    /// naming it; the old body's words no longer search, `mem_get` keeps the body.
    #[test]
    fn revise_supersedes_and_retires_the_old_note() {
        let cx = crate::plugin::Runtime::in_memory("t691-revise").unwrap();
        let old = mem_save(&cx, "decision", "auth model", "sessions forever", Some("p"))
            .unwrap()
            .0;
        let (new, retired) = mem_revise(&cx, old, "auth model v2", "jwt tokens now").unwrap();
        assert_eq!(retired, Some(old));
        assert_ne!(new, old);
        assert_eq!(mem_search(&cx, "sessions", 5).unwrap().len(), 0);
        assert_eq!(mem_search(&cx, "jwt", 5).unwrap()[0].id, new);
        let body = mem_get(&cx, old).unwrap().unwrap();
        assert!(body.starts_with("retired "), "{body}");
        assert!(body.contains(&format!(", superseded by {new}")), "{body}");
        assert!(body.contains("sessions forever"), "{body}");
        // Same title → the upsert updated the row in place; nothing retires.
        let (same, none) = mem_revise(&cx, new, "auth model v2", "jwt tokens v2").unwrap();
        assert_eq!((same, none), (new, None));
        assert!(
            mem_get(&cx, new)
                .unwrap()
                .unwrap()
                .starts_with("jwt tokens v2"),
            "revision landed"
        );
    }

    /// T69.1: a retired note never recalls and never searches, but still reads whole.
    #[test]
    fn retire_removes_from_recall_and_keeps_the_body() {
        let cx = crate::plugin::Runtime::in_memory("t691-retire").unwrap();
        let id = mem_save(&cx, "note", "stale fact", "wrong under new light", None)
            .unwrap()
            .0;
        let out = mem_update(&cx, id, true, None, None).unwrap();
        assert_eq!(out, format!("retired note {id}"));
        if let Some(inj) = recall(&Ctx::new(&cx)) {
            assert!(!inj.text.contains("stale fact"), "{}", inj.text);
        }
        let body = mem_get(&cx, id).unwrap().unwrap();
        assert!(body.starts_with("retired "), "{body}");
        assert!(body.contains("wrong under new light"), "{body}");
        assert!(mem_search(&cx, "wrong", 5).unwrap().is_empty());
        assert!(mem_update(&cx, 999, true, None, None).is_err());
        // A re-save revives the topic instead of editing a tombstone.
        let (again, _) = mem_save(&cx, "note", "stale fact", "right again", None).unwrap();
        assert_eq!(mem_get(&cx, again).unwrap().unwrap(), "right again");
    }

    /// T69.1: a pinned note leads recall ahead of twenty newer ones, byte-stable.
    #[test]
    fn pinned_note_leads_recall_byte_stable() {
        let cx = crate::plugin::Runtime::in_memory("t691-pin").unwrap();
        let keep = mem_save(&cx, "note", "keep me first", "pinned body", None)
            .unwrap()
            .0;
        assert_eq!(
            mem_update(&cx, keep, false, None, Some(true)).unwrap(),
            format!("pinned note {keep}")
        );
        for i in 0..20 {
            mem_save(
                &cx,
                "note",
                &format!("newer-{i}"),
                &format!("body-{i}"),
                None,
            )
            .unwrap();
        }
        let a = recall(&Ctx::new(&cx)).unwrap();
        let b = recall(&Ctx::new(&cx)).unwrap();
        assert_eq!(a.text, b.text);
        assert!(
            a.text.contains(&format!("notes\n{keep} keep me first")),
            "{}",
            a.text
        );
        assert_eq!(
            mem_update(&cx, keep, false, None, Some(false)).unwrap(),
            format!("unpinned note {keep}")
        );
    }

    /// T69.1: the four memory tools stay within the 60-description-token surface budget
    /// (`rtok doctor` prices the same strings).
    #[test]
    fn mcp_surface_stays_within_sixty_description_tokens() {
        let cx = crate::plugin::Runtime::in_memory("t691-surface").unwrap();
        let total: i64 = Memory
            .mcp_tools()
            .iter()
            .map(|t| i64::from(cx.estimate(t.description, Class::Prose)))
            .sum();
        assert!(total <= 60, "memory tool surface is {total} tokens");
    }

    #[test]
    fn same_project_kind_title_updates_in_place() {
        let cx = crate::plugin::Runtime::in_memory("t671").unwrap();
        let (a, first) = mem_save(&cx, "decision", "auth model", "sessions", Some("p")).unwrap();
        let (b, second) = mem_save(&cx, "decision", "auth model", "jwt", Some("p")).unwrap();
        assert_eq!((first, second, a == b), (false, true, true));
        // A different kind or project is another topic.
        let (c, _) = mem_save(&cx, "note", "auth model", "other kind", Some("p")).unwrap();
        let (d, _) = mem_save(&cx, "decision", "auth model", "other project", Some("q")).unwrap();
        assert!(a != c && a != d && c != d);
        assert_eq!(cx.store.list_notes(Some("p")).unwrap().len(), 2);
        let hits = mem_search(&cx, "jwt", 5).unwrap();
        assert_eq!(hits[0].id, a);
        assert!(mem_search(&cx, "sessions", 5).unwrap().is_empty());
    }

    #[test]
    fn twenty_notes_recall_five_titles_under_budget_stable() {
        let cx = crate::plugin::Runtime::in_memory("t62").unwrap();
        // `recall` filters by the project derived from the working directory, so the notes
        // must be saved the same way — a hard-coded name only matched a checkout called `rtok`.
        for i in 0..20 {
            mem_save(
                &cx,
                "note",
                &format!("title-{i}"),
                &format!("body-{i} secret"),
                None,
            )
            .unwrap();
        }
        let a = recall(&Ctx::new(&cx)).unwrap();
        let b = recall(&Ctx::new(&cx)).unwrap();
        assert_eq!(a.text, b.text);
        assert!(!a.text.contains("secret"), "{}", a.text);
        assert_eq!(a.text.lines().count(), 6, "{}", a.text);
        assert!(cx.estimate(&a.text, Class::Prose) <= 200);
        assert_eq!(a.priority, 10);
        let rows = cx.store.list_measurements("memory").unwrap();
        assert_eq!(rows.iter().filter(|r| r.kind == "recall").count(), 2, "{rows:?}");
    }

    #[test]
    fn recall_filters_by_hook_cwd_not_process_cwd() {
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("rtok-mem-cwd-{pid}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let mut cx = crate::plugin::Runtime::in_memory("mem-cwd").unwrap();
        cx.cwd = Some(dir.to_string_lossy().into_owned());
        mem_save(
            &cx,
            "note",
            "other-note",
            "body from elsewhere",
            Some("elsewhere"),
        )
        .unwrap();
        // project_name uses the directory's basename (the last component).
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        mem_save(
            &cx,
            "note",
            "cwd-note",
            "visible under hook cwd",
            Some(&name),
        )
        .unwrap();
        let inj = recall(&Ctx::new(&cx)).unwrap();
        assert!(inj.text.contains("cwd-note"), "{}", inj.text);
        assert!(!inj.text.contains("other-note"), "{}", inj.text);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
