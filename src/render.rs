//! Console rendering for the commands that change files (plan T12.6).
//!
//! Anything rtok would write to a file is shown as a unified diff — the same `--- a/… +++ b/…`,
//! `@@`, `+`/`-` shape `git diff` prints — so a `--dry-run` can be read without learning a
//! second format. Colour is added only when a person is looking at it.

use std::io::IsTerminal;
use std::path::Path;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const OFF: &str = "\x1b[0m";

/// Colour is on only for a real terminal with `NO_COLOR` unset. Pipes, CI logs and the
/// integration tests therefore all read the same plain text.
fn colour() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

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
    if !colour() {
        return text.to_string();
    }
    text.lines()
        .map(|line| {
            if line.starts_with("+++") || line.starts_with("---") {
                format!("{BOLD}{line}{OFF}")
            } else if line.starts_with('@') {
                format!("{CYAN}{line}{OFF}")
            } else if line.starts_with('+') {
                format!("{GREEN}{line}{OFF}")
            } else if line.starts_with('-') {
                format!("{RED}{line}{OFF}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tests run without a terminal, so `colour()` is false and every assertion below is
    // about the text itself. The colouring is one `match` over that same text.
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
    }
}
