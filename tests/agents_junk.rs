// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T182: `rtok agents junk clear` through the binary — the CLI/JSON surface over a fixture
//! home with a database and a stray `rtok.log.<N>` past `[log] files`. The archive-retention
//! half of `scan`/`run` (referenced vs. orphan archive) is a `src/agents/junk.rs` unit test:
//! backdating a call's `ts` needs `Store::set_call_ts`, `#[cfg(test)]`-only and so reachable
//! only from inside the crate, never from this external test binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t182-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
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

#[test]
fn dry_run_lists_the_stale_log_and_yes_removes_it_without_touching_the_db() {
    let home = home("clear");
    let cfg = rtok::config::Config::load_from(&home).expect("config");
    let _store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    assert!(cfg.core.db_path.is_file());

    let stale_log = cfg.log.path.with_file_name(format!(
        "{}.{}",
        cfg.log.path.file_name().unwrap().to_str().unwrap(),
        cfg.log.files + 1
    ));
    fs::create_dir_all(stale_log.parent().unwrap()).unwrap();
    fs::write(&stale_log, b"stale").unwrap();

    let preview = rtok(&["agents", "junk", "clear", "--json"], &home);
    let rows: serde_json::Value = serde_json::from_str(&preview).unwrap();
    let paths: Vec<String> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(paths, vec![stale_log.display().to_string()]);
    assert!(stale_log.is_file(), "dry run must not delete anything");

    rtok(&["agents", "junk", "clear", "--yes", "--json"], &home);
    assert!(!stale_log.exists());
    assert!(
        cfg.core.db_path.is_file(),
        "the database itself is never touched"
    );
}

/// T330.1: `list` is read-only and prints exactly what the `clear` dry run plans.
#[test]
fn list_reports_the_stale_log_with_exact_bytes_and_removes_nothing() {
    let home = home("list");
    let cfg = rtok::config::Config::load_from(&home).expect("config");
    let stale_log = cfg.log.path.with_file_name(format!(
        "{}.{}",
        cfg.log.path.file_name().unwrap().to_str().unwrap(),
        cfg.log.files + 1
    ));
    fs::create_dir_all(stale_log.parent().unwrap()).unwrap();
    fs::write(&stale_log, b"stale").unwrap();

    let json = rtok(&["agents", "junk", "list", "--json"], &home);
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rtok_row = &report["agents"][0];
    assert_eq!(rtok_row["name"], "rtok");
    assert_eq!(rtok_row["kinds"][0]["kind"], "log");
    assert_eq!(rtok_row["kinds"][0]["size_bytes"], 5);
    assert_eq!(report["freed_default_bytes"], 5);

    let text = rtok(&["agents", "junk", "list", "--bytes"], &home);
    assert!(text.contains("Freed by `clear`: 5\n"), "{text}");
    assert!(stale_log.is_file(), "list must not delete anything");
}

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; bytes]).unwrap();
}

/// T330.2: `list` over a fixture HOME. Claude Code and Codex live where `CLAUDE_CONFIG_DIR` and
/// `CODEX_HOME` point, Cursor under its usual folders; every size equals `disk_usage`, Cursor's
/// folders are "not documented", and nothing is read outside the fixture.
#[test]
fn list_shows_each_installed_host_with_its_folders_and_exact_sizes() {
    let home = home("hosts");
    let claude = home.join("cc");
    let codex = home.join("cx");
    write(&claude.join("settings.json"), 10);
    write(&claude.join("debug/a.log"), 100);
    write(&codex.join("config.toml"), 10);
    write(&codex.join("log/codex.log"), 10);
    write(&home.join(".cursor/hooks.json"), 10);
    let cursor_data = home.join("Library/Application Support/Cursor");
    write(&cursor_data.join("state"), 10);
    // The hosts' own config paths still default to `~/.claude` and `~/.codex`.
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();

    let out = Command::new(bin())
        .args(["agents", "junk", "list", "--json"])
        .env("RTOK_HOME", &home)
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", &claude)
        .env("CODEX_HOME", &codex)
        .env("XDG_CACHE_HOME", home.join("xc"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let folder = |agent: &str, path: &Path| -> serde_json::Value {
        let row = report["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == agent)
            .unwrap_or_else(|| panic!("no `{agent}` row: {report}"));
        row["folders"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == path.display().to_string())
            .unwrap_or_else(|| panic!("`{agent}` has no {}: {row}", path.display()))
            .clone()
    };

    let c = folder("claude", &claude);
    assert_eq!(c["size_bytes"], rtok::agents::junk::disk_usage(&claude));
    assert_eq!(
        (&c["role"], &c["documented"]),
        (&"data".into(), &true.into())
    );
    assert_eq!(folder("claude", &claude.join("debug"))["role"], "logs");
    assert_eq!(folder("codex", &codex.join("log"))["role"], "logs");
    let app = folder("cursor", &cursor_data);
    assert_eq!(
        app["size_bytes"],
        rtok::agents::junk::disk_usage(&cursor_data)
    );
    assert_eq!(app["documented"], false);
    assert_eq!(folder("cursor", &home.join(".cursor"))["documented"], false);

    let text = rtok(&["agents", "junk", "list", "--bytes"], &home);
    assert!(text.contains("not documented: not cleared"), "{text}");
    assert!(
        !text.contains('\x1b'),
        "no escape codes into a pipe: {text}"
    );

    let all = rtok(&["agents", "junk", "list", "--all", "--json"], &home);
    let all: serde_json::Value = serde_json::from_str(&all).unwrap();
    assert!(
        all["agents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["host"] == true && a["installed"] == false),
        "--all lists a host that is not installed"
    );
}
