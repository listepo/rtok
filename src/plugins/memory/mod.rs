//! `memory` — agent-written notes in SQLite FTS5: `mem_save` / `mem_search` / `mem_get`,
//! PreCompact checkpoint, SessionStart recall (plan P2 T2.5, P6).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Memory;

impl Plugin for Memory {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "memory",
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }
}
