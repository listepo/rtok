//! The one operator model behind `rtok web` and `rtok tui` (D23, T15.0).
//!
//! Both surfaces render *these* values; neither owns data. Since T15.11 the reading
//! commands are renderers of the same pages (D27): this module is the only place either
//! surface *or* a reading command touches `Store` / `stats` / `doctor`, so a page cannot
//! grow a query of its own and two windows cannot disagree about the same session.

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{Config, layers};
use crate::demon::{self, Service};
use crate::doctor;
use crate::measure::{cache, stats};
use crate::plugins::Registry;
use crate::store::{CallRow, SessionTotals, Store};

/// Everything a surface needs for one refresh. `Default` is the empty frame a surface
/// paints while its first read is still running.
#[derive(Debug, Default, Serialize)]
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
    /// Set when the store will not open or this tick's doctor probe failed (T60.6). Both
    /// surfaces render it as a banner instead of an empty page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Skills page (T63.1): T61.3 listing joined to T61.1 resident/invocations.
    pub skills: SkillsPage,
    /// Archive ids keyed by `calls[].id` (T60.4). Both surfaces read this map; neither
    /// queries the store for an expand handle (D23 / D27).
    #[serde(default)]
    pub ref_ids: BTreeMap<i32, String>,
    /// Stats page (T227): `stats --price`'s table plus `stats --cache`'s table, folded
    /// into one page (D27) — [`stats_page_text`], built from the same transcript scan
    /// [`stats_skills`] already runs for the Skills page (no second aggregation). `None`
    /// when this tick's transcripts read failed.
    pub stats: Option<String>,
    /// Graph page (T230): `graph status`'s index health (rows, files, the T68.3
    /// pending/staleness set, `indexed_at`) plus `graph dead`'s unreferenced-definition
    /// list, folded into one page (D27) — [`graph_page_text`], read from the store on
    /// this tick (no re-indexing). `None` when the `graph` feature is off or the store
    /// read failed.
    pub graph: Option<String>,
    /// Hosts page (T231): `agents list`'s blocks — kind, detected version, installed
    /// surfaces, config path — one per known host variant (D27), so `agents list` /
    /// `agents info` can join `COMMAND_PAGES`. [`hosts_page_text`] reuses the same
    /// probe `agents_list`'s JSON form calls (T168's `--version` noise filter and all)
    /// behind a cache: never blocks a 2 s tick on a cold or stale probe — the tick
    /// renders the last known text, or "probing hosts…" before the first one lands.
    pub hosts: String,
    /// Config page (T228): `rtok config show --sources`'s rows, through
    /// [`config_page_text`] (D27, no second layering). `None` on a failed tick.
    pub config: Option<String>,
    /// Services page (T229): `demon status`'s per-service rows plus `otel status`'s
    /// exporter health, through [`services_page_text`] (D27, no second reader —
    /// [`Model::demon`] and [`otel_status`] already build both). `None` on a failed
    /// tick.
    pub services: Option<String>,
}

/// The shared stats widget: `usage` rows for the overview, `Measurement` rows per plugin.
#[derive(Debug, Default, Clone, Copy, Serialize)]
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

/// Sessions the Sessions page rides (T25.1): the same budget as [`CALLS_ROWS`]. The
/// page is read on every 2 s tick, so it aggregates the newest sessions, not every
/// session the store ever recorded.
pub const SESSIONS_ROWS: usize = 120;

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

/// One `rtok agents list` row: the same host variant `agents::list` prints.
#[derive(Debug, Serialize)]
pub struct AgentListRow {
    pub host: &'static str,
    pub kind: &'static str,
    pub name: &'static str,
    pub present: bool,
    pub app: Option<String>,
    pub version: Option<String>,
    pub config: Vec<String>,
    pub modules: Vec<crate::agents::ModuleRow>,
    pub plugins: Vec<crate::agents::PluginRow>,
}

