//! `rtok stats` (plan T1.2): per-tool sizes, Bash families, MCP groups, CTT.
//!
//! Tool-result tokens use 4 chars/token (`research.md` §2 heuristic) so this report is
//! comparable to that baseline. Usage counters are the API numbers from the transcript.

use super::jsonl::{self, Parsed};
use crate::config::Config;
use crate::render::{Col, table};
use crate::store::Store;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, SystemTime};

const CHARS_PER_TOKEN: f64 = 4.0;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SizeRow {
    pub count: u64,
    pub total_bytes: u64,
    pub mean: u64,
    pub p95: u64,
    pub max: u64,
    pub est_tokens: u64,
    pub ctt: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub sessions: u64,
    pub lines: u64,
    pub malformed: u64,
    pub tools: BTreeMap<String, SizeRow>,
    pub bash_families: BTreeMap<String, SizeRow>,
    /// T50.1: `formatter`, named `rule`, or `default` per Bash stem (transcripts).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bash_filter: BTreeMap<String, String>,
    /// T50.1: `cmd` measurements with `kind = rule` on stems still on [`Rule::default()`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bash_default_rule: BTreeMap<String, SizeRow>,
    pub mcp_groups: BTreeMap<String, SizeRow>,
    pub usage_input: u64,
    pub usage_cache_create: u64,
    pub usage_cache_read: u64,
    pub usage_output: u64,
    pub cache_hit_rate: f64,
    pub median_final_context: u64,
    /// Gate P5 replay: context-token-turns as recorded, and as they would be with the
    /// `archive` policy applied to every tool result (estimate: bytes/4, pointer = head + tail lines).
    #[serde(default)]
    pub ctt_total: u64,
    #[serde(default)]
    pub ctt_archive: u64,
    #[serde(default)]
    pub archive_candidates: u64,
    #[serde(default)]
    pub api: BTreeMap<String, ApiRow>,
    /// `Some` only for `rtok stats --price`: the default report is byte-identical
    /// with and without the price table (T49.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostReport>,
    /// Absent from `--json` when no session edited anything, so the T15.11 goldens hold.
    #[serde(default, skip_serializing_if = "EditRow::is_empty")]
    pub edits: EditRow,
    /// T58.1: native Read of a path already read in-session with Edit/Write/MultiEdit
    /// of that path in between. Absent when none, so the goldens hold.
    #[serde(default, skip_serializing_if = "ReadDeltaRow::is_empty")]
    pub read_delta: ReadDeltaRow,
    /// T65.1: tool_result bytes whose SHA-256 matches an earlier result in the
    /// same session. Absent when none, so the goldens hold.
    #[serde(default, skip_serializing_if = "RepeatRow::is_empty")]
    pub repeat: RepeatRow,
    /// T61.1: skill bodies the transcripts inject as `isMeta` records, per skill
    /// name. Absent when none, so the goldens hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<BTreeMap<String, SkillRow>>,
}

/// What the model re-types to edit (plan T58.3). `old_string` is the span `Edit` and every
/// `MultiEdit.edits[]` entry quote back verbatim; `tool_input_bytes` is every `tool_use`
/// input serialized — the "tool input" denominator of `research.md` §2.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRow {
    pub calls: u64,
    pub old_bytes: u64,
    pub new_bytes: u64,
    pub tool_input_bytes: u64,
}

impl EditRow {
    fn is_empty(&self) -> bool {
        self.calls == 0
    }
}

/// Re-reads that a unified diff could shorten (plan T58.1). `bytes` is those
/// tool-result payloads; `read_bytes` is every native `Read` result in the same
/// window — the denominator of "share of Read bytes".
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadDeltaRow {
    pub calls: u64,
    pub bytes: u64,
    pub read_bytes: u64,
}

impl ReadDeltaRow {
    fn is_empty(&self) -> bool {
        self.calls == 0
    }
}

/// Content-hash repeats within a session (plan T65.1). `bytes` is those
/// tool-result payloads; `result_bytes` is every tool_result in the same
/// window — the denominator of "share of result bytes".
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatRow {
    pub calls: u64,
    pub bytes: u64,
    pub result_bytes: u64,
}

impl RepeatRow {
    fn is_empty(&self) -> bool {
        self.calls == 0
    }
}

/// T61.1: one skill's injected bodies — the `isMeta` user records keyed to that
/// skill's `Skill` tool_use. `resident` is the number the context actually carried:
/// body bytes × the API requests of the session that came after the injection.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRow {
    pub count: u64,
    pub bytes: u64,
    pub mean: u64,
    pub p95: u64,
    pub max: u64,
    pub est_tokens: u64,
    pub resident: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ApiRow {
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
    pub hit: f64,
}

/// One model's USD costs (`rtok stats --price`, T49.1). `cost`/`saved` are
/// `None` for models without a `[stats.prices]` entry: their token counts still
/// print, but no dollar figure is guessed.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CostRow {
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
    /// USD at the model's `$` per MTok row.
    pub cost: Option<f64>,
    /// USD the cache reads saved versus uncached input price.
    pub saved: Option<f64>,
}

/// USD costs over the proxy `usage` rows (`rtok stats --price`, T49.1). Models
/// without a `[stats.prices]` entry are named in `unknown` and priced nowhere —
/// never by guess.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CostReport {
    pub models: BTreeMap<String, CostRow>,
    pub unknown: Vec<String>,
    pub total_cost: f64,
    pub total_saved: f64,
}

/// The `[plugins.archive]` knobs the replay needs, so `collect` stays usable without a `Config`.
#[derive(Debug, Clone, Copy)]
pub struct Replay {
    pub keep_turns: u64,
    pub min_tokens: u64,
    pub head_lines: usize,
    pub tail_lines: usize,
}

