//! Deterministic English fluff stripper that never mutates fenced code bodies.
//!
//! # Why deterministic, not LLM / not caveman's Go proxy
//!
//! Caveman's shrink path is a separate process (and its issue #112 has corrupted inline
//! code). We need something that:
//! - runs inside the rtok binary (D6 — no wrap),
//! - is byte-stable and testable without a network,
//! - **never** touches ``` fences or `` `backtick` `` spans (lossless for code / errors),
//! - never drops negation tokens (`not` / `never` / `no` / `only` / `except`) so “do not
//!   delete” cannot become “delete”.
//!
//! Intensities mirror caveman's lite/full/ultra naming so the inject prompt and this helper
//! stay aligned; `Lite` matches the weak baseline in `tests/mode_bench.rs`, `Full` is what
//! we claim against that baseline.

/// Caveman-style compression intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveIntensity {
    /// Drop only the weakest pleasantries (`Sure!`, `I'd be happy to help`).
    Lite,
    /// Drop filler/hedging and articles (`a`/`an`/`the`) outside fences.
    Full,
    /// Full plus strip common conjunction padding when unambiguous.
    Ultra,
}

/// Phrases stripped at every intensity (weak / "caveman-lite" baseline shares the first two).
const PLEASANTRIES: &[&str] = &[
    "I'd be happy to help you with that.",
    "I'd be happy to help you with that",
    "I'd be happy to help!",
    "I'd be happy to help.",
    "I'd be happy to help",
    "I would be happy to help.",
    "I would be happy to help",
    "Happy to help!",
    "Happy to help.",
    "Of course!",
    "Of course.",
    "Certainly!",
    "Certainly.",
    "Sure thing!",
    "Sure thing.",
    "Sure!",
    "Sure.",
];

const FILLER_WORDS: &[&str] = &[
    "just",
    "really",
    "basically",
    "actually",
    "simply",
    "literally",
];

const ARTICLES: &[&str] = &["a", "an", "the"];

const CONJUNCTION_PAD: &[&str] = &["and then", "and also", "as well as"];

/// Negation / critical words that must never be dropped as whole tokens.
const KEEP_WORDS: &[&str] = &["not", "never", "no", "only", "except"];

/// Strip English fluff from `text` at `intensity`.
///
/// Algorithm (why this shape):
/// 1. Split on ``` fences first — copy each fence body **verbatim** (opening through
///    closing fence). Unclosed fence → preserve the remainder untouched (fail closed on
///    code, never half-edit it).
/// 2. Outside fences only: drop pleasantries (all intensities), then filler + articles at
///    Full/Ultra, then conjunction padding at Ultra.
/// 3. Word drops are whole-token only; [`KEEP_WORDS`] is never stripped.
/// 4. Inline `` `...` `` spans are copied as atoms so error strings stay exact.
///
/// Deterministic and allocation-light; not a semantic summarizer.
pub fn compress_prose(text: &str, intensity: CaveIntensity) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let (prose, after) = rest.split_at(start);
        out.push_str(&compress_outside(prose, intensity));
        // Copy fence: opening ```…\n … closing ```
        let after_open = &after[3..];
        if let Some(end) = after_open.find("```") {
            let fence_end = 3 + end + 3;
            out.push_str(&after[..fence_end]);
            rest = &after[fence_end..];
        } else {
            // Unclosed fence: preserve the remainder untouched.
            out.push_str(after);
            return out;
        }
    }
    out.push_str(&compress_outside(rest, intensity));
    out
}

fn compress_outside(prose: &str, intensity: CaveIntensity) -> String {
    let mut s = prose.to_string();
    for p in PLEASANTRIES {
        // Case-insensitive replace for leading pleasantries.
        s = replace_ci(&s, p, "");
    }
    match intensity {
        CaveIntensity::Lite => {}
        CaveIntensity::Full | CaveIntensity::Ultra => {
            s = strip_words(&s, FILLER_WORDS);
            s = strip_words(&s, ARTICLES);
            if matches!(intensity, CaveIntensity::Ultra) {
                for p in CONJUNCTION_PAD {
                    s = replace_ci(&s, p, " ");
                }
            }
        }
    }
    cleanup_ws(&s)
}

fn replace_ci(hay: &str, needle: &str, with: &str) -> String {
    let lower = hay.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(hay.len());
    let mut i = 0;
    let bytes = hay.as_bytes();
    while let Some(rel) = lower[i..].find(&n) {
        let at = i + rel;
        out.push_str(&hay[i..at]);
        out.push_str(with);
        i = at + needle.len();
        // Advance by needle byte length on original (ASCII phrases only).
        let _ = bytes;
    }
    out.push_str(&hay[i..]);
    out
}

/// Drop whole words from `words` when they appear as standalone tokens (ASCII word chars).
/// Never drops [`KEEP_WORDS`]. Preserves content inside `...` and ``...``.
fn strip_words(text: &str, words: &[&str]) -> String {
    let drop: std::collections::HashSet<&str> = words.iter().copied().collect();
    let keep: std::collections::HashSet<&str> = KEEP_WORDS.iter().copied().collect();
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        // Preserve inline backticks as atomic spans.
        if c == '`' {
            out.push('`');
            for d in chars.by_ref() {
                out.push(d);
                if d == '`' {
                    break;
                }
            }
            continue;
        }
        if c.is_ascii_alphabetic() {
            let mut word = String::new();
            word.push(c);
            while let Some(d) = chars.peek().copied() {
                if d.is_ascii_alphabetic() || d == '\'' {
                    word.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let lower = word.to_ascii_lowercase();
            if keep.contains(lower.as_str()) || !drop.contains(lower.as_str()) {
                out.push_str(&word);
            }
            // else: drop the word
        } else {
            out.push(c);
        }
    }
    out
}

fn cleanup_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for line in s.lines() {
        if !start {
            out.push('\n');
        }
        start = false;
        let mut prev_space = true; // trim line start
        for c in line.chars() {
            if c.is_whitespace() {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            } else {
                out.push(c);
                prev_space = false;
            }
        }
        while out.ends_with(' ') {
            out.pop();
        }
    }
    if s.ends_with('\n') {
        out.push('\n');
    }
    out.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_drops_filler_keeps_fence() {
        let in_ = "Sure! I'd be happy to help. The bug is just really in auth.\n```rs\nfn f() { let the = 1; }\n```\nThen fix it.";
        let out = compress_prose(in_, CaveIntensity::Full);
        assert!(out.contains("```rs\nfn f() { let the = 1; }\n```"), "{out}");
        assert!(!out.to_ascii_lowercase().starts_with("sure"), "{out}");
        assert!(!out.contains("really"), "{out}");
        assert!(!out.contains("just"), "{out}");
    }

    #[test]
    fn never_drops_negation() {
        let in_ = "Do not delete the table. Never run drop. This is no joke.";
        let out = compress_prose(in_, CaveIntensity::Full);
        assert!(out.contains("not"), "{out}");
        assert!(out.contains("Never") || out.contains("never"), "{out}");
        assert!(out.contains("no"), "{out}");
    }

    #[test]
    fn lite_only_pleasantries() {
        let in_ = "Sure! The function really works.";
        let out = compress_prose(in_, CaveIntensity::Lite);
        assert!(!out.starts_with("Sure"), "{out}");
        assert!(out.contains("really"), "{out}");
        assert!(out.contains("The"), "{out}");
    }
}
