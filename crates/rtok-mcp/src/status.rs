//! `McpStatus`: the read-only truth T278's `agents info` needs — computed straight from a
//! config file and a [`McpSpec`], never from the `contains("mcp")`-style guess `installed()`
//! makes on `main` today.

use anyhow::Result;

use crate::config::{self, Fs};
use crate::spec::{McpSpec, Surface};

/// What a host's config file carries at `key_path.<server.name>`, compared with the entry
/// [`McpSpec::entry`] would write there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Present,
    Missing,
    /// An entry exists under that name but does not match rtok's own. `String` is a short,
    /// human-readable diff; T278 owns turning it into the `agents info --json` shape.
    Stale(String),
}

/// The status of one host surface's MCP entry, plus whether a loaded plugin also serves rtok's
/// name (D33: a plugin only counts for the surface it actually serves — the Claude Code plugin
/// must never make Claude Desktop read as installed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpStatus {
    pub surface: Surface,
    pub entry: Entry,
    pub plugin_serves: Option<String>,
}

/// Read `spec`'s config file through `fs` and compare it against the entry rtok would write.
pub fn status(fs: &impl Fs, spec: &McpSpec) -> Result<McpStatus> {
    let entry = match config::read_entry(fs, spec)? {
        None => Entry::Missing,
        Some(have) if have == spec.entry() => Entry::Present,
        Some(have) => Entry::Stale(format!("have {have}, want {}", spec.entry())),
    };
    Ok(McpStatus {
        surface: spec.surface,
        entry,
        plugin_serves: spec.plugin_serves.map(str::to_string),
    })
}
