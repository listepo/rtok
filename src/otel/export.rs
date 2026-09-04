//! T16.5 (D19): read past each watermark, encode, POST, advance on 2xx. Never panics: a
//! failure is one `logs` row (`source = otel`), the marks stay, and the report says so.

use std::fmt;
use std::fmt::Write as _;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::{map, metrics, otlp};
use crate::config::{Config, Endpoint};
use crate::plugin::Ctx;

/// Rows per stream per flush; the rest goes next time.
pub const BATCH: i64 = 1000;

#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub enabled: bool,
    pub spans: usize,
    pub logs: usize,
    pub points: usize,
    pub posted: usize,
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
        if let Some(e) = &self.error {
            write!(f, "\notel: error: {e}")?;
        }
        Ok(())
    }
}

pub fn resource(cx: &Ctx) -> otlp::Resource {
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
pub async fn flush(cx: &Ctx) -> Report {
    let Some(ep) = cx.config.otel.resolve() else {
        return Report::default();
    };
    let mut rep = Report {
        enabled: true,
        ..Report::default()
    };
    if let Err(e) = flush_into(cx, &ep, &mut rep).await {
        let msg = e.to_string();
        cx.log("error", "otel", "flush", &msg);
        rep.error = Some(msg);
    }
    rep
}

async fn flush_into(cx: &Ctx, ep: &Endpoint, rep: &mut Report) -> Result<()> {
    let store = &cx.store;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(u64::from(
            cx.config.otel.flush_secs.max(1),
        )))
        .build()?;
    let res = resource(cx);

    let smark = store.otel_mark("sessions")?;
    let cmark = store.otel_mark("calls")?;
    let sessions = store.sessions_ended_after(smark)?;
    let calls = store.calls_after(cmark, BATCH)?;
    // A tie on `ended_at` is re-read for safety; alone, it is not worth a request.
    let only_ties = calls.is_empty() && sessions.iter().all(|s| s.ended_at == Some(smark));
    if !(sessions.is_empty() && calls.is_empty()) && !only_ties {
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
        post(&client, ep, "/v1/traces", &otlp::traces(&res, &spans)).await?;
        rep.spans = spans.len();
        rep.posted += 1;
        if let Some(c) = calls.last() {
            store.otel_advance("calls", i64::from(c.id))?;
        }
        if let Some(e) = sessions
            .iter()
            .filter_map(|s| s.ended_at)
            .max()
            .filter(|e| *e > smark)
        {
            store.otel_advance("sessions", e)?;
        }
    }

    let lmark = store.otel_mark("logs")?;
    let rows = store.logs_after(lmark, BATCH)?;
    if !rows.is_empty() {
        let recs: Vec<_> = rows.iter().map(map::log_record).collect();
        post(&client, ep, "/v1/logs", &otlp::logs(&res, &recs)).await?;
        rep.logs = recs.len();
        rep.posted += 1;
        if let Some(r) = rows.last() {
            store.otel_advance("logs", i64::from(r.id))?;
        }
    }

    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let sums = metrics::sums(store, now_ns)?;
    post(&client, ep, "/v1/metrics", &otlp::metrics(&res, &sums)).await?;
    rep.points = sums.iter().map(|m| m.points.len()).sum();
    rep.posted += 1;
    Ok(())
}

async fn post(
    client: &reqwest::Client,
    ep: &Endpoint,
    path: &str,
    body: &serde_json::Value,
) -> Result<()> {
    let mut req = client
        .post(format!("{}{path}", ep.url))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(body)?);
    for (k, v) in &ep.headers {
        req = req.header(k, v);
    }
    let resp = req.send().await.map_err(|e| anyhow!("{path}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let text: String = text.chars().take(200).collect();
        return Err(anyhow!("{path}: HTTP {status} {text}"));
    }
    Ok(())
}

/// `flush` on a current-thread runtime: the CLI, `mcp`'s thread and the hook-spawned child.
pub fn flush_blocking(cx: &Ctx) -> Report {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(flush(cx)),
        Err(e) => Report {
            enabled: true,
            error: Some(e.to_string()),
            ..Report::default()
        },
    }
}

/// `rtok otel status`: endpoint, marks, what is pending, the exporter's last log line.
pub fn status(cx: &Ctx) -> Result<String> {
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

/// `proxy`: flush every `flush_secs` on the server's runtime. No-op without an endpoint.
pub fn spawn_tick(cfg: &Config) {
    if cfg.otel.resolve().is_none() {
        return;
    }
    let cfg = cfg.clone();
    tokio::spawn(async move {
        let Ok(cx) = Ctx::open(cfg.clone(), "otel") else {
            return;
        };
        let period = Duration::from_secs(u64::from(cfg.otel.flush_secs.max(1)));
        let mut iv = tokio::time::interval(period);
        iv.tick().await; // the first tick completes at once
        loop {
            iv.tick().await;
            flush(&cx).await;
        }
    });
}

/// `mcp`: the same tick on a plain thread, since the stdio loop is synchronous.
pub fn spawn_ticker(cfg: &Config) {
    if cfg.otel.resolve().is_none() {
        return;
    }
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        let Ok(cx) = Ctx::open(cfg.clone(), "otel") else {
            return;
        };
        let period = Duration::from_secs(u64::from(cfg.otel.flush_secs.max(1)));
        loop {
            std::thread::sleep(period);
            flush_blocking(&cx);
        }
    });
}

/// Hooks (`Stop`, `SessionEnd`): hand the flush to a detached `rtok otel flush` and return
/// in about a millisecond. The child inherits the environment; `RTOK_HOME` names the config.
pub fn spawn_child(cx: &Ctx) {
    if cx.config.otel.resolve().is_none() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.args(["otel", "flush"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !cx.config.home.as_os_str().is_empty() {
        cmd.env("RTOK_HOME", &cx.config.home);
    }
    let _ = cmd.spawn();
}
