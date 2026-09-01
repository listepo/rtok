//! `graph` — adapter over an installed code-graph MCP server: `symbol` / `callers` / `outline`
//! with capped output (plan P8).
//!
//! Replaces: codebase-memory-mcp, code-review-graph, serena, codegraph tool surfaces.
//! Off when nothing is installed.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Graph;

impl Plugin for Graph {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "graph",
            kind: Kind::Adapter,
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
    }
}
