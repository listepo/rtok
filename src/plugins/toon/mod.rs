//! `toon` — tabular JSON → TOON encoding (vendor bench: −42.6 % tokens). Off by default
//! until A/B measured.
//!
//! Replaces: caveman toon, TOON.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Toon;

impl Plugin for Toon {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "toon",
            kind: Kind::Native,
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: false,
        }
    }
}
