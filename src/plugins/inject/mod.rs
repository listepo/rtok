//! `inject` — the single budgeted, byte-stable SessionStart/UserPromptSubmit injector (plan P2, P7).
//!
//! Replaces: caveman shrink-hook, ponytail/caveman modes, lean-ctx banner, engram/claude-mem
//! SessionStart context. Modes are markdown data files, not code (decision D7).

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Inject;

impl Plugin for Inject {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "inject",
            kind: Kind::Native,
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }
}
