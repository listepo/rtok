//! Dashboard HTTP + WebSocket smoke (P19), the T60.5 plugin `set` allow-list,
//! and T60.4 inbound `{"expand": id}`.

use std::future::IntoFuture;
use std::sync::Arc;

use rtok::config::Config;
use rtok::web::{DashState, app};

async fn serve(
    label: &str,
) -> (
    String,
    Arc<DashState>,
    std::path::PathBuf,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let dir = std::env::temp_dir().join(format!("rtok-dash-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = Config::load_from(&dir).expect("config");
    // Hermetic probes: a snapshot ticks `rtok doctor` (T15.6) — and `read_share` parses
    // `stats.transcripts_dir`, which `load_from` leaves at this machine's real
    // `~/.claude/projects` (T74: ~30 s CPU per snapshot on a heavy history).
    cfg.doctor.settings_path = dir.join("missing-settings.json");
    cfg.doctor.claude_json = dir.join("missing-claude.json");
    cfg.doctor.mcp_json = dir.join("missing-mcp.json");
    cfg.stats.transcripts_dir = dir.join("missing-transcripts");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let state = Arc::new(DashState::new(cfg));
    let task = tokio::spawn(axum::serve(listener, app(state.clone())).into_future());
    (addr, state, dir, task)
}

#[tokio::test]
async fn web_health_and_index() {
    let (addr, _state, _dir, task) = serve("health").await;
    let body = reqwest::Client::new()
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("health")
        .text()
        .await
        .expect("body");
    let health: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(health["ok"], true, "{body}");
    let html = reqwest::Client::new()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .expect("index")
        .text()
        .await
        .expect("html");
    assert!(html.contains("canvas"), "{html}");
    task.abort();
}

#[tokio::test]
async fn snapshot_error_when_store_path_is_a_directory() {
    let dir = std::env::temp_dir().join(format!("rtok-web-store-err-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::load_from(&dir).expect("config");
    cfg.core.db_path = dir.join("not-a-db");
    std::fs::create_dir_all(&cfg.core.db_path).unwrap();
    cfg.doctor.settings_path = dir.join("missing-settings.json");
    cfg.doctor.claude_json = dir.join("missing-claude.json");
    cfg.doctor.mcp_json = dir.join("missing-mcp.json");
    // T74: same transcripts leak as `serve` — the snapshot must not parse this
    // machine's real session JSONL.
    cfg.stats.transcripts_dir = dir.join("missing-transcripts");
    let snap = rtok::web::model::snapshot(&cfg);
    assert!(snap.error.is_some(), "{:?}", snap.error);
    let v = serde_json::to_value(&snap).unwrap();
    assert!(v.get("error").is_some(), "{v}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ws_set_accepts_plugin_enabled() {
    let (_addr, state, dir, task) = serve("set-ok").await;
    let before: serde_json::Value = serde_json::from_str(&state.snapshot_json()).expect("snap");
    let cmd = before["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["id"] == "cmd")
        .expect("cmd row");
    assert_eq!(cmd["enabled"], true, "{cmd}");

    let msg = r#"{"set":{"key":"plugins.cmd.enabled","value":false}}"#;
    assert!(state.inbound(msg).is_none(), "allow-listed set is accepted");
    assert!(
        !Config::load_from(&dir).unwrap().plugin_enabled("cmd", true),
        "the write went through config set's writer"
    );
    let after: serde_json::Value = serde_json::from_str(&state.snapshot_json()).expect("snap");
    let cmd = after["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["id"] == "cmd")
        .expect("cmd row");
    assert_eq!(cmd["enabled"], false, "next snapshot reflects the toggle");
    task.abort();
}

#[tokio::test]
async fn ws_set_refuses_other_keys() {
    let (_addr, state, dir, task) = serve("set-no").await;
    let before = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    let reply = state
        .inbound(r#"{"set":{"key":"proxy.port","value":true}}"#)
        .expect("refused");
    let v: serde_json::Value = serde_json::from_str(&reply).expect("message frame");
    assert_eq!(v["type"], "message", "{reply}");
    assert!(
        v["text"].as_str().unwrap_or("").contains("refused key"),
        "{reply}"
    );
    let after = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    assert_eq!(after, before, "a refused key must not write the file");
    let unknown = state
        .inbound(r#"{"set":{"key":"plugins.nope.enabled","value":true}}"#)
        .expect("unknown plugin id is not allow-listed");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&unknown).unwrap()["type"],
        "message"
    );
    task.abort();
}

#[tokio::test]
async fn ws_expand_returns_payload_and_unknown_id() {
    let (_addr, state, dir, task) = serve("expand-ok").await;
    let cfg = Config::load_from(&dir).expect("cfg");
    let cx = rtok::plugin::Runtime::open(cfg, "s").expect("runtime");
    let id = cx
        .store
        .put_archive("s", b"alpha\nNEEDLE\n", &cx.config.core.archive_dir)
        .expect("archive");
    drop(cx);

    let reply = state
        .inbound(&format!(r#"{{"expand":"{id}"}}"#))
        .expect("expand frame");
    let v: serde_json::Value = serde_json::from_str(&reply).expect("json");
    assert_eq!(v["type"], "expand", "{reply}");
    assert_eq!(v["id"], id, "{reply}");
    assert!(
        v["text"].as_str().unwrap_or("").contains("NEEDLE"),
        "{reply}"
    );

    let missing = state
        .inbound(r#"{"expand":"no-such-id"}"#)
        .expect("unknown");
    let v: serde_json::Value = serde_json::from_str(&missing).expect("json");
    assert_eq!(v["type"], "message", "{missing}");
    assert!(
        v["text"]
            .as_str()
            .unwrap_or("")
            .contains("unknown archive id"),
        "{missing}"
    );

    assert!(
        state
            .inbound(r#"{"set":{"key":"plugins.cmd.enabled","value":false}}"#)
            .is_none(),
        "expand must not break the T60.5 set allow-list"
    );
    task.abort();
}

/// T80: the bundle directory is resolved at run time. An installed binary has no
/// source tree, and a 404 there reads as a broken build — the surface must say what
/// is missing and still serve the API.
async fn serve_pkg(
    label: &str,
    pkg: rtok::web::Pkg,
) -> (String, tokio::task::JoinHandle<std::io::Result<()>>) {
    let dir = std::env::temp_dir().join(format!("rtok-pkg-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config::load_from(&dir).expect("config");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let app = rtok::web::app_with_pkg(Arc::new(DashState::new(cfg)), pkg);
    let task = tokio::spawn(axum::serve(listener, app).into_future());
    (addr, task)
}

#[tokio::test]
async fn web_serves_the_resolved_pkg_dir() {
    let pkg = std::env::temp_dir().join(format!("rtok-pkg-src-{}", std::process::id()));
    std::fs::create_dir_all(&pkg).expect("pkg dir");
    std::fs::write(pkg.join("rtok_webui.js"), "export default 1;\n").expect("bundle");
    let (addr, task) = serve_pkg("present", rtok::web::Pkg::Dir(pkg.clone())).await;
    let res = reqwest::get(format!("http://{addr}/pkg/rtok_webui.js"))
        .await
        .expect("pkg");
    assert_eq!(res.status(), 200);
    assert!(res.text().await.expect("body").contains("export default"));
    task.abort();
    let _ = std::fs::remove_dir_all(&pkg);
}

#[tokio::test]
async fn web_without_a_bundle_says_so_instead_of_404() {
    let (addr, task) = serve_pkg("missing", rtok::web::Pkg::Missing).await;
    let res = reqwest::get(format!("http://{addr}/pkg/rtok_webui.js"))
        .await
        .expect("pkg");
    assert_eq!(res.status(), 503);
    let body = res.text().await.expect("body");
    // A fixed message: echoing the response body into the panic is CodeQL `rust/log-injection`.
    assert!(
        body.contains("RTOK_WEB_PKG"),
        "503 body does not name RTOK_WEB_PKG"
    );
    let health = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("health");
    assert_eq!(health.status(), 200, "the API stays up without the UI");
    task.abort();
}

/// T111: a ketch install has only the binary, so the bundle rides inside it. Skipped
/// when this build had no bundle to embed (`build.rs` found no `pkg/`).
#[tokio::test]
async fn web_serves_the_embedded_bundle() {
    let Some((mime, bytes)) = rtok::web::embedded::get("rtok_webui_bg.wasm") else {
        eprintln!("skip: built without a bundle — run `just web-bundle`");
        return;
    };
    assert_eq!(mime, "application/wasm");
    let (addr, task) = serve_pkg("embedded", rtok::web::Pkg::Embedded).await;
    let res = reqwest::get(format!("http://{addr}/pkg/rtok_webui_bg.wasm"))
        .await
        .expect("wasm");
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "application/wasm");
    let body = res.bytes().await.expect("body");
    assert!(body.starts_with(b"\0asm") && body.len() == bytes.len());
    let js = reqwest::get(format!("http://{addr}/pkg/rtok_webui.js"))
        .await
        .expect("js");
    assert_eq!(js.status(), 200);
    let other = reqwest::get(format!("http://{addr}/pkg/nope.js"))
        .await
        .expect("other");
    assert_eq!(other.status(), 404);
    task.abort();
}
