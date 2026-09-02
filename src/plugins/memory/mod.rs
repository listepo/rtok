//! Notes API: `mem_save` / `mem_search` / `mem_get` (plan T6.1).

use crate::plugin::{Ctx, Manifest, Plugin, Surface, ToolDef};
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

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "mem_save",
                description: "Save a note (kind, title, body).",
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
        ]
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

pub fn mem_save(
    cx: &Ctx,
    kind: &str,
    title: &str,
    body: &str,
    project: Option<&str>,
) -> anyhow::Result<i32> {
    let proj = project
        .map(str::to_string)
        .or_else(|| std::env::current_dir().ok().and_then(|d| project_name(&d)));
    cx.store.insert_note(proj.as_deref(), kind, title, body)
}

pub fn mem_search(cx: &Ctx, query: &str, limit: u32) -> anyhow::Result<Vec<crate::store::NoteHit>> {
    cx.store.search_notes(query, limit.max(1))
}

pub fn mem_get(cx: &Ctx, id: i32) -> anyhow::Result<Option<String>> {
    cx.store.get_note_body(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_three_search_hits_first_get_full_body() {
        let cx = Ctx::in_memory("t61").unwrap();
        let a = mem_save(
            &cx,
            "decision",
            "walrus",
            "the walrus journal lives here",
            Some("rtok"),
        )
        .unwrap();
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
}
