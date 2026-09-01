//! rtok — token reduction for AI coding agents, one plugin per method.
//!
//! Library layout (see `architecture.md`):
//! - [`config`]  — `~/.rtok/config.toml`, `RTOK_HOME`
//! - [`store`]   — one SQLite file: events, measurements, archive, notes (FTS5), usage
//! - [`tokens`]  — chars-per-token estimator (±15 %)
//! - [`plugin`]  — the `Plugin` trait, `Manifest`, `Ctx`, `Measurement`
//! - [`plugins`] — the registry and one module per catalogue plugin
//! - [`hooks`]   — Claude Code hook I/O types

pub mod config;
pub mod hooks;
pub mod plugin;
pub mod plugins;
pub mod store;
pub mod tokens;
