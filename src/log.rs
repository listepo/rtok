//! `[log]` — rtok's own log (plan T24.0, decision D26).
//!
//! One rotating text file: the thing an operator reads and `rtok logs` prints. The `logs` table
//! keeps the same lines as rows for `rtok otel`; T24.1 makes both come out of one funnel.

use crate::config::{Config, Log};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Levels, most severe first. A line is written when it is at least as severe as `[log] level`.
const LEVELS: [&str; 4] = ["error", "warn", "info", "debug"];

/// An unknown level ranks most severe: a line whose level we cannot read is the last one worth
/// dropping silently.
fn rank(level: &str) -> usize {
    LEVELS
        .iter()
        .position(|l| l.eq_ignore_ascii_case(level))
        .unwrap_or(0)
}

/// Whether `[log] level` lets this line through — checked before any I/O.
pub fn enabled(cfg: &Config, level: &str) -> bool {
    rank(level) <= rank(&cfg.log.level)
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `2026-09-09 15:04:05`, UTC. The store keeps unix seconds everywhere; a log line is the one
/// place in the binary that has to spell a date out, and std has no calendar.
pub fn stamp(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // civil_from_days: an era is the 400-year cycle starting 0000-03-01.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = era * 400 + yoe + i64::from(m <= 2);
    let (h, min, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02}")
}

/// One line: `<ts> <level> <source>/<name>: <message>`. Newlines in the message would make one
/// event look like several, so they become spaces — `rtok logs` counts lines.
pub fn line(secs: u64, level: &str, source: &str, name: &str, message: &str) -> String {
    let message = message.replace(['\n', '\r'], " ");
    format!("{} {level} {source}/{name}: {message}", stamp(secs))
}

/// Append one line, rotating first when it would take the file past `[log] max_bytes`.
///
/// Never fails upward: a log that cannot be written is not something the caller can act on, and a
/// hook must exit 0 in 10 ms whatever the disk is doing (D1).
pub fn append(cfg: &Config, level: &str, source: &str, name: &str, message: &str) {
    if !enabled(cfg, level) {
        return;
    }
    let _ = write_line(&cfg.log, &line(now(), level, source, name, message));
}

fn write_line(log: &Log, text: &str) -> std::io::Result<()> {
    if let Some(dir) = log.path.parent() {
        fs::create_dir_all(dir)?;
    }
    let len = fs::metadata(&log.path).map(|m| m.len()).unwrap_or(0);
    if len > 0 && len + text.len() as u64 + 1 > log.max_bytes {
        rotate(&log.path, log.files)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log.path)?;
    writeln!(f, "{text}")
}

/// `rtok.log` → `.1`, `.1` → `.2`, and whatever falls past `[log] files` is deleted. Keeping
/// everything is the disk-full bug D26 is about; nothing here is archived, because a log line is
/// not a saving and D2's lossless rule does not reach it.
fn rotate(path: &Path, files: u32) -> std::io::Result<()> {
    if files == 0 {
        return fs::write(path, b""); // no history wanted: start the file over
    }
    let _ = fs::remove_file(nth(path, files));
    for i in (1..files).rev() {
        let _ = fs::rename(nth(path, i), nth(path, i + 1));
    }
    fs::rename(path, nth(path, 1))
}

/// `rtok.log` → `rtok.log.<i>`; the suffix goes after the extension, so the current file keeps the
/// name every editor and `tail` already knows.
fn nth(path: &Path, i: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{i}"));
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn cfg_at(dir: &Path, max_bytes: u64, files: u32) -> Config {
        let mut cfg = Config::default();
        cfg.log.path = dir.join("rtok.log");
        cfg.log.max_bytes = max_bytes;
        cfg.log.files = files;
        cfg.log.level = "debug".into();
        cfg
    }

    #[test]
    fn a_known_epoch_reads_as_a_date() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        // `date -u -r <secs>` for each: an epoch in a leap year, one just after a leap day,
        // and one on a century that is not a leap year.
        assert_eq!(stamp(1_788_966_245), "2026-09-09 15:04:05");
        assert_eq!(stamp(1_709_209_845), "2024-02-29 12:30:45");
        assert_eq!(stamp(4_107_542_400), "2100-03-01 00:00:00");
    }

    #[test]
    fn the_level_is_a_floor_and_an_unknown_level_always_passes() {
        let mut cfg = Config::default();
        cfg.log.level = "warn".into();
        assert!(enabled(&cfg, "error") && enabled(&cfg, "warn"));
        assert!(!enabled(&cfg, "info") && !enabled(&cfg, "debug"));
        assert!(enabled(&cfg, "PANIC"), "an unreadable level is not dropped");
    }

    #[test]
    fn rotation_keeps_files_plus_the_current_one_and_the_newest_line_is_in_it() {
        let dir = tmp("rotate");
        let cfg = cfg_at(&dir, 200, 2);
        for i in 0..50 {
            append(&cfg, "info", "test", "rotate", &format!("line {i}"));
        }
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 3, "{names:?}");
        for want in ["rtok.log", "rtok.log.1", "rtok.log.2"] {
            assert!(names.iter().any(|n| n == want), "{names:?}");
        }
        let live = fs::read_to_string(dir.join("rtok.log")).unwrap();
        assert!(live.contains("line 49"), "{live}");
        assert!(
            live.len() <= 200,
            "the live file is bounded: {}",
            live.len()
        );
    }

    #[test]
    fn a_message_with_newlines_stays_one_line() {
        let dir = tmp("newline");
        let cfg = cfg_at(&dir, 1 << 20, 1);
        append(&cfg, "warn", "test", "multi", "first\nsecond");
        let text = fs::read_to_string(&cfg.log.path).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("first second"), "{text}");
    }

    #[test]
    fn a_line_below_the_level_touches_no_disk() {
        let dir = tmp("quiet");
        let mut cfg = cfg_at(&dir, 1 << 20, 1);
        cfg.log.level = "error".into();
        append(&cfg, "debug", "test", "quiet", "nothing");
        assert!(!cfg.log.path.exists());
    }
}
