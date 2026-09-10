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
use crate::store::{CallRow, SessionTotals, Store};

/// Everything a surface needs for one refresh.
#[derive(Debug, Serialize)]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Overview page: provider usage totals, CTT and the per-turn series.
    pub usage: Overview,
    /// Plugins page: one entry per catalogue plugin.
    pub plugins: Vec<PluginPage>,
    /// Calls page (T15.5): the last [`CALLS_ROWS`] ledger rows, newest first.
    pub calls: Vec<CallRow>,
    /// Sessions page (T25.1): one row per session, newest first.
    pub sessions: Vec<SessionTotals>,
    /// Doctor page (T15.6): what `rtok doctor` reports — hooks, MCP servers, proxy
    /// chains. `None` when this tick's probes failed; the page renders the failure
    /// rather than zeros, the way an unreadable store renders an empty page.
    pub doctor: Option<doctor::Report>,
    /// Logs page (T15.7): the last `[log] lines` log lines, newest first — the same
    /// selection `rtok logs` screens ([`Model::log_lines`], T15.11). Riding the snapshot
    /// makes the page both surfaces' (D23); `[log] lines` is the frame's bound too.
    pub logs: Vec<String>,
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

/// The Overview page (T15.3): the usage totals plus what the tab draws from them —
/// context-token-turns and the per-turn series behind the sparkline. The totals stay
/// flat under the `usage` key, so the `/ws` frame keeps the shape P19 pinned and the
/// Slint UI reads on untouched.
#[derive(Debug, Default, Serialize)]
pub struct Overview {
    #[serde(flatten)]
    pub totals: Stats,
    /// Σ over sessions of Σ over turns `ctx × turns-after`, where `ctx` is the turn's
    /// input-side tokens (`input + cache_create + cache_read`) and `turns-after` counts
    /// the session's later turns — the usage-row mirror of `stats`' `tokens × remain`.
    /// Output is left out on purpose: it re-enters as a later turn's input or cache,
    /// so counting it again would count it twice.
    pub ctt: i64,
    /// Per-turn `ctx`, sessions oldest-first and request order within a session, kept
    /// to the last [`OVERVIEW_TURNS`]. A `usage` row carries no timestamp, so across
    /// sessions this is session order, not wall-clock order.
    pub turns: Vec<i64>,
    /// Persistent operator warnings for the whole disabled period (e.g. proxy plain
    /// mode). Empty when none; omitted from the wire when empty so the P19 shape stays.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerts: Vec<String>,
}

/// Points of the Overview sparkline: wider than any terminal the TUI draws on, and
/// small enough that the `/ws` frame stays cheap at one snapshot per 2 s tick.
pub const OVERVIEW_TURNS: usize = 120;

/// Rows the Calls page rides (T15.5): the same budget as [`OVERVIEW_TURNS`] — more
/// rows than a terminal shows at once, and the `/ws` frame stays cheap at one
/// snapshot per 2 s tick.
pub const CALLS_ROWS: usize = 120;

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
        ("calls", "calls"),
        ("sessions", "sessions"),
        ("doctor", "doctor"),
        ("logs", "logs"),
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

// ── the report pages (T22.1, D24) ────────────────────────────────────────────
//
// `rtok report` renders; it never computes a number of its own (D24). Every figure it
// prints comes from [`ReportLedgers`] — one store open serves every ledger section, and
// each struct carries the rows its numbers were summed from. The Config and Doctor
// sections come from their own pages above ([`config_entries`], [`doctor`]).

/// The report's window and row counts per ledger.
#[derive(Debug, Serialize)]
pub struct ReportWindow {
    /// The configured window, e.g. `"30d"` — the report's own, not `[stats] since`.
    pub since: String,
    pub from_unix: i64,
    pub to_unix: i64,
    /// `YYYY-MM-DD` UTC, through `log::stamp`'s calendar — one date implementation.
    pub from_date: String,
    pub to_date: String,
    pub db_path: String,
    /// `calls` rows with `ts` in the window — the one ledger whose reader returns row
    /// times, so the only one the window can filter.
    pub calls_in_window: u64,
    pub calls_total: u64,
    /// Whole-ledger counts: the measurements and usage readers return no row time to
    /// filter on, and the report says so beside the numbers.
    pub measurements: u64,
    pub usage: u64,
}

