//! Shared proxy-under-test harness (moved out of `tests/proxy.rs` T5.1 so `proxy_bench.rs`
//! can spin up the same real proxy against a fake upstream without duplicating it).

use std::sync::Arc;

use rtok::config::Config;
use rtok::proxy::{ProxyState, app};

pub type Server = (
    String,
    Arc<ProxyState>,
    tokio::task::JoinHandle<std::io::Result<()>>,
);

/// One proxy on a fresh store and a throwaway config dir; `tune` points it at the
/// mock upstream and sets whatever else the case needs (mode, plugin flags).
pub async fn proxy_server(dir_tag: &str, tune: impl FnOnce(&mut Config)) -> Server {
    let dir = std::env::temp_dir().join(format!("rtok-proxy-{dir_tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = Config::load_from(&dir).expect("config");
    // These cases assert `archive`'s own pointers; `compress` (on by default since T127)
    // would summarise them. Its path is covered in `plugins_e2e`; `tune` can turn it on.
    cfg.plugins.compress.enabled = false;
    tune(&mut cfg);
    let state = Arc::new(ProxyState::new(&cfg).expect("proxy state"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr").to_string();
    let task = tokio::spawn(axum::serve(listener, app(state.clone())).into_future());
    (addr, state, task)
}

/// The same throwaway dir `proxy_server` derived from `dir_tag`, for a caller that needs to
/// reopen it later (e.g. an `expand` runtime pointed at the same store, T55.11-style).
pub fn proxy_home_dir(dir_tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rtok-proxy-{dir_tag}-{}", std::process::id()))
}
