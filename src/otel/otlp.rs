//! T16.3 (D19): OTLP/HTTP JSON encoding under the spec's JSON mapping rules — ids as lowercase
//! hex, every int64 a decimal string, enums as integers. Pure: no I/O, no clock.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// One attribute value; the OTLP `AnyValue` subset rtok emits.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Double(f64),
}

pub type Attr = (String, AttrValue);

pub fn s(key: &str, v: impl Into<String>) -> Attr {
    (key.to_string(), AttrValue::Str(v.into()))
}

pub fn i(key: &str, v: i64) -> Attr {
    (key.to_string(), AttrValue::Int(v))
}

pub fn b(key: &str, v: bool) -> Attr {
    (key.to_string(), AttrValue::Bool(v))
}

/// `SpanKind` as the spec numbers it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Internal = 1,
    Server = 2,
    Client = 3,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Unset,
    Ok,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub time_ns: u64,
    pub attrs: Vec<Attr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent: Option<String>,
    pub name: String,
    pub kind: Kind,
    pub start_ns: u64,
    pub end_ns: u64,
    pub attrs: Vec<Attr>,
    pub events: Vec<Event>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogRecord {
    pub time_ns: u64,
    /// OTLP `SeverityNumber`: 1 trace, 5 debug, 9 info, 13 warn, 17 error.
    pub severity: u8,
    pub severity_text: String,
    pub body: String,
    pub attrs: Vec<Attr>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

/// A cumulative, monotonic integer sum: one data point per attribute set.
#[derive(Debug, Clone, PartialEq)]
pub struct Sum {
    pub name: String,
    pub unit: String,
    pub description: String,
    pub start_ns: u64,
    pub time_ns: u64,
    pub points: Vec<(Vec<Attr>, i64)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resource {
    pub attrs: Vec<Attr>,
}

pub fn seconds_to_ns(ts: i64) -> u64 {
    u64::try_from(ts).unwrap_or(0).saturating_mul(1_000_000_000)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 16 bytes of `sha256("rtok:session:" + id)`: one trace per session, stable across flushes.
pub fn trace_id(session: &str) -> String {
    hex(&Sha256::digest(format!("rtok:session:{session}"))[..16])
}

/// 8 bytes of `sha256("rtok:" + kind + ":" + id)`; `kind` keeps `calls` and `sessions` apart.
pub fn span_id(kind: &str, id: &str) -> String {
    hex(&Sha256::digest(format!("rtok:{kind}:{id}"))[..8])
}

fn value(v: &AttrValue) -> Value {
    match v {
        AttrValue::Str(s) => json!({ "stringValue": s }),
        AttrValue::Int(i) => json!({ "intValue": i.to_string() }),
        AttrValue::Bool(b) => json!({ "boolValue": b }),
        AttrValue::Double(d) => json!({ "doubleValue": d }),
    }
}

fn attrs(a: &[Attr]) -> Value {
    Value::Array(
        a.iter()
            .map(|(k, v)| json!({ "key": k, "value": value(v) }))
            .collect(),
    )
}

fn scope() -> Value {
    json!({ "name": "rtok", "version": env!("CARGO_PKG_VERSION") })
}

fn span(sp: &Span) -> Value {
    let mut v = json!({
        "traceId": sp.trace_id,
        "spanId": sp.span_id,
        "name": sp.name,
        "kind": sp.kind as i32,
        "startTimeUnixNano": sp.start_ns.to_string(),
        "endTimeUnixNano": sp.end_ns.to_string(),
        "attributes": attrs(&sp.attrs),
        "events": sp.events.iter().map(|e| json!({
            "name": e.name,
            "timeUnixNano": e.time_ns.to_string(),
            "attributes": attrs(&e.attrs),
        })).collect::<Vec<_>>(),
        "status": match &sp.status {
            Status::Unset => json!({}),
            Status::Ok => json!({ "code": 1 }),
            Status::Error(m) => json!({ "code": 2, "message": m }),
        },
    });
    if let Some(p) = &sp.parent {
        v["parentSpanId"] = json!(p);
    }
    v
}

/// Body for `POST /v1/traces`.
pub fn traces(res: &Resource, spans: &[Span]) -> Value {
    json!({ "resourceSpans": [{
        "resource": { "attributes": attrs(&res.attrs) },
        "scopeSpans": [{ "scope": scope(), "spans": spans.iter().map(span).collect::<Vec<_>>() }],
    }] })
}

/// Body for `POST /v1/logs`.
pub fn logs(res: &Resource, records: &[LogRecord]) -> Value {
    let recs: Vec<Value> = records
        .iter()
        .map(|r| {
            let mut v = json!({
                "timeUnixNano": r.time_ns.to_string(),
                "severityNumber": r.severity,
                "severityText": r.severity_text,
                "body": { "stringValue": r.body },
                "attributes": attrs(&r.attrs),
            });
            if let Some(t) = &r.trace_id {
                v["traceId"] = json!(t);
            }
            if let Some(s) = &r.span_id {
                v["spanId"] = json!(s);
            }
            v
        })
        .collect();
    json!({ "resourceLogs": [{
        "resource": { "attributes": attrs(&res.attrs) },
        "scopeLogs": [{ "scope": scope(), "logRecords": recs }],
    }] })
}

/// Body for `POST /v1/metrics`: cumulative (`aggregationTemporality` 2), monotonic sums.
pub fn metrics(res: &Resource, sums: &[Sum]) -> Value {
    let ms: Vec<Value> = sums
        .iter()
        .map(|m| {
            json!({
                "name": m.name,
                "unit": m.unit,
                "description": m.description,
                "sum": {
                    "aggregationTemporality": 2,
                    "isMonotonic": true,
                    "dataPoints": m.points.iter().map(|(a, n)| json!({
                        "attributes": attrs(a),
                        "startTimeUnixNano": m.start_ns.to_string(),
                        "timeUnixNano": m.time_ns.to_string(),
                        "asInt": n.to_string(),
                    })).collect::<Vec<_>>(),
                },
            })
        })
        .collect();
    json!({ "resourceMetrics": [{
        "resource": { "attributes": attrs(&res.attrs) },
        "scopeMetrics": [{ "scope": scope(), "metrics": ms }],
    }] })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_hex(s: &str) -> bool {
        s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    fn sample() -> Span {
        Span {
            trace_id: trace_id("s1"),
            span_id: span_id("call", "7"),
            parent: Some(span_id("session", "s1")),
            name: "chat claude-x".into(),
            kind: Kind::Client,
            start_ns: seconds_to_ns(1_700_000_000),
            end_ns: seconds_to_ns(1_700_000_000) + 25_000_000,
            attrs: vec![
                s("gen_ai.request.model", "claude-x"),
                i("gen_ai.usage.input_tokens", 100),
            ],
            events: vec![Event {
                name: "rtok.measurement".into(),
                time_ns: seconds_to_ns(1_700_000_000),
                attrs: vec![i("rtok.est_before", 3), b("rtok.ok", true)],
            }],
            status: Status::Error("boom".into()),
        }
    }

    #[test]
    fn ids_are_hex_stable_and_distinct() {
        let t = trace_id("s1");
        assert_eq!(t.len(), 32);
        assert!(is_hex(&t));
        assert_eq!(t, trace_id("s1"));
        assert_ne!(t, trace_id("s2"));
        let sp = span_id("call", "7");
        assert_eq!(sp.len(), 16);
        assert!(is_hex(&sp));
        assert_ne!(sp, span_id("session", "7"));
    }

    #[test]
    fn traces_follow_the_json_mapping() {
        let res = Resource {
            attrs: vec![s("service.name", "rtok")],
        };
        let v = traces(
            &res,
            &[
                sample(),
                Span {
                    kind: Kind::Internal,
                    parent: None,
                    ..sample()
                },
            ],
        );
        let sp = &v["resourceSpans"][0]["scopeSpans"][0]["spans"];
        assert_eq!(
            v["resourceSpans"][0]["resource"]["attributes"][0]["key"],
            "service.name"
        );
        assert_eq!(
            v["resourceSpans"][0]["scopeSpans"][0]["scope"]["name"],
            "rtok"
        );
        assert_eq!(sp[0]["traceId"].as_str().unwrap().len(), 32);
        assert_eq!(sp[0]["spanId"].as_str().unwrap().len(), 16);
        assert_eq!(sp[0]["parentSpanId"].as_str().unwrap().len(), 16);
        assert_eq!(sp[0]["kind"], 3);
        assert_eq!(sp[1]["kind"], 1);
        assert!(sp[1].get("parentSpanId").is_none());
        assert_eq!(sp[0]["startTimeUnixNano"], "1700000000000000000");
        assert_eq!(sp[0]["endTimeUnixNano"], "1700000000025000000");
        assert_eq!(sp[0]["attributes"][0]["value"]["stringValue"], "claude-x");
        assert_eq!(sp[0]["attributes"][1]["value"]["intValue"], "100");
        assert_eq!(
            sp[0]["events"][0]["attributes"][1]["value"]["boolValue"],
            true
        );
        assert_eq!(sp[0]["status"]["code"], 2);
        assert_eq!(sp[0]["status"]["message"], "boom");
    }

    #[test]
    fn logs_and_metrics_follow_the_json_mapping() {
        let res = Resource::default();
        let rec = LogRecord {
            time_ns: 5,
            severity: 13,
            severity_text: "WARN".into(),
            body: "hi".into(),
            attrs: vec![s("rtok.source", "otel")],
            trace_id: Some(trace_id("s1")),
            span_id: None,
        };
        let v = logs(&res, &[rec]);
        let r = &v["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert_eq!(r["timeUnixNano"], "5");
        assert_eq!(r["severityNumber"], 13);
        assert_eq!(r["body"]["stringValue"], "hi");
        assert_eq!(r["traceId"].as_str().unwrap().len(), 32);
        assert!(r.get("spanId").is_none());
        let sum = Sum {
            name: "rtok.tokens".into(),
            unit: "{token}".into(),
            description: "d".into(),
            start_ns: 1,
            time_ns: 2,
            points: vec![(vec![s("gen_ai.token.type", "input")], 42)],
        };
        let v = metrics(&res, &[sum]);
        let m = &v["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0];
        assert_eq!(m["name"], "rtok.tokens");
        assert_eq!(m["sum"]["aggregationTemporality"], 2);
        assert_eq!(m["sum"]["isMonotonic"], true);
        assert_eq!(m["sum"]["dataPoints"][0]["asInt"], "42");
        assert_eq!(m["sum"]["dataPoints"][0]["startTimeUnixNano"], "1");
    }
}