/// One plugin's savings, from `Measurement` rows only (D3: a saving that is not a
/// `Measurement` row does not exist).
#[derive(Debug, Serialize)]
pub struct ReportSavings {
    pub plugin: String,
    pub rows: u64,
    pub est_before: i64,
    pub est_after: i64,
    /// est_before − est_after, net: an `expand` row counts negative, because retrieval
    /// costs tokens.
    pub saved: i64,
}

#[derive(Debug, Serialize)]
pub struct ReportSavingsSection {
    /// One row per catalogue plugin with at least one `Measurement` row.
    pub rows: Vec<ReportSavings>,
    pub total_rows: u64,
    pub total_saved: i64,
    /// Every distinct `Measurement` kind present, sorted — T22.5 matches hook events
    /// against these instead of growing a per-event query.
    pub kinds: Vec<String>,
}

/// Latency of one surface; p50/p95 are nearest-rank over the calls that recorded an `ms`.
#[derive(Debug, Serialize)]
pub struct ReportCalls {
    /// `hook` | `mcp` | `proxy` — the section set's three surfaces.
    pub surface: String,
    pub calls: u64,
    /// Rows with a recorded latency.
    pub timed: u64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ReportCallsSection {
    pub rows: Vec<ReportCalls>,
    pub in_window: u64,
    pub total: u64,
    /// Hook-surface calls in window by event, busiest first (T22.5 rule 4).
    pub hooks: Vec<ReportHook>,
}

/// One hook event's in-window `calls` rows; `"unknown"` when the row names no event.
#[derive(Debug, Serialize)]
pub struct ReportHook {
    pub name: String,
    pub calls: u64,
}

/// Cache busts and their cause, aggregated from the `stats --cache` page.
#[derive(Debug, Serialize)]
pub struct ReportCache {
    pub sessions: u64,
    pub turns: u64,
    pub busts: u64,
    /// Busts per cause (`tools` | `system` | `unknown`), cause order.
    pub by_cause: Vec<(String, u64)>,
    /// Every bust with its turn named, in session/turn order (T22.5 rule 3 keeps
    /// `tools` / `system`). `turn` is 1-based in request order, the numbering
    /// `stats --cache` prints.
    pub detail: Vec<ReportBust>,
}

/// One prompt-cache bust: which session, which turn, what it re-wrote.
#[derive(Debug, Serialize)]
pub struct ReportBust {
    pub session: String,
    pub turn: u64,
    pub cause: String,
    pub cache_create: i64,
    pub cache_read: i64,
}

/// The archive plugin's honesty metric (T5.4): how often a live-zone pointer had to be
/// expanded, and which ids.
#[derive(Debug, Serialize)]
pub struct ReportExpand {
    pub decisions: i64,
    pub expanded: i64,
    /// expanded / decisions; `0.0` with no decisions (the report prints the empty line,
    /// not this zero).
    pub rate: f64,
    /// Archive ids a `rtok expand` froze — `Measurement` rows, kind `expand`, `ref_id`.
    pub expanded_ids: Vec<String>,
    /// Σ est_after over those rows — what the re-reads cost (T22.5 rules 1 and 6).
    pub cost: i64,
    /// How many `expand` rows that sum came from.
    pub cost_rows: u64,
}

/// Every ledger section of the report, from one store open.
#[derive(Debug, Serialize)]
pub struct ReportLedgers {
    pub window: ReportWindow,
    pub savings: ReportSavingsSection,
    pub calls: ReportCallsSection,
    pub cache: ReportCache,
    pub expand: ReportExpand,
}

/// The report's ledger read (T22.1). A store that will not open is an error, as in
/// `cache_health`: a report over an unreadable store is not a report.
pub fn report_ledgers(cfg: &Config) -> Result<ReportLedgers> {
    let store = Store::open(&cfg.core.db_path)?;
    let window = report_window(cfg, &store)?;
    Ok(ReportLedgers {
        calls: report_calls(&store, window.from_unix)?,
        window,
        savings: report_savings(&store)?,
        cache: report_cache(&store)?,
        expand: report_expand(&store)?,
    })
}

fn report_window(cfg: &Config, store: &Store) -> Result<ReportWindow> {
    let since = cfg.report.since.clone();
    let to_unix = crate::log::now() as i64;
    let span = i64::try_from(stats::parse_since(&since)?.as_secs()).unwrap_or(i64::MAX);
    let from_unix = to_unix.saturating_sub(span);
    let date = |secs: i64| crate::log::stamp(secs.max(0) as u64)[..10].to_string();
    let calls = store.calls_after(0, i64::MAX)?;
    let mut measurements = 0;
    for (id, _) in crate::config::CATALOGUE {
        measurements += store.list_measurements(id)?.len() as u64;
    }
    let mut usage = 0;
    for session in store.usage_sessions()? {
        usage += store.usage_rows(&session)?.len() as u64;
    }
    Ok(ReportWindow {
        calls_in_window: calls.iter().filter(|c| c.ts >= from_unix).count() as u64,
        calls_total: calls.len() as u64,
        measurements,
        usage,
        since,
        from_unix,
        to_unix,
        from_date: date(from_unix),
        to_date: date(to_unix),
        db_path: cfg.core.db_path.display().to_string(),
    })
}

fn report_savings(store: &Store) -> Result<ReportSavingsSection> {
    let mut rows = Vec::new();
    let mut kinds = std::collections::BTreeSet::new();
    let (mut total_rows, mut total_saved) = (0u64, 0i64);
    for (id, _) in crate::config::CATALOGUE {
        let ms = store.list_measurements(id)?;
        if ms.is_empty() {
            continue;
        }
        let (mut before, mut after) = (0i64, 0i64);
        for m in &ms {
            before += i64::from(m.est_before);
            after += i64::from(m.est_after);
            kinds.insert(m.kind.clone());
        }
        total_rows += ms.len() as u64;
        total_saved += before - after;
        rows.push(ReportSavings {
            plugin: (*id).to_string(),
            rows: ms.len() as u64,
            est_before: before,
            est_after: after,
            saved: before - after,
        });
    }
    Ok(ReportSavingsSection {
        rows,
        total_rows,
        total_saved,
        kinds: kinds.into_iter().collect(),
    })
}

fn report_calls(store: &Store, from: i64) -> Result<ReportCallsSection> {
    let all = store.calls_after(0, i64::MAX)?;
    let mut rows = Vec::new();
    for surface in ["hook", "mcp", "proxy"] {
        let group: Vec<_> = all
            .iter()
            .filter(|c| c.surface == surface && c.ts >= from)
            .collect();
        let mut ms: Vec<f64> = group.iter().filter_map(|c| c.ms).collect();
        ms.sort_by(|a, b| a.total_cmp(b));
        rows.push(ReportCalls {
            surface: surface.to_string(),
            calls: group.len() as u64,
            timed: ms.len() as u64,
            p50_ms: pct(&ms, 0.5),
            p95_ms: pct(&ms, 0.95),
        });
    }
    Ok(ReportCallsSection {
        in_window: all.iter().filter(|c| c.ts >= from).count() as u64,
        total: all.len() as u64,
        rows,
        hooks: report_hooks(&all, from),
    })
}

/// Hook-surface `calls` rows in window grouped by event name, busiest first.
fn report_hooks(all: &[crate::store::models::Call], from: i64) -> Vec<ReportHook> {
    let mut by_name: std::collections::BTreeMap<String, u64> = Default::default();
    for c in all.iter().filter(|c| c.surface == "hook" && c.ts >= from) {
        *by_name
            .entry(c.name.clone().unwrap_or_else(|| "unknown".into()))
            .or_default() += 1;
    }
    let mut hooks: Vec<ReportHook> = by_name
        .into_iter()
        .map(|(name, calls)| ReportHook { name, calls })
        .collect();
    hooks.sort_by(|a, b| b.calls.cmp(&a.calls).then(a.name.cmp(&b.name)));
    hooks
}

fn report_cache(store: &Store) -> Result<ReportCache> {
    let health = cache::report(store)?;
    let mut by_cause: std::collections::BTreeMap<String, u64> = Default::default();
    let mut detail = Vec::new();
    let (mut turns, mut busts) = (0u64, 0u64);
    for h in &health {
        turns += h.turns.len() as u64;
        busts += h.busts as u64;
        for (i, t) in h.turns.iter().enumerate() {
            if let Some(cause) = &t.bust {
                *by_cause.entry(cause.clone()).or_default() += 1;
                detail.push(ReportBust {
                    session: h.session.clone(),
                    turn: i as u64 + 1,
                    cause: cause.clone(),
                    cache_create: t.cache_create,
                    cache_read: t.cache_read,
                });
            }
        }
    }
    Ok(ReportCache {
        sessions: health.len() as u64,
        turns,
        busts,
        by_cause: by_cause.into_iter().collect(),
        detail,
    })
}

fn report_expand(store: &Store) -> Result<ReportExpand> {
    let (decisions, expanded) = store.archive_decision_counts()?;
    let mut expanded_ids = Vec::new();
    let (mut cost, mut cost_rows) = (0i64, 0u64);
    for m in store.list_measurements("archive")? {
        if m.kind == "expand" {
            cost += i64::from(m.est_after);
            cost_rows += 1;
            if let Some(id) = m.ref_id {
                expanded_ids.push(id);
            }
        }
    }
    Ok(ReportExpand {
        rate: if decisions > 0 {
            expanded as f64 / decisions as f64
        } else {
            0.0
        },
        decisions,
        expanded,
        expanded_ids,
        cost,
        cost_rows,
    })
}

/// Nearest-rank percentile: the `ceil(p·n)`-th smallest value; one sample gives p50 = p95
/// = it. `None` when nothing was timed — the report prints a dash, not a zero.
fn pct(sorted: &[f64], p: f64) -> Option<f64> {
    let idx = ((p * sorted.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted.get(idx).copied()
}

/// The `rtok doctor` page (T15.11): hooks, MCP servers, proxy chains. Every probe runs on
/// this call — the CLI and a cold snapshot miss pay once; each probe is bounded by its
/// `[doctor]` timeout. Surfaces re-read through [`doctor_for_snapshot`] so a 2 s tick
/// cannot re-spawn every MCP server (T15.6 hot path).
pub fn doctor(cfg: &Config) -> Result<doctor::Report> {
    doctor::page(cfg)
}

/// How long a snapshot may reuse the last doctor report. Shorter than a sitting at the
/// Doctor tab feels stale; far longer than the 2 s tick, so MCP spawns are not the tick.
const DOCTOR_SNAPSHOT_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// Snapshot-only doctor: same [`doctor`] probes, cached briefly so `rtok tui` / `rtok web`
/// ticks do not spawn MCP servers every two seconds. `rtok doctor` still goes through
/// [`doctor`] uncached. Tests bypass the cache so Check pins stay byte-identical to a
/// direct probe.
fn doctor_for_snapshot(cfg: &Config) -> Option<doctor::Report> {
    if cfg!(test) {
        return doctor(cfg).ok();
    }
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    struct Entry {
        at: Instant,
        key: String,
        report: Option<doctor::Report>,
    }
    static CACHE: OnceLock<Mutex<Option<Entry>>> = OnceLock::new();
    let key = format!(
        "{} {} {}",
        cfg.doctor.settings_path.display(),
        cfg.doctor.claude_json.display(),
        cfg.doctor.mcp_json.display()
    );
    let lock = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock()
        && let Some(e) = guard.as_ref()
        && e.key == key
        && e.at.elapsed() < DOCTOR_SNAPSHOT_TTL
    {
        return e.report.clone();
    }
    let report = doctor(cfg).ok();
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(Entry {
            at: Instant::now(),
            key,
            report: report.clone(),
        });
    }
    report
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
            calls: self.calls(),
            sessions: self.sessions(0),
            // The one doctor query (D27): the snapshot carries what `rtok doctor`
            // renders, so neither surface grows a probe of its own. Cached briefly —
            // a failed or in-flight tick is `None`, never a failed snapshot.
            doctor: doctor_for_snapshot(self.cfg),
            logs: self.log_lines(None),
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

    /// Calls page (T15.5): the last [`CALLS_ROWS`] ledger rows, newest first, through
    /// the store's one `recent_calls` read — the model keeps no second reader (D27).
    /// No store, or one that will not read: an empty page, like Overview's zeros.
    /// When the proxy is in plain mode, in-memory live passthrough rows are prepended
    /// (negative ids; kind `live_passthrough`) so TUI/web still show traffic flowing.
    pub fn calls(&self) -> Vec<CallRow> {
        let mut rows = self
            .store
            .and_then(|s| s.recent_calls(CALLS_ROWS as i64).ok())
            .unwrap_or_default();
        let live = live_calls(self.cfg);
        if !live.is_empty() {
            let mut merged: Vec<CallRow> = live
                .into_iter()
                .enumerate()
                .map(|(i, c)| live_as_call_row(-(i as i32 + 1), c))
                .collect();
            merged.append(&mut rows);
            merged.truncate(CALLS_ROWS);
            return merged;
        }
        rows
    }

    /// Overview: provider usage totals across every API, plus the CTT and per-turn
    /// series the tab draws (T15.3). The reads are the ones `stats --cache` already
    /// uses (`usage_sessions` + `usage_rows`), so this adds no `Store` method (D27).
    /// No store, or one that will not read: zeros, like before.
    pub fn overview(&self) -> Overview {
        let mut out = Overview {
            alerts: crate::proxy::live::alerts(self.cfg),
            ..Overview::default()
        };
        let Some(store) = self.store else {
            return out;
        };
        if let Ok(rows) = store.usage_by_api() {
            for r in rows {
                out.totals.input += r.input;
                out.totals.output += r.output;
                out.totals.cache_create += r.cache_create;
                out.totals.cache_read += r.cache_read;
            }
        }
        if let Ok(sessions) = store.usage_sessions() {
            let mut turns = Vec::new();
            for session in &sessions {
                let Ok(mut rows) = store.usage_rows(session) else {
                    continue;
                };
                rows.reverse(); // newest-first → request order
                let after = |j: usize| rows.len().saturating_sub(j + 1) as i64;
                for (j, r) in rows.iter().enumerate() {
                    let ctx = r.input + r.cache_create + r.cache_read;
                    out.ctt = out.ctt.saturating_add(ctx.saturating_mul(after(j)));
                    turns.push(ctx);
                }
            }
            let skip = turns.len().saturating_sub(OVERVIEW_TURNS);
            out.turns = turns.into_iter().skip(skip).collect();
        }
        out
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

fn live_calls(cfg: &Config) -> Vec<crate::proxy::LiveCall> {
    if cfg.proxy.enabled && cfg.core.enabled {
        return Vec::new();
    }
    // Prefer the running proxy's /live (cross-process); fall back to this process's ring.
    fetch_live(cfg).unwrap_or_else(crate::proxy::live::snapshot)
}

fn fetch_live(cfg: &Config) -> Option<Vec<crate::proxy::LiveCall>> {
    let url = format!("http://{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let timeout = std::time::Duration::from_millis(cfg.doctor.probe_timeout_ms.max(100));
    let body = http_get_body(&url, "/live", timeout)?;
    serde_json::from_str(&body).ok()
}

fn http_get_body(base: &str, path: &str, timeout: std::time::Duration) -> Option<String> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    let rest = base.split("://").nth(1).unwrap_or(base);
    let hostport = rest.split('/').next().unwrap_or(rest);
    let addr = hostport.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    text.split("\r\n\r\n").nth(1).map(str::to_string)
}

fn live_as_call_row(id: i32, c: crate::proxy::LiveCall) -> CallRow {
    CallRow {
        id,
        ts: c.ts,
        session: "(live)".into(),
        surface: "proxy".into(),
        kind: "live_passthrough".into(),
        plugin: None,
        name: Some(c.path),
        parent_id: None,
        ms: Some(c.ms),
        ok: if c.status < 400 { 1 } else { 0 },
        error: None,
        host: None,
        provider: c.provider,
        model: c.model,
        api: None,
        input: Some(c.request_bytes as i64),
        cache_create: None,
        cache_read: None,
        output: Some(c.response_bytes as i64),
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
        "proxy" => vec![
            kv("enabled", cfg.proxy.enabled),
            kv("mode", &cfg.proxy.mode),
            kv("port", cfg.proxy.port),
        ],
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

    /// T15.3's Check: the Overview page carries the store's own sums — totals, CTT and
    /// the per-turn series — so the tab, `rtok stats --json`'s `api` table and the web
    /// Overview agree on one store. Two sessions: `a` with two turns, `b` with one.
    #[test]
    fn overview_carries_totals_ctt_and_turns() {
        let cx = Runtime::in_memory("dash").unwrap();
        cx.store.insert_proxy_turn("a", 10, 0, 0, 1).unwrap();
        cx.store.insert_proxy_turn("a", 4, 0, 12, 2).unwrap();
        cx.store.insert_proxy_turn("b", 100, 20, 0, 5).unwrap();
        let over = Model::new(&cx.config, Some(&cx.store)).overview();
        assert_eq!(
            (
                over.totals.input,
                over.totals.cache_create,
                over.totals.cache_read,
                over.totals.output
            ),
            (114, 20, 12, 8)
        );
        // `a`: turn ctx 10 × 1 turn after, then ctx 16 × 0; `b`: one turn × 0.
        assert_eq!(over.ctt, 10);
        assert_eq!(over.turns, vec![10, 16, 120]);
        // Gate P15: the same sums `rtok stats --json` prints in its `api` table —
        // `attach_api` reads the same `usage_by_api` rows the Overview sums.
        let mut report = stats::Report::default();
        stats::attach_api(&mut report, &cx.store).unwrap();
        let api = report.api.get("anthropic").expect("one api on record");
        assert_eq!(
            (api.input, api.cache_create, api.cache_read, api.output),
            (
                over.totals.input,
                over.totals.cache_create,
                over.totals.cache_read,
                over.totals.output
            )
        );
        // The wire keeps the P19 shape: totals flat under `usage`, CTT beside them.
        let v = serde_json::to_value(Model::new(&cx.config, Some(&cx.store)).snapshot()).unwrap();
        assert_eq!(v["usage"]["input"], 114);
        assert_eq!(v["usage"]["ctt"], 10);
        assert_eq!(v["usage"]["turns"], serde_json::json!([10, 16, 120]));
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

    /// T15.6's Check: the Doctor page rides the snapshot — the same `Report` the `doctor`
    /// page function returns — so the tab, `rtok doctor` and the web frame agree (D23).
    #[test]
    fn doctor_page_rides_the_snapshot() {
        let cx = Runtime::in_memory("dash").unwrap();
        let mut cfg = cx.config.clone();
        let dir = std::env::temp_dir().join(format!("rtok-model-doctor-{}", std::process::id()));
        cfg.doctor.settings_path = dir.join("missing-settings.json");
        cfg.doctor.claude_json = dir.join("missing-claude.json");
        cfg.doctor.mcp_json = dir.join("missing-mcp.json");
        let snap = Model::new(&cfg, Some(&cx.store)).snapshot();
        let direct = doctor(&cfg).expect("doctor page");
        let carried = snap
            .doctor
            .as_ref()
            .expect("snapshot carries the doctor page");
        assert_eq!(carried.to_text(), direct.to_text());
        // The wire frame gains the page; `tests/surface_parity.rs` pins the key set.
        let v = serde_json::to_value(&snap).unwrap();
        assert!(
            v["doctor"]["hooks_total"].is_number(),
            "doctor rides the frame"
        );
    }

    /// T15.5's Check: the Calls page is the store's one `recent_calls` read verbatim
    /// (D27), newest first, and rides the snapshot's `calls` key — so the tab and the
    /// `/ws` frame carry the same rows. One proxy call with its usage row and host,
    /// provider and model slugs; one hook call with none of those.
    #[test]
    fn calls_page_is_the_store_read_and_rides_the_snapshot() {
        let cx = Runtime::in_memory("dash").unwrap();
        let claude = cx
            .store
            .host_id("claude")
            .unwrap()
            .expect("0002 seeds claude");
        cx.store
            .upsert_session("s", Some(claude), None, None, Some("proxy"))
            .unwrap();
        let (pid, mid) = cx.store.upsert_model("anthropic", "claude-x").unwrap();
        let hook = cx
            .store
            .insert_call("s", "hook", "hook", None, None, None, None, Some("Stop"))
            .unwrap();
        let api = cx
            .store
            .insert_call(
                "s",
                "proxy",
                "api_request",
                Some(claude),
                Some(pid),
                Some(mid),
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        cx.store.set_call_ms(api, 12.5).unwrap();
        cx.store
            .insert_usage("s", Some("claude-x"), "anthropic", 10, 1, 2, 3, api)
            .unwrap();

        let model = Model::new(&cx.config, Some(&cx.store));
        assert_eq!(
            model.calls(),
            cx.store.recent_calls(CALLS_ROWS as i64).unwrap(),
            "the page is the store read, not a second query"
        );
        let rows = model.calls();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, api, "newest first");
        assert_eq!(rows[0].api.as_deref(), Some("anthropic"));
        assert_eq!(
            (
                rows[0].input,
                rows[0].cache_create,
                rows[0].cache_read,
                rows[0].output
            ),
            (Some(10), Some(1), Some(2), Some(3))
        );
        assert_eq!(rows[0].ms, Some(12.5));
        assert_eq!(rows[0].host.as_deref(), Some("claude"));
        assert_eq!(rows[1].id, hook);
        assert_eq!(rows[1].api, None, "a hook call carries no usage linkage");
        // The wire frame gains the page; `tests/surface_parity.rs` pins the key set.
        let v = serde_json::to_value(model.snapshot()).unwrap();
        let wire = v["calls"].as_array().expect("calls rides the frame");
        assert_eq!(wire.len(), 2);
        assert_eq!(wire[0]["api"], "anthropic");
        assert_eq!(wire[0]["ms"], 12.5);
        assert_eq!(wire[1]["api"], serde_json::json!(null));
    }

    /// The report's percentile, pinned where it is defined: nearest rank, so
    /// `tests/report.rs` can assert the p50/p95 the fixture's ms values must produce.
    #[test]
    fn report_pct_is_nearest_rank() {
        assert_eq!(pct(&[], 0.95), None);
        assert_eq!(pct(&[2.0, 4.0], 0.5), Some(2.0));
        assert_eq!(pct(&[2.0, 4.0], 0.95), Some(4.0));
        assert_eq!(pct(&[10.0], 0.5), Some(10.0));
        assert_eq!(pct(&[10.0], 0.95), Some(10.0));
    }
}
