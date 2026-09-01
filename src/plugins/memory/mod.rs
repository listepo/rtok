//! `memory` — agent-written notes in SQLite FTS5: `mem_save` / `mem_search` / `mem_get`,
//! PreCompact checkpoint, SessionStart recall (plan P2 T2.5, P6).
//!
//! Replaces: engram, claude-mem. Zero LLM cost by design.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Memory;

impl Plugin for Memory {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "memory",
            kind: Kind::Native,
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }
}
