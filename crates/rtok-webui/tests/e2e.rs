//! End-to-end tests for the Slint operator UI, through Slint's recommended
//! headless backend (`i-slint-backend-testing`, `=`-pinned to `slint`).
//!
//! Run with `SLINT_EMIT_DEBUG_INFO=1`: the `ElementHandle` tab queries need
//! compiler debug info, and `slint-build` re-runs itself when the var changes.
//!
//! The unit tests in `src/lib.rs` cover the pure `snapshot::parse`; these
//! drive the real compiled `MainWindow`: one `/ws` snapshot through the same
//! [`apply_snapshot`] the WASM client uses, then the bound properties, the
//! derived bindings and the tab clicks.

use i_slint_backend_testing::ElementHandle;
use rtok_webui::{MainWindow, PAGE_IDS, apply_snapshot};
use serde_json::json;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;

fn window() -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    let ui = MainWindow::new().unwrap();
    // Mirror of the WASM `start`: the tab bar is `PAGE_IDS` order (D23).
    ui.set_pages(ModelRc::from(Rc::new(VecModel::from(
        PAGE_IDS
            .iter()
            .map(|id| rtok_webui::PageTab {
                id: SharedString::from(*id),
                title: SharedString::from(*id),
            })
            .collect::<Vec<_>>(),
    ))));
    ui
}

fn snapshot() -> serde_json::Value {
    json!({
        "type": "snapshot",
        "usage": {
            "input": 10, "output": 2, "cache_create": 1, "cache_read": 3,
            "ctt": 5, "turns": [10, 16]
        },
        "plugins": [
            {
                "id": "cmd", "title": "Bash / cmd", "summary": "s", "enabled": true,
                "saves_tokens": true, "fields": [["rewrite", "true"]],
                "stats": {"input": 1, "output": 2, "cache_read": 3, "cache_create": 4,
                          "est_before": 25, "est_after": 10, "rows": 1}
            },
            {
                "id": "read", "title": "Read", "summary": "r", "enabled": false,
                "saves_tokens": false, "fields": [],
                "stats": {"input": 0, "output": 0, "cache_read": 0, "cache_create": 0,
                          "est_before": 0, "est_after": 0, "rows": 0}
            }
        ],
        "calls": [{
            "id": 1, "ts": 3661, "session": "s", "surface": "proxy", "kind": "api_request",
            "plugin": null, "name": "/v1/messages", "parent_id": null, "ms": 12.5, "ok": 1,
            "error": null, "host": "claude", "provider": "anthropic", "model": "x",
            "api": "anthropic", "input": 10, "cache_create": 1, "cache_read": 2, "output": 3
        }],
        "sessions": [{
            "id": "a", "host": "claude", "project": "rtok", "provider": "anthropic",
            "api": "anthropic", "model": "x", "input": 30, "cache_create": 1,
            "cache_read": 7, "output": 7, "started_at": 1, "last_activity": 2, "ended_at": null
        }],
        "doctor": {
            "hooks_total": 3,
            "hooks_by_event": {"Stop": 1},
            "mcp": [{"name": "rtok", "cmd": "rtok mcp", "tools": 4, "desc_tokens": 10}],
            "proxy": "direct",
            "proxy_openai": "direct",
            "mcp_tool_search_disabled": false,
            "bash_max_output_length": null,
            "auto_compact_window": null,
            "instructions": null
        },
        "logs": ["2026-09-10 07:00:00 info web/serve: up"]
    })
}

#[test]
fn fresh_window_is_connecting_with_empty_models() {
    let ui = window();
    assert_eq!(ui.get_status().as_str(), "connecting");
    assert_eq!(ui.get_page_index(), 0);
    assert_eq!(ui.get_page_id().as_str(), "overview");
    assert_eq!(ui.get_usage_ctt(), 0);
    assert_eq!(ui.get_pages().row_count(), PAGE_IDS.len());
    assert_eq!(ui.get_plugins().row_count(), 0);
    assert_eq!(ui.get_calls().row_count(), 0);
    assert_eq!(ui.get_sessions().row_count(), 0);
    assert_eq!(ui.get_logs().row_count(), 0);
}

#[test]
fn snapshot_reaches_every_bound_property() {
    let ui = window();
    apply_snapshot(&ui, &snapshot());
    assert_eq!(ui.get_status().as_str(), "live");
    assert_eq!(ui.get_usage_input(), 10);
    assert_eq!(ui.get_usage_output(), 2);
    assert_eq!(ui.get_usage_cache_read(), 3);
    assert_eq!(ui.get_usage_cache_create(), 1);
    assert_eq!(ui.get_usage_ctt(), 5);
    assert!(ui.get_overview_savings().as_str().contains("cmd: 15 tok"));
    assert!(ui.get_overview_turns().as_str().contains("10 16"));

    let plugins = ui.get_plugins();
    assert_eq!(plugins.row_count(), 2);
    let first = plugins.row_data(0).unwrap();
    assert_eq!(first.id.as_str(), "cmd");
    assert!(first.enabled);
    assert_eq!(first.est_before - first.est_after, 15);

    let calls = ui.get_calls();
    assert_eq!(calls.row_count(), 1);
    let call = calls.row_data(0).unwrap();
    assert_eq!(call.name.as_str(), "/v1/messages");
    assert!(call.detail.as_str().contains("anthropic"));

    let sessions = ui.get_sessions();
    assert_eq!(sessions.row_count(), 1);
    let session = sessions.row_data(0).unwrap();
    assert!(session.live);
    assert!(session.summary.as_str().contains('a'));

    assert!(ui.get_doctor_text().as_str().contains("hooks 3"));
    assert_eq!(ui.get_logs().row_count(), 1);
}

#[test]
fn tab_click_switches_page() {
    let ui = window();
    apply_snapshot(&ui, &snapshot());
    for (i, id) in PAGE_IDS.iter().enumerate() {
        let tab = ElementHandle::find_by_accessible_label(&ui, id)
            .next()
            .unwrap_or_else(|| {
                panic!("tab with accessible label `{id}` (run with SLINT_EMIT_DEBUG_INFO=1)")
            });
        tab.mock_single_click(slint::platform::PointerEventButton::Left);
        assert_eq!(ui.get_page_index(), i as i32, "click on tab `{id}`");
        assert_eq!(
            ui.get_page_id().as_str(),
            *id,
            "page-id binding after `{id}`"
        );
    }
}

#[test]
fn cursors_follow_selected_rows() {
    let ui = window();
    apply_snapshot(&ui, &snapshot());
    ui.set_plugin_cursor(1);
    assert_eq!(ui.get_selected_plugin().id.as_str(), "read");
    ui.set_plugin_cursor(0);
    assert_eq!(ui.get_selected_plugin().id.as_str(), "cmd");
    ui.set_call_cursor(0);
    assert_eq!(ui.get_selected_call().name.as_str(), "/v1/messages");
}

#[test]
fn fail_open_snapshot_renders_empty_pages() {
    let ui = window();
    apply_snapshot(&ui, &json!({}));
    assert_eq!(ui.get_plugins().row_count(), 0);
    assert_eq!(ui.get_calls().row_count(), 0);
    assert_eq!(ui.get_sessions().row_count(), 0);
    assert_eq!(ui.get_logs().row_count(), 0);
    assert!(ui.get_doctor_text().as_str().contains("did not answer"));
}
