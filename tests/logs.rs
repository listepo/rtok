//! T24.2: `rtok logs` and `rtok logs export` against the real binary and rotated files on disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t242-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("logs")).unwrap();
    dir
}

fn rtok(args: &[&str], home: &Path) -> String {
    let out = Command::new(bin())
        .args(args)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rtok {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// 4 lines per file, newest last within each file — the shape rotation leaves behind.
fn seed_rotated(home: &Path) {
    let log = home.join("logs/rtok.log");
    fs::write(&log, "live-1\nlive-2\nlive-3\nlive-4\n").unwrap();
    fs::write(log.with_extension("log.1"), "r1-1\nr1-2\nr1-3\nr1-4\n").unwrap();
    fs::write(log.with_extension("log.2"), "r2-1\nr2-2\nr2-3\nr2-4\n").unwrap();
    fs::write(log.with_extension("log.3"), "r3-1\nr3-2\nr3-3\nr3-4\n").unwrap();
}

const EXPECT: [&str; 10] = [
    "live-4", "live-3", "live-2", "live-1", "r1-4", "r1-3", "r1-2", "r1-1", "r2-4", "r2-3",
];

#[test]
fn logs_prints_newest_first_numbered_across_the_rotation_boundary() {
    let home = home("print");
    seed_rotated(&home);
    let out = rtok(&["logs", "--lines", "10"], &home);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 10, "{out}");
    // First line is the newest written; the tenth is ten lines back, past two file boundaries.
    for (i, want) in EXPECT.iter().enumerate() {
        assert_eq!(lines[i], format!("{} {want}", i + 1), "{out}");
    }
}

#[test]
fn export_is_the_same_lines_with_numbering_and_colour_stripped() {
    let home = home("export");
    seed_rotated(&home);
    let out = rtok(&["logs", "export", "--lines", "10"], &home);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, EXPECT.to_vec(), "{out}");
    // No terminal in a test run, so there is no ANSI to strip either: the numbered screen minus
    // its leading "<n> " is byte-identical to export, in both directions.
    let numbered = rtok(&["logs", "--lines", "10"], &home);
    for (n, e) in numbered.lines().zip(out.lines()) {
        let (_, rest) = n.split_once(' ').unwrap();
        assert_eq!(rest, e);
    }
}

#[test]
fn both_commands_say_so_when_nothing_has_been_logged() {
    let home = home("empty");
    assert_eq!(rtok(&["logs"], &home).trim(), "no logs yet");
    assert_eq!(rtok(&["logs", "export"], &home).trim(), "no logs yet");
}