/// Skills page (T63.1, D23): one row per skill the host lists.
#[derive(Debug, Default, Clone, Serialize)]
pub struct SkillsPage {
    /// Totals: listed, desc bytes ≈ tokens/req (chars/4, research.md §10.2), resident, input share.
    pub header: String,
    pub rows: Vec<SkillPageRow>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SkillPageRow {
    pub name: String,
    pub source: String,
    pub desc_chars: usize,
    pub body_bytes: u64,
    pub invocations: u64,
    pub resident: u64,
    pub last_invoked: String,
    pub never: bool,
}

/// `rtok otel status` as data: endpoint, watermarks, pending rows, last exporter line.
#[derive(Debug, Serialize)]
pub struct OtelStatus {
    pub endpoint: Option<String>,
    pub calls_mark: i64,
    pub calls_pending: i64,
    pub logs_mark: i64,
    pub logs_pending: i64,
    pub sessions_mark: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<OtelLastLog>,
}

#[derive(Debug, Serialize)]
pub struct OtelLastLog {
    pub level: String,
    pub name: String,
    pub message: String,
}

/// Memory plugin status for `rtok memory status` and `--json` (T69.4).
#[derive(Debug, Serialize)]
pub struct MemoryStatus {
    pub since: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub notes: MemoryNotesTotals,
    pub recall: MemoryRecallTotals,
    pub calls: MemoryMcpCalls,
    pub by_project: Vec<MemoryProjectBlock>,
}

#[derive(Debug, Serialize)]
pub struct MemoryNotesTotals {
    pub live: u64,
    pub pinned: u64,
    pub retired: u64,
    pub body_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct MemoryRecallTotals {
    pub recalls: u64,
    pub stood_for_bytes: i64,
    pub injected_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct MemoryMcpCalls {
    pub mem_search: u64,
    pub mem_get: u64,
}

#[derive(Debug, Serialize)]
pub struct MemoryProjectBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub kinds: Vec<MemoryKindAgg>,
}

#[derive(Debug, Serialize)]
pub struct MemoryKindAgg {
    pub kind: String,
    pub live: u64,
    pub pinned: u64,
    pub retired: u64,
    pub body_bytes: i64,
    pub oldest_ts: i64,
    pub newest_ts: i64,
}

/// One `rtok memory status` snapshot — the same type `--json` prints (T69.4).
pub fn memory_status(
    cfg: &Config,
    project: Option<&str>,
    since: Option<&str>,
) -> Result<MemoryStatus> {
    let since_label = since.unwrap_or(&cfg.stats.since);
    let span = stats::parse_since(since_label)?;
    let since_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        - i64::try_from(span.as_secs()).unwrap_or(i64::MAX);
    let store = Store::open(&cfg.core.db_path)?;
    let aggs = store.memory_note_aggs(project)?;
    let mut notes = MemoryNotesTotals {
        live: 0,
        pinned: 0,
        retired: 0,
        body_bytes: 0,
    };
    let mut by_project: BTreeMap<Option<String>, Vec<MemoryKindAgg>> = BTreeMap::new();
    for row in aggs {
        notes.live += row.live;
        notes.pinned += row.pinned;
        notes.retired += row.retired;
        notes.body_bytes += row.body_bytes;
        by_project
            .entry(row.project.clone())
            .or_default()
            .push(MemoryKindAgg {
                kind: row.kind,
                live: row.live,
                pinned: row.pinned,
                retired: row.retired,
                body_bytes: row.body_bytes,
                oldest_ts: row.oldest_ts,
                newest_ts: row.newest_ts,
            });
    }
    let (recalls, stood_for_bytes, injected_bytes) = store.memory_recall_totals(since_unix)?;
    let (mem_search, mem_get) = store.memory_mcp_calls(since_unix)?;
    Ok(MemoryStatus {
        since: since_label.to_string(),
        project: project.map(str::to_string),
        notes,
        recall: MemoryRecallTotals {
            recalls,
            stood_for_bytes,
            injected_bytes,
        },
        calls: MemoryMcpCalls {
            mem_search,
            mem_get,
        },
        by_project: by_project
            .into_iter()
            .map(|(project, kinds)| MemoryProjectBlock { project, kinds })
            .collect(),
    })
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
        ("skills", "skills"),
        ("stats", "stats"),
        ("graph", "graph"),
        ("hosts", "hosts"),
        ("config", "config"),
        ("services", "services"),
    ]
}

/// Open the store at `core.db_path` and read one snapshot. A store that will not open
/// is not fatal for an operator surface: the pages render with zeros and [`Snapshot::error`]
/// carries why (T60.6).
pub fn snapshot(cfg: &Config) -> Snapshot {
    let opened = Store::open(&cfg.core.db_path);
    let store_err = opened.as_ref().err().map(|e| e.to_string());
    let store = opened.ok();
    let mut snap = Model::new(cfg, store.as_ref()).snapshot();
    if let Some(msg) = store_err {
        snap.error = Some(msg);
    } else if snap.doctor.is_none() {
        snap.error = Some("doctor probe failed".into());
    }
    snap
}

/// The Sessions page (T25.1, D27): one row per session, newest first — the same rows
/// the snapshot's `sessions` key carries and `rtok agents sessions` (T25.2) renders.
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
        let _ = stats::attach_bash_cmd(&mut report, &store);
        let _ = stats::attach_checkpoint_notes(&mut report, &store);
        if cfg.stats.price {
            let _ = stats::attach_costs(&mut report, &store, &cfg.stats.prices);
        }
    }
    stats::attach_codex(&mut report, &cfg.stats.codex_dir, since);
    Ok(report)
}

