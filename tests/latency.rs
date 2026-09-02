//! T2.2: spawn `rtok hook PreToolUse` 200×; p95 < 10 ms (release).

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

const N: usize = 200;
const P95_MAX: Duration = Duration::from_millis(10);

#[test]
fn latency_hook_pre_tool_p95_under_10ms() {
    if cfg!(debug_assertions) {
        eprintln!("skip: T2.2 Check is `cargo test --release latency`");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_rtok");
    let fixture = include_bytes!("fixtures/hooks/pre_tool_read.json");
    let tmp = std::env::temp_dir().join(format!("rtok-latency-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("temp home");

    let spawn = || {
        let mut child = Command::new(bin)
            .args(["hook", "PreToolUse"])
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
    let p95 = samples[(N * 95) / 100];
    assert!(
        p95 < P95_MAX,
        "p95 {p95:?} is not < {P95_MAX:?} (min {:?}, max {:?})",
        samples[0],
        samples[N - 1]
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
