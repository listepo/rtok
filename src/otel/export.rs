//! T16.5 (D19): read past each watermark, encode, POST, advance on 2xx. Never panics: a
//! failure is one `logs` row (`source = otel`), the marks stay, and the report says so.
//! T16.9: an exclusive file lock serialises concurrent flushers across processes.
//! Each stream is isolated: a POST error keeps its own mark and continues with the next
//! stream, so one failing pipeline does not stall the others for a round.
//! A 404 on `/v1/traces`, `/v1/logs`, or `/v1/metrics` is a backend without that pipeline
//! (Jaeger serves traces only; a logs-only collector may 404 traces): the stream is skipped,
//! its mark stays, nothing is logged — else every flush would add the `logs` row that the next
//! flush fails on.

use std::fmt;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::{map, metrics, otlp};
use crate::config::{Config, Endpoint};
use crate::plugin::Runtime;

/// Rows per stream per flush; the rest goes next time.
pub const BATCH: i64 = 1000;

/// A `calls` row this young with no `ms` is still running — the proxy inserts the row before
/// forwarding and sets `ms` and the usage in `finish`; a hook's row closes with `dispatch`.
/// Exported as it stood, it was a 0 ms span without usage for good, the mark past it. Older
/// than this it is a crashed call and drains as it is.
const IN_FLIGHT_SECS: i64 = 300;

fn in_flight(c: &crate::store::models::Call, now: i64) -> bool {
    c.ms.is_none()
        && matches!(c.kind.as_str(), "api_request" | "hook")
        && c.ts > now - IN_FLIGHT_SECS
}

#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub enabled: bool,
    pub spans: usize,
    pub logs: usize,
    pub points: usize,
    pub posted: usize,
    /// Streams the backend answered 404 to (`traces`, `logs`, `metrics`).
    pub skipped: Vec<&'static str>,
    pub error: Option<String>,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.enabled {
            return write!(f, "otel: no endpoint");
        }
        write!(
            f,
            "otel: {} spans · {} logs · {} metric points · {} posts",
            self.spans, self.logs, self.points, self.posted
        )?;
        if !self.skipped.is_empty() {
            write!(f, " · not served: {}", self.skipped.join(", "))?;
        }
        if let Some(e) = &self.error {
            write!(f, "\notel: error: {e}")?;
        }
        Ok(())
    }
}

pub fn resource(cx: &Runtime) -> otlp::Resource {
    otlp::Resource {
        attrs: vec![
            otlp::s("service.name", &cx.config.otel.service_name),
            otlp::s("service.version", env!("CARGO_PKG_VERSION")),
            otlp::s("telemetry.sdk.name", "rtok"),
            otlp::s("telemetry.sdk.language", "rust"),
        ],
    }
}

/// One flush: traces (ended sessions + calls), then logs. Errors are reported, not returned.
/// Cross-process single-flight (T16.9): an exclusive file lock beside the DB so proxy /
/// mcp / hook-spawned `rtok otel flush` cannot double-post the same watermarks. Waiting
/// keeps at-least-once: the loser runs after and finds the marks already advanced.
pub async fn flush(cx: &Runtime) -> Report {
    let Some(ep) = cx.config.otel.resolve() else {
        return Report::default();
    };
    let _guard = match flush_lock(cx) {
        Ok(g) => g,
        Err(e) => return lock_error(cx, "flush lock", e),
    };
    run_flush(cx, &ep).await
}

/// Hook-spawned flushes only (T143): coalesces a burst of `Stop`/`SessionEnd` events into at
/// most one running + one queued flush, instead of one blocked `rtok otel flush` process per
/// event piling up against the (blocking) [`flush_lock`]. `try_queue` claims the queued slot
/// without blocking; a caller that finds it already claimed exits at once — the flush already
/// queued started later and will still see any row written before it begins, so nothing is
/// lost. The queued slot is released as soon as the flush lock is held (before flushing), so a
/// new hook-spawned flush can start queuing behind this one right away.
pub async fn flush_coalesced(cx: &Runtime) -> Report {
    let Some(ep) = cx.config.otel.resolve() else {
        return Report::default();
    };
    let queued = match try_queue(cx) {
        Ok(Some(g)) => g,
        Ok(None) => return Report::default(),
        Err(e) => return lock_error(cx, "queue lock", e),
    };
    let _guard = match flush_lock(cx) {
        Ok(g) => g,
        Err(e) => {
            drop(queued);
            return lock_error(cx, "flush lock", e);
        }
    };
    drop(queued);
    run_flush(cx, &ep).await
}

