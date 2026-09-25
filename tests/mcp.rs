//! T4.1 Check: `tools/list` over stdio lists `expand`.
#![allow(unexpected_cfgs)]

mod common;

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

/// T263: `child.stdout` lines with a bounded wait, so a stalled exchange fails, not hangs.
struct LineReader {
    rx: std::sync::mpsc::Receiver<String>,
}

impl LineReader {
    fn new(stdout: std::process::ChildStdout) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::BufRead::read_line(&mut reader, &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if tx.send(line.trim_end().to_string()).is_err() => break,
                    Ok(_) => {}
                }
            }
        });
        Self { rx }
    }

    fn next_line(&self) -> String {
        self.rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("rtok mcp did not answer in time")
    }
}

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

/// T191: a malformed line answers `-32700` instead of silence, and the next valid
/// request on the same connection still answers.
#[test]
fn garbage_line_answers_parse_error_then_tools_list() {
    let tmp = std::env::temp_dir().join(format!("rtok-mcp-garbage-{}", std::process::id()));
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
        .write_all(
            br#"{bad
{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let first: serde_json::Value =
        serde_json::from_str(lines.next().unwrap_or_default()).expect("error line");
    assert_eq!(first["error"]["code"], -32700, "{stdout}");
    assert_eq!(first["id"], serde_json::Value::Null, "{stdout}");
    let rest: String = lines.collect::<Vec<_>>().join("\n");
    assert!(rest.contains("expand"), "{stdout}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// T75: another process holding the store's write lock at spawn time must not kill the
/// server. The startup retention used to run a deferred read-then-write transaction that
/// came back "database is locked" (instantly on SQLITE_BUSY_SNAPSHOT, which skips the
/// busy handler, or after the steady 1 s) and the `?` took the whole process down — the
/// client saw "Server disconnected".
#[test]
fn mcp_serves_while_another_process_holds_the_store_writer() {
    let tmp = std::env::temp_dir().join(format!("rtok-mcp-locked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let db = tmp.join("rtok.db");
    // Settle the store (migrations) and drop it: the binary's only startup write left
    // is the retention purge.
    drop(rtok::store::Store::open(&db).unwrap());
    // Hold the WAL writer lock for 2.5 s — past the steady 1 s busy timeout even
    // after the binary's own startup latency reaches the purge.
    let holder = common::hold_store_writer(&db, std::time::Duration::from_millis(2500));
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &tmp)
        .env("RTOK_CORE_DB_PATH", db.to_str().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        )
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    holder.join().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr {err}");
    assert!(!err.contains("database is locked"), "stderr {err}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rtok"), "no serverInfo in {stdout}");
    assert!(
        stdout.contains("expand"),
        "no tools/list result in {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The handshake must introduce this server as rtok: `ServerInfo::default()` filled
/// `serverInfo` from rmcp's own build env, so clients saw `{"name":"rmcp"}`.
#[test]
fn initialize_names_the_server_rtok() {
    let tmp = std::env::temp_dir().join(format!("rtok-mcp-init-{}", std::process::id()));
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
        .write_all(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        )
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().next().unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(line).expect("initialize response");
    assert_eq!(v["result"]["serverInfo"]["name"], "rtok", "{stdout}");
    assert!(
        v["result"]["serverInfo"]["version"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "{stdout}"
    );
    // T213: a client that requests a version this server supports gets that exact version
    // back (MCP lifecycle spec, "Initialization" — https://modelcontextprotocol.io/specification),
    // not whatever `ProtocolVersion::default()` happens to resolve to in the linked `rmcp`.
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18", "{stdout}");
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

/// T263: a client with the `roots` capability is asked `roots/list` after `initialized`;
/// the first `file://` root becomes the cwd, so `symbol` finds a repo the launch cwd is not.
#[test]
fn mcp_moves_into_the_first_file_root_after_roots_list() {
    let home = std::env::temp_dir().join(format!("rtok-mcp-roots-home-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("rtok-mcp-roots-out-{}", std::process::id()));
    let repo = std::env::temp_dir().join(format!("rtok-mcp-roots-repo-{}", std::process::id()));
    for d in [&home, &outside, &repo] {
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(repo.join("a.rs"), "fn alpha() {}\n").unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &home)
        .current_dir(&outside)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let lines = LineReader::new(child.stdout.take().expect("stdout"));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{"roots":{{"listChanged":true}}}},"clientInfo":{{"name":"t","version":"1"}}}}}}"#
    )
    .unwrap();
    let init: serde_json::Value =
        serde_json::from_str(&lines.next_line()).expect("initialize response");
    assert_eq!(init["result"]["serverInfo"]["name"], "rtok", "{init}");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    let roots_req: serde_json::Value =
        serde_json::from_str(&lines.next_line()).expect("roots/list request");
    assert_eq!(roots_req["method"], "roots/list", "{roots_req}");
    assert_eq!(roots_req["id"], "rtok-roots", "{roots_req}");

    let uri =
        url::Url::from_directory_path(repo.canonicalize().unwrap()).expect("file:// uri for repo");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":"rtok-roots","result":{{"roots":[{{"uri":"{uri}","name":"r"}}]}}}}"#
    )
    .unwrap();

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"symbol","arguments":{{"name":"alpha"}}}}}}"#
    )
    .unwrap();
    let call: serde_json::Value =
        serde_json::from_str(&lines.next_line()).expect("tools/call response");
    assert_eq!(call["result"]["isError"], false, "{call}");
    assert!(
        call["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("a.rs"),
        "{call}"
    );

    drop(stdin);
    let _ = child.wait();
    for d in [&home, &outside, &repo] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// T263: launched in `/` (Claude.app) with no roots, `symbol` refuses at once instead of
/// walking the disk.
#[test]
fn mcp_refuses_symbol_at_filesystem_root_without_walking() {
    let home = std::env::temp_dir().join(format!("rtok-mcp-noroot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", &home)
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let lines = LineReader::new(child.stdout.take().expect("stdout"));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"t","version":"1"}}}}}}"#
    )
    .unwrap();
    let _init = lines.next_line();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();

    let start = Instant::now();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"symbol","arguments":{{"name":"alpha"}}}}}}"#
    )
    .unwrap();
    let call: serde_json::Value =
        serde_json::from_str(&lines.next_line()).expect("tools/call response");
    let ms = start.elapsed().as_millis();
    assert!(ms < 5000, "symbol at / took {ms} ms: must refuse, not walk");
    assert_eq!(call["result"]["isError"], true, "{call}");
    assert!(
        call["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("no project root"),
        "{call}"
    );

    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&home);
}
