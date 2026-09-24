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
