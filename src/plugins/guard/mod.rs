//! `guard` — deny an identical read/command repeated within N turns, pointing at the prior result.
//!
//! Replaces: token-optimizer refetch_guard / loop detection.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Guard;

impl Plugin for Guard {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "guard",
            kind: Kind::Native,
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }
}
