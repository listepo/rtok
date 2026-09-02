//! rtok — token reduction for AI coding agents, one plugin per method.
//!
//! Library layout (see `architecture.md`):
//! - [`cli`]     — clap subcommand tree (`rtok config`, `rtok hook`, …)
//! - [`config`]  — `~/.rtok/config.toml`, `RTOK_HOME`
//! - [`store`]   — one SQLite file: events, measurements, archive, notes (FTS5), usage
//! - [`tokens`]  — chars-per-token estimator (±15 %)
//! - [`plugin`]  — the `Plugin` trait, `Manifest`, `Ctx`, `Measurement`
//! - [`plugins`] — the registry and one module per catalogue plugin
//! - [`hooks`]   — Claude Code hook I/O types
//! - [`doctor`]  — `rtok doctor`
//! - [`measure`] — JSONL ingest, `rtok stats` (P1)

pub mod cli;
pub mod config;
pub mod doctor;
pub mod expand;
pub mod hooks;
pub mod measure;
pub mod plugin;
pub mod plugins;
pub mod setup;
pub mod store;
pub mod tokens;

/// The plugin contract at the crate root, so an external plugin crate writes
/// `use rtok::{Ctx, Manifest, Plugin, Surface};`.
pub use plugin::*;
