//! Native mode helpers for terse (caveman-style) prose and YAGNI (ponytail-style) ladder.
//!
//! # Why this module exists
//!
//! Plan decisions **D6** (no third-party code on the hot path) and **D7** (prompt modes are
//! data) split the caveman / ponytail ideas into two layers:
//!
//! 1. **Prompt modes as markdown** under `modes/` (`terse.md`, `yagni.md`), injected by
//!    `plugins::inject` under the shared 800-token budget. Agents that follow instructions
//!    get the behaviour for free — no Go proxy, no JS skill wrapper.
//! 2. **This module** — deterministic Rust helpers used by benches (and later hooks if we
//!    wire them) as a **safety net** when the model ignores the prompt: fluff stripping that
//!    never mutates fenced code, and a typed YAGNI ladder that cannot “always pick Minimum”.
//!
//! We deliberately do **not** vendor or wrap JuliusBrussee/caveman or DietrichGebert/ponytail.
//! Those tools inspired the behaviour; the implementation is native and measured in
//! `tests/mode_bench.rs`. Numbers and the “why we are better” write-up live in
//! [`research.md`](../../research.md) (modes subsection) and
//! [`docs/comparison.md`](../../docs/comparison.md) § Prompt-level.
//!
//! Aliases `cave` → `terse` and `pony` → `yagni` are inject-only naming sugar; the files and
//! this module keep the rtok names.

mod compress;
mod ladder;

pub use compress::{CaveIntensity, compress_prose};
pub use ladder::{
    LadderContext, LadderDecision, PonyIntensity, evaluate_ladder, naive_always_minimum,
};
