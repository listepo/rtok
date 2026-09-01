//! `archive` — replace old, large `tool_result` blocks in the live zone with pointers (plan P5).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Archive;

impl Plugin for Archive {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "archive",
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: true,
        }
    }
}
