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

/// T201: guard's `post_tool` used to sha256 + write to disk synchronously on every cached
/// Read/Bash, and the hook's stdin read was unbounded — a multi-MB `PostToolUse` body paid
/// two full hashing passes plus disk I/O on top of the JSON parse. With the archive cap in
/// `src/plugins/guard/mod.rs` and the sha skip in `Store::spill`, only the (unavoidable) JSON
/// parse is left, so an in-process dispatch of a 5 MB body stays well under this looser bound.
/// Same debug skip as the p95 gates above: under `just check`'s fully parallel `nextest run`
/// every logical CPU is busy with other tests, and even this 50 ms budget is not safe from
/// that contention (measured 152 ms under full-suite load, ~1 ms in isolation) — run with
/// `cargo test --release --test latency` for a real measurement.
#[test]
fn hook_dispatches_a_5mb_post_tool_body_under_50ms() {
    if cfg!(debug_assertions) {
        eprintln!("skip: T201 Check is `cargo test --release --test latency`");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("rtok-latency-5mb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("temp home");
    let mut cfg = rtok::config::Config::default();
    cfg.core.db_path = tmp.join("rtok.db");
    cfg.core.archive_dir = tmp.join("archive");

    let raw = include_str!("fixtures/hooks/post_tool.json");
    let mut v: serde_json::Value = serde_json::from_str(raw).unwrap();
    v["tool_response"]["stdout"] = serde_json::Value::String("x".repeat(5 * 1024 * 1024));
    let stdin = serde_json::to_vec(&v).unwrap();

    let mut out = Vec::new();
    let start = std::time::Instant::now();
    rtok::hooks::run("PostToolUse", stdin.as_slice(), &mut out, &cfg);
    let took = start.elapsed();

    assert!(
        serde_json::from_slice::<serde_json::Value>(&out).is_ok(),
        "hook must print valid JSON"
    );
    assert!(
        took < Duration::from_millis(50),
        "5 MB PostToolUse dispatch took {took:?}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// T200: a second connection holding `BEGIN EXCLUSIVE` for 500 ms must not stall
/// the hook. `hooks::run` fails open through its few-ms lock bound and still
/// prints valid JSON, well under 100 ms.
///
/// T237: the proof that it gave up rather than waited is the ledger — a hook that
/// outwaited the holder would have written its `calls` row. The wall bound stays as
/// the second check; on Windows it is 250 ms (still half the hold): the 5 ms busy
/// handler sleeps 1 + 2 + 2 ms and each Windows `Sleep` rounds up to the 15.6 ms
/// timer tick, so the `windows-latest` debug run took 107 ms (ci run 35947867095).
#[test]
fn hook_returns_despite_exclusive_lock() {
    use diesel::Connection;
    use diesel::connection::SimpleConnection;

    let tmp = std::env::temp_dir().join(format!("rtok-latency-locked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("temp home");
    let db = tmp.join("rtok.db");
    let cfg = rtok::testutil::config_in(&tmp);

    // Warm the store once so the locked run exercises contention, not migration.
    let fixture = include_bytes!("fixtures/hooks/pre_tool_read.json");
    let mut warm = Vec::new();
    rtok::hooks::run("PreToolUse", &fixture[..], &mut warm, &cfg);
    assert!(
        serde_json::from_slice::<serde_json::Value>(&warm).is_ok(),
        "warmup hook must print valid JSON"
    );
    let calls = || {
        let store = rtok::store::Store::open(&db).unwrap();
        store.recent_calls(i64::MAX).unwrap().len()
    };
    let before = calls();
    assert!(before > 0, "the warm hook must record its call");

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
    assert_eq!(
        calls(),
        before,
        "the hook outwaited the exclusive lock instead of failing open ({took:?})"
    );
    let bound = if cfg!(windows) { 250 } else { 100 };
    assert!(
        took < std::time::Duration::from_millis(bound),
        "hook waited {took:?} under an exclusive lock"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
