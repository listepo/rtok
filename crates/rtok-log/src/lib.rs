//! A rotating text log with no host attached.
//!
//! One file, a level floor, and a rename that is safe when two processes append at once.
//! See the crate README for the line shape and what this crate refuses to own.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in a day. The stamp splits a unix time into a date and a clock with this.
const SECS_PER_DAY: u64 = 86_400;
/// Days from the civil epoch (0000-03-01) to 1970-01-01. Howard Hinnant's `civil_from_days`.
const UNIX_TO_CIVIL_DAYS: i64 = 719_468;
/// Length of the 400-year cycle that algorithm counts in.
const DAYS_PER_ERA: i64 = 146_097;

/// Levels, most severe first. A line is written when it is at least as severe as the floor.
const LEVELS: [&str; 4] = ["error", "warn", "info", "debug"];

/// Where lines go, and how far they may grow. Borrowed: the caller already owns the config.
#[derive(Debug, Clone, Copy)]
pub struct FileLog<'a> {
    /// Live file. Rotated siblings are `<path>.1`, `<path>.2`, …
    pub path: &'a Path,
    /// Rotate once the next line would make the live file longer than this.
    pub max_bytes: u64,
    /// How many rotated siblings to keep. `0` truncates the live file and keeps none.
    pub files: u32,
    /// Floor. A line quieter than this is dropped before any I/O.
    pub level: &'a str,
}

/// Unix seconds, or `0` when the clock is before the epoch.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether `floor` lets `level` through. An unknown name ranks most severe.
pub fn enabled(floor: &str, level: &str) -> bool {
    rank(level) <= rank(floor)
}

/// `2026-09-09 15:04:05`, UTC.
pub fn stamp(secs: u64) -> String {
    let (days, rem) = (secs / SECS_PER_DAY, secs % SECS_PER_DAY);
    let z = days as i64 + UNIX_TO_CIVIL_DAYS;
    let era = z.div_euclid(DAYS_PER_ERA);
    let doe = z.rem_euclid(DAYS_PER_ERA);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = era * 400 + yoe + i64::from(m <= 2);
    let (h, min, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02}")
}

/// One line. Newlines in `message` become spaces so one event cannot look like several.
pub fn line(secs: u64, level: &str, source: &str, name: &str, message: &str) -> String {
    let message = message.replace(['\n', '\r'], " ");
    format!("{} {level} {source}/{name}: {message}", stamp(secs))
}

/// `errors.log` beside `log_path` (`~/.rtok/logs/rtok.log` → `~/.rtok/logs/errors.log`).
pub fn error_path(log_path: &Path) -> PathBuf {
    log_path.with_file_name("errors.log")
}

/// Append one line when `log.level` allows it. I/O errors are dropped: a log that cannot
/// be written is not something the caller can act on.
pub fn append(log: &FileLog<'_>, level: &str, source: &str, name: &str, message: &str) {
    append_split(log, None, level, source, name, message);
}

/// Same as [`append`], and a copy of every `error` line into `errors` (same bytes).
/// `warn`, `info` and `debug` stay in `main` only. An `error` is also written to `main`
/// when the floor allows it, which it always does: `error` is the most severe level.
pub fn append_split(
    main: &FileLog<'_>,
    errors: Option<&Path>,
    level: &str,
    source: &str,
    name: &str,
    message: &str,
) {
    let text = line(now(), level, source, name, message);
    if enabled(main.level, level) {
        let _ = write_line(main, &text);
    }
    if level.eq_ignore_ascii_case("error")
        && let Some(path) = errors
    {
        let err = FileLog {
            path,
            max_bytes: main.max_bytes,
            files: main.files,
            level: "error",
        };
        let _ = write_line(&err, &text);
    }
}

/// Append `text` as its own line, rotating first when it would pass `max_bytes`.
pub fn write_line(log: &FileLog<'_>, text: &str) -> io::Result<()> {
    if let Some(dir) = log.path.parent() {
        fs::create_dir_all(dir)?;
    }
    let incoming = text.len() as u64 + 1;
    if over(log, incoming) {
        rotate_if_over(log, incoming)?;
    }
    writeln!(open_append(log.path)?, "{text}")
}

/// `path` → `path.<i>`. The suffix sits after the extension, so the live name stays stable.
pub fn rotated_path(path: &Path, i: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{i}"));
    PathBuf::from(s)
}

fn rank(level: &str) -> usize {
    LEVELS
        .iter()
        .position(|l| l.eq_ignore_ascii_case(level))
        .unwrap_or(0)
}

