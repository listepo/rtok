//! In-memory live view of plain-proxy traffic (when `proxy.enabled` / `core.enabled` is
//! false). Nothing here is persisted: process exit clears it. TUI/web read it via
//! [`snapshot`] (same process) or `GET /live` (cross-process).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Cap matches the Calls page budget so the operator model stays cheap.
pub const LIVE_CAP: usize = 120;

/// One plain-proxy request summary for the operator surfaces — no DB row behind it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveCall {
    pub ts: i64,
    pub method: String,
    pub path: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: u16,
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub ms: f64,
}

/// Process-wide ring the proxy process fills; operator surfaces clone it each tick.
fn ring() -> &'static Arc<Mutex<VecDeque<LiveCall>>> {
    static RING: OnceLock<Arc<Mutex<VecDeque<LiveCall>>>> = OnceLock::new();
    RING.get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(LIVE_CAP))))
}

/// Record one finished plain-proxy request (newest at the front).
pub fn push(call: LiveCall) {
    let Ok(mut q) = ring().lock() else {
        return;
    };
    q.push_front(call);
    while q.len() > LIVE_CAP {
        q.pop_back();
    }
}

/// Newest-first copy for `/live` and same-process snapshot reads.
pub fn snapshot() -> Vec<LiveCall> {
    ring()
        .lock()
        .map(|q| q.iter().cloned().collect())
        .unwrap_or_default()
}

/// Test helper: empty the ring between cases.
pub fn clear() {
    if let Ok(mut q) = ring().lock() {
        q.clear();
    }
}

/// Persistent operator warning while config keeps the proxy in plain mode.
pub fn alerts(cfg: &crate::config::Config) -> Vec<String> {
    if cfg.proxy.enabled && cfg.core.enabled {
        return Vec::new();
    }
    let which = match (cfg.proxy.enabled, cfg.core.enabled) {
        (false, false) => "[proxy] enabled and [core] enabled",
        (false, true) => "[proxy] enabled",
        (true, false) => "[core] enabled",
        (true, true) => unreachable!(),
    };
    vec![format!(
        "proxy disabled — live passthrough (not recorded). Set {which} = true and restart to restore compress/bookkeeping."
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn ring_keeps_newest_first_and_caps() {
        clear();
        for i in 0..(LIVE_CAP + 5) {
            push(LiveCall {
                ts: i as i64,
                method: "POST".into(),
                path: format!("/v1/{i}"),
                provider: None,
                model: None,
                status: 200,
                request_bytes: 1,
                response_bytes: 2,
                ms: 1.0,
            });
        }
        let s = snapshot();
        assert_eq!(s.len(), LIVE_CAP);
        assert_eq!(s[0].ts, (LIVE_CAP + 4) as i64);
        assert_eq!(s.last().unwrap().ts, 5);
        clear();
    }

    #[test]
    fn alerts_only_when_a_kill_switch_is_off() {
        let mut cfg = Config::default();
        assert!(alerts(&cfg).is_empty());
        cfg.proxy.enabled = false;
        assert_eq!(alerts(&cfg).len(), 1);
        cfg.proxy.enabled = true;
        cfg.core.enabled = false;
        assert!(alerts(&cfg)[0].contains("[core] enabled"));
    }
}