fn lock_error(cx: &Runtime, what: &str, e: std::io::Error) -> Report {
    let msg = format!("{what}: {e}");
    cx.log("error", "otel", "flush", &msg);
    Report {
        enabled: true,
        error: Some(msg),
        ..Report::default()
    }
}

async fn run_flush(cx: &Runtime, ep: &Endpoint) -> Report {
    let mut rep = Report {
        enabled: true,
        ..Report::default()
    };
    if let Err(e) = flush_into(cx, ep, &mut rep).await {
        let msg = e.to_string();
        cx.log("error", "otel", "flush", &msg);
        rep.error = Some(msg);
    } else if let Some(msg) = rep.error.as_deref() {
        cx.log("error", "otel", "flush", msg);
    }
    rep
}

fn otel_lock_path(cx: &Runtime, suffix: &str) -> PathBuf {
    cx.config.core.db_path.with_extension(suffix)
}

fn open_lock_file(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
}

/// Advisory lock file next to the DB. Held for the whole flush; dropped on return.
fn flush_lock(cx: &Runtime) -> std::io::Result<FlushLock> {
    let file = open_lock_file(&otel_lock_path(cx, "otel-flush.lock"))?;
    rtok_sys::lock_exclusive(&file)?;
    Ok(FlushLock { file })
}

struct FlushLock {
    file: File,
}

impl Drop for FlushLock {
    fn drop(&mut self) {
        let _ = rtok_sys::unlock(&self.file);
    }
}

/// T143: non-blocking "one flush is already queued behind the running one" marker.
pub(crate) struct QueuedLock {
    file: File,
}

impl Drop for QueuedLock {
    fn drop(&mut self) {
        let _ = rtok_sys::unlock(&self.file);
    }
}

/// `Ok(Some(guard))` when this caller claimed the queued slot; `Ok(None)` when another
/// hook-spawned flush already holds it. `pub(crate)` so `testutil` can simulate one for
/// integration tests (`tests/otel.rs`) without duplicating the lock file's path logic.
pub(crate) fn try_queue(cx: &Runtime) -> std::io::Result<Option<QueuedLock>> {
    let file = open_lock_file(&otel_lock_path(cx, "otel-flush-queued.lock"))?;
    Ok(rtok_sys::try_lock_exclusive(&file)?.then_some(QueuedLock { file }))
}

/// Cheap non-blocking check for `spawn_child`: true while some other process holds the
/// queued slot, so the hook does not even spawn a process that would just exit at once.
/// Any error here fails open (spawns as before) rather than blocking the hook.
fn queued_lock_held(cx: &Runtime) -> bool {
    let Ok(file) = open_lock_file(&otel_lock_path(cx, "otel-flush-queued.lock")) else {
        return false;
    };
    match rtok_sys::try_lock_exclusive(&file) {
        Ok(true) => {
            let _ = rtok_sys::unlock(&file);
            false
        }
        Ok(false) => true,
        Err(_) => false,
    }
}

