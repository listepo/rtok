//! T16.5: the exporter against a mock collector — every row once, never twice, marks only on
//! 2xx, and the CLI does nothing without an endpoint.

use std::path::{Path, PathBuf};
use std::process::Command;

use httpmock::prelude::*;
use rtok::config::Config;
use rtok::otel::export::flush_blocking;
use rtok::plugin::{Ctx, Measurement};

fn home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-otel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ctx(dir: &Path, endpoint: &str) -> Ctx {
    let mut cfg = Config::default();
    cfg.core.db_path = dir.join("rtok.db");
    cfg.core.archive_dir = dir.join("archive");
    cfg.otel.endpoint = endpoint.into();
    cfg.otel.headers = "x-key=k".into();
    cfg.otel.flush_secs = 2;
    let cx = Ctx::open(cfg, "s1").unwrap();
    cx.store
        .upsert_session("s1", None, Some("p"), Some("/w"), Some("startup"))
        .unwrap();
    cx
}

/// Three calls (hook, mcp, proxy) with io, one measurement, one log row.
fn seed(cx: &Ctx) {
    let s = &cx.store;
    let hook = s
        .insert_call(
            "s1",
            "hook",
            "hook",
            None,
            None,
            None,
            None,
            Some("PostToolUse"),
        )
        .unwrap();
    let req = br#"{"hook_event_name":"PostToolUse","tool_name":"Read","tool_use_id":"toolu_1","tool_input":{"file_path":"/x"},"tool_response":"body"}"#;
    s.insert_call_io(hook, Some(req), None, 65536, None)
        .unwrap();
    let mcp = s
        .insert_call(
            "s1",
            "mcp",
            "tool_call",
            None,
            None,
            None,
            Some("graph"),
            Some("symbol"),
        )
        .unwrap();
    s.insert_call_io(
        mcp,
        Some(br#"{"name":"b"}"#),
        Some(b"chain.rs:4"),
        65536,
        None,
    )
    .unwrap();
    let (pid, mid) = s.upsert_model("anthropic", "claude-x").unwrap();
    let chat = s
        .insert_call(
            "s1",
            "proxy",
            "api_request",
            None,
            Some(pid),
            Some(mid),
            None,
            Some("/v1/messages"),
        )
        .unwrap();
    s.insert_usage("s1", Some("claude-x"), "anthropic", 100, 10, 20, 30, chat)
        .unwrap();
    s.insert_measurement(
        "s1",
        &Measurement {
            plugin: "proxy",
            kind: "raw",
            before_bytes: 10,
            after_bytes: 5,
            est_before: 3,
            est_after: 1,
            ref_id: None,
            call_id: Some(chat),
        },
    )
    .unwrap();
    s.insert_log(
        "info",
        "hook",
        "dispatch",
        "hello",
        Some("s1"),
        Some(hook),
        None,
    )
    .unwrap();
}

#[test]
fn every_row_posts_once_and_the_marks_advance() {
    let server = MockServer::start();
    let traces = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/traces")
            .header("content-type", "application/json")
            .header("x-key", "k")
            .body_includes("execute_tool Read")
            .body_includes("execute_tool symbol")
            .body_includes("chat claude-x")
            .body_includes("gen_ai.usage.cache_read.input_tokens")
            .body_includes("rtok.measurement");
        then.status(200).body("{}");
    });
    let logs = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/logs")
            .body_includes(r#""stringValue":"hello""#);
        then.status(200).body("{}");
    });
    // Metrics are whole-table sums: posted on every flush, with no watermark (T16.7).
    let sums = server.mock(|when, then| {
        when.method(POST).path("/v1/metrics");
        then.status(200).body("{}");
    });
    let dir = home("ok");
    let cx = ctx(&dir, &server.base_url());
    seed(&cx);
    let r = flush_blocking(&cx);
    assert_eq!(r.error, None, "{r}");
    assert_eq!((r.enabled, r.spans, r.logs, r.posted), (true, 3, 1, 3));
    traces.assert_calls(1);
    logs.assert_calls(1);
    assert_eq!(cx.store.otel_mark("calls").unwrap(), 3);
    assert_eq!(cx.store.otel_mark("logs").unwrap(), 1);
    let r = flush_blocking(&cx);
    assert_eq!((r.spans, r.logs, r.posted), (0, 0, 1));
    traces.assert_calls(1);
    logs.assert_calls(1);
    sums.assert_calls(2);
    // An ended session ships its root span; a second flush does not repeat it.
    cx.store.end_session("s1", 1_700_000_000).unwrap();
    let root = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/traces")
            .body_includes("invoke_agent");
        then.status(200).body("{}");
    });
    assert_eq!(flush_blocking(&cx).spans, 1);
    assert_eq!(flush_blocking(&cx).spans, 0);
    root.assert_calls(1);
    assert_eq!(cx.store.otel_mark("sessions").unwrap(), 1_700_000_000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_keeps_the_marks_and_is_logged() {
    let server = MockServer::start();
    let traces = server.mock(|when, then| {
        when.method(POST).path("/v1/traces");
        then.status(500).body("nope");
    });
    let dir = home("fail");
    let cx = ctx(&dir, &server.base_url());
    seed(&cx);
    let r = flush_blocking(&cx);
    let err = r.error.clone().expect("a 500 is an error");
    assert!(err.contains("/v1/traces: HTTP 500"), "{err}");
    assert_eq!(r.posted, 0);
    traces.assert_calls(1);
    assert_eq!(cx.store.otel_mark("calls").unwrap(), 0);
    assert_eq!(cx.store.otel_mark("logs").unwrap(), 0);
    let last = cx.store.last_log("otel").unwrap().expect("failure logged");
    assert_eq!(
        (last.level.as_str(), last.name.as_str()),
        ("error", "flush")
    );
    let (calls, logs) = cx.store.otel_pending().unwrap();
    assert_eq!((calls, logs), (3, 2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_without_an_endpoint_does_nothing() {
    let dir = home("cli");
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_rtok"))
            .args(args)
            .env("RTOK_HOME", &dir)
            .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT")
            .env_remove("OTEL_EXPORTER_OTLP_HEADERS")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(run(&["otel", "flush"]).trim(), "otel: no endpoint");
    let status = run(&["otel", "status"]);
    assert!(status.starts_with("endpoint: none"), "{status}");
    assert!(status.contains("calls: mark 0 · 0 pending"), "{status}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// T16.6: `Stop` hands the flush to a child and returns; the trace lands without the hook
/// waiting for it, and the hook stays fast when the endpoint is unreachable.
#[test]
fn stop_hook_spawns_the_flush_and_stays_under_10ms() {
    let server = MockServer::start();
    let traces = server.mock(|when, then| {
        when.method(POST).path("/v1/traces");
        then.status(200).body("{}");
    });
    let dir = home("stop");
    // A config file the child will read, plus rows worth posting.
    {
        let cx = ctx(&dir, &server.base_url());
        seed(&cx);
    }
    let mut cfg = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    cfg.push_str(&format!(
        "\n[otel]\nendpoint = \"{}\"\nflush_secs = 2\n",
        server.base_url()
    ));
    std::fs::write(dir.join("config.toml"), cfg).unwrap();

    let run = |event: &str, payload: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
            .args(["hook", event])
            .env("RTOK_HOME", &dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let start = std::time::Instant::now();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        start.elapsed()
    };
    run(
        "Stop",
        r#"{"session_id":"s1","hook_event_name":"Stop","reason":"end_turn"}"#,
    );
    // The child posts on its own; the hook did not wait for it.
    for _ in 0..40 {
        if traces.calls() > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(traces.calls() >= 1, "the spawned child posted the trace");

    // SessionEnd closes the session so the root span can ship.
    run(
        "SessionEnd",
        r#"{"session_id":"s1","hook_event_name":"SessionEnd","reason":"clear"}"#,
    );
    let cx = ctx(&dir, &server.base_url());
    let ended = cx
        .store
        .sessions_ended_after(1)
        .unwrap()
        .into_iter()
        .any(|s| s.id == "s1");
    assert!(ended, "SessionEnd set ended_at");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The hook path must not pay for an endpoint that never answers.
#[test]
fn hooks_stay_fast_with_an_unreachable_endpoint() {
    let dir = home("slow");
    let mut cfg = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    cfg.push_str("\n[otel]\nendpoint = \"http://127.0.0.1:9\"\nflush_secs = 5\n");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), cfg).unwrap();
    let payload = r#"{"session_id":"s1","hook_event_name":"Stop","reason":"end_turn"}"#;
    let once = || {
        let start = std::time::Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
            .args(["hook", "Stop"])
            .env("RTOK_HOME", &dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "hooks fail open");
        assert_eq!(out.stdout, b"{}");
        start.elapsed()
    };
    once();
    let n = if cfg!(debug_assertions) { 20 } else { 100 };
    let mut samples: Vec<_> = (0..n).map(|_| once()).collect();
    samples.sort();
    let p95 = samples[(n * 95) / 100];
    let bar = if cfg!(debug_assertions) {
        std::time::Duration::from_millis(200)
    } else {
        std::time::Duration::from_millis(10)
    };
    assert!(
        p95 < bar,
        "p95 {p95:?} not under {bar:?} (max {:?})",
        samples[n - 1]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// T16.7: the sums are whole-table aggregates — no watermark, repeated every flush.
#[test]
fn metrics_repeat_the_totals_every_flush() {
    let server = MockServer::start();
    let metrics = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/metrics")
            .body_includes("rtok.tokens")
            .body_includes("rtok.tokens.saved")
            .body_includes("rtok.calls")
            .body_includes(r#""isMonotonic":true"#)
            .body_includes(r#""aggregationTemporality":2"#)
            .body_includes(r#""asInt":"100""#)
            .body_includes(r#""asInt":"2""#);
        then.status(200).body("{}");
    });
    server.mock(|when, then| {
        when.method(POST).path("/v1/traces");
        then.status(200).body("{}");
    });
    server.mock(|when, then| {
        when.method(POST).path("/v1/logs");
        then.status(200).body("{}");
    });
    let dir = home("metrics");
    let cx = ctx(&dir, &server.base_url());
    seed(&cx);
    let r = flush_blocking(&cx);
    assert_eq!(r.error, None, "{r}");
    // 4 token types × 1 model, 1 saved row, 3 call rows.
    assert_eq!(r.points, 8);
    metrics.assert_calls(1);
    // No watermark: a second flush posts the same totals again.
    let r = flush_blocking(&cx);
    assert_eq!((r.spans, r.logs, r.points), (0, 0, 8));
    metrics.assert_calls(2);
    let _ = std::fs::remove_dir_all(&dir);
}
