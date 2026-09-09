//! The one operator model behind `rtok web` and `rtok tui` (D23, T15.0).
//!
//! Both surfaces render *these* values; neither owns data. Since T15.11 the reading
//! commands are renderers of the same pages (D27): this module is the only place either
//! surface *or* a reading command touches `Store` / `stats` / `doctor`, so a page cannot
//! grow a query of its own and two windows cannot disagree about the same session.

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

use crate::config::{Config, layers};
use crate::demon::{self, Service};
use crate::doctor;
use crate::measure::{cache, stats};
use crate::plugins::Registry;
use crate::store::{SessionTotals, Store};

/// Everything a surface needs for one refresh.
#[derive(Debug, Serialize)]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Overview page: provider usage totals.
    pub usage: Stats,
    /// Plugins page: one entry per catalogue plugin.
    pub plugins: Vec<PluginPage>,
    /// Sessions page (T25.1): one row per session, newest first.
    pub sessions: Vec<SessionTotals>,
}

/// The shared stats widget: `usage` rows for the overview, `Measurement` rows per plugin.
#[derive(Debug, Default, Serialize)]
pub struct Stats {
    pub input: i64,
    pub output: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub est_before: i64,
    pub est_after: i64,
    pub rows: u64,
}

/// A plugin's page: its manifest, the static copy it contributes through
/// `Plugin::dashboard_page`, and the stats widget when it saves tokens.
#[derive(Debug, Serialize)]
pub struct PluginPage {
    pub id: &'static str,
    pub enabled: bool,
    pub surfaces: Vec<&'static str>,
    pub title: String,
    pub summary: String,
    pub saves_tokens: bool,
    pub fields: Vec<(String, String)>,
    pub stats: Option<Stats>,
}

/// The pages the model offers, each as `(page, snapshot key)` — the wire key that
/// carries the page's numbers; `type` is the wire envelope, not a page. D23: a page
/// that exists on one surface and not the other is a defect, and
/// `tests/surface_parity.rs` holds every surface to this list.
pub fn pages() -> &'static [(&'static str, &'static str)] {
    &[
        ("overview", "usage"),
        ("plugins", "plugins"),
        ("sessions", "sessions"),
    ]
}

/// Open the store at `core.db_path` and read one snapshot. A store that will not open
/// is not fatal for an operator surface: the pages render with zeros.
pub fn snapshot(cfg: &Config) -> Snapshot {
    let store = Store::open(&cfg.core.db_path).ok();
    Model::new(cfg, store.as_ref()).snapshot()
}

/// The Sessions page (T25.1, D27): one row per session, newest first — the same rows
/// the snapshot's `sessions` key carries and `rtok agent sessions` (T25.2) renders.
/// `since` is a `started_at` floor in unix seconds; `0` asks for every session.
/// The store is required, like `plugin_stats`: a page whose whole content is the
/// store's rows reports an unreadable store rather than rendering an empty page.
pub fn sessions(cfg: &Config, since: i64) -> Result<Vec<SessionTotals>> {
    let store = Store::open(&cfg.core.db_path)?;
    Ok(Model::new(cfg, Some(&store)).sessions(since))
}

/// The `rtok stats` page (T15.11): the transcript report — `sessions` here counts transcript
/// files, the report's own definition — with the store's per-API `usage` attached. The store
/// stays optional, as it always was: a store that will not open costs the report its `api`
/// table and nothing else. The store's own definition of a session (`sessions` rows joined to
/// `usage`, D27) is the Sessions page T25.1 adds; both definitions live here, one per page,
/// rather than one number quietly serving two questions.
pub fn stats_report(cfg: &Config) -> Result<stats::Report> {
    let since = stats::parse_since(&cfg.stats.since)?;
    let mut report = stats::collect(
        &cfg.stats.transcripts_dir,
        since,
        &cfg.stats.plugin,
        stats::Replay::from_cfg(cfg),
    )?;
    if let Ok(store) = Store::open(&cfg.core.db_path) {
        let _ = stats::attach_api(&mut report, &store);
    }
    Ok(report)
}