async fn flush_into(cx: &Runtime, ep: &Endpoint, rep: &mut Report) -> Result<()> {
    let store = &cx.store;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(u64::from(
            cx.config.otel.flush_secs.max(1),
        )))
        // T53.3: same webpki roots as the proxy (see `tls`).
        .use_preconfigured_tls(crate::tls::preconfigured()?)
        .build()?;
    let res = resource(cx);

    let smark = store.otel_mark("sessions")?;
    let stail = store.otel_mark("sessions_tail")?;
    let cmark = store.otel_mark("calls")?;
    let sessions = store.sessions_pending_export(smark, stail, BATCH)?;
    let mut calls = store.calls_after(cmark, BATCH)?;
    // Stop in front of the first row still running: the mark advances to the last exported
    // id, so the finished span ships on the flush after `finish`.
    let now = i64::try_from(crate::log::now()).unwrap_or(i64::MAX);
    if let Some(i) = calls.iter().position(|c| in_flight(c, now)) {
        calls.truncate(i);
    }
    if !(sessions.is_empty() && calls.is_empty()) {
        let mut spans = Vec::with_capacity(sessions.len() + calls.len());
        for se in &sessions {
            let host = match se.host_id {
                Some(id) => store.host_slug(id)?,
                None => None,
            };
            spans.push(map::session_span(se, host.as_deref()));
        }
        for c in &calls {
            let d = store.call_detail(c)?;
            spans.push(map::call_span(c, &d, &cx.config.otel));
        }
        match post(&client, ep, "/v1/traces", &otlp::traces(&res, &spans)).await {
            Ok(true) => {
                rep.spans = spans.len();
                rep.posted += 1;
                if let Some(c) = calls.last() {
                    store.otel_advance("calls", i64::from(c.id))?;
                }
                advance_sessions(store, smark, stail, &sessions)?;
            }
            Ok(false) => rep.skipped.push("traces"),
            Err(e) => push_error(rep, e.to_string()),
        }
    }

    let lmark = store.otel_mark("logs")?;
    let rows = store.logs_after(lmark, BATCH)?;
    if !rows.is_empty() {
        let recs: Vec<_> = rows.iter().map(map::log_record).collect();
        match post(&client, ep, "/v1/logs", &otlp::logs(&res, &recs)).await {
            Ok(true) => {
                rep.logs = recs.len();
                rep.posted += 1;
                if let Some(r) = rows.last() {
                    store.otel_advance("logs", i64::from(r.id))?;
                }
            }
            Ok(false) => rep.skipped.push("logs"),
            Err(e) => push_error(rep, e.to_string()),
        }
    }

    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let sums = metrics::sums(store, now_ns)?;
    match post(&client, ep, "/v1/metrics", &otlp::metrics(&res, &sums)).await {
        Ok(true) => {
            rep.points = sums.iter().map(|m| m.points.len()).sum();
            rep.posted += 1;
        }
        Ok(false) => rep.skipped.push("metrics"),
        Err(e) => push_error(rep, e.to_string()),
    }
    Ok(())
}

/// A failed stream keeps its watermark for the next flush; the message joins the report
/// so one `logs` row covers every stream that failed.
fn push_error(rep: &mut Report, msg: String) {
    match &mut rep.error {
        Some(prev) => {
            prev.push_str("; ");
            prev.push_str(&msg);
        }
        None => rep.error = Some(msg),
    }
}

fn advance_sessions(
    store: &crate::store::Store,
    smark: i64,
    stail: i64,
    exported: &[crate::store::models::Session],
) -> Result<()> {
    let Some(last) = exported
        .iter()
        .max_by(|a, b| a.ended_at.cmp(&b.ended_at).then_with(|| a.id.cmp(&b.id)))
    else {
        return Ok(());
    };
    let last_e = last.ended_at.unwrap();
    let n_at_last = exported
        .iter()
        .filter(|s| s.ended_at == Some(last_e))
        .count();
    store.otel_advance("sessions", last_e)?;
    let new_tail = if last_e > smark {
        i64::try_from(n_at_last)?
    } else {
        stail + i64::try_from(n_at_last)?
    };
    store.otel_advance("sessions_tail", new_tail)?;
    Ok(())
}

/// `Ok(false)`: HTTP 404, the backend has no pipeline for this stream.
async fn post(
    client: &reqwest::Client,
    ep: &Endpoint,
    path: &str,
    body: &serde_json::Value,
) -> Result<bool> {
    let mut req = client
        .post(format!("{}{path}", ep.url))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(body)?);
    for (k, v) in &ep.headers {
        req = req.header(k, v);
    }
    let resp = req.send().await.map_err(|e| anyhow!("{path}: {e}"))?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let text: String = text.chars().take(200).collect();
        return Err(anyhow!("{path}: HTTP {status} {text}"));
    }
    Ok(true)
}

/// `flush` on a current-thread runtime: the CLI and `mcp`'s ticker thread. Never coalesced —
/// a manual `rtok otel flush` (or the ticker) always runs, queued slot or not.
pub fn flush_blocking(cx: &Runtime) -> Report {
    block_on_current_thread(flush(cx))
}

