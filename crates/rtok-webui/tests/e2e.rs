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
use rtok_webui::{MainWindow, PAGE_IDS, apply_snapshot, init_theme};
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
    init_theme(&ui);
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
            "id": 1, "ts": 3661, "session": "a", "surface": "proxy", "kind": "api_request",
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
fn theme_toggle_flips_palette() {
    let ui = window();
    assert!(ui.get_dark());
    ui.invoke_theme_toggle();
    assert!(!ui.get_dark());
    ui.invoke_theme_toggle();
    assert!(ui.get_dark());
}

#[test]
fn snapshot_error_reaches_the_banner() {
    let ui = window();
    apply_snapshot(
        &ui,
        &json!({"type":"snapshot","usage":{},"plugins":[],"calls":[],"sessions":[],"logs":[],"doctor":null,"error":"store unreadable"}),
    );
    assert_eq!(ui.get_error().as_str(), "store unreadable");
}

#[test]
fn session_click_opens_detail() {
    let ui = window();
    apply_snapshot(&ui, &snapshot());
    let tab = ElementHandle::find_by_accessible_label(&ui, "sessions")
        .next()
        .unwrap_or_else(|| panic!("sessions tab (run with SLINT_EMIT_DEBUG_INFO=1)"));
    tab.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_page_id().as_str(), "sessions");
    let row = ElementHandle::find_by_accessible_label(&ui, "session a")
        .next()
        .unwrap_or_else(|| panic!("session row (run with SLINT_EMIT_DEBUG_INFO=1)"));
    row.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_session_cursor(), 0);
    let detail = ui.get_selected_session().detail;
    assert!(detail.as_str().contains("project rtok"), "{detail}");
    assert!(detail.as_str().contains("api anthropic"), "{detail}");
    assert!(detail.as_str().contains("started"), "{detail}");
    assert!(detail.as_str().contains("last"), "{detail}");
    assert!(detail.as_str().contains("ended live"), "{detail}");
    assert!(detail.as_str().contains("usage (anthropic)"), "{detail}");
    assert!(detail.as_str().contains("/v1/messages"), "{detail}");
}

#[test]
fn plugin_toggle_click_sends_the_set() {
    let ui = window();
    apply_snapshot(&ui, &snapshot());
    let tab = ElementHandle::find_by_accessible_label(&ui, "plugins")
        .next()
        .unwrap_or_else(|| panic!("plugins tab (run with SLINT_EMIT_DEBUG_INFO=1)"));
    tab.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_page_id().as_str(), "plugins");

    let hit = Rc::new(std::cell::Cell::new(None::<(String, bool)>));
    let hit2 = hit.clone();
    ui.on_toggle_plugin(move |id, on| {
        hit2.set(Some((id.to_string(), on)));
    });
    let toggle = ElementHandle::find_by_accessible_label(&ui, "toggle cmd")
        .next()
        .unwrap_or_else(|| panic!("plugin toggle (run with SLINT_EMIT_DEBUG_INFO=1)"));
    toggle.mock_single_click(slint::platform::PointerEventButton::Left);
    let (id, on) = hit.take().expect("toggle callback fired");
    assert_eq!(id, "cmd");
    assert!(!on, "cmd starts enabled, click turns it off");
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

#[test]
fn sessions_live_only_checkbox_hides_ended() {
    let ui = window();
    let mut v = snapshot();
    v["sessions"] = json!([
        {
            "id": "a", "host": "claude", "project": "rtok", "provider": "anthropic",
            "api": "anthropic", "model": "x", "input": 30, "cache_create": 1,
            "cache_read": 7, "output": 7, "started_at": 1, "last_activity": 2, "ended_at": null
        },
        {
            "id": "b", "host": "claude", "project": "rtok", "provider": "anthropic",
            "api": "anthropic", "model": "x", "input": 5, "cache_create": 0,
            "cache_read": 0, "output": 1, "started_at": 3, "last_activity": 4, "ended_at": 5
        }
    ]);
    apply_snapshot(&ui, &v);
    let tab = ElementHandle::find_by_accessible_label(&ui, "sessions")
        .next()
        .unwrap_or_else(|| panic!("sessions tab (run with SLINT_EMIT_DEBUG_INFO=1)"));
    tab.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_page_id().as_str(), "sessions");
    assert_eq!(ui.get_sessions().row_count(), 2);
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "session b")
            .next()
            .is_some(),
        "ended session is listed"
    );
    let filter = ElementHandle::find_by_accessible_label(&ui, "live only")
        .next()
        .unwrap_or_else(|| panic!("live-only checkbox (run with SLINT_EMIT_DEBUG_INFO=1)"));
    filter.mock_single_click(slint::platform::PointerEventButton::Left);
    assert!(ui.get_sessions_live_only(), "checkbox toggles");
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "session a")
            .next()
            .is_some(),
        "live row stays"
    );
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "session b")
            .next()
            .is_none(),
        "ended row is hidden"
    );
}

#[test]
fn skills_never_only_checkbox_hides_invoked() {
    let ui = window();
    let mut v = snapshot();
    v["skills"] = json!({
        "header": "3 skills · 58 desc bytes ≈ 14 tok/req · 0 resident · 0.0% of input",
        "rows": [
            {"name":"hot","source":"user","desc_chars":40,"body_bytes":100,"invocations":3,"resident":800,"last_invoked":"—","never":false},
            {"name":"plug","source":"plugin:x","desc_chars":8,"body_bytes":50,"invocations":1,"resident":200,"last_invoked":"—","never":false},
            {"name":"cold","source":"project","desc_chars":10,"body_bytes":20,"invocations":0,"resident":0,"last_invoked":"never","never":true}
        ]
    });
    apply_snapshot(&ui, &v);
    let tab = ElementHandle::find_by_accessible_label(&ui, "skills")
        .next()
        .unwrap_or_else(|| panic!("skills tab (run with SLINT_EMIT_DEBUG_INFO=1)"));
    tab.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_page_id().as_str(), "skills");
    assert_eq!(ui.get_skills().row_count(), 3);
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "skill hot")
            .next()
            .is_some(),
        "invoked skill is listed"
    );
    let filter = ElementHandle::find_by_accessible_label(&ui, "never invoked only")
        .next()
        .unwrap_or_else(|| panic!("never-invoked checkbox (run with SLINT_EMIT_DEBUG_INFO=1)"));
    filter.mock_single_click(slint::platform::PointerEventButton::Left);
    assert!(ui.get_skills_never_only(), "checkbox toggles");
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "skill cold")
            .next()
            .is_some(),
        "never-invoked row stays"
    );
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "skill hot")
            .next()
            .is_none(),
        "invoked row is hidden"
    );
}
