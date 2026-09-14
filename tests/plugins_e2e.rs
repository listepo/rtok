//! T38.2: one e2e case per catalogue plugin through its surface (Measurement rows where owed).
use rtok::config::Config;
use rtok::proxy::{ProxyState, app};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tmp(n: &str) -> Home {
    let t = std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("rtok-t382-{n}-{}-{t}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Home(d)
}
fn run(home: &Home, args: &[&str], input: &str, cwd: &Path) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rtok"));
    c.args(args).env("RTOK_HOME", &home.0);
    c.env("HOME", &home.0).current_dir(cwd);
    c.stdin(Stdio::piped()).stdout(Stdio::piped());
    c.stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    drop(child.stdin.take().unwrap().write_all(input.as_bytes()));
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{args:?} {err}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn js(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}
fn kinds(home: &Home, plugin: &str) -> Vec<String> {
    let s = rtok::store::Store::open(&home.0.join("rtok.db")).unwrap();
    let rows = s.list_measurements(plugin).unwrap();
    rows.into_iter().map(|r| r.kind).collect()
}
fn tool(home: &Home, cwd: &Path, name: &str, args: &str) -> String {
    let p = format!("{{\"name\":\"{name}\",\"arguments\":{args}}}");
    let req = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{p}}}");
    let v = js(&run(home, &["mcp"], &req, cwd));
    let t = &v["result"]["content"][0]["text"];
    t.as_str().unwrap_or("").into()
}
const UP: &str = r#"{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"ok"}],"model":"m","stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":2}}"#;
type Srv = (String, Arc<ProxyState>);
async fn serve(home: &Home, mode: &str, patch: impl FnOnce(&mut Config)) -> Srv {
    let srv = Box::leak(Box::new(httpmock::MockServer::start()));
    let m = srv.mock(|w, t| {
        w.method(httpmock::Method::POST).path("/v1/messages");
        t.status(200).body(UP);
    });
    let _ = Box::leak(Box::new(m));
    let mut cfg = Config::load_from(&home.0).unwrap();
    cfg.proxy.upstream = srv.base_url();
    cfg.proxy.mode = mode.into();
    patch(&mut cfg);
    let st = Arc::new(ProxyState::new(&cfg).unwrap());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a = l.local_addr().unwrap().to_string();
    tokio::spawn(axum::serve(l, app(st.clone())).into_future());
    (a, st)
}
async fn post(a: &str, body: Vec<u8>) -> String {
    let r = reqwest::Client::new().post(format!("http://{a}/v1/messages"));
    let r = r.header("content-type", "application/json");
    let r = r.body(body).send().await.unwrap();
    r.text().await.unwrap()
}
fn user_msg(t: usize, content: &str) -> String {
    let h = r#"{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu-"#;
    format!("{h}{t}\",\"content\":{content}}}]}}")
}
fn turns(s: &str, mk: impl Fn(usize) -> String) -> Vec<u8> {
    let ms = (1..=6).map(mk).collect::<Vec<_>>().join(",");
    format!(r#"{{"model":"m","messages":[{ms}],"metadata":{{"user_id":"{s}"}}}}"#).into_bytes()
}
fn big(t: usize, p: &str) -> String {
    let v: Vec<_> = (1..=600).map(|i| format!("{t}:{i}:{p}")).collect();
    user_msg(t, &serde_json::to_string(&v.join("\n")).unwrap())
}
fn tab(t: usize) -> String {
    let v: Vec<_> = (1..=8).map(|i| json!({"a":i,"b":i,"c":i})).collect();
    user_msg(t, &serde_json::to_string(&v).unwrap())
}
#[test]
fn measure_stats_json_parses() {
    let home = tmp("measure");
    let out = run(&home, &["stats", "--json"], "", &home.0);
    assert!(js(&out).get("sessions").is_some(), "{out}");
}
#[test]
fn cmd_run_records_measurement() {
    let home = tmp("cmd");
    let out = run(&home, &["run", "echo", "hello-t382"], "", &home.0);
    assert!(out.contains("hello-t382"), "{out}");
    assert!(!kinds(&home, "cmd").is_empty());
}
#[test]
fn read_dedup_on_second_mcp_read() {
    let home = tmp("read");
    std::fs::write(home.0.join("a.txt"), "line one\nline two\n").unwrap();
    let first = tool(&home, &home.0, "read", r#"{"path":"a.txt"}"#);
    let second = tool(&home, &home.0, "read", r#"{"path":"a.txt"}"#);
    assert!(first.contains("line one"), "{first}");
    assert!(second.contains("unchanged since"), "{second}");
    assert!(kinds(&home, "read").iter().any(|k| k == "dedup"));
}
#[test]
fn memory_save_then_search() {
    let home = tmp("memory");
    let a = r#"{"kind":"note","title":"t382 milk","body":"oat milk"}"#;
    let saved = tool(&home, &home.0, "mem_save", a);
    let found = tool(&home, &home.0, "mem_search", r#"{"query":"milk"}"#);
    assert!(saved.contains("id"), "{saved}");
    assert!(found.contains("t382 milk"), "{found}");
}
#[test]
fn graph_outline_caps_with_measurement() {
    let home = tmp("graph");
    std::fs::write(home.0.join("a.rs"), "fn a() {}\n").unwrap();
    let out = tool(&home, &home.0, "outline", r#"{"path":"a.rs"}"#);
    assert!(out.contains("fn a"), "{out}");
    assert!(kinds(&home, "graph").iter().any(|k| k == "cap"));
}
#[test]
fn inject_session_start_records_measurement() {
    let home = tmp("inject");
    let c = home.0.join("config.toml");
    std::fs::write(c, "[plugins.inject]\nmodes=[\"terse\"]\n").unwrap();
    let input = r#"{"session_id":"s-inject","cwd":"/tmp","hook_event_name":"SessionStart","source":"startup"}"#;
    let out = run(&home, &["hook", "SessionStart"], input, &home.0);
    assert!(js(&out).is_object(), "{out}");
    assert!(kinds(&home, "inject").iter().any(|k| k == "inject"));
}
#[test]
fn guard_denies_repeat_bash() {
    let home = tmp("guard");
    let post = r#"{"session_id":"s-guard","cwd":"/tmp","tool_name":"Bash","tool_input":{"command":"echo dup-t382"},"hook_event_name":"PostToolUse","tool_response":{"stdout":"dup-output-body"}}"#;
    run(&home, &["hook", "PostToolUse"], post, &home.0);
    let pre = r#"{"session_id":"s-guard","cwd":"/tmp","tool_name":"Bash","tool_input":{"command":"echo dup-t382"},"hook_event_name":"PreToolUse"}"#;
    let out = run(&home, &["hook", "PreToolUse"], pre, &home.0);
    let d = &js(&out)["hookSpecificOutput"]["permissionDecision"];
    assert!(d == "deny", "{out}");
    assert!(kinds(&home, "guard").iter().any(|k| k == "guard"));
}
#[tokio::test]
async fn proxy_passthrough_records_usage() {
    let home = tmp("proxy");
    let (a, st) = serve(&home, "passthrough", |_| {}).await;
    let body = r#"{"model":"m","messages":[{"role":"user","content":"hi"}],"metadata":{"user_id":"s-proxy"}}"#.as_bytes().to_vec();
    assert_eq!(post(&a, body).await, UP);
    for _ in 0..200 {
        if st.store.usage_rows("s-proxy").unwrap().len() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(st.store.usage_rows("s-proxy").unwrap().len(), 1);
}
#[tokio::test]
async fn archive_compress_rewrites_old_blocks() {
    let home = tmp("archive");
    let (a, st) = serve(&home, "compress", |_| {}).await;
    let req = turns("s-archive", |t| big(t, "padding"));
    assert_eq!(post(&a, req.clone()).await, UP);
    assert_eq!(post(&a, req).await, UP);
    assert_eq!(st.store.measurement_count("archive").unwrap(), 4);
}
#[tokio::test]
async fn toon_encode_tabular_blocks() {
    let home = tmp("toon");
    let (a, st) = serve(&home, "compress", |c| c.plugins.toon.enabled = true).await;
    let req = turns("s-toon", tab);
    assert_eq!(post(&a, req.clone()).await, UP);
    assert_eq!(post(&a, req).await, UP);
    assert_eq!(st.store.measurement_count("toon").unwrap(), 4);
    assert_eq!(st.store.measurement_count("archive").unwrap(), 0);
}
#[tokio::test]
async fn compress_summary_row_visible_in_stats() {
    let home = tmp("compress");
    let (a, st) = serve(&home, "compress", |c| c.plugins.compress.enabled = true).await;
    let req = turns("s-compress", |t| big(t, &"x".repeat(90)));
    assert_eq!(post(&a, req.clone()).await, UP);
    assert_eq!(post(&a, req).await, UP);
    assert!(st.store.measurement_count("compress").unwrap() > 0);
    let a = ["stats", "--plugin", "compress", "--json"];
    let out = run(&home, &a, "", &home.0);
    let v = js(&out);
    let rows = v["rows"].as_array().unwrap();
    let hit = rows.iter().any(|r| r["kind"] == "summary");
    assert!(hit, "{out}");
}