/// `flush_coalesced` on a current-thread runtime: only the hook-spawned child (T143).
pub fn flush_coalesced_blocking(cx: &Runtime) -> Report {
    let _trace = FlushTrace::new();
    block_on_current_thread(flush_coalesced(cx))
}

/// T143 test hook only: when `RTOK_OTEL_FLUSH_TRACE` names a directory, mark this process's
/// whole lifetime with `<dir>/<pid>.run` so `tests/otel.rs` can count concurrent hook-spawned
/// flush children by reading a directory it owns, instead of querying the host process table
/// (`ps`/`kill` are off-limits for tests — see the T143 task notes). The env var is unset in
/// every real run, so the only production cost is one `var_os` lookup.
struct FlushTrace(Option<PathBuf>);

impl FlushTrace {
    fn new() -> Self {
        let Some(dir) = std::env::var_os("RTOK_OTEL_FLUSH_TRACE") else {
            return Self(None);
        };
        let path = PathBuf::from(dir).join(format!("{}.run", std::process::id()));
        let _ = File::create(&path);
        Self(Some(path))
    }
}

impl Drop for FlushTrace {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn block_on_current_thread(fut: impl std::future::Future<Output = Report>) -> Report {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(fut),
        Err(e) => Report {
            enabled: true,
            error: Some(e.to_string()),
            ..Report::default()
        },
    }
}

/// `rtok otel status`: endpoint, marks, what is pending, the exporter's last log line.
pub fn status(cx: &Runtime) -> Result<String> {
    let mut out = String::new();
    match cx.config.otel.resolve() {
        Some(ep) => writeln!(out, "endpoint: {}", ep.url)?,
        None => writeln!(
            out,
            "endpoint: none (set [otel] endpoint or OTEL_EXPORTER_OTLP_ENDPOINT)"
        )?,
    }
    let store = &cx.store;
    let (calls, logs) = store.otel_pending()?;
    writeln!(
        out,
        "calls: mark {} · {calls} pending",
        store.otel_mark("calls")?
    )?;
    writeln!(
        out,
        "logs: mark {} · {logs} pending",
        store.otel_mark("logs")?
    )?;
    writeln!(out, "sessions: mark {}", store.otel_mark("sessions")?)?;
    if let Some(l) = store.last_log("otel")? {
        writeln!(out, "last: {} {} {}", l.level, l.name, l.message)?;
    }
    Ok(out)
}

// ── triggers (T16.6 wires them in; none of them runs on the hook path) ──────

/// `proxy` and `mcp`: flush every `flush_secs` on a plain thread. No-op without an endpoint.
/// The flush is blocking (diesel, `flock`), so it never runs as a task on a server's runtime.
pub fn spawn_ticker(cfg: &Config) {
    if cfg.otel.resolve().is_none() {
        return;
    }
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        let Ok(cx) = Runtime::open(cfg.clone(), "otel") else {
            return;
        };
        let period = Duration::from_secs(u64::from(cfg.otel.flush_secs.max(1)));
        loop {
            std::thread::sleep(period);
            flush_blocking(&cx);
        }
    });
}