impl Replay {
    pub fn from_cfg(cfg: &Config) -> Self {
        let a = &cfg.plugins.archive;
        Self {
            keep_turns: u64::from(a.keep_turns),
            min_tokens: u64::from(a.min_tokens),
            head_lines: a.head_lines as usize,
            tail_lines: a.tail_lines as usize,
        }
    }
}

impl Report {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_table(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "sessions {}  lines {}  malformed {}\n",
            self.sessions, self.lines, self.malformed
        ));
        s.push_str(&format!(
            "usage input={} cache_create={} cache_read={} output={}  hit={:.1}%  median_context={}\n",
            self.usage_input,
            self.usage_cache_create,
            self.usage_cache_read,
            self.usage_output,
            self.cache_hit_rate * 100.0,
            self.median_final_context
        ));
        if !self.api.is_empty() {
            // The old fixed widths ride along as column floors, so the bytes a golden
            // pinned do not move (T25.2 moved the padding into `render::table`).
            let cols = [
                Col::left(24),
                Col::right(8),
                Col::right(12),
                Col::right(10),
                Col::right(6),
                Col::right(6),
            ];
            let mut rows = vec![vec![
                "api".into(),
                "input".into(),
                "cache_create".into(),
                "cache_read".into(),
                "output".into(),
                "hit".into(),
            ]];
            for (api, r) in &self.api {
                rows.push(vec![
                    api.clone(),
                    r.input.to_string(),
                    r.cache_create.to_string(),
                    r.cache_read.to_string(),
                    r.output.to_string(),
                    format!("{:.1}%", r.hit * 100.0),
                ]);
            }
            s.push_str(&table(&cols, &rows));
        }
        if self.ctt_total > 0 {
            let pct =
                100.0 * (self.ctt_total as f64 - self.ctt_archive as f64) / self.ctt_total as f64;
            s.push_str(&format!(
                "archive replay (estimate) ctt {} → {}  -{pct:.1}%  over {} results\n",
                self.ctt_total, self.ctt_archive, self.archive_candidates
            ));
        }
        if let Some(cost) = &self.cost {
            s.push_str(&cost.to_table());
        }
        if self.edits.calls > 0 {
            let e = &self.edits;
            s.push_str(&format!(
                "edit calls {}  old_string {} B  new_string {} B  old/tool_input {:.1}%  old/output_tokens (est) {:.1}%\n",
                e.calls,
                e.old_bytes,
                e.new_bytes,
                pct(e.old_bytes, e.tool_input_bytes),
                pct(est_tokens(e.old_bytes), self.usage_output)
            ));
        }
        if self.read_delta.calls > 0 {
            let d = &self.read_delta;
            s.push_str(&format!(
                "read delta calls {}  bytes {}  of Read bytes {}  {:.1}%\n",
                d.calls,
                d.bytes,
                d.read_bytes,
                pct(d.bytes, d.read_bytes)
            ));
        }
        if self.repeat.calls > 0 {
            let d = &self.repeat;
            s.push_str(&format!(
                "repeat calls {}  bytes {}  of result bytes {}  {:.1}%\n",
                d.calls,
                d.bytes,
                d.result_bytes,
                pct(d.bytes, d.result_bytes)
            ));
        }
        s.push_str(&format_section("tool", &self.tools));
        s.push_str(&format_bash_section(&self.bash_families, &self.bash_filter));
        if !self.bash_default_rule.is_empty() {
            s.push_str(&format_section("bash_default", &self.bash_default_rule));
        }
        s.push_str(&format_section("mcp", &self.mcp_groups));
        if let Some(skills) = &self.skills {
            s.push_str(&skills_section(skills));
        }
        s
    }
}

fn format_bash_section(
    rows: &BTreeMap<String, SizeRow>,
    kinds: &BTreeMap<String, String>,
) -> String {
    let cols = [
        Col::left(24),
        Col::left(8),
        Col::right(7),
        Col::right(12),
        Col::right(8),
        Col::right(8),
        Col::right(8),
        Col::right(12),
        Col::right(12),
    ];
    let mut out = vec![vec![
        "bash".to_string(),
        "filter".into(),
        "count".into(),
        "bytes".into(),
        "mean".into(),
        "p95".into(),
        "max".into(),
        "est_tokens".into(),
        "ctt".into(),
    ]];
    for (name, r) in rows {
        out.push(vec![
            name.clone(),
            kinds
                .get(name)
                .cloned()
                .unwrap_or_else(|| "default".to_string()),
            r.count.to_string(),
            r.total_bytes.to_string(),
            r.mean.to_string(),
            r.p95.to_string(),
            r.max.to_string(),
            r.est_tokens.to_string(),
            r.ctt.to_string(),
        ]);
    }
    table(&cols, &out)
}

/// T50.1: label each transcript Bash family and rank default-rule `cmd` savings.
pub fn attach_bash_cmd(report: &mut Report, store: &Store) -> Result<()> {
    let settings = crate::plugins::cmd::rules::Settings::builtin();
    for name in report.bash_families.keys() {
        let kind = crate::plugins::cmd::formatters::filter_kind(&settings, name);
        report.bash_filter.insert(name.clone(), kind.to_string());
    }
    for r in store.list_measurements("cmd")? {
        if r.kind != "rule" {
            continue;
        }
        let fam = r
            .ref_id
            .as_deref()
            .and_then(|id| id.split(':').next())
            .unwrap_or("");
        if fam.is_empty()
            || crate::plugins::cmd::formatters::filter_kind(&settings, fam) != "default"
        {
            continue;
        }
        add(
            &mut report.bash_default_rule,
            fam,
            r.after_bytes.max(0) as u64,
            est_tokens(r.after_bytes.max(0) as u64),
            0,
        );
    }
    finish_rows(&mut report.bash_default_rule);
    Ok(())
}
fn format_section(title: &str, rows: &BTreeMap<String, SizeRow>) -> String {
    // The section's own title sits in the first column of its header line; the fixed
    // widths are floors now (`render::table`, T25.2), bytes unchanged.
    let cols = [
        Col::left(24),
        Col::right(7),
        Col::right(12),
        Col::right(8),
        Col::right(8),
        Col::right(8),
        Col::right(12),
        Col::right(12),
    ];
    let mut out = vec![vec![
        title.to_string(),
        "count".into(),
        "bytes".into(),
        "mean".into(),
        "p95".into(),
        "max".into(),
        "est_tokens".into(),
        "ctt".into(),
    ]];
    for (name, r) in rows {
        out.push(vec![
            name.clone(),
            r.count.to_string(),
            r.total_bytes.to_string(),
            r.mean.to_string(),
            r.p95.to_string(),
            r.max.to_string(),
            r.est_tokens.to_string(),
            r.ctt.to_string(),
        ]);
    }
    table(&cols, &out)
}

