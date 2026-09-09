//! The one operator model behind `rtok web` and `rtok tui` (D23, T15.0).
//!
//! Both surfaces render *these* values; neither owns data. This module is the only
//! place either surface touches `Store` / `stats` / `doctor`, so a page cannot grow a
//! query of its own and drift from `rtok stats`.

use serde::Serialize;

use crate::config::Config;
use crate::plugins::Registry;
use crate::store::Store;

/// Everything a surface needs for one refresh.
#[derive(Debug, Serialize)]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Overview page: provider usage totals.
    pub usage: Stats,
    /// Plugins page: one entry per catalogue plugin.
    pub plugins: Vec<PluginPage>,
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

/// Open the store at `core.db_path` and read one snapshot. A store that will not open
/// is not fatal for an operator surface: the pages render with zeros.
pub fn snapshot(cfg: &Config) -> Snapshot {
    let store = Store::open(&cfg.core.db_path).ok();
    Model::new(cfg, store.as_ref()).snapshot()
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
        }
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
}