/// The `rtok stats --plugin <id>` page: one catalogue plugin's `Measurement` rows as data.
/// The store is required — this is the one `stats` view that reports an unreadable store.
pub fn plugin_stats(cfg: &Config, plugin: &str) -> Result<Value> {
    let store = Store::open(&cfg.core.db_path)?;
    let rows = store.list_measurements(plugin)?;
    // `archive_hits` still isolates the expand-rate metric, but `rows` no longer drops
    // expand rows from the listing (T207): they are a cost, not a saving, so they count
    // negative (`ReportSavings::saved`'s contract) rather than being hidden — dropping
    // them here made this page's row count disagree with the Plugins page's.
    let archive_hits = rows.iter().filter(|r| r.kind == "expand").count();
    let rows: Vec<Value> = rows
        .into_iter()
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
    /// One row per plugin with at least one `Measurement` row — every plugin the
    /// ledger has seen, not just the catalogue (T207).
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

/// One token sink ranked by raw bytes in `Measurement` rows (T59.8).
#[derive(Clone, Debug, Serialize)]
pub struct ReportSink {
    /// `read` | `cmd` | `mcp`
    pub class: String,
    /// File path, command stem, or `server/tool`.
    pub sink: String,
    pub before_bytes: i64,
    pub rows: u64,
    /// Which rtok switch would shorten this sink.
    pub switch: String,
}

#[derive(Debug, Serialize)]
pub struct ReportSinksSection {
    pub rows: Vec<ReportSink>,
}

/// Every ledger section of the report, from one store open.
#[derive(Debug, Serialize)]
pub struct ReportLedgers {
    pub window: ReportWindow,
    pub savings: ReportSavingsSection,
    pub sinks: ReportSinksSection,
    pub calls: ReportCallsSection,
    pub cache: ReportCache,
    pub expand: ReportExpand,
}

/// The report's ledger read (T22.1). A store that will not open is an error, as in
/// `cache_health`: a report over an unreadable store is not a report.
pub fn report_ledgers(cfg: &Config) -> Result<ReportLedgers> {
    let store = Store::open(&cfg.core.db_path)?;
    // One scan of the whole `calls` table, shared: each section used to run its own.
    let calls = store.calls_after(0, i64::MAX)?;
    let window = report_window(cfg, &store, &calls)?;
    Ok(ReportLedgers {
        calls: report_calls(&calls, window.from_unix),
        window,
        savings: report_savings(&store)?,
        sinks: report_sinks(&store, cfg)?,
        cache: report_cache(&store)?,
        expand: report_expand(&store)?,
    })
}

fn report_window(
    cfg: &Config,
    store: &Store,
    calls: &[crate::store::models::Call],
) -> Result<ReportWindow> {
    let since = cfg.report.since.clone();
    let to_unix = crate::log::now() as i64;
    let span = i64::try_from(stats::parse_since(&since)?.as_secs()).unwrap_or(i64::MAX);
    let from_unix = to_unix.saturating_sub(span);
    let date = |secs: i64| crate::log::stamp(secs.max(0) as u64)[..10].to_string();
    Ok(ReportWindow {
        calls_in_window: calls.iter().filter(|c| c.ts >= from_unix).count() as u64,
        calls_total: calls.len() as u64,
        // T207: one `COUNT(*)` each, the whole ledger — the old per-catalogue-plugin
        // and per-session loops undercounted (out-of-tree plugins never showed up
        // despite the "Whole-ledger counts" label below) and cost an N+1 per report.
        measurements: store.count_measurements()? as u64,
        usage: store.count_usage()? as u64,
        since,
        from_unix,
        to_unix,
        from_date: date(from_unix),
        to_date: date(to_unix),
        db_path: cfg.core.db_path.display().to_string(),
    })
}

fn report_savings(store: &Store) -> Result<ReportSavingsSection> {
    // T207: one grouped read for every plugin the ledger has ever recorded a
    // `Measurement` for — not just the catalogue, so out-of-tree/WASM plugins count —
    // folded from (plugin, kind) down to (plugin). `saved` keeps an `expand` group
    // negative (`ReportSavings::saved`'s contract): it is a cost, not a saving, so it
    // nets out of the total instead of being dropped from it.
    let mut by_plugin: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new();
    let mut kinds = std::collections::BTreeSet::new();
    for t in store.measurement_totals()? {
        kinds.insert(t.kind);
        let e = by_plugin.entry(t.plugin).or_insert((0, 0, 0));
        e.0 += t.rows;
        e.1 += t.est_before;
        e.2 += t.est_after;
    }
    let (mut total_rows, mut total_saved) = (0u64, 0i64);
    let rows = by_plugin
        .into_iter()
        .map(|(plugin, (n, before, after))| {
            total_rows += n as u64;
            total_saved += before - after;
            ReportSavings {
                plugin,
                rows: n as u64,
                est_before: before,
                est_after: after,
                saved: before - after,
            }
        })
        .collect();
    Ok(ReportSavingsSection {
        rows,
        total_rows,
        total_saved,
        kinds: kinds.into_iter().collect(),
    })
}

fn report_sinks(store: &Store, cfg: &Config) -> Result<ReportSinksSection> {
    let mut by: BTreeMap<(String, String), (i64, u64)> = BTreeMap::new();
    for (plugin, _) in crate::config::CATALOGUE {
        for m in store.list_measurements(plugin)? {
            if m.before_bytes <= 0 {
                continue;
            }
            let (class, sink) = sink_label(plugin, &m.kind, m.ref_id.as_deref());
            let e = by.entry((class, sink)).or_insert((0, 0));
            e.0 += m.before_bytes;
            e.1 += 1;
        }
    }
    let mut rows: Vec<ReportSink> = by
        .into_iter()
        .map(|((class, sink), (before_bytes, rows))| {
            let switch = sink_switch(cfg, &class, &sink);
            ReportSink {
                class,
                sink,
                before_bytes,
                rows,
                switch,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.before_bytes
            .cmp(&a.before_bytes)
            .then_with(|| a.sink.cmp(&b.sink))
    });
    rows.truncate(10);
    Ok(ReportSinksSection { rows })
}

fn sink_label(plugin: &str, kind: &str, ref_id: Option<&str>) -> (String, String) {
    match plugin {
        "cmd" if kind == "wrap" => {
            let sink = ref_id
                .and_then(|r| r.rsplit_once(':').map(|(s, _)| s.to_string()))
                .unwrap_or_else(|| "mcp".into());
            ("mcp".into(), sink)
        }
        "cmd" => {
            let stem = ref_id
                .and_then(|r| r.split_once(':').map(|(s, _)| s.to_string()))
                .unwrap_or_else(|| kind.to_string());
            ("cmd".into(), stem)
        }
        "read" => ("read".into(), ref_id.unwrap_or("unknown").to_string()),
        _ => (plugin.to_string(), ref_id.unwrap_or(kind).to_string()),
    }
}

fn sink_switch(cfg: &Config, class: &str, sink: &str) -> String {
    match class {
        "read" => format!(
            "[plugins.read] default_mode = {}",
            cfg.plugins.read.default_mode
        ),
        "cmd" => {
            // Without the `cmd` plugin `no rule` reads as "no rule yet" (T0.4: one
            // plugin feature must build alone).
            #[cfg(feature = "cmd")]
            let has = crate::plugins::cmd::rules::defaults()
                .iter()
                .any(|r| r.match_cmd == sink);
            #[cfg(not(feature = "cmd"))]
            let has = false;
            if has {
                format!("[{sink}] rule")
            } else {
                format!("[{sink}] rule (default head/tail)")
            }
        }
        "mcp" => "rtok mcp -- <server> (wrap)".into(),
        _ => "none: already shortened".into(),
    }
}

fn report_calls(all: &[crate::store::models::Call], from: i64) -> ReportCallsSection {
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
    ReportCallsSection {
        in_window: all.iter().filter(|c| c.ts >= from).count() as u64,
        total: all.len() as u64,
        rows,
        hooks: report_hooks(all, from),
    }
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

/// One `agents list` row for a host variant — shared by the full list and the
/// `info`-filtered form.
fn agent_row(
    a: &dyn crate::agents::Agent,
    v: &crate::agents::Variant,
    cfg: &Config,
) -> AgentListRow {
    let present = crate::agents::present(a, v, cfg);
    let app = crate::agents::app_path(v).map(|p| p.display().to_string());
    let version = app.as_ref().map(|_| crate::agents::app_version(v));
    let config = a
        .files(cfg, v.kind)
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let (modules, plugins) = if present {
        (
            crate::agents::module_rows(a, v.kind, cfg),
            crate::agents::plugin_rows(a, v.kind, cfg),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    AgentListRow {
        host: a.id(),
        kind: v.kind.as_str(),
        name: v.name,
        present,
        app,
        version,
        config,
        modules,
        plugins,
    }
}

/// `rtok agents list` as data — one row per known host variant.
pub fn agents_list(cfg: &Config) -> Vec<AgentListRow> {
    let mut out = Vec::new();
    for id in crate::agents::HOSTS {
        let Some(a) = crate::agents::host(id) else {
            continue;
        };
        for v in a.variants() {
            out.push(agent_row(a, v, cfg));
        }
    }
    out
}

/// `rtok agents info <host>` as data — the same variant blocks [`agents_list`] prints,
/// filtered to `ids` (already-resolved host ids).
pub fn agents_listed(cfg: &Config, ids: &[&str]) -> Vec<AgentListRow> {
    crate::agents::visit_hosts(ids, |a, v| agent_row(a, v, cfg))
}

/// `rtok otel status` as data — the same watermarks the table prints.
pub fn otel_status(cfg: &Config) -> Result<OtelStatus> {
    let store = Store::open(&cfg.core.db_path)?;
    let (calls_pending, logs_pending) = store.otel_pending()?;
    let last = store.last_log("otel")?.map(|l| OtelLastLog {
        level: l.level,
        name: l.name,
        message: l.message,
    });
    Ok(OtelStatus {
        endpoint: cfg.otel.resolve().map(|ep| ep.url),
        calls_mark: store.otel_mark("calls")?,
        calls_pending,
        logs_mark: store.otel_mark("logs")?,
        logs_pending,
        sessions_mark: store.otel_mark("sessions")?,
        last,
    })
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
        "{}{}{}",
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

/// T63.1: T61.3 listing × T61.1 resident. One accessor, two UIs (D23). Does not walk skill files.
pub fn skills_from(
    listing: Option<&doctor::SkillsAudit>,
    stats: Option<&BTreeMap<String, stats::SkillRow>>,
    usage_input: u64,
    stats_scanned: bool,
) -> SkillsPage {
    let mut rows: Vec<SkillPageRow> = listing
        .map(|a| a.rows.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            let st = stats.and_then(|m| m.get(&r.name));
            let invocations = r.invocations.or_else(|| st.map(|s| s.count)).unwrap_or(0);
            let never = r
                .invocations
                .map(|n| n == 0)
                .unwrap_or(stats_scanned && invocations == 0);
            SkillPageRow {
                name: r.name.clone(),
                source: r.source.clone(),
                desc_chars: r.desc_chars,
                body_bytes: r.body_bytes,
                invocations,
                resident: st.map(|s| s.resident).unwrap_or(0),
                last_invoked: if never { "never".into() } else { "—".into() },
                never,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.resident.cmp(&a.resident).then(a.name.cmp(&b.name)));
    let desc_bytes = listing.map(|a| a.desc_bytes).unwrap_or(0);
    let resident_bytes: u64 = rows.iter().map(|r| r.resident).sum();
    let desc_tokens = desc_bytes / 4;
    let share = if usage_input == 0 {
        0.0
    } else {
        100.0 * (resident_bytes / 4) as f64 / usage_input as f64
    };
    SkillsPage {
        header: format!(
            "{} skills · {} desc bytes ≈ {} tok/req · {} resident · {:.1}% of input",
            rows.len(),
            desc_bytes,
            desc_tokens,
            resident_bytes,
            share
        ),
        rows,
    }
}

/// T63.1 / T227: one transcript scan feeds both the Skills page (T61.3 listing joined
/// to T61.1 resident/invocations) and the Stats page ([`stats_page_text`]) — cached
/// briefly so `rtok tui` / `rtok web` ticks do not re-parse every transcript every two
/// seconds, the same shape as [`doctor_for_snapshot`]'s cache. `rtok stats` itself
/// still goes through [`stats_report`] uncached.
#[allow(clippy::type_complexity)]
fn stats_skills(
    cfg: &Config,
) -> (
    Option<BTreeMap<String, stats::SkillRow>>,
    u64,
    bool,
    Option<String>,
) {
    if cfg!(test) {
        return (None, 0, false, None);
    }
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    struct Entry {
        at: Instant,
        key: String,
        skills: Option<BTreeMap<String, stats::SkillRow>>,
        usage_input: u64,
        scanned: bool,
        text: Option<String>,
    }
    static CACHE: OnceLock<Mutex<Option<Entry>>> = OnceLock::new();
    let key = format!("{}{}", cfg.stats.transcripts_dir.display(), cfg.stats.since);
    let lock = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock()
        && let Some(e) = guard.as_ref()
        && e.key == key
        && e.at.elapsed() < DOCTOR_SNAPSHOT_TTL
    {
        return (e.skills.clone(), e.usage_input, e.scanned, e.text.clone());
    }
    // T227: cost costs no network (`[stats.prices]` is a local table), so the page
    // always carries it — `rtok stats` still needs `--price` to print it.
    let mut priced = cfg.clone();
    priced.stats.price = true;
    let (skills, usage_input, scanned, text) = match stats_report(&priced) {
        Ok(report) => {
            let text = Some(stats_page_text(cfg, &report));
            (report.skills, report.usage_input, true, text)
        }
        Err(_) => (None, 0, false, None),
    };
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(Entry {
            at: Instant::now(),
            key,
            skills: skills.clone(),
            usage_input,
            scanned,
            text: text.clone(),
        });
    }
    (skills, usage_input, scanned, text)
}

/// The Stats page (T227): [`stats_skills`]'s one scan rendered as `rtok stats
/// --price`'s table (D24), plus `rtok stats --cache`'s table appended below — D27's
/// one page for two commands, neither re-aggregated.
fn stats_page_text(cfg: &Config, report: &stats::Report) -> String {
    let mut out = report.to_table();
    if let Ok(health) = cache_health(cfg) {
        out.push_str("\ncache health\n");
        out.push_str(&cache::table(&health));
    }
    out
}

/// Dead-symbol lines shown on the Graph page before it says "and N more" instead of
/// flooding the page (T230). A cheap per-tick bound, not a token budget — `graph dead
/// --json` (uncapped) still has the rest.
const GRAPH_DEAD_CAP: usize = 200;

/// The Graph page (T230): `graph status::collect`/`format_table` (same text `rtok
/// graph status` prints) plus `graph::dead_candidates` (the same rows `rtok graph
/// dead --json` prints, minus the `index_for` walk `--json` runs first), capped for
/// display. Opens its own store handle like [`doctor`] — a few count queries plus a
/// scan of the store's dead candidates, not a re-index. `None` when the `graph`
/// feature is off (build-min) or either read failed.
#[cfg(feature = "graph")]
fn graph_page_text(cfg: &Config) -> Option<String> {
    use crate::plugins::graph::{dead_candidates, status};

    let rt = crate::plugin::Runtime::open(cfg.clone(), "web-graph").ok()?;
    let ctx = crate::plugin::Ctx::new(&rt);
    let root = std::env::current_dir().ok()?;
    let health = status::collect(&ctx, &root).ok()?;
    let mut out = status::format_table(&health);
    out.push_str("\ndead symbols\n");
    // T230: reads the store as it stands, no `index_for` walk — cheap per tick.
    match dead_candidates(&ctx, &root) {
        Ok(rows) if rows.is_empty() => out.push_str(" none\n"),
        Ok(rows) => {
            let total = rows.len();
            for r in rows.iter().take(GRAPH_DEAD_CAP) {
                out.push_str(&format!(" {}:{} {} {}\n", r.path, r.line, r.kind, r.name));
            }
            if total > GRAPH_DEAD_CAP {
                out.push_str(&format!(
                    " … {} more, capped at {GRAPH_DEAD_CAP} — `rtok graph dead --json` has the rest\n",
                    total - GRAPH_DEAD_CAP
                ));
            }
        }
        Err(e) => out.push_str(&format!(" dead scan failed: {e}\n")),
    }
    Some(out)
}

#[cfg(not(feature = "graph"))]
fn graph_page_text(_cfg: &Config) -> Option<String> {
    None
}

/// The Hosts page (T231): [`crate::agents::list`]'s blocks, verbatim — the same
/// per-variant kind/version/config/module text `rtok agents list` prints, built from
/// the same probe `agents_list`'s JSON form calls (D27, no duplicated logic).
/// `agents list` spawns one `--version` per host variant (T168) — too slow for a 2 s
/// snapshot tick — so this reuses [`doctor_for_snapshot`]'s cache shape, except a
/// cold or stale entry never blocks the tick: a background thread refreshes it while
/// the tick renders the last known text, or "probing hosts…" before the first probe
/// lands.
fn hosts_page_text(cfg: &Config) -> String {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    struct Entry {
        at: Instant,
        text: String,
    }
    static CACHE: OnceLock<Mutex<Option<Entry>>> = OnceLock::new();
    static REFRESHING: AtomicBool = AtomicBool::new(false);

    let lock = CACHE.get_or_init(|| Mutex::new(None));
    let last_known = lock.lock().ok().and_then(|g| {
        g.as_ref()
            .map(|e| (e.text.clone(), e.at.elapsed() < DOCTOR_SNAPSHOT_TTL))
    });
    if !matches!(last_known, Some((_, true))) && !REFRESHING.swap(true, Ordering::SeqCst) {
        let cfg = cfg.clone();
        std::thread::spawn(move || {
            let text = crate::agents::list(&cfg);
            if let Some(lock) = CACHE.get()
                && let Ok(mut guard) = lock.lock()
            {
                *guard = Some(Entry {
                    at: Instant::now(),
                    text,
                });
            }
            REFRESHING.store(false, Ordering::SeqCst);
        });
    }
    last_known
        .map(|(text, _)| text)
        .unwrap_or_else(|| "probing hosts…\n".to_string())
}

/// The Config page (T228): [`config_entries`]'s rows, the same ones `config
/// show`/`config get` already build (D27). `cfg.home` keeps a `--config`-rooted
/// snapshot reading that home, never the real one. Read-only: unlike `config show`, a
/// snapshot tick never runs `ensure_user_file` — a page view must not create files.
fn config_page_text(cfg: &Config) -> Option<String> {
    let fig = layers::figment(&cfg.home, None, None);
    let mut out = String::new();
    for (key, value, source) in layers::entries(&fig) {
        out.push_str(&format!("{key} = {value} ({source})\n"));
    }
    Some(out)
}

/// The Services page (T229): [`demon::rows`]'s per-service state — the same rows
/// `demon status` already builds (D27) — plus [`otel_status`]'s exporter health, so
/// both commands can join `COMMAND_PAGES`. Plain text, not [`demon::table`]: that
/// colours the state word for a terminal, which the web `BodyText` cannot render.
/// No `last_error` column: nothing records one per service today, so the row points
/// at the service's own log file instead of fabricating a message. `None` on a
/// failed tick, like [`config_page_text`].
fn services_page_text(cfg: &Config) -> Option<String> {
    let rows = demon::rows(cfg, &[]).ok()?;
    let mut out = String::new();
    for r in &rows {
        out.push_str(&format!(
            "{}  {}  pid={}  uptime={}  log={}\n",
            r.service,
            if r.running { "running" } else { "stopped" },
            r.child.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            r.uptime_secs
                .map(|s| format!("{s}s"))
                .unwrap_or_else(|| "-".into()),
            r.log.display(),
        ));
    }
    if let Ok(otel) = otel_status(cfg) {
        out.push_str(&format!(
            "otel endpoint={} calls_mark={} calls_pending={} logs_mark={} \
             logs_pending={} sessions_mark={}\n",
            otel.endpoint.as_deref().unwrap_or("-"),
            otel.calls_mark,
            otel.calls_pending,
            otel.logs_mark,
            otel.logs_pending,
            otel.sessions_mark,
        ));
        if let Some(last) = &otel.last {
            out.push_str(&format!(
                "last flush: [{}] {} {}\n",
                last.level, last.name, last.message
            ));
        }
    }
    Some(out)
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
        let calls = self.calls();
        let ref_ids = self
            .store
            .and_then(|s| {
                s.archive_ref_ids(&calls.iter().map(|c| c.id).collect::<Vec<_>>())
                    .ok()
            })
            .unwrap_or_default();
        let doctor = doctor_for_snapshot(self.cfg);
        let (st, usage, scanned, stats_text) = stats_skills(self.cfg);
        let skills = skills_from(
            doctor.as_ref().and_then(|d| d.skills.as_ref()),
            st.as_ref(),
            usage,
            scanned,
        );
        Snapshot {
            kind: "snapshot",
            usage: self.overview(),
            plugins: self.plugins(),
            calls,
            sessions: self.sessions(0),
            // The one doctor query (D27): the snapshot carries what `rtok doctor`
            // renders, so neither surface grows a probe of its own. Cached briefly —
            // a failed or in-flight tick is `None`, never a failed snapshot.
            doctor,
            logs: self.log_lines(None),
            error: None,
            skills,
            ref_ids,
            // T227: the same scan `stats_skills` already ran for the Skills page.
            stats: stats_text,
            // T230: reads the store on this tick — see `graph_page_text`.
            graph: graph_page_text(self.cfg),
            // T231: cached in the background — see `hosts_page_text`.
            hosts: hosts_page_text(self.cfg),
            // T228: reads the layered figment fresh each tick — see `config_page_text`.
            config: config_page_text(self.cfg),
            // T229: reuses `demon::rows` and `otel_status` — see `services_page_text`.
            services: services_page_text(self.cfg),
        }
    }

    /// Sessions page (T25.1): the newest [`SESSIONS_ROWS`] sessions, newest first, through
    /// the store's one `session_totals` query — the model does not keep a second reader
    /// (D27), and it does not decide what "live" means: `ended_at` is in the row and
    /// the renderers filter. No store (or one that will not read): an empty page,
    /// like Overview's zeros.
    pub fn sessions(&self, since: i64) -> Vec<SessionTotals> {
        self.store
            .and_then(|s| s.recent_session_totals(since, SESSIONS_ROWS as i64).ok())
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
    /// series the tab draws (T15.3), from `usage_by_api` and `usage_ctt`.
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
        if let Ok((ctt, turns)) = store.usage_ctt(OVERVIEW_TURNS as i64) {
            out.ctt = ctt;
            out.turns = turns;
        }
        out
    }

    /// Plugins: the catalogue, each with its page and — when it saves tokens — its stats.
    pub fn plugins(&self) -> Vec<PluginPage> {
        // T207: one grouped read for the whole page instead of a `list_measurements`
        // per catalogue plugin — the N+1 `report_savings` had too, and the same read.
        let totals = self.plugin_stat_totals();
        Registry::new(self.cfg)
            .pages()
            .into_iter()
            .map(|(m, enabled, mut page)| {
                page.fields.extend(config_fields(m.id, self.cfg));
                if m.id == "memory"
                    && let Some(store) = self.store
                    && let Ok(aggs) = store.memory_note_aggs(None)
                {
                    let live: u64 = aggs.iter().map(|r| r.live).sum();
                    let pinned: u64 = aggs.iter().map(|r| r.pinned).sum();
                    let retired: u64 = aggs.iter().map(|r| r.retired).sum();
                    page.fields.push(("notes live".into(), live.to_string()));
                    page.fields
                        .push(("notes pinned".into(), pinned.to_string()));
                    page.fields
                        .push(("notes retired".into(), retired.to_string()));
                }
                PluginPage {
                    id: m.id,
                    enabled,
                    surfaces: m.surfaces.iter().map(|s| s.as_str()).collect(),
                    title: page.title,
                    summary: page.summary,
                    saves_tokens: page.saves_tokens,
                    fields: page.fields,
                    stats: page
                        .saves_tokens
                        .then(|| totals.get(m.id).copied().unwrap_or_default()),
                }
            })
            .collect()
    }

    /// `rows`/est_before/est_after per plugin, every `Measurement` kind summed in — an
    /// `expand` row counts negative here too (`ReportSavings::saved`'s contract), the
    /// same [`Store::measurement_totals`] read `report_savings` builds its rows from, so
    /// the Plugins page and `rtok report` cannot disagree about one plugin's rows again
    /// (T207).
    fn plugin_stat_totals(&self) -> BTreeMap<String, Stats> {
        let mut by = BTreeMap::new();
        let Some(store) = self.store else {
            return by;
        };
        let Ok(totals) = store.measurement_totals() else {
            return by;
        };
        for t in totals {
            let s: &mut Stats = by.entry(t.plugin).or_default();
            s.rows += t.rows as u64;
            s.est_before += t.est_before;
            s.est_after += t.est_after;
        }
        by
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
    // Same-process ring first (tests, in-process proxy). Only ask a running
    // proxy over HTTP when this process has nothing — otherwise an empty or
    // foreign `/live` on the default port would hide real local rows.
    let local = crate::proxy::live::snapshot();
    if !local.is_empty() {
        return local;
    }
    fetch_live(cfg).unwrap_or_default()
}

fn fetch_live(cfg: &Config) -> Option<Vec<crate::proxy::LiveCall>> {
    let url = format!("http://{}:{}", cfg.proxy.bind, cfg.proxy.port);
    let timeout = std::time::Duration::from_millis(cfg.doctor.probe_timeout_ms.max(100));
    let body = crate::doctor::http_get(&url, "/live", timeout)?;
    serde_json::from_str(&body).ok()
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

/// The Calls page size column: bytes for in-memory plain-proxy rows, tokens when a
/// `usage` row is linked (`api` set), otherwise `-`.
pub fn call_size_label(c: &CallRow) -> String {
    if c.kind == "live_passthrough" {
        return match (c.input, c.output) {
            (None, None) => "-".into(),
            _ => format!("{} B", c.input.unwrap_or(0) + c.output.unwrap_or(0)),
        };
    }
    if c.api.is_some() && c.input.is_some() {
        return format!("{} tok", call_linked_tokens(c));
    }
    "-".into()
}

/// Sum of the four counters on a usage-linked call row.
pub fn call_linked_tokens(c: &CallRow) -> i64 {
    c.input.unwrap_or(0)
        + c.cache_create.unwrap_or(0)
        + c.cache_read.unwrap_or(0)
        + c.output.unwrap_or(0)
}

/// Session drill-down (T60.3, D23): the snapshot's `SessionTotals` row plus the
/// snapshot's calls filtered by that id. Both surfaces render this pair; neither
/// grows a second session type or a second query (D27).
/// Archive payload for both surfaces (T60.4, D23): [`crate::expand::fetch`] then
/// [`crate::expand::render_lines`] with `[expand] max_lines`. A live-zone pointer
/// freezes exactly as `rtok expand` does — one function, no second path. `grep`
/// is `--grep` parity for the TUI `/` filter and the web filter box.
pub fn expand_payload(cfg: &Config, id: &str, grep: Option<&str>) -> Option<String> {
    let cx = crate::plugin::Runtime::open(cfg.clone(), "expand").ok()?;
    let bytes = crate::expand::fetch(&cx, id).ok()??;
    let text = String::from_utf8_lossy(&bytes);
    crate::expand::render_lines(&text, id, None, grep, 0, cfg.expand.max_lines).ok()
}

pub fn session_detail<'a>(
    snapshot: &'a Snapshot,
    id: &str,
) -> Option<(&'a SessionTotals, Vec<&'a CallRow>)> {
    let session = snapshot.sessions.iter().find(|s| s.id == id)?;
    Some((
        session,
        snapshot.calls.iter().filter(|c| c.session == id).collect(),
    ))
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
            kv("prompt_recall", p.memory.prompt_recall),
            kv("sync_tokens", p.memory.sync_tokens),
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
    use rstest::rstest;

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

    /// T207's Check: `Store::measurement_totals`'s `GROUP BY` against the per-row loop it
    /// replaced, at a size the old per-plugin `list_measurements` N+1 would make slow —
    /// several plugins (one outside the catalogue), several kinds, `expand` included.
    #[test]
    fn plugin_stats_matches_sql_aggregates_on_10k_rows() {
        let cx = Runtime::in_memory("dash-10k").unwrap();
        let plugins = ["cmd", "archive", "wasm_widget"];
        let kinds = ["filter", "expand", "rule"];
        for i in 0..10_000i64 {
            cx.record(&Measurement {
                plugin: plugins[(i % plugins.len() as i64) as usize],
                kind: kinds[(i % kinds.len() as i64) as usize],
                before_bytes: 100,
                after_bytes: 40,
                est_before: (i % 50) as u32 + 1,
                est_after: (i % 20) as u32,
                ref_id: None,
                call_id: None,
            })
            .unwrap();
        }

        // The naive per-row aggregate every one of the three callers used to hand-roll.
        let mut want: BTreeMap<(String, String), (i64, i64, i64)> = BTreeMap::new();
        for plugin in plugins {
            for m in cx.store.list_measurements(plugin).unwrap() {
                let e = want
                    .entry((plugin.to_string(), m.kind.clone()))
                    .or_insert((0, 0, 0));
                e.0 += 1;
                e.1 += i64::from(m.est_before);
                e.2 += i64::from(m.est_after);
            }
        }
        let got: BTreeMap<(String, String), (i64, i64, i64)> = cx
            .store
            .measurement_totals()
            .unwrap()
            .into_iter()
            .map(|t| ((t.plugin, t.kind), (t.rows, t.est_before, t.est_after)))
            .collect();
        assert_eq!(got, want);

        // The Plugins page sums the same read across kinds, per plugin (T207).
        let totals = Model::new(&cx.config, Some(&cx.store)).plugin_stat_totals();
        for plugin in ["cmd", "archive"] {
            let want_plugin = want.iter().filter(|((p, _), _)| p == plugin).fold(
                (0u64, 0i64, 0i64),
                |(rows, before, after), (_, (n, b, a))| (rows + *n as u64, before + b, after + a),
            );
            let s = totals.get(plugin).unwrap();
            assert_eq!((s.rows, s.est_before, s.est_after), want_plugin, "{plugin}");
        }
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

    /// `usage_ctt` against the per-session loop it replaced, past the sparkline cap and with
    /// sessions interleaved.
    #[test]
    fn overview_matches_the_per_session_loop() {
        let cx = Runtime::in_memory("dash-ref").unwrap();
        for i in 0..150i64 {
            let s = ["a", "b", "c"][(i % 3) as usize];
            cx.store.insert_proxy_turn(s, i, i % 7, i % 5, 1).unwrap();
        }
        let (mut ctt, mut turns) = (0i64, Vec::new());
        for s in cx.store.usage_sessions().unwrap() {
            let mut rows = cx.store.usage_rows(&s).unwrap();
            rows.reverse();
            let n = rows.len() as i64;
            for (j, r) in rows.iter().enumerate() {
                let c = r.input + r.cache_create + r.cache_read;
                ctt += c * (n - j as i64 - 1);
                turns.push(c);
            }
        }
        let over = Model::new(&cx.config, Some(&cx.store)).overview();
        assert_eq!(over.ctt, ctt);
        assert_eq!(over.turns, turns[turns.len() - OVERVIEW_TURNS..]);
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
        let cfg = cx.config.clone();
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

    #[test]
    fn live_calls_prefer_in_process_ring_over_empty_http() {
        // Regression: a listener on the default proxy port that answers `/live`
        // with `[]` used to hide rows pushed into this process's ring.
        let _ring = crate::proxy::live::test_lock();
        crate::proxy::live::clear();
        crate::proxy::live::push(crate::proxy::LiveCall {
            ts: 1,
            method: "POST".into(),
            path: "/v1/messages".into(),
            provider: None,
            model: None,
            status: 200,
            request_bytes: 100,
            response_bytes: 50,
            ms: 5.0,
        });
        let mut cfg = Config::default();
        cfg.proxy.enabled = false;
        cfg.core.enabled = true;
        let rows = Model::new(&cfg, None).calls();
        crate::proxy::live::clear();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].kind, "live_passthrough");
        assert_eq!(call_size_label(&rows[0]), "150 B");
    }

    #[rstest]
    #[case("live_passthrough", None, None, None, None, None, "-")]
    #[case("live_passthrough", Some(100), None, None, Some(50), None, "150 B")]
    #[case(
        "api_request",
        Some(10),
        Some(1),
        Some(2),
        Some(3),
        Some("anthropic"),
        "16 tok"
    )]
    #[case("hook", None, None, None, None, None, "-")]
    fn call_size_label_distinguishes_bytes_from_tokens(
        #[case] kind: &str,
        #[case] input: Option<i64>,
        #[case] cache_create: Option<i64>,
        #[case] cache_read: Option<i64>,
        #[case] output: Option<i64>,
        #[case] api: Option<&str>,
        #[case] want: &str,
    ) {
        let row = CallRow {
            id: 1,
            ts: 0,
            session: "s".into(),
            surface: "proxy".into(),
            kind: kind.into(),
            plugin: None,
            name: None,
            parent_id: None,
            ms: None,
            ok: 1,
            error: None,
            host: None,
            provider: None,
            model: None,
            api: api.map(str::to_string),
            input,
            cache_create,
            cache_read,
            output,
        };
        assert_eq!(call_size_label(&row), want);
    }

    #[test]
    fn session_detail_filters_snapshot_calls_by_id() {
        let cfg = crate::testutil::config("session-detail").0;
        let mut snap = Model::new(&cfg, None).snapshot();
        snap.sessions = vec![SessionTotals {
            id: "a".into(),
            host: None,
            project: Some("rtok".into()),
            provider: None,
            api: Some("anthropic".into()),
            model: None,
            input: 30,
            cache_create: 1,
            cache_read: 7,
            output: 7,
            started_at: 1,
            last_activity: 2,
            ended_at: None,
        }];
        snap.calls = vec![
            CallRow {
                id: 1,
                ts: 1,
                session: "a".into(),
                surface: "proxy".into(),
                kind: "api_request".into(),
                plugin: None,
                name: Some("/v1/messages".into()),
                parent_id: None,
                ms: None,
                ok: 1,
                error: None,
                host: None,
                provider: None,
                model: None,
                api: None,
                input: None,
                cache_create: None,
                cache_read: None,
                output: None,
            },
            CallRow {
                id: 2,
                ts: 2,
                session: "other".into(),
                surface: "hook".into(),
                kind: "hook".into(),
                plugin: None,
                name: Some("Skip".into()),
                parent_id: None,
                ms: None,
                ok: 1,
                error: None,
                host: None,
                provider: None,
                model: None,
                api: None,
                input: None,
                cache_create: None,
                cache_read: None,
                output: None,
            },
        ];
        let (session, calls) = session_detail(&snap, "a").expect("session a");
        assert_eq!(session.project.as_deref(), Some("rtok"));
        assert_eq!(session.api.as_deref(), Some("anthropic"));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name.as_deref(), Some("/v1/messages"));
        assert!(session_detail(&snap, "missing").is_none());
    }

    /// T60.4: `expand_payload` is `expand::fetch` + `[expand] max_lines`; a live-zone
    /// pointer freezes as the CLI does; the snapshot carries the call's archive id.
    #[test]
    fn expand_payload_caps_greps_and_freezes_like_cli() {
        let mut cfg = crate::testutil::config("expand-model").0;
        cfg.expand.max_lines = 2;
        let cx = crate::plugin::Runtime::open(cfg.clone(), "proxy-sess").unwrap();
        let id = cx
            .store
            .put_archive(
                "proxy-sess",
                b"alpha\nbeta\ngamma\nHIT\n",
                &cfg.core.archive_dir,
            )
            .unwrap();
        cx.store
            .put_archive_decision("tu-1", &id, "proxy-sess", &format!("[archived {id}]"))
            .unwrap();
        cx.store
            .upsert_session("s", None, None, None, None)
            .unwrap();
        let call = cx
            .store
            .insert_call(
                "s",
                "hook",
                "plugin_run",
                None,
                None,
                None,
                Some("cmd"),
                None,
            )
            .unwrap();
        cx.record(&Measurement {
            plugin: "cmd",
            kind: "filter",
            before_bytes: 10,
            after_bytes: 4,
            est_before: 3,
            est_after: 1,
            ref_id: Some(id.clone()),
            call_id: Some(call),
        })
        .unwrap();
        drop(cx);

        let text = expand_payload(&cfg, &id, None).expect("payload");
        assert!(text.contains("alpha"), "{text}");
        assert!(text.contains("omitted"), "{text}");
        let hit = expand_payload(&cfg, &id, Some("HIT")).expect("grep");
        assert!(hit.contains("HIT"), "{hit}");
        let snap = snapshot(&cfg);
        assert_eq!(snap.ref_ids.get(&call), Some(&id));
        let cx = crate::plugin::Runtime::open(cfg, "check").unwrap();
        assert!(
            cx.store
                .archive_decision("proxy-sess", "tu-1")
                .unwrap()
                .unwrap()
                .expanded
        );
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

    fn listed(
        name: &str,
        source: &str,
        desc: usize,
        body: u64,
        calls: Option<u64>,
    ) -> doctor::SkillRow {
        doctor::SkillRow {
            name: name.into(),
            source: source.into(),
            desc_chars: desc,
            body_bytes: body,
            invocations: calls,
            warn_desc: false,
            warn_body: false,
            warn_never: calls == Some(0),
        }
    }

    #[test]
    fn skills_from_joins_listing_and_resident_without_stats_rows() {
        let listing = doctor::SkillsAudit {
            rows: vec![
                listed("hot", "user", 40, 100, Some(3)),
                listed("cold", "project", 10, 20, Some(0)),
                listed("plug", "plugin:x", 8, 50, Some(1)),
            ],
            desc_bytes: 58,
        };
        let mut stats = BTreeMap::new();
        stats.insert(
            "hot".into(),
            stats::SkillRow {
                count: 3,
                bytes: 100,
                mean: 33,
                p95: 100,
                max: 100,
                est_tokens: 25,
                resident: 800,
            },
        );
        stats.insert(
            "plug".into(),
            stats::SkillRow {
                count: 1,
                bytes: 50,
                mean: 50,
                p95: 50,
                max: 50,
                est_tokens: 12,
                resident: 200,
            },
        );
        let page = skills_from(Some(&listing), Some(&stats), 10_000, true);
        assert_eq!(
            page.rows
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["hot", "plug", "cold"]
        );
        assert!(page.rows[2].never && page.rows[2].last_invoked == "never");
        assert_eq!(page.rows[0].resident, 800);
        assert!(page.header.contains("3 skills"));
        assert!(page.header.contains("58 desc bytes ≈ 14 tok/req"));
        let empty = skills_from(None, None, 0, false);
        assert!(empty.rows.is_empty());
        assert!(empty.header.starts_with("0 skills"));
    }
}
