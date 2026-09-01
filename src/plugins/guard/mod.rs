//! `guard` — deny an identical read/command repeated within N turns, pointing at the prior result.
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Guard;

impl Plugin for Guard {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "guard",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }
}