/// The `rtok stats --plugin <id>` page: one catalogue plugin's `Measurement` rows as data.
/// The store is required — this is the one `stats` view that reports an unreadable store.
pub fn plugin_stats(cfg: &Config, plugin: &str) -> Result<Value> {
    let store = Store::open(&cfg.core.db_path)?;
    let rows = store.list_measurements(plugin)?;
    let archive_hits = rows.iter().filter(|r| r.kind == "expand").count();
    let rows: Vec<Value> = rows
        .into_iter()
        .filter(|r| r.kind != "expand")
        .map(|r| {
            json!({
                "kind": r.kind,
                "before": r.before_bytes,
                "after": r.after_bytes,
                "est_before": r.est_before,
                "est_after": r.est_after,
                "ref_id": r.ref_id,
            })
        })
        .collect();
    let mut out = json!({
        "plugin": plugin,
        "archive_hits": archive_hits,
        "rows": rows,
    });
    if plugin == "archive" {
        // T5.4 honesty metric: how often a live-zone pointer had to be expanded.
        let (decisions, expanded) = store.archive_decision_counts()?;
        let rate = if decisions > 0 {
            expanded as f64 / decisions as f64
        } else {
            0.0
        };
        out["decisions"] = json!(decisions);
        out["expanded"] = json!(expanded);
        out["expand_rate"] = json!(rate);
    }
    Ok(out)
}

/// The `rtok stats --cache` page: per-session prompt-cache health from the proxy's
/// `usage` rows.
pub fn cache_health(cfg: &Config) -> Result<Vec<cache::SessionHealth>> {
    let store = Store::open(&cfg.core.db_path)?;
    cache::report(&store)
}

/// The `rtok doctor` page (T15.11): hooks, MCP servers, proxy chains. Every probe runs on
/// this call — a snapshot tick never pays for them.
pub fn doctor(cfg: &Config) -> Result<doctor::Report> {
    doctor::page(cfg)
}

/// One row of the `rtok config show` page: an effective key, its value, and which layer
/// (`default|user|project|env|flag`) set it.
#[derive(Debug, Serialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub source: String,
}

/// The `rtok config show` page (T15.11): every effective key with its origin. Built from
/// the layered figment rather than an extracted `Config`, so `show` still works on a file
/// that would not extract — wrong types are `validate`'s to report, not `show`'s.
/// `config_file` is the `--config` override; `None` resolves through `RTOK_CONFIG` /
/// `<home>/config.toml`.
pub fn config_entries(home: &Path, config_file: Option<&Path>) -> Result<Vec<ConfigEntry>> {
    Config::ensure_user_file(home, config_file)?;
    let fig = layers::figment(home, config_file, None);
    Ok(layers::entries(&fig)
        .into_iter()
        .map(|(key, value, source)| ConfigEntry { key, value, source })
        .collect())
}

/// Reads the pages. Borrows the store so tests can pass an in-memory one.
pub struct Model<'a> {
    cfg: &'a Config,
    store: Option<&'a Store>,
}

