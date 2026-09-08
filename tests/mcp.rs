//! T4.1 Check: `tools/list` over stdio lists `expand`.
#![allow(unexpected_cfgs)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

#[test]
fn tools_list_includes_expand() {
    let tmp = std::env::temp_dir().join(format!("rtok-mcp-list-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("expand"), "{stdout}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// T8.16: with the watcher on, `rtok mcp` exits promptly at stdin EOF.
#[test]
fn mcp_with_watcher_exits_on_stdin_eof() {
    let home = std::env::temp_dir().join(format!("rtok-mcp-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        "[plugins.graph]\nwatch = \"notify\"\n",
    )
    .unwrap();
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("a.rs"), "fn a() {}\n").unwrap();
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_rtok"))
            .arg("mcp")
            .env("RTOK_HOME", &home)
            .current_dir(&repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn rtok mcp")
    };
    // Warm-up: the first spawn on a machine pays dyld/Gatekeeper, not the watcher.
    let mut warm = spawn();
    drop(warm.stdin.take());
    let _ = warm.wait_with_output();
    let mut child = spawn();
    drop(child.stdin.take());
    let start = Instant::now();
    let out = child.wait_with_output().expect("wait");
    let ms = start.elapsed().as_millis();
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(ms < 500, "mcp with watcher took {ms} ms to exit at EOF");
    let _ = std::fs::remove_dir_all(&home);
}

/// T8.17: no watchman socket → one fallback line, then the notify watcher, then EOF exit.
#[test]
fn mcp_watchman_without_daemon_falls_back_once() {
    let home = std::env::temp_dir().join(format!("rtok-mcp-wman-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        "[plugins.graph]\nwatch = \"watchman\"\n",
    )
    .unwrap();
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("a.rs"), "fn a() {}\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .env_remove("WATCHMAN_SOCK")
        .current_dir(&repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait");
    let err = String::from_utf8_lossy(&out.stderr);
    let n = err
        .lines()
        .filter(|l| l.contains("falling back to notify"))
        .count();
    assert!(out.status.success(), "stderr {err}");
    assert_eq!(n, 1, "want one fallback line, stderr {err}");
    let _ = std::fs::remove_dir_all(&home);
}

/// T8.17 Gate P8d (3): a live watchman daemon registers the MCP cwd.
#[cfg(feature = "graph-watchman")]
#[test]
fn mcp_watchman_watch_list_names_the_root() {
    if Command::new("/opt/homebrew/bin/watchman")
        .arg("version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return;
    }
    let home = std::env::temp_dir().join(format!("rtok-mcp-wlist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        "[plugins.graph]\nwatch = \"watchman\"\n",
    )
    .unwrap();
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("a.rs"), "fn a() {}\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &home)
        .env(
            "PATH",
            format!(
                "/opt/homebrew/bin:{}",
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .current_dir(&repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    // The watchman connect + subscribe happens off the stdin loop; poll the
    // daemon (≤ 5 s) instead of asserting a fixed 400 ms sleep.
    let root = repo.canonicalize().unwrap();
    let mut listed = String::new();
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let list = Command::new("/opt/homebrew/bin/watchman")
            .arg("watch-list")
            .output()
            .expect("watch-list");
        listed = String::from_utf8_lossy(&list.stdout).into_owned();
        if listed.contains(&root.display().to_string()) {
            break;
        }
    }
    drop(child.stdin.take());
    let _ = child.wait_with_output();
    assert!(
        listed.contains(&root.display().to_string()),
        "watch-list missing {}: {listed}",
        root.display()
    );
    // Leave no root behind: the next run asserts its own root only.
    let _ = Command::new("/opt/homebrew/bin/watchman")
        .args(["watch-del", &root.display().to_string()])
        .output();
    let _ = std::fs::remove_dir_all(&home);
}