/// T61.1: the injected skill bodies, one row per skill, `resident` = the bytes the
/// later API requests of the same session actually carried.
fn skills_section(skills: &BTreeMap<String, SkillRow>) -> String {
    let cols = [
        Col::left(24),
        Col::right(7),
        Col::right(12),
        Col::right(8),
        Col::right(8),
        Col::right(8),
        Col::right(12),
        Col::right(14),
    ];
    let mut out = vec![vec![
        "skill".to_string(),
        "count".into(),
        "bytes".into(),
        "mean".into(),
        "p95".into(),
        "max".into(),
        "est_tokens".into(),
        "resident".into(),
    ]];
    for (name, r) in skills {
        out.push(vec![
            name.clone(),
            r.count.to_string(),
            r.bytes.to_string(),
            r.mean.to_string(),
            r.p95.to_string(),
            r.max.to_string(),
            r.est_tokens.to_string(),
            r.resident.to_string(),
        ]);
    }
    table(&cols, &out)
}

pub fn parse_since(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (n, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = n.parse().map_err(|_| anyhow::anyhow!("bad --since {s}"))?;
    let per_unit = match unit {
        "" | "d" => 86_400u64,
        "h" => 3_600,
        _ => bail!("bad --since unit in {s}"),
    };
    // `--since 99999999999999999d` used to panic in a debug build and wrap in a release one.
    let secs = n
        .checked_mul(per_unit)
        .ok_or_else(|| anyhow::anyhow!("--since {s} is out of range"))?;
    Ok(Duration::from_secs(secs))
}

pub fn attach_api(report: &mut Report, store: &Store) -> Result<()> {
    for row in store.usage_by_api()? {
        let denom = row
            .cache_read
            .saturating_add(row.cache_create)
            .saturating_add(row.input);
        let hit = if denom == 0 {
            0.0
        } else {
            row.cache_read as f64 / denom as f64
        };
        report.api.insert(
            row.api,
            ApiRow {
                input: row.input,
                cache_create: row.cache_create,
                cache_read: row.cache_read,
                output: row.output,
                hit,
            },
        );
    }
    Ok(())
}

/// Codex CLI sessions as one more `api` row (T49.2), read from `dir` with the same `since`
/// window as the Claude Code transcripts. Absent dir or no `token_count` line → no row.
pub fn attach_codex(report: &mut Report, dir: &Path, since: Duration) {
    let cutoff = SystemTime::now()
        .checked_sub(since)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    if let Some(row) = super::codex::collect(dir, cutoff) {
        report.api.insert("codex".into(), row);
    }
}

/// USD for one model's counters at its `$` per MTok row: `(cost, saved)`.
/// `saved` is what the cache reads saved versus uncached input price — the only
/// saving computable from the `usage` rows alone (T49.1). Dust below a tenth of
/// a microdollar rounds away so JSON goldens stay exact.
pub fn row_cost(
    input: i64,
    cache_create: i64,
    cache_read: i64,
    output: i64,
    price: &crate::config::ModelPrice,
) -> (f64, f64) {
    let leg = |n: i64, rate: f64| n as f64 / 1e6 * rate;
    let cost = leg(input, price.input)
        + leg(cache_create, price.cache_write)
        + leg(cache_read, price.cache_read)
        + leg(output, price.output);
    let saved = leg(cache_read, (price.input - price.cache_read).max(0.0));
    let round = |v: f64| (v * 1e6).round() / 1e6;
    (round(cost), round(saved))
}

/// Price the store's per-model `usage` into `report.cost` (`rtok stats --price`,
/// T49.1). Models without a `[stats.prices]` entry land in `unknown` and stay
/// out of the totals.
pub fn attach_costs(
    report: &mut Report,
    store: &Store,
    prices: &BTreeMap<String, crate::config::ModelPrice>,
) -> Result<()> {
    let mut costs = CostReport::default();
    for row in store.usage_by_model()? {
        let priced = prices.get(&row.model).map(|price| {
            row_cost(
                row.input,
                row.cache_create,
                row.cache_read,
                row.output,
                price,
            )
        });
        if priced.is_none() {
            costs.unknown.push(row.model.clone());
        }
        let (cost, saved) = priced.unzip();
        costs.total_cost += cost.unwrap_or(0.0);
        costs.total_saved += saved.unwrap_or(0.0);
        costs.models.insert(
            row.model,
            CostRow {
                input: row.input,
                cache_create: row.cache_create,
                cache_read: row.cache_read,
                output: row.output,
                cost,
                saved,
            },
        );
    }
    costs.unknown.sort();
    costs.unknown.dedup();
    let round = |v: f64| (v * 1e6).round() / 1e6;
    costs.total_cost = round(costs.total_cost);
    costs.total_saved = round(costs.total_saved);
    report.cost = Some(costs);
    Ok(())
}

impl CostReport {
    fn to_table(&self) -> String {
        let mut s = String::from("cost (USD at [stats.prices] $/MTok; `-` = no price row)\n");
        if self.models.is_empty() && self.unknown.is_empty() {
            s.push_str("  no usage rows\n");
            return s;
        }
        let cols = [
            Col::left(24),
            Col::right(12),
            Col::right(12),
            Col::right(12),
            Col::right(12),
            Col::right(10),
            Col::right(10),
        ];
        let mut rows = vec![vec![
            "model".into(),
            "input".into(),
            "cache_create".into(),
            "cache_read".into(),
            "output".into(),
            "cost".into(),
            "saved".into(),
        ]];
        for (model, r) in &self.models {
            let money = |v: Option<f64>| v.map_or_else(|| "-".into(), |v| format!("{v:.2}"));
            rows.push(vec![
                model.clone(),
                r.input.to_string(),
                r.cache_create.to_string(),
                r.cache_read.to_string(),
                r.output.to_string(),
                money(r.cost),
                money(r.saved),
            ]);
        }
        s.push_str(&table(&cols, &rows));
        s.push_str(&format!(
            "cost total ${:.2} (cache reads saved ${:.2}; {} model(s) without a price)\n",
            self.total_cost,
            self.total_saved,
            self.unknown.len()
        ));
        s
    }
}

pub fn collect(dir: &Path, since: Duration, plugin: &str, replay: Replay) -> Result<Report> {
    let cutoff = SystemTime::now()
        .checked_sub(since)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut report = Report::default();
    let mut finals = Vec::new();
    for p in super::codex::jsonl_paths(dir, cutoff) {
        // One unreadable transcript is one malformed entry, not the end of the report.
        let Ok(parsed) = jsonl::parse_path(&p) else {
            report.malformed += 1;
            continue;
        };
        fold_session(&parsed, plugin, replay, &mut report, &mut finals);
    }
    finish_rows(&mut report.tools);
    finish_rows(&mut report.bash_families);
    finish_rows(&mut report.mcp_groups);
    let denom = report.usage_cache_read + report.usage_cache_create + report.usage_input;
    report.cache_hit_rate = if denom == 0 {
        0.0
    } else {
        report.usage_cache_read as f64 / denom as f64
    };
    finals.sort_unstable();
    report.median_final_context = if finals.is_empty() {
        0
    } else {
        finals[finals.len() / 2]
    };
    Ok(report)
}

fn fold_session(
    parsed: &Parsed,
    plugin: &str,
    replay: Replay,
    report: &mut Report,
    finals: &mut Vec<u64>,
) {
    report.sessions += 1;
    report.lines += parsed.lines;
    report.malformed += parsed.malformed;
    let n = u64::from(parsed.turns);
    let mut id_name: BTreeMap<&str, &str> = BTreeMap::new();
    let mut id_family: BTreeMap<&str, String> = BTreeMap::new();
    let mut id_skill: BTreeMap<&str, String> = BTreeMap::new();
    for u in &parsed.tool_uses {
        id_name.insert(u.id.as_str(), u.name.as_str());
        if u.name == "Bash"
            && let Some(cmd) = u.input.get("command").and_then(|v| v.as_str())
        {
            id_family.insert(u.id.as_str(), bash_family(cmd));
        }
        if u.name == "Skill"
            && let Some(skill) = u.input.get("skill").and_then(|v| v.as_str())
        {
            id_skill.insert(u.id.as_str(), skill.to_string());
        }
        fold_edits(&mut report.edits, u);
    }
    fold_read_delta(&mut report.read_delta, parsed);
    fold_repeat(&mut report.repeat, parsed);
    for r in &parsed.tool_results {
        let name = id_name
            .get(r.tool_use_id.as_str())
            .copied()
            .unwrap_or("unknown");
        if !plugin.is_empty() && plugin != name && !name.contains(plugin) {
            continue;
        }
        let bytes = r.content.len() as u64;
        let tokens = est_tokens(bytes);
        let remain = n.saturating_sub(u64::from(r.turn));
        let ctt = tokens.saturating_mul(remain);
        let after = replay_ctt(&r.content, tokens, remain, replay);
        report.ctt_total += ctt;
        report.ctt_archive += after;
        if after != ctt {
            report.archive_candidates += 1;
        }
        add(&mut report.tools, name, bytes, tokens, ctt);
        if name == "Bash" {
            let fam = id_family
                .get(r.tool_use_id.as_str())
                .map(String::as_str)
                .unwrap_or("other");
            add(&mut report.bash_families, fam, bytes, tokens, ctt);
        }
        if let Some(grp) = mcp_group(name) {
            add(&mut report.mcp_groups, grp, bytes, tokens, ctt);
        }
    }
    fold_skills(parsed, &id_skill, report);
    for u in &parsed.usages {
        report.usage_input += u64::from(u.input_tokens);
        report.usage_cache_create += u64::from(u.cache_creation_input_tokens);
        report.usage_cache_read += u64::from(u.cache_read_input_tokens);
        report.usage_output += u64::from(u.output_tokens);
    }
    if let Some(last) = parsed.usages.last() {
        finals.push(
            u64::from(last.input_tokens)
                + u64::from(last.cache_creation_input_tokens)
                + u64::from(last.cache_read_input_tokens),
        );
    }
}

/// T61.1: fold the session's injected skill bodies into one row per skill, keyed by
/// the `Skill` tool_use's `input.skill`. `resident` multiplies the body bytes by the
/// API requests of this session at or after the injection turn — what the context
/// actually carried.
fn fold_skills(parsed: &Parsed, id_skill: &BTreeMap<&str, String>, report: &mut Report) {
    if parsed.injected.is_empty() {
        return;
    }
    let map = report.skills.get_or_insert_with(BTreeMap::new);
    for inj in &parsed.injected {
        let Some(name) = id_skill.get(inj.tool_use_id.as_str()) else {
            continue;
        };
        let later = parsed.usages.iter().filter(|u| u.turn >= inj.turn).count() as u64;
        let row = map.entry(name.clone()).or_default();
        row.count += 1;
        row.bytes += inj.bytes;
        row.max = row.max.max(inj.bytes);
        row.est_tokens += est_tokens(inj.bytes);
        row.resident += inj.bytes.saturating_mul(later);
    }
    for row in map.values_mut() {
        row.mean = row.bytes.checked_div(row.count).unwrap_or(0);
        row.p95 = row.max;
    }
}

/// What `ctt` becomes when the `archive` plugin (T5.3) swaps this result for its pointer
/// after `keep_turns`: full size while young, head + tail lines afterwards.
fn replay_ctt(content: &str, tokens: u64, remain: u64, rp: Replay) -> u64 {
    if tokens < rp.min_tokens || remain <= rp.keep_turns {
        return tokens.saturating_mul(remain);
    }
    let lines: Vec<&str> = content.lines().collect();
    let kept: usize = lines
        .iter()
        .take(rp.head_lines)
        .chain(lines.iter().rev().take(rp.tail_lines))
        .map(|l| l.len() + 1)
        .sum();
    let pointer = est_tokens(kept as u64 + 64); // + the `[archived …]` line itself
    tokens.saturating_mul(rp.keep_turns) + pointer.saturating_mul(remain - rp.keep_turns)
}

/// T58.3: every tool input counts toward the denominator; `Edit` and each `MultiEdit.edits[]`
/// entry add the `old_string` the model had to re-type and the `new_string` it meant to write.
fn fold_edits(row: &mut EditRow, u: &jsonl::ToolUse) {
    row.tool_input_bytes += u.input.to_string().len() as u64;
    let edits: Vec<&Value> = match u.name.as_str() {
        "Edit" => vec![&u.input],
        "MultiEdit" => u
            .input
            .get("edits")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default(),
        _ => return,
    };
    for e in edits {
        let len = |k: &str| {
            e.get(k)
                .and_then(Value::as_str)
                .map_or(0, |s| s.len() as u64)
        };
        row.calls += 1;
        row.old_bytes += len("old_string");
        row.new_bytes += len("new_string");
    }
}

/// T65.1: SHA-256 of each tool_result in order. A later result whose digest
/// equals an earlier one in the same session is a repeat — its bytes are the
/// content-hash dedup surface. Guard's input-key cache is a different axis.
fn fold_repeat(row: &mut RepeatRow, parsed: &Parsed) {
    let mut seen = BTreeSet::new();
    for r in &parsed.tool_results {
        let bytes = r.content.len() as u64;
        row.result_bytes += bytes;
        let sha = crate::store::hex_sha256(r.content.as_bytes());
        if !seen.insert(sha) {
            row.calls += 1;
            row.bytes += bytes;
        }
    }
}

/// T58.1: walk this session's tool uses in order. A native `Read` of a path that
/// was already read, with an `Edit` / `Write` / `MultiEdit` of that path in
/// between, is a changed re-read — its result bytes are the delta-read surface.
fn fold_read_delta(row: &mut ReadDeltaRow, parsed: &Parsed) {
    let sizes: BTreeMap<&str, u64> = parsed
        .tool_results
        .iter()
        .map(|r| (r.tool_use_id.as_str(), r.content.len() as u64))
        .collect();
    let mut paths: Vec<(String, bool, bool)> = Vec::new();
    for u in &parsed.tool_uses {
        let Some(path) = tool_path(&u.input) else {
            continue;
        };
        match u.name.as_str() {
            "Read" => {
                let bytes = sizes.get(u.id.as_str()).copied().unwrap_or(0);
                row.read_bytes += bytes;
                if let Some((_, seen, dirty)) = paths.iter_mut().find(|(p, ..)| same_path(p, path))
                {
                    if *seen && *dirty {
                        row.calls += 1;
                        row.bytes += bytes;
                    }
                    *seen = true;
                    *dirty = false;
                } else {
                    paths.push((path.to_string(), true, false));
                }
            }
            "Edit" | "Write" | "MultiEdit" => {
                if let Some((_, _, dirty)) = paths.iter_mut().find(|(p, ..)| same_path(p, path)) {
                    *dirty = true;
                } else {
                    paths.push((path.to_string(), false, true));
                }
            }
            _ => {}
        }
    }
}

fn tool_path(input: &Value) -> Option<&str> {
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty())
}

