//! Dashboard HTTP + WebSocket smoke (P19).

use std::future::IntoFuture;
use std::sync::Arc;

use rtok::config::Config;
use rtok::web::{DashState, app};

async fn serve(label: &str) -> (String, tokio::task::JoinHandle<std::io::Result<()>>) {
    let dir = std::env::temp_dir().join(format!("rtok-dash-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config::load_from(&dir).expect("config");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let task =
        tokio::spawn(axum::serve(listener, app(Arc::new(DashState::new(cfg)))).into_future());
    (addr, task)
}

#[tokio::test]
async fn web_health_and_index() {
    let (addr, task) = serve("health").await;
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
