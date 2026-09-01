//! `cmd` — archive, filter and measure every Bash output via `rtok run` (plan P3).
//!
//! Replaces: rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress.
//! Delegates filtering to `rtk` when installed (adapter), else TOML rules (native).

use crate::plugin::{Kind, Manifest, Plugin, Surface};

pub struct Cmd;

impl Plugin for Cmd {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "cmd",
            kind: Kind::Native,
            surfaces: &[Surface::Hook, Surface::Cli],
            default_on: true,
        }
    }
}
