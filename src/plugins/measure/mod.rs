//! `measure` — session-log ingest, proxy usage capture, `rtok stats` / `rtok bench` (plan P1, P9).
//!
//! Replaces: rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Measure;

impl Plugin for Measure {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "measure",
            kind: Kind::Native,
            surfaces: &[Surface::Cli, Surface::Proxy],
            default_on: true,
        }
    }
}
