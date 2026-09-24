//! Slint WASM client for `rtok dashboard`. Uses the browser WebSocket API (`web_sys`).
//!
//! Page tabs are [`PAGE_IDS`] — the same ids `rtok::web::model::pages()` offers (D23).
//! A page that exists on the TUI and not here is a defect; `tests/surface_parity.rs`
//! holds the lists together.

slint::include_modules!();

use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Page ids the WASM UI renders, in `model::pages()` order (D23 / T19.4).
/// `tests/surface_parity.rs` asserts this equals `rtok::web::model::pages()`.
pub const PAGE_IDS: &[&str] = &[
    "overview", "plugins", "calls", "sessions", "doctor", "logs", "skills", "stats",
];

/// Pure snapshot → view fields. Native-testable; the WASM `load_snapshot` applies these
/// onto the Slint window. Fail-open: missing keys become empty pages, never a panic.
pub mod snapshot {
    use serde_json::Value;

    /// What one snapshot paints onto the Slint properties.
    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct View {
        pub usage_input: i32,
        pub usage_output: i32,
        pub usage_cache_read: i32,
        pub usage_cache_create: i32,
        pub usage_ctt: i32,
        pub overview_savings: String,
        pub overview_turns: String,
        pub plugins: Vec<Plugin>,
        pub calls: Vec<Call>,
        pub sessions: Vec<Session>,
        pub doctor_text: String,
        pub logs: Vec<String>,
        pub error: String,
        pub skills_header: String,
        pub skills: Vec<Skill>,
        pub stats_text: String,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct Plugin {
        pub id: String,
        pub title: String,
        pub summary: String,
        pub fields: String,
        pub enabled: bool,
        pub saves_tokens: bool,
        pub input_tokens: i32,
        pub output_tokens: i32,
        pub cache_read: i32,
        pub cache_create: i32,
        pub est_before: i32,
        pub est_after: i32,
        pub rows: i32,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct Call {
        pub when: String,
        pub surface: String,
        pub kind: String,
        pub name: String,
        pub session: String,
        pub ms: String,
        pub tokens: String,
        pub subtitle: String,
        pub detail: String,
        pub ref_id: String,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct Session {
        pub id: String,
        pub host: String,
        pub provider: String,
        pub model: String,
        pub input: i32,
        pub output: i32,
        pub cache_create: i32,
        pub cache_read: i32,
        pub live: bool,
        pub status: String,
        pub activity: String,
        pub summary: String,
        pub detail: String,
    }

    /// Parse a `/ws` snapshot JSON into the view the UI binds.
    pub fn parse(v: &Value) -> View {
        let usage = &v["usage"];
        View {
            usage_input: i32_of(&usage["input"]),
            usage_output: i32_of(&usage["output"]),
            usage_cache_read: i32_of(&usage["cache_read"]),
            usage_cache_create: i32_of(&usage["cache_create"]),
            usage_ctt: i32_of(&usage["ctt"]),
            overview_savings: savings_text(v),
            overview_turns: turns_text(usage),
            plugins: plugins_of(v),
            calls: calls_of(v),
            sessions: sessions_of(v),
            doctor_text: doctor_of(&v["doctor"]),
            logs: logs_of(v),
            error: v.get("error").and_then(|e| e.as_str()).unwrap_or("").to_string(),
            skills_header: v["skills"]["header"].as_str().unwrap_or("").to_string(),
            skills: skills_of(v),
            stats_text: stats_of(&v["stats"]),
        }
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct Skill {
        pub name: String,
        pub source: String,
        pub desc_chars: String,
        pub body_bytes: String,
        pub invocations: String,
        pub resident: String,
        pub last_invoked: String,
        pub never: bool,
        pub summary: String,
    }

    fn skills_of(v: &Value) -> Vec<Skill> {
        let Some(rows) = v["skills"]["rows"].as_array() else {
            return Vec::new();
        };
        rows.iter()
            .map(|r| {
                let name = r["name"].as_str().unwrap_or("-").to_string();
                let source = r["source"].as_str().unwrap_or("-").to_string();
                let desc = r["desc_chars"].as_u64().unwrap_or(0);
                let body = r["body_bytes"].as_u64().unwrap_or(0);
                let calls = r["invocations"].as_u64().unwrap_or(0);
                let resident = r["resident"].as_u64().unwrap_or(0);
                let last = r["last_invoked"].as_str().unwrap_or("—").to_string();
                let never = r["never"].as_bool().unwrap_or(false);
                Skill {
                    summary: format!("{name} {source} desc {desc}c body {body}B calls {calls} res {resident} {last}"),
                    name,
                    source,
                    desc_chars: desc.to_string(),
                    body_bytes: body.to_string(),
                    invocations: calls.to_string(),
                    resident: resident.to_string(),
                    last_invoked: last,
                    never,
                }
            })
            .collect()
    }

    fn plugins_of(v: &Value) -> Vec<Plugin> {
        let Some(plugins) = v["plugins"].as_array() else {
            return Vec::new();
        };
        plugins
            .iter()
            .map(|p| {
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
                Plugin {
                    id: str_of(&p["id"]),
                    title: str_of(&p["title"]),
                    summary: str_of(&p["summary"]),
                    fields,
                    enabled: p["enabled"].as_bool().unwrap_or(false),
                    saves_tokens: p["saves_tokens"].as_bool().unwrap_or(false),
                    input_tokens: i32_of(&stats["input"]),
                    output_tokens: i32_of(&stats["output"]),
                    cache_read: i32_of(&stats["cache_read"]),
                    cache_create: i32_of(&stats["cache_create"]),
                    est_before: i32_of(&stats["est_before"]),
                    est_after: i32_of(&stats["est_after"]),
                    rows: i32_of(&stats["rows"]),
                }
            })
            .collect()
    }

    fn calls_of(v: &Value) -> Vec<Call> {
        let Some(rows) = v["calls"].as_array() else {
            return Vec::new();
        };
        rows.iter()
            .map(|c| {
                let name = opt_str(&c["name"]).unwrap_or_else(|| "-".into());
                let ms = c["ms"]
                    .as_f64()
                    .map(|m| format!("{m:.1}"))
                    .unwrap_or_else(|| "-".into());
                let tokens = match c["input"].as_i64() {
                    Some(_) => {
                        let sum = i64_of(&c["input"])
                            + i64_of(&c["cache_create"])
                            + i64_of(&c["cache_read"])
                            + i64_of(&c["output"]);
                        format!("{sum} tok")
                    }
                    None => "-".into(),
                };
                let id = c["id"].as_i64().unwrap_or(0);
                let ref_id = v["ref_ids"]
                    .get(id.to_string())
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                Call {
                    when: time_of(c["ts"].as_i64().unwrap_or(0)),
                    surface: str_of(&c["surface"]),
                    kind: str_of(&c["kind"]),
                    name: name.clone(),
                    session: str_of(&c["session"]),
                    ms: ms.clone(),
                    tokens: tokens.clone(),
                    subtitle: format!("{name} / {} / {ms} ms / {tokens}", str_of(&c["session"])),
                    detail: call_detail(c, &name, &ms, &tokens, &ref_id),
                    ref_id,
                }
            })
            .collect()
    }

    fn call_detail(c: &Value, name: &str, ms: &str, tokens: &str, ref_id: &str) -> String {
        let dash = |k: &str| opt_str(&c[k]).unwrap_or_else(|| "-".into());
        let ref_id = if ref_id.is_empty() { "-" } else { ref_id };
        format!(
            "session {session} · surface {surface} · kind {kind}\n\
             name {name} · plugin {plugin} · host {host}\n\
             provider {provider} · model {model} · api {api}\n\
             ms {ms} · tokens {tokens} · ok {ok}\n\
             ref_id {ref_id}",
            session = str_of(&c["session"]),
            surface = str_of(&c["surface"]),
            kind = str_of(&c["kind"]),
            plugin = dash("plugin"),
            host = dash("host"),
            provider = dash("provider"),
            model = dash("model"),
            api = dash("api"),
            ok = c["ok"].as_i64().unwrap_or(0),
        )
    }

    fn sessions_of(v: &Value) -> Vec<Session> {
        let Some(rows) = v["sessions"].as_array() else {
            return Vec::new();
        };
        rows.iter()
            .map(|s| {
                let id = str_of(&s["id"]);
                let host = opt_str(&s["host"]).unwrap_or_else(|| "-".into());
                let provider = opt_str(&s["provider"]).unwrap_or_else(|| "-".into());
                let model = opt_str(&s["model"]).unwrap_or_else(|| "-".into());
                let input = i32_of(&s["input"]);
                let output = i32_of(&s["output"]);
                let cache_create = i32_of(&s["cache_create"]);
                let cache_read = i32_of(&s["cache_read"]);
                let live = s["ended_at"].is_null();
                let live_label = if live { "live" } else { "ended" };
                Session {
                    summary: format!("{id}  {live_label}"),
                    activity: format!(
                        "{host} / {provider} / {model} / last {}",
                        s["last_activity"].as_i64().unwrap_or(0)
                    ),
                    status: format!(
                        "in {input}  out {output}  cache+ {cache_create}  cache-r {cache_read}"
                    ),
                    detail: session_detail(v, &id),
                    id,
                    host,
                    provider,
                    model,
                    input,
                    output,
                    cache_create,
                    cache_read,
                    live,
                }
            })
            .collect()
    }

    /// Mirror of `model::session_detail`: this session's snapshot JSON row plus the
    /// snapshot's calls filtered by that id. The WASM crate cannot call the rtok
    /// accessor, so the filter is rebuilt from the same keys (T60.3 / D23).
    fn session_detail(v: &Value, id: &str) -> String {
        let Some(s) = v["sessions"]
            .as_array()
            .and_then(|rows| rows.iter().find(|s| s["id"].as_str() == Some(id)))
        else {
            return String::new();
        };
        let dash = |k: &str| opt_str(&s[k]).unwrap_or_else(|| "-".into());
        let ended = s["ended_at"]
            .as_i64()
            .map(time_of)
            .unwrap_or_else(|| "live".into());
        let api = dash("api");
        let mut lines = vec![
            format!("project {} · api {api}", dash("project")),
            format!(
                "started {} · last {} · ended {ended}",
                time_of(i64_of(&s["started_at"])),
                time_of(i64_of(&s["last_activity"])),
            ),
            format!(
                "usage ({api}) input {} cache create {} cache read {} output {}",
                i64_of(&s["input"]),
                i64_of(&s["cache_create"]),
                i64_of(&s["cache_read"]),
                i64_of(&s["output"]),
            ),
        ];
        let calls: Vec<&Value> = v["calls"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|c| c["session"].as_str() == Some(id))
                    .collect()
            })
            .unwrap_or_default();
        lines.push(format!("calls {}", calls.len()));
        for c in calls {
            lines.push(format!(
                "{} {} {} {}",
                time_of(i64_of(&c["ts"])),
                str_of(&c["surface"]),
                str_of(&c["kind"]),
                opt_str(&c["name"]).unwrap_or_else(|| "-".into()),
            ));
        }
        lines.join("\n")
    }

    fn logs_of(v: &Value) -> Vec<String> {
        v["logs"]
            .as_array()
            .map(|a| a.iter().map(str_of).collect())
            .unwrap_or_default()
    }

    /// Mirror of `doctor::Report::to_text` over the wire JSON — the WASM client has no
    /// `rtok` types, so it rebuilds the same lines from the snapshot.
    fn doctor_of(v: &Value) -> String {
        if v.is_null() {
            return "doctor did not answer this tick — `rtok doctor` has the details".into();
        }
        let mut out = format!("hooks {}\n", v["hooks_total"].as_u64().unwrap_or(0));
        if let Some(obj) = v["hooks_by_event"].as_object() {
            for (ev, n) in obj {
                out.push_str(&format!("  {ev} {}\n", n.as_u64().unwrap_or(0)));
            }
        }
        out.push_str("mcp\n");
        if let Some(mcp) = v["mcp"].as_array() {
            for s in mcp {
                out.push_str(&format!(
                    "  {} ({} tools, ~{} desc tokens) {}\n",
                    str_of(&s["name"]),
                    s["tools"].as_u64().unwrap_or(0),
                    s["desc_tokens"].as_u64().unwrap_or(0),
                    str_of(&s["cmd"]),
                ));
            }
        }
        out.push_str(&format!("proxy {}\n", str_of(&v["proxy"])));
        out.push_str(&format!("proxy openai {}\n", str_of(&v["proxy_openai"])));
        if v["mcp_tool_search_disabled"].as_bool().unwrap_or(false) {
            out.push_str("mcp_tool_search likely disabled (ANTHROPIC_BASE_URL is set)\n");
        }
        out.push_str(&format!(
            "BASH_MAX_OUTPUT_LENGTH {}\n",
            opt_str(&v["bash_max_output_length"]).unwrap_or_else(|| "(unset)".into())
        ));
        out.push_str(&format!(
            "autoCompactWindow {}\n",
            opt_str(&v["auto_compact_window"]).unwrap_or_else(|| "(unset)".into())
        ));
        if let Some(audit) = v.get("instructions").filter(|a| !a.is_null()) {
            out.push_str("instructions\n");
            if let Some(rows) = audit["rows"].as_array() {
                for r in rows {
                    let warn = r["warn"].as_bool().unwrap_or(false);
                    out.push_str(&format!(
                        "  {} {} tokens {}{}\n",
                        str_of(&r["name"]),
                        r["tokens"].as_u64().unwrap_or(0),
                        str_of(&r["path"]),
                        if warn { " WARN" } else { "" }
                    ));
                }
            }
            if let Some(dups) = audit["duplicates"].as_array() {
                for d in dups {
                    let sent = d.get(0).map(str_of).unwrap_or_default();
                    let names: Vec<String> = d
                        .get(1)
                        .and_then(|n| n.as_array())
                        .map(|a| a.iter().map(str_of).collect())
                        .unwrap_or_default();
                    out.push_str(&format!("  duplicate `{sent}` in {}\n", names.join(", ")));
                }
            }
        }
        out
    }

    /// The Stats page (T227): the wire already carries `rtok stats --price`'s table
    /// plus `stats --cache`'s table as one rendered string — no reconstruction here,
    /// unlike [`doctor_of`], since the model sends the text itself.
    fn stats_of(v: &Value) -> String {
        v.as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "stats did not answer this tick — `rtok stats` has the details".into())
    }

    fn savings_text(v: &Value) -> String {
        let Some(plugins) = v["plugins"].as_array() else {
            return "no measured savings yet".into();
        };
        let mut saved: Vec<(&str, i64)> = plugins
            .iter()
            .filter_map(|p| {
                let id = p["id"].as_str()?;
                let stats = p.get("stats")?;
                if stats.is_null() {
                    return None;
                }
                let s = i64_of(&stats["est_before"]) - i64_of(&stats["est_after"]);
                (s > 0).then_some((id, s))
            })
            .collect();
        if saved.is_empty() {
            return "no measured savings yet".into();
        }
        saved.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        saved
            .into_iter()
            .map(|(id, s)| format!("{id}: {s} tok"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn turns_text(usage: &Value) -> String {
        let Some(turns) = usage["turns"].as_array() else {
            return "ctx tokens per turn (no usage rows yet)".into();
        };
        if turns.is_empty() {
            return "ctx tokens per turn (no usage rows yet)".into();
        }
        let preview: Vec<String> = turns
            .iter()
            .take(24)
            .map(|t| t.as_i64().unwrap_or(0).to_string())
            .collect();
        let more = if turns.len() > 24 {
            format!(" … ({} turns)", turns.len())
        } else {
            String::new()
        };
        format!(
            "ctx tokens per turn (last {}): {}{}",
            turns.len(),
            preview.join(" "),
            more
        )
    }

    fn time_of(ts: i64) -> String {
        // HH:MM:SS UTC from unix seconds — enough for the list; detail has the rest.
        let ts = ts.max(0) as u64;
        let secs = ts % 86400;
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    }

    fn str_of(v: &Value) -> String {
        v.as_str().unwrap_or("").to_string()
    }

    fn opt_str(v: &Value) -> Option<String> {
        v.as_str().map(str::to_string)
    }

    fn i32_of(v: &Value) -> i32 {
        v.as_i64().unwrap_or(0) as i32
    }

    fn i64_of(v: &Value) -> i64 {
        v.as_i64().unwrap_or(0)
    }
}

// Used by the wasm client's filter box and the unit tests — nothing native.
#[cfg(any(target_family = "wasm", test))]
fn filter_expand(text: &str, needle: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    text.lines()
        .filter(|line| line.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Apply one `/ws` snapshot onto the window: the one call path the WASM
/// client and the e2e tests share. Fail-open like [`snapshot::parse`]:
/// missing keys become empty pages, never a panic.
pub fn apply_snapshot(ui: &MainWindow, v: &serde_json::Value) {
    let view = snapshot::parse(v);
    ui.set_usage_input(view.usage_input);
    ui.set_usage_output(view.usage_output);
    ui.set_usage_cache_read(view.usage_cache_read);
    ui.set_usage_cache_create(view.usage_cache_create);
    ui.set_usage_ctt(view.usage_ctt);
    ui.set_overview_savings(SharedString::from(view.overview_savings));
    ui.set_overview_turns(SharedString::from(view.overview_turns));
    ui.set_doctor_text(SharedString::from(view.doctor_text));
    ui.set_stats_text(SharedString::from(view.stats_text));

    let plugins: Vec<PluginRow> = view
        .plugins
        .into_iter()
        .map(|p| PluginRow {
            id: SharedString::from(p.id),
            title: SharedString::from(p.title),
            summary: SharedString::from(p.summary),
            fields: SharedString::from(p.fields),
            enabled: p.enabled,
            saves_tokens: p.saves_tokens,
            input_tokens: p.input_tokens,
            output_tokens: p.output_tokens,
            cache_read: p.cache_read,
            cache_create: p.cache_create,
            est_before: p.est_before,
            est_after: p.est_after,
            rows: p.rows,
        })
        .collect();
    ui.set_plugins(ModelRc::from(Rc::new(VecModel::from(plugins))));

    let calls: Vec<CallRow> = view
        .calls
        .into_iter()
        .map(|c| CallRow {
            when: SharedString::from(c.when),
            surface: SharedString::from(c.surface),
            kind: SharedString::from(c.kind),
            name: SharedString::from(c.name),
            session: SharedString::from(c.session),
            ms: SharedString::from(c.ms),
            tokens: SharedString::from(c.tokens),
            subtitle: SharedString::from(c.subtitle),
            detail: SharedString::from(c.detail),
            ref_id: SharedString::from(c.ref_id),
        })
        .collect();
    ui.set_calls(ModelRc::from(Rc::new(VecModel::from(calls))));

    let sessions: Vec<SessionRow> = view
        .sessions
        .into_iter()
        .map(|s| SessionRow {
            id: SharedString::from(s.id),
            host: SharedString::from(s.host),
            provider: SharedString::from(s.provider),
            model: SharedString::from(s.model),
            input: s.input,
            output: s.output,
            cache_create: s.cache_create,
            cache_read: s.cache_read,
            live: s.live,
            status: SharedString::from(s.status),
            activity: SharedString::from(s.activity),
            summary: SharedString::from(s.summary),
            detail: SharedString::from(s.detail),
        })
        .collect();
    ui.set_sessions(ModelRc::from(Rc::new(VecModel::from(sessions))));

    let logs: Vec<SharedString> = view.logs.into_iter().map(SharedString::from).collect();
    ui.set_logs(ModelRc::from(Rc::new(VecModel::from(logs))));
    ui.set_error(SharedString::from(view.error));
    ui.set_skills_header(SharedString::from(view.skills_header));
    let skills: Vec<SkillRow> = view
        .skills
        .into_iter()
        .map(|s| SkillRow {
            name: SharedString::from(s.name),
            source: SharedString::from(s.source),
            desc_chars: SharedString::from(s.desc_chars),
            body_bytes: SharedString::from(s.body_bytes),
            invocations: SharedString::from(s.invocations),
            resident: SharedString::from(s.resident),
            last_invoked: SharedString::from(s.last_invoked),
            never: s.never,
            summary: SharedString::from(s.summary),
        })
        .collect();
    ui.set_skills(ModelRc::from(Rc::new(VecModel::from(skills))));
    ui.set_status(SharedString::from("live"));
}

/// Load theme from `localStorage` / `prefers-color-scheme` (T60.9).
#[cfg(target_family = "wasm")]
fn init_theme(ui: &MainWindow) {
    let dark = read_theme_storage().unwrap_or_else(system_prefers_dark);
    ui.set_dark(dark);
    let ui_weak = ui.as_weak();
    ui.on_theme_toggle(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let next = !ui.get_dark();
        ui.set_dark(next);
        write_theme_storage(next);
    });
}

#[cfg(not(target_family = "wasm"))]
pub fn init_theme(ui: &MainWindow) {
    let ui_weak = ui.as_weak();
    ui.on_theme_toggle(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        ui.set_dark(!ui.get_dark());
    });
}

#[cfg(target_family = "wasm")]
const THEME_KEY: &str = "rtok-theme";

#[cfg(target_family = "wasm")]
fn read_theme_storage() -> Option<bool> {
    let storage = web_sys::window()?.local_storage().ok()??;
    match storage.get_item(THEME_KEY).ok()?.as_deref() {
        Some("dark") => Some(true),
        Some("light") => Some(false),
        _ => None,
    }
}

#[cfg(target_family = "wasm")]
fn write_theme_storage(dark: bool) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(THEME_KEY, if dark { "dark" } else { "light" });
    }
}

#[cfg(target_family = "wasm")]
fn system_prefers_dark() -> bool {
    use web_sys::MediaQueryList;
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok())
        .flatten()
        .map(|m: MediaQueryList| m.matches())
        .unwrap_or(true)
}

#[cfg(target_family = "wasm")]
mod wasm {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageEvent, WebSocket};

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        let ui = MainWindow::new().expect("slint window");
        ui.set_pages(ModelRc::from(Rc::new(VecModel::from(
            PAGE_IDS
                .iter()
                .map(|id| PageTab {
                    id: SharedString::from(*id),
                    title: SharedString::from(*id),
                })
                .collect::<Vec<_>>(),
        ))));
        super::init_theme(&ui);
        track_viewport(&ui);
        connect(&ui, 0);
        ui.run().expect("slint run");
    }

    /// Size the window to the browser viewport: without this the canvas keeps
    /// `preferred-width/height` (960×640) and never fills the screen.
    fn fit_viewport(ui: &MainWindow) {
        let Some(win) = web_sys::window() else { return };
        let (Ok(w), Ok(h)) = (win.inner_width(), win.inner_height()) else { return };
        let (Some(w), Some(h)) = (w.as_f64(), h.as_f64()) else { return };
        ui.window()
            .set_size(slint::LogicalSize::new(w as f32, h as f32));
    }

    /// Keep the window glued to the viewport across browser resizes and zooms.
    fn track_viewport(ui: &MainWindow) {
        fit_viewport(ui);
        let weak = ui.as_weak();
        let on_resize = Closure::<dyn FnMut()>::new(move || {
            if let Some(ui) = weak.upgrade() {
                fit_viewport(&ui);
            }
        });
        let _ = web_sys::window().and_then(|w| {
            w.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref())
                .ok()
        });
        on_resize.forget();
    }

    fn connect(ui: &MainWindow, attempt: u32) {
        ui.set_status(SharedString::from(if attempt == 0 {
            "connecting"
        } else {
            "reconnecting"
        }));
        let loc = web_sys::window().expect("window").location();
        let host = loc.host().unwrap_or_else(|_| "127.0.0.1:3333".into());
        let ws = match WebSocket::new(&format!("ws://{host}/ws")) {
            Ok(ws) => Rc::new(ws),
            Err(_) => {
                schedule_reconnect(&ui.as_weak(), attempt);
                return;
            }
        };
        let ui_weak = ui.as_weak();
        let on_open = Closure::<dyn FnMut()>::new(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status(SharedString::from("live"));
            }
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        let ui_weak = ui.as_weak();
        let ws_send = ws.clone();
        ui.on_toggle_plugin(move |id, value| {
            let msg = serde_json::json!({
                "set": {
                    "key": format!("plugins.{id}.enabled"),
                    "value": value,
                }
            });
            let _ = ws_send.send_with_str(&msg.to_string());
        });
        let ws_expand = ws.clone();
        ui.on_expand_archive(move |id| {
            let msg = serde_json::json!({ "expand": id.as_str() });
            let _ = ws_expand.send_with_str(&msg.to_string());
        });
        let ui_filter = ui.as_weak();
        ui.on_filter_expand(move |needle| {
            let Some(ui) = ui_filter.upgrade() else {
                return;
            };
            let body = ui.get_expand_text();
            ui.set_expand_view(SharedString::from(filter_expand(
                body.as_str(),
                needle.as_str(),
            )));
        });
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
            if v.get("type").and_then(|t| t.as_str()) == Some("message") {
                let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("error");
                ui.set_status(SharedString::from(text));
                return;
            }
            if v.get("type").and_then(|t| t.as_str()) == Some("expand") {
                let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
                ui.set_expand_text(SharedString::from(text));
                let needle = ui.get_expand_filter();
                ui.set_expand_view(SharedString::from(filter_expand(text, needle.as_str())));
                return;
            }
            super::apply_snapshot(&ui, &v);
        });
        ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        let ui_weak = ui.as_weak();
        let on_close = Closure::<dyn FnMut()>::new(move || {
            schedule_reconnect(&ui_weak, attempt.saturating_add(1));
        });
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();
    }

    fn schedule_reconnect(ui_weak: &slint::Weak<MainWindow>, attempt: u32) {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_status(SharedString::from("reconnecting"));
        }
        let delay_ms = (1000u32)
            .saturating_mul(1 << attempt.min(4))
            .min(30_000);
        let ui_weak = ui_weak.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            if let Some(ui) = ui_weak.upgrade() {
                connect(&ui, attempt);
            }
        });
        let _ = web_sys::window().and_then(|w| {
            w.set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                delay_ms as i32,
            )
            .ok()
        });
        closure.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_ids_cover_the_d23_set() {
        assert_eq!(
            PAGE_IDS,
            [
                "overview", "plugins", "calls", "sessions", "doctor", "logs", "skills", "stats"
            ]
        );
    }

    #[test]
    fn snapshot_with_sessions_calls_logs_doctor_makes_pages_visible() {
        let v = json!({
            "type": "snapshot",
            "usage": {
                "input": 10, "output": 2, "cache_create": 1, "cache_read": 3,
                "ctt": 5, "turns": [10, 16]
            },
            "plugins": [{
                "id": "cmd", "title": "Bash / cmd", "summary": "s", "enabled": true,
                "saves_tokens": true, "fields": [["rewrite", "true"]],
                "stats": {"input": 0, "output": 0, "cache_read": 0, "cache_create": 0,
                          "est_before": 25, "est_after": 10, "rows": 1}
            }],
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
            "logs": ["2026-09-10 07:00:00 info web/serve: up"],
            "stats": "sessions 1  compact 0  checkpoint 0  no_checkpoint 1  lines 1  malformed 0\n"
        });
        let view = snapshot::parse(&v);
        assert!(
            PAGE_IDS.contains(&"overview")
                && PAGE_IDS.contains(&"sessions")
                && PAGE_IDS.contains(&"calls")
                && PAGE_IDS.contains(&"logs")
                && PAGE_IDS.contains(&"doctor")
                && PAGE_IDS.contains(&"plugins")
                && PAGE_IDS.contains(&"skills")
                && PAGE_IDS.contains(&"stats"),
            "every model page id is a WASM tab"
        );
        assert_eq!(view.usage_ctt, 5);
        assert!(view.overview_savings.contains("cmd: 15 tok"));
        assert_eq!(view.plugins.len(), 1);
        assert_eq!(view.calls.len(), 1);
        assert!(view.calls[0].detail.contains("anthropic"));
        assert!(view.calls[0].detail.contains("ref_id"));
        assert_eq!(view.sessions.len(), 1);
        assert!(view.sessions[0].live);
        assert!(view.sessions[0].detail.contains("project rtok"));
        assert!(view.sessions[0].detail.contains("/v1/messages"));
        assert!(view.doctor_text.contains("hooks 3"));
        assert!(view.doctor_text.contains("rtok"));
        assert_eq!(view.logs, vec!["2026-09-10 07:00:00 info web/serve: up"]);
        assert!(view.stats_text.contains("sessions 1"));
    }

    #[test]
    fn calls_bind_snapshot_ref_ids() {
        let v = json!({
            "type": "snapshot",
            "usage": {},
            "plugins": [],
            "calls": [{
                "id": 7, "ts": 0, "session": "s", "surface": "hook", "kind": "hook", "ok": 1
            }],
            "sessions": [],
            "logs": [],
            "ref_ids": {"7": "abc123"}
        });
        let view = snapshot::parse(&v);
        assert_eq!(view.calls[0].ref_id, "abc123");
        assert!(view.calls[0].detail.contains("ref_id abc123"));
        assert_eq!(super::filter_expand("a\nb-hit\nc", "hit"), "b-hit");
    }

    #[test]
    fn doctor_of_renders_instruction_audit() {
        let v = json!({
            "type": "snapshot",
            "usage": {},
            "plugins": [],
            "calls": [],
            "sessions": [],
            "logs": [],
            "doctor": {
                "hooks_total": 0,
                "hooks_by_event": {},
                "mcp": [],
                "proxy": "direct",
                "proxy_openai": "direct",
                "mcp_tool_search_disabled": false,
                "bash_max_output_length": null,
                "auto_compact_window": null,
                "instructions": {
                    "rows": [
                        {"name": "CLAUDE.md", "tokens": 42, "path": "/p/CLAUDE.md", "warn": true},
                        {"name": "AGENTS.md", "tokens": 10, "path": "/p/AGENTS.md", "warn": false}
                    ],
                    "duplicates": [["same line", ["a.md", "b.md"]]]
                }
            }
        });
        let view = snapshot::parse(&v);
        let marker = "autoCompactWindow (unset)\n";
        let start = view.doctor_text.find(marker).expect("compact line") + marker.len();
        assert_eq!(
            &view.doctor_text[start..],
            "instructions\n  CLAUDE.md 42 tokens /p/CLAUDE.md WARN\n  AGENTS.md 10 tokens /p/AGENTS.md\n  duplicate `same line` in a.md, b.md\n"
        );
    }

    #[test]
    fn missing_doctor_is_a_failed_tick_not_zeros() {
        let v = json!({"type": "snapshot", "doctor": null, "plugins": [], "calls": [], "sessions": [], "logs": [], "usage": {}});
        let view = snapshot::parse(&v);
        assert!(view.doctor_text.contains("did not answer"));
    }

    #[test]
    fn missing_stats_is_a_failed_tick_not_empty() {
        let v = json!({"type": "snapshot", "stats": null, "plugins": [], "calls": [], "sessions": [], "logs": [], "usage": {}});
        let view = snapshot::parse(&v);
        assert!(view.stats_text.contains("did not answer"));
    }
}
