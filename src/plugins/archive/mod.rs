//! `archive` — replace old, large `tool_result` blocks in the live zone with pointers (plan P5).
//!
//! Replaces: token-optimizer archive_result, headroom CCR, caveman retrieve.
//! Decisions are keyed by `tool_use_id` and persisted so the frozen prefix stays byte-stable.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Archive;

impl Plugin for Archive {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "archive",
            kind: Kind::Native,
            surfaces: &[Surface::Proxy, Surface::Mcp],
            default_on: true,
        }
    }
}