impl<'a> Model<'a> {
    pub fn new(cfg: &'a Config, store: Option<&'a Store>) -> Self {
        Self { cfg, store }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: "snapshot",
            usage: self.overview(),
            plugins: self.plugins(),
            sessions: self.sessions(0),
        }
    }

    /// Sessions page (T25.1): every session the store knows, newest first, through the
    /// store's one `session_totals` query — the model does not keep a second reader
    /// (D27), and it does not decide what "live" means: `ended_at` is in the row and
    /// the renderers filter. No store (or one that will not read): an empty page,
    /// like Overview's zeros.
    pub fn sessions(&self, since: i64) -> Vec<SessionTotals> {
        self.store
            .and_then(|s| s.session_totals(since).ok())
            .unwrap_or_default()
    }

    /// Overview: provider usage totals across every API.
    pub fn overview(&self) -> Stats {
        let mut s = Stats::default();
        let Some(store) = self.store else {
            return s;
        };
        if let Ok(rows) = store.usage_by_api() {
            for r in rows {
                s.input += r.input;
                s.output += r.output;
                s.cache_create += r.cache_create;
                s.cache_read += r.cache_read;
            }
        }
        s
    }

    /// Plugins: the catalogue, each with its page and — when it saves tokens — its stats.
    pub fn plugins(&self) -> Vec<PluginPage> {
        Registry::new(self.cfg)
            .pages()
            .into_iter()
            .map(|(m, enabled, mut page)| {
                page.fields.extend(config_fields(m.id, self.cfg));
                PluginPage {
                    id: m.id,
                    enabled,
                    surfaces: m.surfaces.iter().map(|s| s.as_str()).collect(),
                    title: page.title,
                    summary: page.summary,
                    saves_tokens: page.saves_tokens,
                    fields: page.fields,
                    stats: page.saves_tokens.then(|| self.plugin_stats(m.id)),
                }
            })
            .collect()
    }

    fn plugin_stats(&self, id: &str) -> Stats {
        let mut s = Stats::default();
        let Some(store) = self.store else {
            return s;
        };
        if let Ok(rows) = store.list_measurements(id) {
            s.rows = rows.len() as u64;
            for r in rows {
                s.est_before += i64::from(r.est_before);
                s.est_after += i64::from(r.est_after);
            }
        }
        s
    }

    /// Logs page (T15.11): the last `n` log lines, newest first — `[log] lines` when
    /// `None`. The selection is the model's; the numbering and colour are the CLI's.
    pub fn log_lines(&self, n: Option<usize>) -> Vec<String> {
        crate::log::tail(self.cfg, n)
    }

    /// Demon page (T15.11): one row per supervised service, state asked of the kernel
    /// rather than read out of the state file.
    pub fn demon(&self, named: &[Service]) -> Result<Vec<demon::Row>> {
        demon::rows(self.cfg, named)
    }
}

fn config_fields(id: &str, cfg: &Config) -> Vec<(String, String)> {
    let p = &cfg.plugins;
    match id {
        "cmd" => vec![
            kv("rewrite", p.cmd.rewrite),
            kv("trailer_min_lines", p.cmd.trailer_min_lines),
        ],
        "read" => vec![
            kv("default_mode", &p.read.default_mode),
            kv("max_chars", p.read.max_chars),
        ],
        "archive" => vec![
            kv("keep_turns", p.archive.keep_turns),
            kv("min_tokens", p.archive.min_tokens),
        ],
        "inject" => vec![kv("budget_tokens", p.inject.budget_tokens)],
        "guard" => vec![kv("window_turns", p.guard.window_turns)],
        "memory" => vec![
            kv("recall_titles", p.memory.recall_titles),
            kv("recall_tokens", p.memory.recall_tokens),
        ],
        "graph" => vec![kv("max_tokens", p.graph.max_tokens)],
        "toon" => vec![kv("min_rows", p.toon.min_rows)],
        "proxy" => vec![kv("mode", &cfg.proxy.mode), kv("port", cfg.proxy.port)],
        "measure" => vec![kv("since", &cfg.stats.since)],
        _ => vec![],
    }
}

