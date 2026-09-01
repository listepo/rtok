//! `proxy` — `ANTHROPIC_BASE_URL` passthrough with SSE streaming and usage capture (plan P5).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use crate::plugin::{Manifest, Plugin, Surface};

pub struct Proxy;

impl Plugin for Proxy {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "proxy",
            surfaces: &[Surface::Proxy],
            default_on: true,
        }
    }
}
