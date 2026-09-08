//! Slint WASM client for `rtok dashboard`. Uses the browser WebSocket API (`web_sys`).

slint::include_modules!();

#[cfg(target_family = "wasm")]
mod wasm {
    use super::*;
    use slint::{ModelRc, SharedString, VecModel};
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{MessageEvent, WebSocket};

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        let ui = MainWindow::new().expect("slint window");
        connect(&ui);
        ui.run().expect("slint run");
    }

    fn connect(ui: &MainWindow) {
        let loc = web_sys::window().expect("window").location();
        let host = loc.host().unwrap_or_else(|_| "127.0.0.1:3333".into());
        let ws = WebSocket::new(&format!("ws://{host}/ws")).expect("websocket");
        let ui_weak = ui.as_weak();
        let on_msg = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            let Some(text) = ev.data().as_string() else {
                return;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                return;
            };
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            load_snapshot(&ui, &v);
        });
        ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();
    }

    fn load_snapshot(ui: &MainWindow, v: &serde_json::Value) {
        let usage = &v["usage"];
        ui.set_usage_input(js_i32(&usage["input"]));
        ui.set_usage_output(js_i32(&usage["output"]));
        ui.set_usage_cache_read(js_i32(&usage["cache_read"]));
        ui.set_usage_cache_create(js_i32(&usage["cache_create"]));
        let mut rows = Vec::new();
        if let Some(plugins) = v["plugins"].as_array() {
            for p in plugins {
                let stats = &p["stats"];
                let fields = p["fields"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|pair| {
                                let k = pair.get(0)?.as_str()?;
                                let v = pair.get(1)?.as_str()?;
                                Some(format!("{k}: {v}"))
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                rows.push(PluginRow {
                    id: js_str(&p["id"]),
                    title: js_str(&p["title"]),
                    summary: js_str(&p["summary"]),
                    fields: SharedString::from(fields),
                    enabled: p["enabled"].as_bool().unwrap_or(false),
                    saves_tokens: p["saves_tokens"].as_bool().unwrap_or(false),
                    input_tokens: js_i32(&stats["input"]),
                    output_tokens: js_i32(&stats["output"]),
                    cache_read: js_i32(&stats["cache_read"]),
                    cache_create: js_i32(&stats["cache_create"]),
                    est_before: js_i32(&stats["est_before"]),
                    est_after: js_i32(&stats["est_after"]),
                    rows: js_i32(&stats["rows"]),
                });
            }
        }
        ui.set_plugins(ModelRc::from(Rc::new(VecModel::from(rows))));
        ui.set_status(SharedString::from("live"));
    }

    fn js_str(v: &serde_json::Value) -> SharedString {
        SharedString::from(v.as_str().unwrap_or(""))
    }

    fn js_i32(v: &serde_json::Value) -> i32 {
        v.as_i64().unwrap_or(0) as i32
    }
}