/// Hooks (`Stop`, `SessionEnd`): hand the flush to a detached `rtok otel flush --coalesce`
/// and return in about a millisecond. T143: skipped outright when a flush is already queued
/// (cheap non-blocking check) — that one will post whatever this event wrote, so spawning
/// here would just be a process that exits at once. The child inherits the environment;
/// `RTOK_HOME` names the config.
pub fn spawn_child(cx: &Runtime) {
    if cx.config.otel.resolve().is_none() {
        return;
    }
    if queued_lock_held(cx) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.args(["otel", "flush", "--coalesce"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !cx.config.home.as_os_str().is_empty() {
        cmd.env("RTOK_HOME", &cx.config.home);
    }
    let _ = cmd.spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil;

    /// A loopback port nothing listens on: bind, read the port, drop the listener.
    fn closed_port() -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        format!("http://127.0.0.1:{port}")
    }

    fn runtime_with(tag: &str, endpoint: &str) -> Runtime {
        let (mut cfg, _) = testutil::config(tag);
        cfg.otel.endpoint = endpoint.into();
        cfg.otel.flush_secs = 1;
        Runtime::open(cfg, tag).unwrap()
    }

    /// The env var turns export on without a key; "no endpoint" cases skip when it is set.
    fn env_endpoint_set() -> bool {
        std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some_and(|v| !v.is_empty())
    }

    #[test]
    fn resource_names_the_service_version_and_sdk() {
        let (mut cfg, _) = testutil::config("otel-res");
        cfg.otel.service_name = "my-svc".into();
        let cx = Runtime::open(cfg, "s").unwrap();
        assert_eq!(
            resource(&cx).attrs,
            [
                otlp::s("service.name", "my-svc"),
                otlp::s("service.version", env!("CARGO_PKG_VERSION")),
                otlp::s("telemetry.sdk.name", "rtok"),
                otlp::s("telemetry.sdk.language", "rust"),
            ]
        );
    }

    #[test]
    fn unreachable_collector_returns_the_error_and_marks_nothing() {
        let cx = runtime_with("otel-unreach", &closed_port());
        let s = &cx.store;
        s.upsert_session("otel-unreach", None, None, None, None)
            .unwrap();
        s.insert_call("otel-unreach", "cli", "cmd", None, None, None, None, None)
            .unwrap();
        cx.log("info", "test", "seed", "one log row");
        let pending = s.otel_pending().unwrap();

        let r = flush_blocking(&cx);

        assert!(r.enabled);
        let err = r.error.as_deref().expect("a refused connect is an error");
        for stream in ["/v1/traces", "/v1/logs", "/v1/metrics"] {
            assert!(err.contains(stream), "{err}");
        }
        assert_eq!((r.spans, r.logs, r.points, r.posted), (0, 0, 0, 0));
        assert!(r.skipped.is_empty(), "a refusal is not a 404");
        for stream in ["calls", "logs", "sessions", "sessions_tail"] {
            assert_eq!(s.otel_mark(stream).unwrap(), 0, "{stream} mark moved");
        }
        let last = s.last_log("otel").unwrap().expect("failure logged");
        assert_eq!(
            (last.level.as_str(), last.name.as_str()),
            ("error", "flush")
        );
        let (calls, logs) = s.otel_pending().unwrap();
        assert_eq!(calls, pending.0, "calls stay pending");
        assert_eq!(logs, pending.1 + 1, "the error row joins the pending logs");
    }

    #[test]
    fn flush_without_an_endpoint_is_a_disabled_no_op() {
        if env_endpoint_set() {
            return;
        }
        let cx = runtime_with("otel-off", "");
        let r = flush_blocking(&cx);
        assert_eq!(r, Report::default());
        assert_eq!(r.to_string(), "otel: no endpoint");
        assert!(cx.store.last_log("otel").unwrap().is_none());
    }

    /// The ticker thread has no stop handle; it dies with the process. What the caller relies
    /// on: `spawn_ticker` returns at once, and the first flush waits a full period.
    #[test]
    fn ticker_returns_at_once_and_waits_a_period_before_flushing() {
        let (mut cfg, _) = testutil::config("otel-tick");
        cfg.otel.endpoint = closed_port();
        cfg.otel.flush_secs = 3600;
        let cx = Runtime::open(cfg.clone(), "otel-tick").unwrap();
        cx.log("info", "test", "seed", "pending");
        let t = std::time::Instant::now();
        spawn_ticker(&cfg);
        assert!(
            t.elapsed() < Duration::from_millis(500),
            "{:?}",
            t.elapsed()
        );
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            cx.store.last_log("otel").unwrap().is_none(),
            "no flush before the first period"
        );
    }

    #[test]
    fn ticker_without_an_endpoint_spawns_nothing() {
        if env_endpoint_set() {
            return;
        }
        let (cfg, dir) = testutil::config("otel-tick-off");
        spawn_ticker(&cfg);
        assert!(!cfg.core.db_path.exists(), "no thread opened the store");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn errors_from_several_streams_join_in_one_report_line() {
        let mut rep = Report {
            enabled: true,
            ..Report::default()
        };
        push_error(&mut rep, "/v1/traces: a".into());
        push_error(&mut rep, "/v1/logs: b".into());
        assert_eq!(rep.error.as_deref(), Some("/v1/traces: a; /v1/logs: b"));
        assert_eq!(
            rep.to_string(),
            "otel: 0 spans · 0 logs · 0 metric points · 0 posts\notel: error: /v1/traces: a; /v1/logs: b"
        );
    }

    /// T143: the queued slot is a plain non-blocking lock — first caller wins, later callers
    /// back off while it is held, and it frees for the next caller once dropped. Held whether
    /// or not a flush is actually running (the flush lock is independent).
    #[test]
    fn queued_slot_first_attempt_wins_the_rest_back_off_until_released() {
        let (cfg, dir) = testutil::config("otel-queue");
        let cx = Runtime::open(cfg, "otel-queue").unwrap();
        let first = try_queue(&cx).unwrap();
        assert!(first.is_some(), "the first attempt claims the slot");
        assert!(
            try_queue(&cx).unwrap().is_none(),
            "a second attempt backs off while the slot is held"
        );
        assert!(
            try_queue(&cx).unwrap().is_none(),
            "a third attempt also backs off"
        );
        drop(first);
        let fourth = try_queue(&cx).unwrap();
        assert!(fourth.is_some(), "the slot frees once the guard drops");
        assert!(
            otel_lock_path(&cx, "otel-flush-queued.lock").exists(),
            "the lock file lives next to the DB"
        );
        drop(fourth);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T143: `spawn_child`'s pre-check mirrors `try_queue`'s state without taking the slot.
    #[test]
    fn queued_lock_held_reports_the_slot_without_taking_it() {
        let (cfg, dir) = testutil::config("otel-queue-held");
        let cx = Runtime::open(cfg, "otel-queue-held").unwrap();
        assert!(!queued_lock_held(&cx), "nothing queued yet");
        let guard = try_queue(&cx).unwrap().expect("free to claim");
        assert!(queued_lock_held(&cx), "now held by `guard`");
        // Checking must not itself claim the slot: a real queuer can still claim it after.
        assert!(queued_lock_held(&cx), "checking again does not release it");
        drop(guard);
        assert!(!queued_lock_held(&cx), "released once the guard drops");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T143: `flush_lock` and the queued slot are independent files — holding one never
    /// blocks the other, which is what lets a queued flush wait for the running one.
    #[test]
    fn the_queued_slot_and_the_flush_lock_are_independent() {
        let (cfg, dir) = testutil::config("otel-queue-indep");
        let cx = Runtime::open(cfg, "otel-queue-indep").unwrap();
        let _flush_guard = flush_lock(&cx).unwrap();
        let queued = try_queue(&cx).unwrap();
        assert!(
            queued.is_some(),
            "the flush lock being held must not block the queued slot"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// T143: a collector that accepts the connection and never answers must not hold the
    /// flush lock forever — the client's own timeout (`otel.flush_secs`, already the
    /// documented POST timeout) bounds every stream, so the flush errors out and a later
    /// flush can still proceed.
    #[test]
    fn a_hung_collector_still_releases_the_lock_within_the_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = stop.clone();
        listener.set_nonblocking(true).unwrap();
        let acceptor = std::thread::spawn(move || {
            let mut held = Vec::new();
            while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok((s, _)) = listener.accept() {
                    held.push(s); // never read or write: the client hangs waiting for a reply
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        let (mut cfg, dir) = testutil::config("otel-hung");
        cfg.otel.endpoint = format!("http://{addr}");
        cfg.otel.flush_secs = 1; // also the POST timeout (config/default.toml)
        let cx = Runtime::open(cfg, "otel-hung").unwrap();
        cx.log("info", "test", "seed", "one log row");

        let start = std::time::Instant::now();
        let r = flush_blocking(&cx);
        let elapsed = start.elapsed();
        assert!(
            r.error.is_some(),
            "a hang must surface as an error, not succeed silently: {r:?}"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "the flush must bound itself well under 'forever', took {elapsed:?}"
        );

        // The lock released: a second flush is not blocked behind the first.
        let start2 = std::time::Instant::now();
        let r2 = flush_blocking(&cx);
        assert!(r2.error.is_some());
        assert!(
            start2.elapsed() < Duration::from_secs(10),
            "the lock from the first flush must already be released"
        );

        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        acceptor.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