/// Exact match, or relative-vs-absolute (`src/a.rs` vs `/repo/src/a.rs`).
fn same_path(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a = Path::new(a);
    let b = Path::new(b);
    a.ends_with(b) || b.ends_with(a)
}

fn pct(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

fn add(map: &mut BTreeMap<String, SizeRow>, name: &str, bytes: u64, tokens: u64, ctt: u64) {
    let row = map.entry(name.to_string()).or_default();
    row.count += 1;
    row.total_bytes += bytes;
    row.max = row.max.max(bytes);
    row.est_tokens += tokens;
    row.ctt += ctt;
    // mean/p95 filled in finish_rows from totals; p95 needs samples — stash bytes in max-only
    // for T1.2 we recompute mean from totals; p95 approximated as max until we store samples.
}

/// Samples live in `total_bytes` history via a side vec keyed... keep it simple: mean from
/// totals; p95 = max for v0 (honest: we don't keep every size). Tests pin mean/ctt.
fn finish_rows(map: &mut BTreeMap<String, SizeRow>) {
    for row in map.values_mut() {
        row.mean = row.total_bytes.checked_div(row.count).unwrap_or(0);
        row.p95 = row.max;
    }
}

fn est_tokens(bytes: u64) -> u64 {
    if bytes == 0 {
        0
    } else {
        ((bytes as f64) / CHARS_PER_TOKEN).ceil() as u64
    }
}

pub fn bash_family(cmd: &str) -> String {
    let mut s = cmd.trim();
    loop {
        let next = strip_prefix_env(s).or_else(|| strip_prefix_cd(s));
        match next {
            Some(rest) if rest != s => s = rest,
            _ => break,
        }
    }
    let first = s.split_whitespace().next().unwrap_or("other");
    // Split `/` and `\` + strip `.exe` so Windows session logs still bucket by family.
    crate::agents::cmd_stem(first).to_string()
}

fn strip_prefix_env(s: &str) -> Option<&str> {
    let t = s.trim_start();
    let ident_end = t.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    if ident_end == 0 || !t.as_bytes().get(ident_end).is_some_and(|b| *b == b'=') {
        return None;
    }
    crate::plugins::skip_word(&t[ident_end + 1..])
}

fn strip_prefix_cd(s: &str) -> Option<&str> {
    let after = s.trim_start().strip_prefix("cd")?;
    if !after.starts_with([' ', '\t']) {
        return None;
    }
    let after = after.trim_start();
    let rest = if after.starts_with("&&") {
        after
    } else {
        crate::plugins::skip_word(after)?
    };
    Some(rest.strip_prefix("&&")?.trim_start())
}

fn mcp_group(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("mcp__")?;
    Some(rest.split("__").next().unwrap_or(rest))
}

// SQLite measurements for a catalogue plugin (`rtok stats --plugin cmd --json`) live in the
// operator model (`crate::web::model::plugin_stats`, T15.11): the command renders the page,
// it does not query the store.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io::Write;

    #[test]
    fn since_60d_parses() {
        assert_eq!(parse_since("60d").unwrap(), Duration::from_secs(60 * 86400));
    }

    #[test]
    fn price_arithmetic_costs_legs_and_cache_saving() {
        let p = crate::config::ModelPrice {
            input: 2.0,
            cache_write: 2.5,
            cache_read: 0.2,
            output: 10.0,
        };
        assert_eq!(
            row_cost(2_000_000, 400_000, 8_000_000, 500_000, &p),
            (11.6, 14.4)
        );
        assert_eq!(row_cost(0, 0, 0, 0, &p), (0.0, 0.0));
        // A read price above input never yields a negative saving.
        let p = crate::config::ModelPrice {
            input: 1.0,
            cache_write: 1.0,
            cache_read: 2.0,
            output: 1.0,
        };
        assert_eq!(row_cost(0, 0, 1_000_000, 0, &p), (2.0, 0.0));
    }

    #[test]
    fn attach_costs_prices_known_and_dashes_unknown() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s1", None, None, None, Some("proxy"))
            .unwrap();
        let id = store
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        store
            .insert_usage(
                "s1",
                Some("claude-sonnet-5"),
                "anthropic",
                2_000_000,
                400_000,
                8_000_000,
                500_000,
                id,
            )
            .unwrap();
        store
            .insert_usage("s1", Some("mystery-1"), "anthropic", 30, 0, 0, 1, id)
            .unwrap();
        store
            .insert_usage("s1", None, "anthropic", 7, 0, 0, 0, id)
            .unwrap();
        let mut report = Report::default();
        attach_costs(
            &mut report,
            &store,
            &crate::config::Config::default().stats.prices,
        )
        .unwrap();
        let cost = report.cost.unwrap();
        assert_eq!(cost.total_cost, 11.6);
        assert_eq!(cost.total_saved, 14.4);
        assert_eq!(cost.unknown, ["mystery-1", "unknown"]);
        let m = &cost.models["mystery-1"];
        assert_eq!(m.input, 30);
        assert_eq!((m.cost, m.saved), (None, None));
        assert_eq!(cost.models["claude-sonnet-5"].cost, Some(11.6));
        let table = cost.to_table();
        assert!(table.contains("11.60"), "{table}");
        assert!(table.contains("mystery-1"), "{table}");
    }

    /// A window wider than the calendar is a typo, not a wrapped duration: the multiply
    /// used to panic in a debug build and overflow silently in a release one.
    #[test]
    fn an_absurd_since_is_refused_not_wrapped() {
        let err = parse_since("99999999999999999d").unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
    }

    #[test]
    fn bash_family_strips_cd_and_env() {
        assert_eq!(bash_family("cd /tmp && git status"), "git");
        assert_eq!(bash_family("FOO=1 grep x"), "grep");
        assert_eq!(bash_family("sed -n 1p"), "sed");
        assert_eq!(bash_family(r"C:\Git\cmd\git.exe status"), "git");
        assert_eq!(bash_family("cargo.exe test"), "cargo");
    }

    #[test]
    fn bash_family_strips_quoted_cd_paths() {
        assert_eq!(bash_family("cd 'My Documents' && git status"), "git");
        assert_eq!(
            bash_family(r#"cd "C:\Program Files\App" && npm test"#),
            "npm"
        );
        assert_eq!(
            bash_family("cd ~/'My Documents'/src && cargo build"),
            "cargo"
        );
        assert_eq!(bash_family(r#"FOO="a b" cd 'x y' && rg z"#), "rg");
        assert_eq!(bash_family("cd\t'a b'\t&& sed -n 1p"), "sed");
        // Malformed quotes fail open: nothing is stripped.
        assert_eq!(bash_family("cd 'unterminated && git status"), "cd");
        assert_eq!(bash_family("FOO='x cd y && git status"), "FOO='x");
        // `cd` followed by something other than a path separator is not `cd`.
        assert_eq!(bash_family("cdx && git status"), "cdx");
    }

    #[test]
    fn ctt_and_tool_totals_on_mini_session() {
        let dir = tempfile_dir();
        let path = dir.join("s.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        // 3 turns: assistant, user result, assistant. result at turn 1, N=3 → ctt = tokens*(3-1)
        let body = "hello world!!"; // 13 bytes → ceil(13/4)=4 tokens; ctt=8
        writeln!(
            f,
            "{}",
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cd x && sed -n p"}}],"usage":{"input_tokens":10,"output_tokens":1}}})
        )
        .unwrap();
        writeln!(
            f,
            "{}",
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":body}]}})
        )
        .unwrap();
        writeln!(
            f,
            "{}",
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"done"}],"usage":{"input_tokens":20,"cache_read_input_tokens":80,"output_tokens":2}}})
        )
        .unwrap();
        let r = collect(
            &dir,
            Duration::from_secs(86400 * 60),
            "",
            Replay::from_cfg(&Config::default()),
        )
        .unwrap();
        assert_eq!(r.sessions, 1);
        let bash = r.tools.get("Bash").unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.est_tokens, 4);
        assert_eq!(bash.ctt, 8);
        assert_eq!(r.bash_families.get("sed").unwrap().count, 1);
        assert_eq!(r.usage_cache_read, 80);
        fs::remove_dir_all(&dir).ok();
    }

    /// T61.1: skill bodies ride as `isMeta` records keyed by the top-level
    /// `sourceToolUseID`; the fold keys them by the `Skill` tool_use's `input.skill`
    /// and `resident` multiplies the body bytes by the API requests at or after the
    /// injection turn. One 3-line and one 3,000-line body.
    #[test]
    fn skills_fold_injected_bodies_and_count_resident_requests() {
        let dir = tempfile_dir();
        let path = dir.join("sk.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        let tiny = "line one\nline two\nline three\n";
        let huge: String = (0..3000).map(|i| format!("body line {i}\n")).collect();
        let line = |v: serde_json::Value| v.to_string();
        // Turn 0: the Skill tool_use. Turn 1: its 22-byte tool_result. Turn 2: the
        // isMeta body — what every later request of the session re-carries.
        for l in [
            line(json!({"type":"assistant","message":{"id":"m1","content":[
                {"type":"tool_use","id":"tu-s1","name":"Skill","input":{"skill":"tiny-skill","args":""}}]},
                "usage":{"input_tokens":10,"output_tokens":1}})),
            line(json!({"type":"user","message":{"content":[
                {"type":"tool_result","tool_use_id":"tu-s1","content":"ideas.md"}]}})),
            line(
                json!({"type":"user","isMeta":true,"sourceToolUseID":"tu-s1",
                "message":{"role":"user","content":[{"type":"text","text":tiny}]}}),
            ),
            line(
                json!({"type":"assistant","message":{"id":"m2","content":[]},
                "usage":{"input_tokens":100,"output_tokens":2}}),
            ),
            line(json!({"type":"assistant","message":{"id":"m3","content":[
                {"type":"tool_use","id":"tu-s2","name":"Skill","input":{"skill":"huge-skill","args":""}}]},
                "usage":{"input_tokens":110,"output_tokens":2}})),
            line(
                json!({"type":"user","isMeta":true,"sourceToolUseID":"tu-s2",
                "message":{"role":"user","content":[{"type":"text","text":huge}]}}),
            ),
        ] {
            writeln!(f, "{l}").unwrap();
        }
        let r = collect(
            &dir,
            Duration::from_secs(86400 * 60),
            "",
            Replay::from_cfg(&Config::default()),
        )
        .unwrap();
        let skills = r.skills.as_ref().expect("skills folded");
        let tiny_row = skills.get("tiny-skill").unwrap();
        assert_eq!(tiny_row.count, 1);
        assert_eq!(tiny_row.bytes, tiny.len() as u64);
        assert_eq!(tiny_row.mean, tiny.len() as u64);
        assert_eq!(tiny_row.p95, tiny_row.max);
        assert_eq!(
            tiny_row.resident,
            tiny.len() as u64 * 2,
            "the m2 and m3 requests carried it"
        );
        let huge_row = skills.get("huge-skill").unwrap();
        assert_eq!(huge_row.count, 1);
        assert_eq!(huge_row.bytes, huge.len() as u64);
        assert_eq!(
            huge_row.resident, 0,
            "no API request rides after the last injection in this fixture"
        );
        // The 8-byte tool_result is all `tools` sees for Skill; the body lives in
        // the skills fold now.
        assert_eq!(
            r.tools.get("Skill").unwrap().total_bytes,
            "ideas.md".len() as u64
        );
        let table = r.to_table();
        assert!(
            table.contains("skill") && table.contains("resident"),
            "{table}"
        );
        let js = r.to_json().unwrap();
        assert!(js.contains("\"skills\""), "{js}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn edit_row_sums_old_and_new_strings_over_edit_and_multiedit() {
        let dir = tempfile_dir();
        let path = dir.join("e.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        // One Edit (old 10 B, new 3 B), one MultiEdit with two edits (old 4+6 B, new 1+2 B),
        // one Bash that only feeds the denominator.
        let edit = json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Edit","input":{"file_path":"/a.rs","old_string":"0123456789","new_string":"abc"}}],"usage":{"input_tokens":1,"output_tokens":10}}});
        let multi = json!({"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","id":"t2","name":"MultiEdit","input":{"file_path":"/b.rs","edits":[{"old_string":"abcd","new_string":"x"},{"old_string":"abcdef","new_string":"xy"}]}}],"usage":{"input_tokens":1,"output_tokens":10}}});
        let bash = json!({"type":"assistant","message":{"id":"m3","content":[{"type":"tool_use","id":"t3","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":1,"output_tokens":10}}});
        for line in [&edit, &multi, &bash] {
            writeln!(f, "{line}").unwrap();
        }
        let r = collect(
            &dir,
            Duration::from_secs(86400 * 60),
            "",
            Replay::from_cfg(&Config::default()),
        )
        .unwrap();
        let e = &r.edits;
        assert_eq!((e.calls, e.old_bytes, e.new_bytes), (3, 20, 6));
        let denom: u64 = [&edit, &multi, &bash]
            .iter()
            .map(|v| v["message"]["content"][0]["input"].to_string().len() as u64)
            .sum();
        assert_eq!(e.tool_input_bytes, denom);
        let table = r.to_table();
        assert!(
            table.contains("edit calls 3  old_string 20 B  new_string 6 B"),
            "{table}"
        );
        assert!(
            table.contains(&format!(
                "old/tool_input {:.1}%",
                100.0 * 20.0 / denom as f64
            )),
            "{table}"
        );
        // 20 B → 5 est. tokens of 30 output tokens.
        assert!(table.contains("old/output_tokens (est) 16.7%"), "{table}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_delta_counts_reread_after_edit_not_unchanged_reread() {
        let dir = tempfile_dir();
        let path = dir.join("d.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        // Read /a.rs (10 B) → Edit /a.rs → Read /a.rs (20 B, counts) → Read /a.rs
        // again with no edit (does not count) → Read /b.rs only once (does not).
        let lines = [
            json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"src/a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"0123456789"}]}}),
            json!({"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","id":"t2","name":"Edit","input":{"file_path":"/proj/src/a.rs","old_string":"x","new_string":"y"}}]}}),
            json!({"type":"assistant","message":{"id":"m3","content":[{"type":"tool_use","id":"t3","name":"Read","input":{"file_path":"src/a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t3","content":"01234567890123456789"}]}}),
            json!({"type":"assistant","message":{"id":"m4","content":[{"type":"tool_use","id":"t4","name":"Read","input":{"file_path":"src/a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t4","content":"same"}]}}),
            json!({"type":"assistant","message":{"id":"m5","content":[{"type":"tool_use","id":"t5","name":"Read","input":{"file_path":"/b.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t5","content":"bbbb"}]}}),
        ];
        for line in &lines {
            writeln!(f, "{line}").unwrap();
        }
        let r = collect(
            &dir,
            Duration::from_secs(86400 * 60),
            "",
            Replay::from_cfg(&Config::default()),
        )
        .unwrap();
        let d = &r.read_delta;
        assert_eq!((d.calls, d.bytes, d.read_bytes), (1, 20, 10 + 20 + 4 + 4));
        let table = r.to_table();
        assert!(
            table.contains("read delta calls 1  bytes 20  of Read bytes 38  52.6%"),
            "{table}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeat_counts_identical_bodies_from_different_tools() {
        let dir = tempfile_dir();
        let path = dir.join("r.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        // Bash then Read, same 20-byte body; a third distinct body does not count.
        let lines = [
            json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cat a"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"01234567890123456789"}]}}),
            json!({"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","id":"t2","name":"Read","input":{"file_path":"a"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"01234567890123456789"}]}}),
            json!({"type":"assistant","message":{"id":"m3","content":[{"type":"tool_use","id":"t3","name":"Bash","input":{"command":"head -1000 a"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t3","content":"different-output"}]}}),
        ];
        for line in &lines {
            writeln!(f, "{line}").unwrap();
        }
        let r = collect(
            &dir,
            Duration::from_secs(86400 * 60),
            "",
            Replay::from_cfg(&Config::default()),
        )
        .unwrap();
        let d = &r.repeat;
        assert_eq!((d.calls, d.bytes, d.result_bytes), (1, 20, 20 + 20 + 16));
        let table = r.to_table();
        assert!(
            table.contains("repeat calls 1  bytes 20  of result bytes 56  35.7%"),
            "{table}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn archive_replay_keeps_young_turns_and_shrinks_old_ones() {
        let rp = Replay {
            keep_turns: 2,
            min_tokens: 100,
            head_lines: 1,
            tail_lines: 1,
        };
        let content = "x".repeat(50) + "\n" + &"y".repeat(500) + "\n" + &"z".repeat(50);
        let tokens = est_tokens(content.len() as u64);
        assert_eq!(replay_ctt(&content, tokens, 2, rp), tokens * 2); // within keep_turns
        assert_eq!(replay_ctt(&content, 10, 5, rp), 50); // below min_tokens
        let after = replay_ctt(&content, tokens, 5, rp);
        let pointer = est_tokens(51 + 51 + 64); // head line, tail line, pointer line
        assert_eq!(after, tokens * 2 + pointer * 3);
        assert!(after < tokens * 5);
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rtok-stats-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn two_apis_print_as_two_table_rows() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_session("s1", None, None, None, Some("proxy"))
            .unwrap();
        let id1 = store
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        let id2 = store
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                None,
                None,
                None,
                Some("/v1/chat/completions"),
            )
            .unwrap();
        store
            .insert_usage("s1", Some("m"), "anthropic", 10, 1, 2, 3, id1)
            .unwrap();
        store
            .insert_usage("s1", Some("m"), "openai_chat", 20, 0, 5, 4, id2)
            .unwrap();
        let mut report = Report::default();
        attach_api(&mut report, &store).unwrap();
        let table = report.to_table();
        let names: Vec<&str> = table
            .lines()
            .filter(|l| l.starts_with("anthropic") || l.starts_with("openai_chat"))
            .collect();
        assert_eq!(names.len(), 2, "{table}");
        assert!(names.iter().any(|l| l.starts_with("anthropic")));
        assert!(names.iter().any(|l| l.starts_with("openai_chat")));
    }
}