fn open_append(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn over(log: &FileLog<'_>, incoming: u64) -> bool {
    let len = fs::metadata(log.path).map(|m| m.len()).unwrap_or(0);
    len > 0 && len + incoming > log.max_bytes
}

/// The size is checked again under an exclusive lock. Two processes that both saw a full
/// file must not both rename: the second would shift the first's fresh `.1` to `.2` and
/// then fail to rename a live file that is already gone. The lock is a sibling in the
/// temp dir, not the log file. Windows will not rename a file this process still holds
/// locked, even with delete sharing.
fn rotate_if_over(log: &FileLog<'_>, incoming: u64) -> io::Result<()> {
    let lock = open_rotate_lock(log.path)?;
    lock.lock()?;
    if over(log, incoming) {
        rotate(log.path, log.files)?;
    }
    Ok(())
}

fn open_rotate_lock(path: &Path) -> io::Result<fs::File> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let lock_path =
        std::env::temp_dir().join(format!("rtok-log-rotate-{:x}.lock", hasher.finish()));
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
}

fn rotate(path: &Path, files: u32) -> io::Result<()> {
    if files == 0 {
        return fs::write(path, b"");
    }
    let _ = fs::remove_file(rotated_path(path, files));
    for i in (1..files).rev() {
        let _ = fs::rename(rotated_path(path, i), rotated_path(path, i + 1));
    }
    fs::rename(path, rotated_path(path, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn log<'a>(path: &'a Path, max_bytes: u64, files: u32) -> FileLog<'a> {
        FileLog {
            path,
            max_bytes,
            files,
            level: "debug",
        }
    }

    #[test]
    fn a_known_epoch_reads_as_a_date() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_788_966_245), "2026-09-09 15:04:05");
        assert_eq!(stamp(1_709_209_845), "2024-02-29 12:30:45");
        assert_eq!(stamp(4_107_542_400), "2100-03-01 00:00:00");
    }

    #[test]
    fn the_level_is_a_floor_and_an_unknown_level_always_passes() {
        assert!(enabled("warn", "error") && enabled("warn", "warn"));
        assert!(!enabled("warn", "info") && !enabled("warn", "debug"));
        assert!(enabled("warn", "PANIC"));
    }

    #[test]
    fn a_rotation_decided_on_a_stale_size_is_dropped_under_the_lock() {
        let dir = tmp("rotate-race");
        let path = dir.join("app.log");
        let log = log(&path, 100, 3);
        fs::write(rotated_path(&path, 1), "history\n").unwrap();
        fs::write(&path, "fresh\n").unwrap();
        rotate_if_over(&log, 10).unwrap();
        assert_eq!(
            fs::read_to_string(rotated_path(&path, 1)).unwrap(),
            "history\n"
        );
        assert!(!rotated_path(&path, 2).exists());
        fs::write(&path, "x".repeat(99)).unwrap();
        rotate_if_over(&log, 10).unwrap();
        assert_eq!(
            fs::read_to_string(rotated_path(&path, 2)).unwrap(),
            "history\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_keeps_files_plus_the_current_one_and_the_newest_line_is_in_it() {
        let dir = tmp("rotate");
        let path = dir.join("app.log");
        let log = log(&path, 200, 2);
        for i in 0..50 {
            append(&log, "info", "test", "rotate", &format!("line {i}"));
        }
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 3, "{names:?}");
        for want in ["app.log", "app.log.1", "app.log.2"] {
            assert!(names.iter().any(|n| n == want), "{names:?}");
        }
        let live = fs::read_to_string(&path).unwrap();
        assert!(live.contains("line 49"), "{live}");
        assert!(
            live.len() <= 200,
            "the live file is bounded: {}",
            live.len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_message_with_newlines_stays_one_line() {
        let dir = tmp("newline");
        let path = dir.join("app.log");
        append(
            &log(&path, 1 << 20, 1),
            "warn",
            "test",
            "multi",
            "first\nsecond",
        );
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("first second"), "{text}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn errors_are_copied_and_warnings_stay_in_the_main_file() {
        let dir = tmp("split");
        let path = dir.join("app.log");
        let errors = error_path(&path);
        let log = log(&path, 1 << 20, 1);
        append_split(&log, Some(&errors), "warn", "app", "boot", "careful");
        append_split(&log, Some(&errors), "info", "app", "boot", "note");
        append_split(&log, Some(&errors), "error", "app", "boot", "broken");
        let main = fs::read_to_string(&path).unwrap();
        let err = fs::read_to_string(&errors).unwrap();
        assert!(
            main.contains("careful") && main.contains("note") && main.contains("broken"),
            "{main}"
        );
        assert!(err.contains("broken"), "{err}");
        assert!(!err.contains("careful") && !err.contains("note"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_line_below_the_level_touches_no_disk() {
        let dir = tmp("quiet");
        let path = dir.join("app.log");
        let log = FileLog {
            path: &path,
            max_bytes: 1 << 20,
            files: 1,
            level: "error",
        };
        append(&log, "debug", "test", "quiet", "nothing");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
