//! `read` — MCP `read` / `search` / `tree` with modes, size caps and re-read dedup (plan P4).
//!
//! Replaces: lean-ctx ctx_read/search/tree (78 tools), token-optimizer read_cache/structure_map.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Read;

impl Plugin for Read {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "read",
            kind: Kind::Native,
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }
}
