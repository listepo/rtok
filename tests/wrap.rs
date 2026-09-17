//! T51.4: `rtok wrap -- <cmd>` runs one command with both base URLs on the proxy.

use std::process::{Command, Output};
use std::time::Duration;

use httpmock::prelude::*;

const FIXTURE: &[u8] = include_bytes!("fixtures/proxy/anthropic_messages_body.json");

struct Home(std::path::PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(n: &str) -> Home {
    let t = std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("rtok-wrap-{n}-{}-{t}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Home(d)
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port")
        .local_addr()
        .expect("addr")
        .port()
}

/// The fake agent: echoes the two base URLs on stdout, nothing else.
fn echo_agent() -> Vec<String> {
    if cfg!(windows) {
        vec![
            "cmd".into(),
            "/c".into(),
            "echo %ANTHROPIC_BASE_URL%^|%OPENAI_BASE_URL%".into(),
        ]
    } else {
        vec![
            "sh".into(),
            "-c".into(),
            "printf '%s|%s' \"$ANTHROPIC_BASE_URL\" \"$OPENAI_BASE_URL\"".into(),
        ]
    }
}

fn run_wrap(home: &Home, port: u16, agent: &[String], upstream: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtok"));
    cmd.arg("wrap").arg("--").args(agent);
    cmd.env("RTOK_HOME", &home.0).env("HOME", &home.0);
    cmd.env("RTOK_PROXY_PORT", port.to_string());
    cmd.current_dir(&home.0);
    if let Some(up) = upstream {
        cmd.env("RTOK_UPSTREAM", up);
    }
    cmd.output().expect("spawn rtok wrap")
}

#[test]
fn wrap_sets_both_base_urls_and_stays_silent() {
    let home = tmp("env");
    let agent = echo_agent();
    // The free port can be stolen between discovery and the proxy's bind; retry only
    // that setup race, never a real mismatch.
    for attempt in 1..=3 {
        let port = free_port();
        let out = run_wrap(&home, port, &agent, None);
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        let want = format!("http://127.0.0.1:{port}|http://127.0.0.1:{port}/v1");
        let got = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        if out.status.success() && got == want && err.is_empty() {
            return;
        }
        assert!(
            err.contains("proxy unavailable"),
            "attempt {attempt}: stdout={got:?} stderr={err:?}"
        );
    }
    panic!("proxy never bound its port in 3 attempts");
}

#[test]
fn wrap_forwards_the_exit_code() {
    let home = tmp("code");
    let port = free_port();
    let agent: Vec<String> = if cfg!(windows) {
        ["cmd", "/c", "exit 3"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        ["sh", "-c", "exit 3"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    let out = run_wrap(&home, port, &agent, None);
    assert_eq!(out.status.code(), Some(3), "exit 3 must come back as 3");
}

/// The proxy `wrap` ensures records like any other proxy request: one `usage` row plus
/// the `calls` / `call_io` / `tokens` rows behind it.
#[test]
fn ensured_proxy_records_usage_rows() {
    let home = tmp("usage");
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/messages");
        then.status(200)
            .header("content-type", "application/json")
            .body(FIXTURE);
    });
    let mut cfg = rtok::config::Config::load_from(&home.0).expect("config");
    cfg.proxy.upstream = server.base_url();
    for attempt in 1..=3 {
        cfg.proxy.port = free_port();
        if rtok::proxy::cli::ensure_proxy(&cfg) {
            break;
        }
        assert!(attempt < 3, "proxy never bound its port in 3 attempts");
    }
    let port = cfg.proxy.port;

    let session = "sess-wrap-usage";
    let body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": "hi"}],
        "metadata": {"user_id": session},
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/v1/messages"))
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&body).expect("json"))
            .send()
            .await
            .expect("post through the ensured proxy")
    });

    let store = rtok::store::Store::open(&home.0.join("rtok.db")).expect("store");
    let mut rows = Vec::new();
    for _ in 0..400 {
        rows = store.usage_rows(session).expect("usage read");
        if !rows.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(rows.len(), 1, "exactly one usage row");
    assert_eq!(
        (
            rows[0].input,
            rows[0].cache_create,
            rows[0].cache_read,
            rows[0].output
        ),
        (10, 0, 0, 2),
        "the fixture's counters"
    );
    assert_eq!(store.count_kind("api_request").expect("calls"), 1);
    assert_eq!(store.count_call_io().expect("call_io"), 1);
    assert_eq!(store.count_tokens().expect("tokens"), 1);
}
