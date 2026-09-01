//! `read` — MCP `read` / `search` / `tree` with modes, size caps and re-read dedup (plan P4).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Read;

impl Plugin for Read {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "read",
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }
}
