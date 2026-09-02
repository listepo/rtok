//! `graph` — `symbol` / `callers` / `outline` over a symbol index rtok builds itself
//! with tree-sitter-tags, with capped output (plan P8).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Ctx, Manifest, Plugin, PostToolUse, Surface};

pub mod index;

pub struct Graph;

impl Plugin for Graph {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "graph",
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        if ev.tool_name != "Edit" && ev.tool_name != "Write" {
            return None;
        }
        if let Some(p) = ev.tool_input.get("file_path").and_then(|v| v.as_str()) {
            let _ = cx.store.mark_symbols_stale(p);
        }
        None
    }
}
