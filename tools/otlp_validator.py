"""A standalone OTLP/HTTP JSON receiver that validates what `rtok otel flush` posts.

Independent of the Rust tests: it re-checks the spec's JSON rules (hex ids of the right
length, every int64 a decimal string, numeric enums, required fields) against the real
bodies, and prints what a backend would show. Run: python3 otlp_validator.py [port]
"""
import json, re, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

HEX = re.compile(r"^[0-9a-f]+$")
seen, problems = [], []


def fail(msg):
    problems.append(msg)


def check_attrs(attrs, where):
    for a in attrs:
        if "key" not in a or "value" not in a:
            fail(f"{where}: attribute without key/value: {a}")
        v = a.get("value", {})
        if "intValue" in v and not isinstance(v["intValue"], str):
            fail(f"{where}: intValue must be a string, got {v['intValue']!r}")


def check_traces(body):
    for rs in body.get("resourceSpans", []):
        check_attrs(rs.get("resource", {}).get("attributes", []), "resource")
        for ss in rs.get("scopeSpans", []):
            for sp in ss.get("spans", []):
                name = sp.get("name", "?")
                for key, n in (("traceId", 32), ("spanId", 16)):
                    v = sp.get(key, "")
                    if len(v) != n or not HEX.match(v):
                        fail(f"span {name}: {key} must be {n} lowercase hex chars, got {v!r}")
                if "parentSpanId" in sp and len(sp["parentSpanId"]) != 16:
                    fail(f"span {name}: parentSpanId must be 16 hex chars")
                for key in ("startTimeUnixNano", "endTimeUnixNano"):
                    if not isinstance(sp.get(key), str):
                        fail(f"span {name}: {key} must be a decimal string")
                if not isinstance(sp.get("kind"), int):
                    fail(f"span {name}: kind must be an integer")
                check_attrs(sp.get("attributes", []), f"span {name}")
                for ev in sp.get("events", []):
                    check_attrs(ev.get("attributes", []), f"event {ev.get('name')}")
                seen.append(("span", name, len(sp.get("attributes", []))))


def check_logs(body):
    for rl in body.get("resourceLogs", []):
        for sl in rl.get("scopeLogs", []):
            for r in sl.get("logRecords", []):
                if not isinstance(r.get("timeUnixNano"), str):
                    fail("log: timeUnixNano must be a decimal string")
                if not isinstance(r.get("severityNumber"), int):
                    fail("log: severityNumber must be an integer")
                if "traceId" in r and len(r["traceId"]) != 32:
                    fail("log: traceId must be 32 hex chars")
                seen.append(("log", r.get("body", {}).get("stringValue", ""), r.get("severityNumber")))


def check_metrics(body):
    for rm in body.get("resourceMetrics", []):
        for sm in rm.get("scopeMetrics", []):
            for m in sm.get("metrics", []):
                s = m.get("sum")
                if not s:
                    fail(f"metric {m.get('name')}: expected a sum")
                    continue
                if s.get("aggregationTemporality") != 2:
                    fail(f"metric {m.get('name')}: cumulative temporality is 2")
                for dp in s.get("dataPoints", []):
                    if not isinstance(dp.get("asInt"), str):
                        fail(f"metric {m.get('name')}: asInt must be a decimal string")
                    check_attrs(dp.get("attributes", []), f"metric {m.get('name')}")
                seen.append(("metric", m.get("name"), len(s.get("dataPoints", []))))


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("content-length", 0))
        raw = self.rfile.read(n)
        ct = self.headers.get("content-type", "")
        if ct != "application/json":
            fail(f"{self.path}: content-type must be application/json, got {ct!r}")
        try:
            body = json.loads(raw)
        except Exception as e:
            fail(f"{self.path}: body is not JSON: {e}")
            body = {}
        {"/v1/traces": check_traces, "/v1/logs": check_logs, "/v1/metrics": check_metrics}.get(
            self.path, lambda _b: fail(f"unexpected path {self.path}")
        )(body)
        print(f"{self.path}: {len(raw)} bytes")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(b"{}")

    def log_message(self, *a):
        pass


def report():
    print("\n--- what a backend would show ---")
    for kind, name, extra in seen:
        print(f"  {kind:7} {name}  ({extra})")
    print(f"\n{len(problems)} problem(s)")
    for p in problems:
        print(f"  ! {p}")


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4318
    srv = HTTPServer(("127.0.0.1", port), Handler)
    print(f"listening on http://127.0.0.1:{port}")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        report()
