//! `inject` — the single budgeted, byte-stable SessionStart/UserPromptSubmit injector (plan P2, P7).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Inject;

impl Plugin for Inject {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "inject",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }
}