fn kv(k: &str, v: impl ToString) -> (String, String) {
    (k.into(), v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Measurement, Runtime};

    fn fixture() -> Runtime {
        let cx = Runtime::in_memory("dash").unwrap();
        cx.record(&Measurement {
            plugin: "cmd",
            kind: "filter",
            before_bytes: 100,
            after_bytes: 40,
            est_before: 25,
            est_after: 10,
            ref_id: None,
            call_id: None,
        })
        .unwrap();
        cx
    }

    #[test]
    fn snapshot_lists_catalogue_and_hides_stats_on_measure() {
        let cx = fixture();
        let snap = Model::new(&cx.config, Some(&cx.store)).snapshot();
        assert!(snap.plugins.iter().any(|p| p.id == "cmd"));
        let measure = snap.plugins.iter().find(|p| p.id == "measure").unwrap();
        assert!(!measure.saves_tokens);
        assert!(measure.stats.is_none());
        let cmd = snap.plugins.iter().find(|p| p.id == "cmd").unwrap();
        let stats = cmd.stats.as_ref().unwrap();
        assert_eq!(stats.est_before, 25);
        assert_eq!(stats.est_after, 10);
    }

    /// The wire the P19 UI reads: keys and values as `json!` produced them.
    #[test]
    fn json_shape_is_what_p19_pinned() {
        let cx = fixture();
        let v = serde_json::to_value(Model::new(&cx.config, Some(&cx.store)).snapshot()).unwrap();
        assert_eq!(v["type"], "snapshot");
        assert!(v["usage"]["cache_read"].is_i64());
        let cmd = v["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "cmd")
            .unwrap();
        assert_eq!(cmd["stats"]["est_before"], 25);
        assert_eq!(cmd["saves_tokens"], true);
        assert!(cmd["fields"].is_array());
        assert!(cmd["surfaces"].is_array());
        assert_eq!(cmd["title"], "Bash / cmd");
    }

    /// T25.1's Check: the Sessions page carries exactly the store call's numbers (one
    /// reader, D27) and rides the snapshot the `/ws` frame sends — three sessions
    /// across the two hosts the migrations seed, one ended, one zeroed.
    #[test]
    fn sessions_page_matches_the_store_and_rides_the_snapshot() {
        let cx = Runtime::in_memory("dash").unwrap();
        let claude = cx
            .store
            .host_id("claude")
            .unwrap()
            .expect("0002 seeds claude");
        let pi = cx.store.host_id("pi").unwrap().expect("0010 seeds pi");
        cx.store
            .upsert_session("a", Some(claude), Some("rtok"), None, Some("proxy"))
            .unwrap();
        cx.store
            .upsert_session("b", Some(pi), Some("rtok"), None, None)
            .unwrap();
        cx.store
            .upsert_session("c", Some(pi), None, None, None)
            .unwrap();
        let (pid, mid) = cx.store.upsert_model("anthropic", "claude-x").unwrap();
        let call_a = cx
            .store
            .insert_call(
                "a",
                "proxy",
                "api_request",
                Some(claude),
                Some(pid),
                Some(mid),
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        let call_b = cx
            .store
            .insert_call(
                "b",
                "proxy",
                "api_request",
                Some(pi),
                None,
                None,
                None,
                Some("/v1/chat/completions"),
            )
            .unwrap();
        cx.store
            .insert_usage("a", Some("claude-x"), "anthropic", 10, 1, 2, 3, call_a)
            .unwrap();
        cx.store
            .insert_usage("a", Some("claude-x"), "anthropic", 20, 0, 5, 4, call_a)
            .unwrap();
        cx.store
            .insert_usage("b", Some("gpt-x"), "openai_chat", 7, 2, 0, 1, call_b)
            .unwrap();
        cx.store.end_session("b", 2_500).unwrap();

        let model = Model::new(&cx.config, Some(&cx.store));
        assert_eq!(
            model.sessions(0),
            cx.store.session_totals(0).unwrap(),
            "the page is the store call, not a second query"
        );
        let snap = model.snapshot();
        assert_eq!(snap.sessions.len(), 3);
        let a = snap.sessions.iter().find(|r| r.id == "a").unwrap();
        assert_eq!(
            (a.input, a.cache_create, a.cache_read, a.output),
            (30, 1, 7, 7)
        );
        assert_eq!(a.host.as_deref(), Some("claude"));
        assert_eq!(a.provider.as_deref(), Some("anthropic"));
        assert!(a.ended_at.is_none(), "a never ended");
        let b = snap.sessions.iter().find(|r| r.id == "b").unwrap();
        assert_eq!(
            (b.input, b.cache_create, b.cache_read, b.output),
            (7, 2, 0, 1)
        );
        assert_eq!(b.ended_at, Some(2_500));
        let c = snap.sessions.iter().find(|r| r.id == "c").unwrap();
        assert_eq!((c.input, c.output), (0, 0));
        assert_eq!(
            c.last_activity, c.started_at,
            "no rows yet: started is all there is"
        );
        // The wire frame gains the page; `tests/surface_parity.rs` pins the key set.
        let v = serde_json::to_value(&snap).unwrap();
        let rows = v["sessions"].as_array().expect("sessions rides the frame");
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .any(|r| r["id"] == "a" && r["input"] == 30 && r["cache_read"] == 7)
        );
    }
}
