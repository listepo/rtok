//! End to end: the real `rtok web` binary on a real port — the page, the bundle it
//! loads (embedded since T111) and a real WebSocket client on `/ws`. `tests/web.rs`
//! drives the router in-process; this is what a user's browser sees.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

struct Web {
    child: Child,
    addr: String,
    home: PathBuf,
}

impl Drop for Web {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port()
}

/// `rtok web` with `HOME`/`RTOK_HOME` in a temp dir, so the snapshot never reads this
/// machine's `~/.claude` (T74) and the `set` below writes a throwaway config.
async fn start(label: &str) -> Web {
    let home = std::env::temp_dir().join(format!("rtok-web-e2e-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("home");
    let port = free_port().to_string();
    let child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .args(["web", "--host", "127.0.0.1", "--port", &port])
        .env("RTOK_HOME", &home)
        .env("HOME", &home)
        .env_remove("RTOK_WEB_PKG")
        .current_dir(&home)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rtok web");
    let web = Web {
        child,
        addr: format!("127.0.0.1:{port}"),
        home,
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(r) = reqwest::get(format!("http://{}/health", web.addr)).await
            && r.status() == 200
        {
            return web;
        }
        assert!(Instant::now() < deadline, "rtok web never answered /health");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn bundle(name: &str) -> Option<Vec<u8>> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/rtok-webui/pkg")
            .join(name),
    )
    .ok()
}

#[tokio::test]
async fn web_serves_the_page_and_its_bundle() {
    let web = start("http").await;
    let base = format!("http://{}", web.addr);

    let health = reqwest::get(format!("{base}/health"))
        .await
        .expect("health")
        .text()
        .await
        .expect("body");
    let health: Value = serde_json::from_str(&health).expect("json");
    assert_eq!(health["ok"], true, "{health}");

    let html = reqwest::get(format!("{base}/")).await.expect("index");
    assert_eq!(html.status(), 200);
    let html = html.text().await.expect("html");
    // The page body is server-controlled input: keep it out of the assert message so a failure
    // cannot inject forged lines into the test log.
    assert!(
        html.contains("./pkg/rtok_webui.js"),
        "index page does not reference ./pkg/rtok_webui.js"
    );

    let Some(wasm) = bundle("rtok_webui_bg.wasm") else {
        eprintln!("skip bundle checks: built without a bundle — run `just web-bundle`");
        return;
    };
    let js = reqwest::get(format!("{base}/pkg/rtok_webui.js"))
        .await
        .expect("js");
    assert_eq!(js.status(), 200);
    assert!(
        js.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("javascript")
    );
    let res = reqwest::get(format!("{base}/pkg/rtok_webui_bg.wasm"))
        .await
        .expect("wasm");
    assert_eq!(res.status(), 200);
    // `WebAssembly.instantiateStreaming` refuses any other media type.
    assert_eq!(res.headers()["content-type"], "application/wasm");
    assert_eq!(res.bytes().await.expect("body").as_ref(), wasm.as_slice());
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Next text frame as JSON, skipping pings; fails after 15 s.
async fn next_json(ws: &mut Ws) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(15), ws.next())
            .await
            .expect("frame within 15 s")
            .expect("socket open")
            .expect("frame");
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).expect("json frame");
        }
    }
}

/// Reads frames until one of `kind` arrives (snapshots keep ticking every 2 s).
async fn next_of(ws: &mut Ws, kind: &str) -> Value {
    loop {
        let v = next_json(ws).await;
        if v["type"] == kind {
            return v;
        }
    }
}

fn plugin_enabled(snap: &Value, id: &str) -> bool {
    snap["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("{id} row in {snap}"))["enabled"]
        .as_bool()
        .expect("enabled")
}

#[tokio::test]
async fn websocket_streams_snapshots_and_answers_commands() {
    let web = start("ws").await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", web.addr))
        .await
        .expect("ws connect");

    let first = next_json(&mut ws).await;
    assert_eq!(first["type"], "snapshot", "{first}");
    assert!(plugin_enabled(&first, "cmd"));

    // Unknown archive id → a message frame, and the socket stays up.
    ws.send(Message::text(r#"{"expand":"no-such-id"}"#))
        .await
        .expect("send expand");
    let msg = next_of(&mut ws, "message").await;
    assert!(
        msg["text"].as_str().unwrap().contains("unknown archive id"),
        "{msg}"
    );

    // T60.5 allow-list: a non-plugin key is refused over the wire.
    ws.send(Message::text(
        r#"{"set":{"key":"core.db_path","value":true}}"#,
    ))
    .await
    .expect("send refused set");
    let msg = next_of(&mut ws, "message").await;
    assert!(
        msg["text"].as_str().unwrap().contains("refused key"),
        "{msg}"
    );

    // An allow-listed toggle is written and the next snapshot carries it.
    ws.send(Message::text(
        r#"{"set":{"key":"plugins.cmd.enabled","value":false}}"#,
    ))
    .await
    .expect("send set");
    // A snapshot already in flight may predate the write; the next one carries it.
    let mut snap = next_of(&mut ws, "snapshot").await;
    if plugin_enabled(&snap, "cmd") {
        snap = next_of(&mut ws, "snapshot").await;
    }
    assert!(
        !plugin_enabled(&snap, "cmd"),
        "toggle not in the snapshot after the write"
    );
    let written = std::fs::read_to_string(web.home.join("config.toml")).expect("config written");
    assert!(written.contains("enabled = false"), "{written}");

    ws.close(None).await.expect("close");
}
