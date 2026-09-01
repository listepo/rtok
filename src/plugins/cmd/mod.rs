//! `cmd` — archive, filter and measure every Bash output via `rtok run` (plan P3).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Cmd;

impl Plugin for Cmd {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "cmd",
            surfaces: &[Surface::Hook, Surface::Cli],
            default_on: true,
        }
    }
}
