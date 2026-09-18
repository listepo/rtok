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
    // Hermetic doctor probes: a snapshot ticks `rtok doctor` (T15.6).
    cfg.doctor.settings_path = dir.join("missing-settings.json");
    cfg.doctor.claude_json = dir.join("missing-claude.json");
    cfg.doctor.mcp_json = dir.join("missing-mcp.json");
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
    assert!(v["text"].as_str().unwrap_or("").contains("NEEDLE"), "{reply}");

    let missing = state
        .inbound(r#"{"expand":"no-such-id"}"#)
        .expect("unknown");
    let v: serde_json::Value = serde_json::from_str(&missing).expect("json");
    assert_eq!(v["type"], "message", "{missing}");
    assert!(
        v["text"].as_str().unwrap_or("").contains("unknown archive id"),
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
