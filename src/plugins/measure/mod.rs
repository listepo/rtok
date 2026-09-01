//! `measure` — session-log ingest, proxy usage capture, `rtok stats` / `rtok bench` (plan P1, P9).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Measure;

impl Plugin for Measure {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "measure",
            surfaces: &[Surface::Cli, Surface::Proxy],
            default_on: true,
        }
    }
}
