//! T2.2: spawn `rtok hook PreToolUse` 200×; p95 < 10 ms (release).
//! Gate P17 asks the same of `PostToolUse`; both print p50/p95/max under `--nocapture`.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

mod common;

const N: usize = 200;
const P95_MAX: Duration = Duration::from_millis(10);

fn p95_under_10ms(event: &str, fixture: &[u8]) {
    if cfg!(debug_assertions) {
        eprintln!("skip: T2.2 Check is `cargo test --release latency`");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_rtok");
    let tmp = std::env::temp_dir().join(format!("rtok-latency-{event}-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("temp home");

    let spawn = || {
        let mut child = Command::new(bin)
            .args(["hook", event])
            .env("RTOK_HOME", &tmp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn rtok");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(fixture)
            .expect("write fixture");
        child.wait_with_output().expect("wait")
    };
    let warm = spawn();
    assert!(warm.status.success(), "warmup hook must exit 0");

    let mut samples = Vec::with_capacity(N);
    for _ in 0..N {
        let start = std::time::Instant::now();
        let out = spawn();
        samples.push(start.elapsed());
        assert!(out.status.success(), "hook must fail open with exit 0");
        assert_eq!(out.stdout, b"{}");
    }

    samples.sort();
    let p95 = common::p95(&samples);
    eprintln!(
        "{event}: n={N} p50 {:?} p95 {p95:?} max {:?}",
        samples[N / 2],
        samples[N - 1]
    );
    assert!(
        p95 < P95_MAX,
        "{event} p95 {p95:?} is not < {P95_MAX:?} (min {:?}, max {:?})",
        samples[0],
        samples[N - 1]
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn latency_hook_pre_tool_p95_under_10ms() {
    p95_under_10ms(
        "PreToolUse",
        include_bytes!("fixtures/hooks/pre_tool_read.json"),
    );
}

#[test]
fn latency_hook_post_tool_p95_under_10ms() {
    p95_under_10ms(
        "PostToolUse",
        include_bytes!("fixtures/hooks/post_tool.json"),
    );
}

/// T200: a second connection holding `BEGIN EXCLUSIVE` for 500 ms must not stall
/// the hook. `hooks::run` fails open through its few-ms lock bound and still
/// prints valid JSON, well under 100 ms.
#[test]
fn hook_returns_despite_exclusive_lock() {
    use diesel::Connection;
    use diesel::connection::SimpleConnection;

    let tmp = std::env::temp_dir().join(format!("rtok-latency-locked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("temp home");
    let db = tmp.join("rtok.db");
    let mut cfg = rtok::config::Config::default();
    cfg.core.db_path = db.clone();
    cfg.core.archive_dir = tmp.join("archive");

    // Warm the store once so the locked run exercises contention, not migration.
    let fixture = include_bytes!("fixtures/hooks/pre_tool_read.json");
    let mut warm = Vec::new();
    rtok::hooks::run("PreToolUse", &fixture[..], &mut warm, &cfg);
    assert!(
        serde_json::from_slice::<serde_json::Value>(&warm).is_ok(),
        "warmup hook must print valid JSON"
    );

    let (held, held_ack) = std::sync::mpsc::channel();
    let url = db.to_str().unwrap().to_string();
    let holder = std::thread::spawn(move || {
        let mut conn = diesel::sqlite::SqliteConnection::establish(&url).unwrap();
        conn.batch_execute("PRAGMA busy_timeout = 1000; PRAGMA journal_mode = WAL;")
            .unwrap();
        conn.batch_execute("BEGIN EXCLUSIVE;").unwrap();
        held.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        conn.batch_execute("COMMIT;").unwrap();
    });
    held_ack.recv().unwrap();

    let start = std::time::Instant::now();
    let mut out = Vec::new();
    rtok::hooks::run("PreToolUse", &fixture[..], &mut out, &cfg);
    let took = start.elapsed();
    holder.join().unwrap();

    let v: serde_json::Value =
        serde_json::from_slice(&out).expect("locked hook must print valid JSON");
    assert!(v.is_object(), "{v}");
    assert!(
        took < std::time::Duration::from_millis(100),
        "hook waited {took:?} under an exclusive lock"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
