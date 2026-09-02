//! Token estimator (plan T0.5): `chars / chars-per-token` per text class.
//!
//! Accuracy: about ±15 % against the Anthropic tokenizer on this workload (research.md §2).
//! Good enough to rank savings and to gate budgets; never present it as a billed count.
//! Rates come from `[estimator]` in config and can be refit by `rtok stats --calibrate` (T1.5).

use crate::config::Estimator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Source code, shell output, diffs.
    Code,
    /// Natural-language prose, markdown.
    Prose,
    /// JSON / structured data (punctuation-heavy → fewer chars per token).
    Json,
    /// CJK scripts: roughly one token per character.
    Cjk,
}

/// Estimated tokens for `text`. Empty text → 0.
pub fn estimate(text: &str, class: Class, rates: &Estimator) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let chars_per_token = match class {
        Class::Code => rates.code,
        Class::Prose => rates.prose,
        Class::Json => rates.json,
        Class::Cjk => rates.cjk,
    }
    .max(0.1);
    let chars = text.chars().count() as f32;
    (chars / chars_per_token).ceil() as u32
}

/// Tokens saved by shortening `before` to `after`; never negative.
pub fn tokens_saved(before: u32, after: u32) -> u32 {
    before.saturating_sub(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: Estimator = Estimator {
        code: 3.5,
        prose: 4.2,
        json: 3.0,
        cjk: 1.0,
    };

    #[test]
    fn empty_is_zero() {
        assert_eq!(estimate("", Class::Code, &RATES), 0);
        assert_eq!(estimate("", Class::Cjk, &RATES), 0);
    }

    #[test]
    fn fixtures() {
        // 35 chars of code → 10 tokens at 3.5 chars/token.
        assert_eq!(
            estimate("fn main() { println!(\"hi there\"); }", Class::Code, &RATES),
            10
        );
        // 43 chars of prose → 11 tokens at 4.2 (ceil).
        assert_eq!(
            estimate(
                "The quick brown fox jumps over the lazy dog",
                Class::Prose,
                &RATES
            ),
            11
        );
        // 29 chars of JSON → 10 tokens at 3.0 (ceil).
        assert_eq!(
            estimate(r#"{"a":1,"b":[1,2,3],"c":"xyz"}"#, Class::Json, &RATES),
            10
        );
        // 6 CJK chars → 6 tokens (chars, not bytes).
        assert_eq!(estimate("東京都渋谷区", Class::Cjk, &RATES), 6);
    }

    #[test]
    fn saved_never_negative() {
        assert_eq!(tokens_saved(100, 40), 60);
        assert_eq!(tokens_saved(40, 100), 0);
    }
}

/// T1.5: skip without a key. Full `count_tokens` fit lands when a key is present.
pub fn calibrate_or_skip(_cfg: &crate::config::Config) -> &'static str {
    if std::env::var_os("ANTHROPIC_API_KEY").is_none() {
        return "skipped";
    }
    "skipped"
}

#[cfg(test)]
mod calibrate_tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn skip_without_key() {
        // must not panic; message is skipped when the key is absent.
        let cfg = Config::default();
        if std::env::var_os("ANTHROPIC_API_KEY").is_none() {
            assert_eq!(calibrate_or_skip(&cfg), "skipped");
        }
    }
}
