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
}
