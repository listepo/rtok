//! `toon` — tabular JSON → TOON encoding (vendor bench: −42.6 % tokens). Off by default
//! until A/B measured.
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Toon;

impl Plugin for Toon {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "toon",
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: false,
        }
    }
}
