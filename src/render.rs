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

use crate::store::SessionTotals;

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

/// One column of a [`table`]: a width floor and whether cells align right. The floor is
/// how a hand-rolled table moves over without changing a byte — its old fixed width
/// becomes the floor and the column simply never grows past it.
pub struct Col {
    /// Never narrower than this, whatever the cells hold.
    pub min: usize,
    /// Numbers right-align; text left-aligns.
    pub right: bool,
}

impl Col {
    pub fn left(min: usize) -> Self {
        Self { min, right: false }
    }

    pub fn right(min: usize) -> Self {
        Self { min, right: true }
    }
}

/// The one padded-column table (T25.2): every column as wide as its widest cell and never
/// narrower than its floor, columns separated by one space, one line per row — the first
/// row is the header and widens columns like any other. `stats`' api and tool sections and
/// `stats --cache` render through it with their old widths as floors (byte-identical to
/// the `format!`s they had), and `agent sessions` passes small floors so the columns fit
/// their content. A left-aligned last column would keep its trailing pad, so end tables
/// on a right-aligned column, as every caller here does.
pub fn table(cols: &[Col], rows: &[Vec<String>]) -> String {
    let width = |j: usize| {
        let mut w = cols[j].min;
        for row in rows {
            if let Some(cell) = row.get(j) {
                w = w.max(cell.chars().count());
            }
        }
        w
    };
    let widths: Vec<usize> = (0..cols.len()).map(width).collect();
    let mut out = String::new();
    for row in rows {
        let line = cols
            .iter()
            .enumerate()
            .map(|(j, col)| {
                let cell = row.get(j).map(String::as_str).unwrap_or("");
                let pad = " ".repeat(widths[j].saturating_sub(cell.chars().count()));
                if col.right {
                    format!("{pad}{cell}")
                } else {
                    format!("{cell}{pad}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// `45s`, `5m03s`, `2h05m`, `3d04h` — how long a session has run (T25.2). Two units at
/// most, so the column stays narrow while the magnitude stays readable at a glance.
pub fn duration(secs: i64) -> String {
    let secs = secs.max(0) as u64; // clock skew is not a negative runtime
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else if secs < 86_400 {
        format!("{}h{:02}m", secs / 3_600, secs % 3_600 / 60)
    } else {
        format!("{}d{:02}h", secs / 86_400, secs % 86_400 / 3_600)
    }
}

/// The `rtok agent sessions` table (T25.2): the operator model's Sessions page as console
/// rows, newest first (the query's order). Live sessions only unless `all`; the run column
/// is `now - started` while live and `ended - started` once ended, with `now` passed in so
/// the rendering is testable. Dates reuse [`crate::log::stamp`] — the one calendar in the
/// binary — and both cache counts are shown, labelled, because "cache" is two numbers.
pub fn sessions_table(rows: &[SessionTotals], all: bool, now: i64) -> String {
    let cols = [
        Col::left(0),
        Col::left(0),
        Col::left(0),
        Col::right(0),
        Col::right(0),
        Col::right(0),
        Col::right(0),
        Col::left(0),
        Col::right(0),
    ];
    let header = [
        "agent",
        "provider",
        "model",
        "input",
        "output",
        "cache_read",
        "cache_create",
        "started",
        "run",
    ]
    .map(String::from)
    .to_vec();
    let shown: Vec<SessionTotals> = rows
        .iter()
        .filter(|r| all || r.ended_at.is_none())
        .cloned()
        .collect();
    let mut body = vec![header];
    body.extend(shown.iter().map(|r| {
        vec![
            r.host.clone().unwrap_or_else(|| "-".into()),
            r.provider
                .clone()
                .or_else(|| r.api.clone())
                .unwrap_or_else(|| "-".into()),
            r.model.clone().unwrap_or_else(|| "-".into()),
            r.input.to_string(),
            r.output.to_string(),
            r.cache_read.to_string(),
            r.cache_create.to_string(),
            crate::log::stamp(r.started_at.max(0) as u64),
            duration(match r.ended_at {
                Some(end) => end - r.started_at,
                None => now - r.started_at,
            }),
        ]
    }));
    let mut out = table(&cols, &body);
    if shown.is_empty() {
        // No row survived the liveness filter: header plus a line that says so.
        out.push_str(if all {
            "no sessions\n"
        } else {
            "nothing is running\n"
        });
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

    /// Widths are computed over header and cells together, with a per-column floor; a
    /// floor above every cell is the old hand-rolled fixed width, byte for byte.
    #[test]
    fn a_table_pads_to_its_widest_cell_or_its_floor() {
        let cols = [Col::left(24), Col::right(8), Col::right(5)];
        let rows = vec![
            vec!["api".into(), "input".into(), "hit".into()],
            vec!["anthropic".into(), "10".into(), "15.4%".into()],
        ];
        assert_eq!(
            table(&cols, &rows),
            "api                         input   hit\n\
             anthropic                      10 15.4%\n"
        );
        // A cell wider than every floor widens the column instead of overrunning it.
        let rows = vec![
            vec!["api".into(), "input".into(), "hit".into()],
            vec![
                "a-very-long-api-name".into(),
                "123456789".into(),
                "5%".into(),
            ],
        ];
        assert_eq!(
            table(&cols, &rows),
            "api                          input   hit\n\
             a-very-long-api-name     123456789    5%\n"
        );
    }

    #[test]
    fn durations_use_two_units_at_most() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(65), "1m05s");
        assert_eq!(duration(3_661), "1h01m");
        assert_eq!(duration(86_400 + 3_600), "1d01h");
        assert_eq!(duration(-5), "0s", "clock skew is not a negative runtime");
    }

    fn totals(id: &str, host: Option<&str>, ended: Option<i64>, started: i64) -> SessionTotals {
        SessionTotals {
            id: id.into(),
            host: host.map(String::from),
            project: Some("rtok".into()),
            provider: Some("anthropic".into()),
            api: Some("anthropic".into()),
            model: Some("claude-x".into()),
            input: 30,
            cache_create: 1,
            cache_read: 7,
            output: 7,
            started_at: started,
            last_activity: started + 100,
            ended_at: ended,
        }
    }

    /// T25.2's Check, rendered: live rows by default, ended ones with `--all`, run is
    /// now − started while live and ended − started once ended, and an empty page is a
    /// header plus a line saying nothing is running.
    #[test]
    fn a_sessions_page_renders_live_rows_and_durations() {
        let rows = vec![
            totals("live", Some("claude"), None, 1_788_966_245),
            totals(
                "gone",
                Some("pi"),
                Some(1_788_900_000 + 7_265),
                1_788_900_000,
            ),
        ];
        // 15:04:05 minus 65s of live runtime; the ended one ran 2h01m (two units max).
        let now = 1_788_966_245 + 65;
        let live = sessions_table(&rows, false, now);
        assert!(live.starts_with("agent"), "header first: {live}");
        assert_eq!(live.lines().count(), 2, "header plus one live row: {live}");
        assert!(
            live.contains("claude") && live.contains("claude-x"),
            "{live}"
        );
        assert!(
            live.contains("2026-09-09 15:04:05"),
            "started is a date: {live}"
        );
        assert!(live.contains("1m05s"), "live run is now - started: {live}");
        // Host column: the ended row's host must not appear without --all.
        assert!(
            !live.lines().any(|l| l.starts_with("pi")),
            "the ended row is hidden without --all: {live}"
        );
        let all = sessions_table(&rows, true, now);
        assert_eq!(all.lines().count(), 3, "--all adds the ended row: {all}");
        assert!(all.lines().any(|l| l.starts_with("pi")), "{all}");
        assert!(
            all.contains(" 2h01m"),
            "ended run is ended - started: {all}"
        );
    }

    #[test]
    fn an_empty_sessions_page_says_nothing_is_running() {
        let out = sessions_table(&[], false, 0);
        assert_eq!(out.lines().count(), 2, "header plus the line: {out}");
        assert!(out.starts_with("agent"), "header first: {out}");
        assert!(out.ends_with("nothing is running\n"), "{out}");
        assert_eq!(
            sessions_table(&[], true, 0),
            format!(
                "{}\nno sessions\n",
                sessions_table(&[], false, 0).lines().next().unwrap()
            )
        );
    }
}
