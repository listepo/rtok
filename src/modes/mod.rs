//! Native mode helpers for terse (caveman-style) prose and YAGNI (ponytail-style) ladder.
//!
//! Prompt modes live as markdown under `modes/` and are injected by `plugins::inject`.
//! This module is the structured safety net beyond prompt-only behaviour (D6/D7):
//! deterministic fluff stripping and a typed decision ladder — not a wrap of third-party tools.

mod compress;
mod ladder;

pub use compress::{CaveIntensity, compress_prose};
pub use ladder::{
    LadderContext, LadderDecision, PonyIntensity, evaluate_ladder, naive_always_minimum,
};
