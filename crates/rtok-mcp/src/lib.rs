//! `crates/rtok-mcp` (T277): the crate boundary around rtok's own MCP install/status logic,
//! duplicated today per host in `src/agents/<host>/mod.rs` (`register_mcp` / `unregister_mcp` /
//! `installed`, plus the MCP half of `d21_plugin_apply` in `src/agents/mod.rs`). T275's fix for
//! the Claude Desktop entry removal had to be repeated by hand on eight other hosts because
//! there was no shared call path; this crate is that call path.
//!
//! PR 1 (this crate) wires up nothing yet — no host in `src/agents/` calls into it. It stands
//! on its own, proven by the table-driven test in [`ops`]. Hosts move onto it one at a time
//! starting with Claude (T277 PR 2); see the crate's `AGENTS.md`-adjacent plan card (`plan.md`
//! `### T277`) for the full migration order.
//!
//! Layout:
//! - [`spec`]: `McpSpec`, the per-host-client data (config path, [`spec::Format`], dotted key
//!   path, entry shape, duplicate-name behaviour) that used to live in each host's own module.
//! - [`registry`]: rtok's own MCP servers — the one list `McpSpec::entry` builds from.
//! - [`config`]: read/write/remove one entry in a host's config file through the [`config::Fs`]
//!   trait, in whatever shape that host uses (JSON, JSONC, TOML).
//! - [`status`]: `McpStatus`, computed from a config file and a spec — the truth source T278
//!   needs instead of `installed`'s `contains("mcp")` guess.
//! - [`ops`]: `apply`, the one entry point a host installer calls to install, update, or remove
//!   its entry.

pub mod config;
pub mod ops;
pub mod registry;
pub mod spec;
pub mod status;
