//! Structured YAGNI ladder — typed decisions instead of prompt-only “always minimum”.
//!
//! # Why a typed ladder, not only `modes/yagni.md`
//!
//! Ponytail is a prompt file: the model may still emit a custom helper when stdlib would do.
//! A prompt-only agent that “tries to be lazy” but has no structure often collapses to
//! **always write the minimum new code** — which is wrong when the right answer is skip,
//! reuse, or native platform. `evaluate_ladder` makes the rung order explicit and testable.
//!
//! Rung order (first match wins), same as the yagni mode text:
//! YAGNI skip → reuse → stdlib → native → installed dep → one-liner → minimum.
//!
//! `must_not_simplify` short-circuits to [`LadderDecision::Minimum`] so security / data-loss
//! / a11y / validation work is never golfed away — that override beats even `speculative`.
//!
//! [`naive_always_minimum`] is the honest “prompt-only / unstructured” baseline used in
//! `tests/mode_bench.rs`, not a claim about ponytail's published LOC numbers.

/// Ponytail-style intensity. Affects only how aggressively speculative work is skipped;
/// the rung order stays the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PonyIntensity {
    /// Prefer naming a lazier alternative; still returns the same rungs (tests use Full).
    Lite,
    /// Enforce the ladder.
    Full,
    /// Speculative work is skipped even more readily (same flags; ultra trusts speculative).
    Ultra,
}

/// Inputs describing what the agent already knows about a coding task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LadderContext {
    /// Need is speculative / not requested → skip.
    pub speculative: bool,
    /// A helper/util already exists in the repo.
    pub existing_helper: bool,
    /// Standard library covers it.
    pub stdlib: bool,
    /// Native platform feature covers it (date input, CSS, DB constraint, …).
    pub native: bool,
    /// An already-installed dependency covers it.
    pub installed_dep: bool,
    /// A correct one-liner exists.
    pub one_liner: bool,
    /// Security / data-loss / a11y / validation — must not be golfed away.
    pub must_not_simplify: bool,
}

/// Chosen rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderDecision {
    YagniSkip,
    Reuse,
    Stdlib,
    NativePlatform,
    InstalledDep,
    OneLiner,
    Minimum,
}

/// Climb the ladder. `must_not_simplify` forces [`LadderDecision::Minimum`] (safe).
/// Otherwise first true flag in YAGNI → reuse → stdlib → native → dep → one-liner wins;
/// if none match, [`LadderDecision::Minimum`].
pub fn evaluate_ladder(ctx: &LadderContext) -> LadderDecision {
    evaluate_ladder_at(ctx, PonyIntensity::Full)
}

/// Apply the same rung precedence as [`evaluate_ladder`].
///
/// `intensity` is currently ignored and reserved for future speculative bias.
pub fn evaluate_ladder_at(ctx: &LadderContext, intensity: PonyIntensity) -> LadderDecision {
    let _ = intensity; // rung order is intensity-invariant; ultra may tighten speculative upstream.
    if ctx.must_not_simplify {
        return LadderDecision::Minimum;
    }
    if ctx.speculative {
        return LadderDecision::YagniSkip;
    }
    if ctx.existing_helper {
        return LadderDecision::Reuse;
    }
    if ctx.stdlib {
        return LadderDecision::Stdlib;
    }
    if ctx.native {
        return LadderDecision::NativePlatform;
    }
    if ctx.installed_dep {
        return LadderDecision::InstalledDep;
    }
    if ctx.one_liner {
        return LadderDecision::OneLiner;
    }
    LadderDecision::Minimum
}

/// Naive baseline: always pick Minimum (unstructured “yagni prompt only”).
pub fn naive_always_minimum(_ctx: &LadderContext) -> LadderDecision {
    LadderDecision::Minimum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_forces_minimum() {
        let ctx = LadderContext {
            speculative: true,
            must_not_simplify: true,
            ..Default::default()
        };
        assert_eq!(evaluate_ladder(&ctx), LadderDecision::Minimum);
    }

    #[test]
    fn speculative_skips() {
        let ctx = LadderContext {
            speculative: true,
            stdlib: true,
            ..Default::default()
        };
        assert_eq!(evaluate_ladder(&ctx), LadderDecision::YagniSkip);
    }

    #[test]
    fn native_before_dep() {
        let ctx = LadderContext {
            native: true,
            installed_dep: true,
            ..Default::default()
        };
        assert_eq!(evaluate_ladder(&ctx), LadderDecision::NativePlatform);
    }
}
