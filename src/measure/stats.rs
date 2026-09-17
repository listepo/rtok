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
use std::collections::BTreeMap;
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
        s.push_str(&format_section("tool", &self.tools));
        s.push_str(&format_section("bash", &self.bash_families));
        s.push_str(&format_section("mcp", &self.mcp_groups));
        s
    }
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
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let meta = e.metadata().ok();
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if mtime < cutoff {
                continue;
            }
            // One unreadable transcript is one malformed entry, not the end of the report.
            let Ok(parsed) = jsonl::parse_path(&p) else {
                report.malformed += 1;
                continue;
            };
            fold_session(&parsed, plugin, replay, &mut report, &mut finals);
        }
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
    for u in &parsed.tool_uses {
        id_name.insert(u.id.as_str(), u.name.as_str());
        if u.name == "Bash"
            && let Some(cmd) = u.input.get("command").and_then(|v| v.as_str())
        {
            id_family.insert(u.id.as_str(), bash_family(cmd));
        }
    }
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
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first);
    let stem = if base.len() >= 4 && base[base.len() - 4..].eq_ignore_ascii_case(".exe") {
        &base[..base.len() - 4]
    } else {
        base
    };
    stem.to_string()
}

fn strip_prefix_env(s: &str) -> Option<&str> {
    let t = s.trim_start();
    let ident_end = t.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    if ident_end == 0 || !t.as_bytes().get(ident_end).is_some_and(|b| *b == b'=') {
        return None;
    }
    skip_word(&t[ident_end + 1..])
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
        skip_word(after)?
    };
    Some(rest.strip_prefix("&&")?.trim_start())
}

/// Skips one shell word (bare, or with `'…'` / `"…"` segments such as `~/'My Documents'`)
/// and returns what follows it, left-trimmed. An unterminated quote yields `None` so the
/// caller fails open and leaves the command untouched.
fn skip_word(s: &str) -> Option<&str> {
    let mut quote = None;
    for (i, b) in s.bytes().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'\'' || b == b'"' => quote = Some(b),
            None if b.is_ascii_whitespace() => return Some(s[i..].trim_start()),
            None => {}
        }
    }
    quote.is_none().then_some("")
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
        let p = std::env::temp_dir().join(format!("rtok-stats-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
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
