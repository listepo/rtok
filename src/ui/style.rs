// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! Emoji and colour for rtok's own human-facing lines (status, success, warning, error,
//! summaries, `--help`). One table, so every command marks a warning the same way.
//!
//! What agents read stays plain: the filtered command output, hook and MCP JSON, `--json`,
//! and anything written to a pipe or a file. Two rules keep it that way:
//! - an emoji needs `[ui] emoji` *and* a terminal on that stream;
//! - colour needs `[ui] color` *and* owo-colors' answer for that stream — a terminal,
//!   `NO_COLOR` unset, `TERM` not `dumb`, or `CLICOLOR_FORCE` / `FORCE_COLOR` set — the same
//!   check `render` uses for diffs and log levels.
//!
//! `[ui] color = false` turns colour off process-wide (owo-colors' override), so the diffs,
//! state words and log levels in [`crate::render`] follow the same key.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::builder::styling::{AnsiColor, Effects, Styles};
use owo_colors::{OwoColorize, Stream};

use crate::config::Ui;

/// `[ui] emoji`, as last loaded. On until a config says otherwise.
static EMOJI: AtomicBool = AtomicBool::new(true);

/// `--help` and clap's own errors. clap decides colour itself (terminal, `NO_COLOR`,
/// `CLICOLOR_FORCE`); the config is not loaded yet when it parses argv.
pub const CLAP: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());

/// What a line reports. Picks the emoji and the colour together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Success,
    Info,
    Warn,
    Error,
}

impl Kind {
    pub fn emoji(self) -> &'static str {
        match self {
            Kind::Success => "✅",
            Kind::Info => "💡",
            Kind::Warn => "⚠️",
            Kind::Error => "❌",
        }
    }
}

/// Apply `[ui]` for the rest of the process. Called by every config load.
pub fn configure(ui: &Ui) {
    EMOJI.store(ui.emoji, Ordering::Relaxed);
    if ui.color {
        owo_colors::unset_override();
    } else {
        owo_colors::set_override(false);
    }
}

/// The toggle rule, without the process state: an emoji needs the key and a terminal.
pub fn emoji_on(configured: bool, terminal: bool) -> bool {
    configured && terminal
}

fn terminal(stream: Stream) -> bool {
    match stream {
        Stream::Stdout => std::io::stdout().is_terminal(),
        Stream::Stderr => std::io::stderr().is_terminal(),
    }
}

/// `text` painted in `kind`'s colour where `stream` takes colour, else unchanged.
fn paint(kind: Kind, text: &str, stream: Stream) -> String {
    match kind {
        Kind::Success => text.if_supports_color(stream, |t| t.green()).to_string(),
        Kind::Info => text.if_supports_color(stream, |t| t.cyan()).to_string(),
        Kind::Warn => text.if_supports_color(stream, |t| t.yellow()).to_string(),
        Kind::Error => text.if_supports_color(stream, |t| t.red()).to_string(),
    }
}

/// `text` with `kind`'s emoji in front when `emoji` is set.
pub fn prefix(kind: Kind, text: &str, emoji: bool) -> String {
    if emoji {
        format!("{} {text}", kind.emoji())
    } else {
        text.to_string()
    }
}

/// One line for `stream`: emoji and colour where both are on, the bare text otherwise.
pub fn line(kind: Kind, stream: Stream, text: &str) -> String {
    let emoji = emoji_on(EMOJI.load(Ordering::Relaxed), terminal(stream));
    prefix(kind, &paint(kind, text, stream), emoji)
}

/// A success line for stdout (`ok …`, `… started`).
pub fn success(text: &str) -> String {
    line(Kind::Success, Stream::Stdout, text)
}

/// A neutral status line for stdout (`… not running`, `nothing to update`).
pub fn info(text: &str) -> String {
    line(Kind::Info, Stream::Stdout, text)
}

/// A warning line for stderr (`warning: …`).
pub fn warn(text: &str) -> String {
    line(Kind::Warn, Stream::Stderr, text)
}

/// An error line for stderr (`Error: …`).
pub fn error(text: &str) -> String {
    line(Kind::Error, Stream::Stderr, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(true, true, true)]
    #[case(true, false, false)]
    #[case(false, true, false)]
    #[case(false, false, false)]
    fn an_emoji_needs_the_key_and_a_terminal(
        #[case] key: bool,
        #[case] tty: bool,
        #[case] on: bool,
    ) {
        assert_eq!(emoji_on(key, tty), on);
    }

    #[test]
    fn the_prefix_is_one_emoji_and_a_space() {
        assert_eq!(prefix(Kind::Success, "ok x", true), "✅ ok x");
        assert_eq!(prefix(Kind::Warn, "warning: y", true), "⚠️ warning: y");
        assert_eq!(prefix(Kind::Error, "Error: z", false), "Error: z");
    }

    #[test]
    fn every_kind_has_its_own_emoji() {
        let all = [Kind::Success, Kind::Info, Kind::Warn, Kind::Error].map(Kind::emoji);
        let mut uniq = all.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), all.len());
    }

    /// The tests run without a terminal: every line is its bare text, byte for byte, which
    /// is what a pipe, a hook or a trycmd fixture sees.
    #[test]
    fn off_a_terminal_a_line_is_its_bare_text() {
        assert_eq!(success("ok a"), "ok a");
        assert_eq!(info("b not running"), "b not running");
        assert_eq!(warn("warning: c"), "warning: c");
        assert_eq!(error("Error: d"), "Error: d");
    }

    /// Colour is owo-colors' call per stream; `[ui] color = false` overrides it off even
    /// where it would paint, and `true` hands the decision back.
    #[test]
    fn the_color_key_overrides_a_forced_terminal() {
        owo_colors::with_override(true, || {
            assert_eq!(
                paint(Kind::Error, "x", Stream::Stderr),
                "\u{1b}[31mx\u{1b}[39m"
            );
        });
        configure(&Ui {
            emoji: false,
            color: false,
        });
        assert_eq!(paint(Kind::Error, "x", Stream::Stderr), "x");
        assert!(!EMOJI.load(Ordering::Relaxed));
        configure(&Ui::default());
        assert!(EMOJI.load(Ordering::Relaxed));
    }
}
