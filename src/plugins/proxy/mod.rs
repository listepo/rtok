//! `proxy` — `ANTHROPIC_BASE_URL` passthrough with SSE streaming and usage capture (plan P5).
//!
//! Replaces: headroom proxy, caveman-proxy. Never touches `system`, `tools`, or the last
//! `keep_turns` turns.

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Proxy;

impl Plugin for Proxy {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "proxy",
            kind: Kind::Native,
            surfaces: &[Surface::Proxy],
            default_on: true,
        }
    }
}
