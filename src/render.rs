//! Console rendering for the commands that change files (plan T12.6, T20.2).
//!
//! Anything rtok would write to a file is shown as a unified diff — the same `--- a/… +++ b/…`,
//! `@@`, `+`/`-` shape `git diff` prints — so a `--dry-run` can be read without learning a
//! second format.
//!
//! Whether that diff is coloured is owo-colors' `if_supports_color` to decide, per stream. It
//! answers the whole question rtok used to answer by hand with one `isatty` and one `NO_COLOR`
//! lookup: a tty, `NO_COLOR`, `CLICOLOR` / `CLICOLOR_FORCE`, and `TERM=dumb`. Piping any of
//! these commands into a file or a CI log still yields plain text.

use std::path::Path;

use owo_colors::{OwoColorize, Stream};

/// A `git diff` of one file, three lines of context, coloured. Empty when nothing differs.
pub fn file_diff(path: &Path, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let text = similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("a/{}", path.display()),
            &format!("b/{}", path.display()),
        )
        .to_string();
    paint(&text)
}

/// Colour diff-shaped text: green additions, red removals, cyan hunk headers, bold file headers.
/// Lines that carry no marker — the installers' own `7 additions`, `no changes` — pass through.
pub fn paint(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("+++") || line.starts_with("---") {
                line.if_supports_color(Stream::Stdout, |t| t.bold())
                    .to_string()
            } else if line.starts_with('@') {
                line.if_supports_color(Stream::Stdout, |t| t.cyan())
                    .to_string()
            } else if line.starts_with('+') {
                line.if_supports_color(Stream::Stdout, |t| t.green())
                    .to_string()
            } else if line.starts_with('-') {
                line.if_supports_color(Stream::Stdout, |t| t.red())
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A spinner for a walk with no known length. indicatif draws to stderr and draws nothing at
/// all when stderr is not a terminal, so a piped or redirected run stays byte-clean.
pub fn spinner(what: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    if let Ok(style) =
        indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg} {pos} files · {elapsed}")
    {
        pb.set_style(style);
    }
    pb.set_message(what.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb
}

/// Colour a stored log line's level (`<date> <time> <level> <source>/<name>: <message>`, T24.0's
/// `log::line`): red error, yellow warn, dim debug, info plain. `rtok logs export` prints the same
/// line through no such call, so piping stays byte-plain.
pub fn log_line(text: &str) -> String {
    let mut parts = text.splitn(4, ' ');
    let (Some(date), Some(time), Some(level), Some(rest)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return text.to_string();
    };
    let level = match level {
        "error" => level
            .if_supports_color(Stream::Stdout, |t| t.red())
            .to_string(),
        "warn" => level
            .if_supports_color(Stream::Stdout, |t| t.yellow())
            .to_string(),
        "debug" => level
            .if_supports_color(Stream::Stdout, |t| t.dimmed())
            .to_string(),
        _ => level.to_string(),
    };
    format!("{date} {time} {level} {rest}")
}

/// A state word for a status table: green when the thing is up, red when it is not.
pub fn state(word: &str, ok: bool) -> String {
    if ok {
        word.if_supports_color(Stream::Stdout, |t| t.green())
            .to_string()
    } else {
        word.if_supports_color(Stream::Stdout, |t| t.red())
            .to_string()
    }
}

/// The `rtok plugins` listing: id, on/off, comma-joined surfaces. One formatter for both
/// callers (T15.11): the CLI renders the operator model's Plugins page with it, and
/// `Registry::table` renders manifests with it — so the two cannot drift.
pub fn plugins_table(rows: &[(&str, bool, Vec<&str>)]) -> String {
    let mut out = format!("{:<9}{:<9}surfaces\n", "id", "enabled");
    for (id, on, surfaces) in rows {
        out.push_str(&format!(
            "{:<9}{:<9}{}\n",
            id,
            if *on { "on" } else { "off" },
            surfaces.join(",")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tests run without a terminal, so `if_supports_color` yields the text unchanged and
    // every assertion below is about that text. The colouring is one `match` over the same.
    #[test]
    fn one_changed_line_reads_as_a_git_diff() {
        let out = file_diff(Path::new("config.toml"), "a = 1\nb = 2\n", "a = 1\nb = 3\n");
        assert!(out.contains("--- a/config.toml"), "{out}");
        assert!(out.contains("+++ b/config.toml"), "{out}");
        assert!(out.contains("-b = 2"), "{out}");
        assert!(out.contains("+b = 3"), "{out}");
        assert!(out.contains(" a = 1"), "context line kept: {out}");
    }

    #[test]
    fn equal_input_is_no_diff_at_all() {
        assert!(file_diff(Path::new("x"), "same\n", "same\n").is_empty());
    }

    #[test]
    fn plain_text_passes_through_when_colour_is_off() {
        assert_eq!(
            paint("+ added\n- gone\nno changes"),
            "+ added\n- gone\nno changes"
        );
        assert_eq!(state("running", true), "running");
    }

    #[test]
    fn a_log_line_keeps_its_shape_with_colour_off() {
        let line = "2026-09-09 15:04:05 warn test/tail: disk almost full";
        assert_eq!(log_line(line), line);
        assert_eq!(log_line("not a log line"), "not a log line");
    }
}
