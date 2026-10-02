//! `rtok agents usage` (T358): tokens and estimated cost per agent, day and month, from the
//! agents' own logs (`measure::usage`), from what passed through rtok (`usage` rows in the
//! store), or both (T358.1, T358.2). Costs come from `[stats.prices]` through `measure::stats::row_cost`, the same
//! price table and arithmetic as `rtok stats --price` (T49.1), and a model without a price
//! counts in every token total and is named in `unpriced`, never guessed.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::Serialize;

use crate::config::{Config, ModelPrice};
use crate::measure::stats::{parse_since, row_cost};
use crate::measure::usage::{Skipped, read};
use crate::render::{Col, table};
use crate::store::{Store, UsageSlice};

/// Token legs and the estimated cost of one table row. `cost_usd` is `None` when no model in
/// the row has a price: `-` in the table, `null` in JSON, never `$0.00` (that means free).
#[derive(Debug, Default, Serialize)]
pub struct Row {
    pub tokens: i64,
    pub input: i64,
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub cost_usd: Option<f64>,
}

impl Row {
    fn add(&mut self, b: &UsageSlice, cost: Option<f64>) {
        self.input += b.input;
        self.cache_write += b.cache_create;
        self.cache_read += b.cache_read;
        self.output += b.output;
        self.tokens += b.input + b.cache_create + b.cache_read + b.output;
        if let Some(c) = cost {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + c);
        }
    }

    fn rounded(mut self) -> Self {
        self.cost_usd = self.cost_usd.map(|c| (c * 1e6).round() / 1e6);
        self
    }
}

#[derive(Debug, Serialize)]
pub struct Totals {
    #[serde(flatten)]
    pub row: Row,
    /// Distinct session ids.
    pub sessions: usize,
    /// Distinct `(agent, day)` pairs with usage, what ccusage calls daily rows.
    pub daily_rows: usize,
    /// `rtok` and `both`: what the ledger says rtok removed, net of `expand`.
    #[serde(flatten)]
    pub saved: Option<Saved>,
}

/// Tokens rtok saved (`est_before - est_after` over the `measurements` ledger) and their
/// worth at the average input price the same host's requests paid; `usd` is `None` when no
/// model of that host has a price. An estimate, never a measurement.
#[derive(Debug, Serialize)]
pub struct Saved {
    #[serde(rename = "saved_tokens")]
    pub tokens: i64,
    #[serde(rename = "saved_usd")]
    pub usd: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Agent {
    pub host: String,
    pub name: String,
    #[serde(flatten)]
    pub row: Row,
    /// `both` only: tokens of this agent that also passed through rtok.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub through_rtok_tokens: Option<i64>,
    /// `both` only: `through_rtok_tokens` over the logs' tokens, so an agent that bypasses
    /// the proxy reads low. `None` when the logs hold no tokens for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    #[serde(flatten)]
    pub saved: Option<Saved>,
}

#[derive(Debug, Serialize)]
pub struct ModelRow {
    pub model: String,
    #[serde(flatten)]
    pub row: Row,
}

#[derive(Debug, Serialize)]
pub struct Period {
    pub period: String,
    #[serde(flatten)]
    pub row: Row,
}

#[derive(Debug, Serialize)]
pub struct Unpriced {
    pub model: String,
    pub host: String,
    pub tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub source: &'static str,
    pub tz: String,
    /// The last day with usage, in `tz`.
    pub through: Option<String>,
    pub totals: Totals,
    /// Distinct model ids without a price.
    pub unpriced_models: usize,
    pub unpriced: Vec<Unpriced>,
    pub agents: Vec<Agent>,
    /// `--by model` only: the model rows the middle table shows instead of `agents`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<ModelRow>>,
    pub periods: Vec<Period>,
    /// Hosts whose files exist but could not be read: named, counted nowhere.
    pub skipped: Vec<Skipped>,
    #[serde(skip)]
    daily: bool,
}

/// The report `[agents.usage]` describes, over the store's `usage` rows. `now` anchors a
/// `since = "30d"` window.
pub fn report(cfg: &Config, store: &Store, now: i64) -> Result<Report> {
    let o = &cfg.agents.usage;
    let source = match o.source.as_str() {
        "logs" => "logs",
        "rtok" => "rtok",
        "both" => "both",
        other => bail!("agents.usage.source `{other}`: expected `logs`, `rtok` or `both`"),
    };
    let daily = match o.period.as_str() {
        "monthly" => false,
        "daily" => true,
        other => bail!("agents.usage.period `{other}`: expected `monthly` or `daily`"),
    };
    let by = match o.by.as_str() {
        by @ ("agent" | "model") => by,
        other => bail!("agents.usage.by `{other}`: expected `agent` or `model`"),
    };
    // Droid is not a host rtok installs into (`HOSTS`), but its sessions are on the machine.
    let known: Vec<&str> = super::HOSTS.iter().copied().chain(["droid"]).collect();
    if let Some(bad) = o.hosts.iter().find(|h| !known.contains(&h.as_str())) {
        bail!(
            "unknown host `{bad}` in agents.usage.hosts; known: {}",
            known.join(", ")
        );
    }
    let tz = zone(&o.tz)?;
    let since = bound(&o.since, &tz, now, "since")?.unwrap_or(0);
    let until = match o.until.as_str() {
        "" => i64::MAX,
        d => {
            let day: Date = d.parse().with_context(|| {
                format!("agents.usage.until `{d}`: expected a date such as 2026-09-30")
            })?;
            start_of(day.tomorrow()?, &tz)?
        }
    };

    let wanted = |h: &str| o.hosts.is_empty() || o.hosts.iter().any(|w| w == h);
    let mut skipped = Vec::new();
    let mut from_logs = || {
        let mut l = read(cfg, since);
        l.slices.retain(|b| b.ts < until);
        skipped = l.skipped.into_iter().filter(|s| wanted(&s.host)).collect();
        l.slices
    };
    let (primary, rtok) = match source {
        "rtok" => (store.usage_slices(since, until)?, true),
        "logs" => (from_logs(), false),
        _ => (from_logs(), true),
    };
    // What passed through rtok, per host, and the input price those requests paid on
    // average: the rate the saved tokens are valued at (an estimate, labelled as one).
    let mut through: BTreeMap<String, i64> = BTreeMap::new();
    let mut input_rate: BTreeMap<String, (i64, f64)> = BTreeMap::new();
    let mut saved_by_host: BTreeMap<String, i64> = BTreeMap::new();
    let mut saved_unattributed = 0;
    if rtok {
        let slices = if source == "rtok" {
            primary.clone()
        } else {
            store.usage_slices(since, until)?
        };
        for b in slices {
            let host = host_label(&b);
            if !wanted(&host) {
                continue;
            }
            *through.entry(host.clone()).or_default() +=
                b.input + b.cache_create + b.cache_read + b.output;
            if let Some(p) = price(&cfg.stats.prices, b.model.as_deref().unwrap_or("unknown")) {
                let e = input_rate.entry(host).or_default();
                e.0 += b.input;
                e.1 += b.input as f64 * p.input;
            }
        }
        for (host, saved) in store.measurement_saved_by_host(since, until)? {
            match host {
                Some(h) if wanted(&h) => *saved_by_host.entry(h).or_default() += saved,
                Some(_) => {}
                None if o.hosts.is_empty() => saved_unattributed += saved,
                None => {}
            }
        }
    }

    let (mut totals, mut sessions, mut days) = (Row::default(), BTreeSet::new(), BTreeSet::new());
    let mut rows: [BTreeMap<String, Row>; 3] = Default::default();
    let [agents, periods, models] = &mut rows;
    let mut unpriced: BTreeMap<(String, String), i64> = BTreeMap::new();
    for b in primary {
        let host = host_label(&b);
        if !wanted(&host) {
            continue;
        }
        let model = b.model.as_deref().unwrap_or("unknown");
        let cost = price(&cfg.stats.prices, model)
            .map(|p| row_cost(b.input, b.cache_create, b.cache_read, b.output, p).0);
        let day = Timestamp::from_second(b.ts)?
            .to_zoned(tz.clone())
            .date()
            .to_string();
        let period = if daily {
            day.clone()
        } else {
            day[..7].to_string()
        };
        if cost.is_none() {
            let legs = b.input + b.cache_create + b.cache_read + b.output;
            *unpriced
                .entry((model.to_string(), host.clone()))
                .or_default() += legs;
        }
        totals.add(&b, cost);
        agents.entry(host.clone()).or_default().add(&b, cost);
        periods.entry(period).or_default().add(&b, cost);
        models.entry(model.to_string()).or_default().add(&b, cost);
        sessions.insert(b.session);
        days.insert((host, day));
    }
    // `both` lists an agent that only passed through rtok too, with no logged tokens.
    if source == "both" {
        for host in through.keys() {
            agents.entry(host.clone()).or_default();
        }
    }

    let saved = |host: &str, tokens: i64| Saved {
        tokens,
        usd: input_rate
            .get(host)
            .filter(|(n, _)| *n > 0)
            .map(|(n, cost)| round6(tokens as f64 * (cost / *n as f64) / 1e6)),
    };
    let through_day = days.iter().map(|(_, d)| d.clone()).max();
    let mut agents: Vec<Agent> = std::mem::take(agents)
        .into_iter()
        .map(|(host, row)| {
            let via = (source == "both").then(|| through.get(&host).copied().unwrap_or(0));
            Agent {
                name: name(&host),
                through_rtok_tokens: via,
                coverage: via
                    .filter(|_| row.tokens > 0)
                    .map(|t| (t as f64 / row.tokens as f64 * 1e4).round() / 1e4),
                saved: rtok.then(|| saved(&host, saved_by_host.get(&host).copied().unwrap_or(0))),
                host,
                row: row.rounded(),
            }
        })
        .collect();
    agents.sort_by(|a, b| dearest(&a.row, &b.row));
    let mut models: Vec<ModelRow> = std::mem::take(models)
        .into_iter()
        .map(|(model, row)| ModelRow {
            model,
            row: row.rounded(),
        })
        .collect();
    models.sort_by(|a, b| dearest(&a.row, &b.row));
    let total_saved = rtok.then(|| Saved {
        tokens: saved_by_host.values().sum::<i64>() + saved_unattributed,
        usd: agents
            .iter()
            .filter_map(|a| a.saved.as_ref()?.usd)
            .reduce(|x, y| x + y)
            .map(round6),
    });
    Ok(Report {
        source,
        tz: tz.iana_name().unwrap_or("local").to_string(),
        through: through_day,
        unpriced_models: unpriced
            .keys()
            .map(|(m, _)| m)
            .collect::<BTreeSet<_>>()
            .len(),
        unpriced: unpriced
            .into_iter()
            .map(|((model, host), tokens)| Unpriced {
                model,
                host,
                tokens,
            })
            .collect(),
        totals: Totals {
            row: totals.rounded(),
            sessions: sessions.len(),
            daily_rows: days.len(),
            saved: total_saved,
        },
        agents,
        models: (by == "model").then_some(models),
        periods: std::mem::take(periods)
            .into_iter()
            .map(|(period, row)| Period {
                period,
                row: row.rounded(),
            })
            .collect(),
        skipped,
        daily,
    })
}

/// Dearest first, then most tokens; a row with no priced model (`None`) sorts below every
/// priced one.
fn dearest(a: &Row, b: &Row) -> std::cmp::Ordering {
    let by_cost = b
        .cost_usd
        .unwrap_or(-1.0)
        .total_cmp(&a.cost_usd.unwrap_or(-1.0));
    by_cost.then(b.tokens.cmp(&a.tokens))
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// The agent a request belongs to: its host id, else `unattributed (<api>)` for a proxy
/// request whose session has no host.
fn host_label(b: &UsageSlice) -> String {
    b.host
        .clone()
        .unwrap_or_else(|| format!("unattributed ({})", b.api))
}

/// The name a person knows the host by (`Claude Code`), else the host id.
fn name(host: &str) -> String {
    super::host(host)
        .and_then(|a| {
            let vs = a.variants();
            vs.iter()
                .find(|v| v.kind == super::Kind::Cli)
                .or(vs.first())
        })
        .map_or_else(|| host.to_string(), |v| v.name.to_string())
}

/// `tz` as an IANA zone; empty is the system zone (jiff reads `TZ` and the OS setting).
fn zone(tz: &str) -> Result<TimeZone> {
    if tz.is_empty() {
        return Ok(TimeZone::system());
    }
    TimeZone::get(tz).with_context(|| {
        format!("agents.usage.tz `{tz}`: expected an IANA zone such as Europe/Kyiv")
    })
}

/// Midnight starting `day` in `tz`, in unix seconds.
fn start_of(day: Date, tz: &TimeZone) -> Result<i64> {
    Ok(day.to_zoned(tz.clone())?.timestamp().as_second())
}

/// A window start: empty is unbounded, a date is that day's midnight in `tz`, anything else
/// is a duration back from `now` (`stats::parse_since`).
fn bound(value: &str, tz: &TimeZone, now: i64, key: &str) -> Result<Option<i64>> {
    if value.is_empty() {
        return Ok(None);
    }
    if let Ok(day) = value.parse::<Date>() {
        return start_of(day, tz).map(Some);
    }
    let back = parse_since(value).with_context(|| {
        format!("agents.usage.{key} `{value}`: expected a date or a duration such as 30d")
    })?;
    Ok(Some(
        now - i64::try_from(back.as_secs()).unwrap_or(i64::MAX),
    ))
}

/// The `[stats.prices]` row for `model`: the exact id, else without a provider prefix
/// (`anthropic/…`) and a trailing `-YYYYMMDD` date.
fn price<'a>(prices: &'a BTreeMap<String, ModelPrice>, model: &str) -> Option<&'a ModelPrice> {
    let bare = model.rsplit('/').next().unwrap_or(model);
    let undated = match bare.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => bare,
    };
    prices
        .get(model)
        .or_else(|| prices.get(bare))
        .or_else(|| prices.get(undated))
}

/// `22.1K`, `155.45M`, `26.52B`: decimal SI, two decimals, trailing zeros dropped.
fn units(n: i64) -> String {
    let (v, suffix) = match n {
        1_000_000_000.. => (n as f64 / 1e9, "B"),
        1_000_000.. => (n as f64 / 1e6, "M"),
        1_000.. => (n as f64 / 1e3, "K"),
        _ => return n.to_string(),
    };
    let text = format!("{v:.2}");
    format!(
        "{}{suffix}",
        text.trim_end_matches('0').trim_end_matches('.')
    )
}

fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let parts: Vec<&str> = digits
        .as_bytes()
        .rchunks(3)
        .rev()
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect();
    parts.join(",")
}

fn usd(cost: Option<f64>) -> String {
    let Some(c) = cost else {
        return "-".into();
    };
    // A negative estimate (more `expand` than savings) keeps its sign.
    let cents = (c.abs() * 100.0).round() as u64;
    let sign = if c < 0.0 && cents > 0 { "-" } else { "" };
    format!("{sign}${}.{:02}", grouped(cents / 100), cents % 100)
}

impl Report {
    /// Where the numbers come from, as the header says it.
    fn from(&self) -> String {
        let logs = format!(
            "logs from {} agent{}",
            self.agents.len(),
            if self.agents.len() == 1 { "" } else { "s" }
        );
        match self.source {
            "logs" => logs,
            "both" => format!("{logs} and through rtok"),
            _ => "through rtok".into(),
        }
    }

    /// The middle table: one row per model with `--by model`, else one per agent, with the
    /// `Through rtok` / `Coverage` (`both`) and `Saved` (`rtok`, `both`) columns.
    fn middle_table(&self) -> String {
        if let Some(models) = &self.models {
            let cols = [Col::left(0), Col::right(0), Col::right(0)];
            let mut rows = vec![vec![
                "Model".into(),
                "Tokens".into(),
                "Estimated cost".into(),
            ]];
            rows.extend(
                models
                    .iter()
                    .map(|m| vec![m.model.clone(), units(m.row.tokens), usd(m.row.cost_usd)]),
            );
            return table(&cols, &rows);
        }
        let both = self.source == "both";
        let saved = self.source != "logs";
        let mut head: Vec<String> = ["Agent", "Tokens", "Estimated cost"].map(Into::into).into();
        let mut cols = vec![Col::left(0), Col::right(0), Col::right(0)];
        if both {
            head.extend(["Through rtok".into(), "Coverage".into()]);
        }
        if saved {
            head.extend(["Saved tokens".into(), "Saved est.".into()]);
        }
        cols.resize_with(head.len(), || Col::right(0));
        let mut rows = vec![head];
        rows.extend(self.agents.iter().map(|a| {
            let mut r = vec![a.name.clone(), units(a.row.tokens), usd(a.row.cost_usd)];
            if both {
                r.push(units(a.through_rtok_tokens.unwrap_or(0)));
                r.push(
                    a.coverage
                        .map_or_else(|| "-".into(), |c| format!("{:.0}%", c * 100.0)),
                );
            }
            if let Some(s) = &a.saved {
                r.extend([units(s.tokens), usd(s.usd)]);
            }
            r
        }));
        table(&cols, &rows)
    }

    /// The default screen: header, summary, the unpriced warning, per-agent table, then the
    /// month (or day) rows.
    pub fn to_text(&self) -> String {
        let from = self.from();
        let Some(through) = &self.through else {
            return format!(
                "rtok agents usage: {from}, no usage recorded ({})\n",
                self.tz
            );
        };
        let t = &self.totals;
        let mut out = format!(
            "rtok agents usage: {from}, up to {through} ({})\n\n  {} tokens\n  {} estimated cost\n  {} sessions\n  {} daily rows\n",
            self.tz,
            units(t.row.tokens),
            usd(t.row.cost_usd),
            grouped(t.sessions as u64),
            grouped(t.daily_rows as u64),
        );
        if let Some(s) = &t.saved {
            out.push_str(&format!(
                "  rtok saved {} tokens ({})\n",
                units(s.tokens),
                s.usd.map_or_else(
                    || "no price for an estimate".into(),
                    |u| format!("≈ {}", usd(Some(u)))
                ),
            ));
        }
        if self.unpriced_models > 0 {
            let n = self.unpriced_models;
            out.push_str(&format!(
                "\n! Cost is incomplete: {n} model{} no price in [stats.prices], so {} tokens are not in\n  the estimate. `rtok agents usage --unpriced` lists them.\n",
                if n == 1 { " has" } else { "s have" },
                if n == 1 { "its" } else { "their" },
            ));
        }
        let cols = [Col::left(0), Col::right(0), Col::right(0)];
        out.push_str(&format!("\n{}", self.middle_table()));
        let (label, head) = if self.daily {
            ("Daily totals", "Day")
        } else {
            ("Monthly totals", "Month")
        };
        let mut rows = vec![vec![head.into(), "Tokens".into(), "Estimated cost".into()]];
        rows.extend(
            self.periods
                .iter()
                .map(|p| vec![p.period.clone(), units(p.row.tokens), usd(p.row.cost_usd)]),
        );
        out.push_str(&format!("\n{label}\n{}", table(&cols, &rows)));
        out
    }

    /// `--unpriced`: the models the estimate leaves out, with agent and tokens.
    pub fn unpriced_text(&self) -> String {
        if self.unpriced.is_empty() {
            return "every model has a price in [stats.prices]\n".into();
        }
        let cols = [Col::left(0), Col::left(0), Col::right(0)];
        let mut rows = vec![vec!["Model".into(), "Agent".into(), "Tokens".into()]];
        rows.extend(
            self.unpriced
                .iter()
                .map(|u| vec![u.model.clone(), u.host.clone(), units(u.tokens)]),
        );
        table(&cols, &rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{config_in, tmp_dir};

    fn at(utc: &str) -> i64 {
        utc.parse::<Timestamp>().unwrap().as_second()
    }

    fn price(input: f64, cache_write: f64, cache_read: f64, output: f64) -> ModelPrice {
        ModelPrice {
            input,
            cache_write,
            cache_read,
            output,
        }
    }

    /// A fixture store and config: two priced models (one reached through its provider
    /// prefix and date suffix), one unpriced, one session with no host. Kyiv is UTC+3 in
    /// October, so the first Claude request is on 2026-10-02 there and 2026-10-01 in UTC.
    fn fixture() -> (Config, Store) {
        let mut cfg = config_in(&tmp_dir("usage"));
        cfg.stats.prices = BTreeMap::from([
            ("claude-test".into(), price(1.0, 1.25, 0.1, 5.0)),
            ("gpt-x".into(), price(2.0, 2.0, 0.0, 10.0)),
        ]);
        cfg.agents.usage.source = "rtok".into();
        cfg.agents.usage.tz = "Europe/Kyiv".into();
        let store = Store::open_in_memory().unwrap();
        let rows = [
            (
                "s1",
                Some("claude"),
                "claude-test",
                "2026-10-01T22:30:00Z",
                [1_000_000, 0, 2_000_000, 100_000],
            ),
            (
                "s1",
                Some("claude"),
                "claude-test",
                "2026-10-02T10:00:00Z",
                [500_000, 0, 0, 0],
            ),
            (
                "s2",
                Some("codex"),
                "openai/gpt-x-20260901",
                "2026-09-15T12:00:00Z",
                [1_000_000, 0, 0, 200_000],
            ),
            (
                "s2",
                Some("codex"),
                "mystery",
                "2026-09-15T12:05:00Z",
                [4_000, 0, 0, 0],
            ),
            (
                "s3",
                None,
                "claude-test",
                "2026-10-02T09:00:00Z",
                [2_000, 0, 0, 0],
            ),
        ];
        for (session, host, model, utc, legs) in rows {
            store
                .insert_usage_at(session, host, model, at(utc), legs)
                .unwrap();
        }
        // The ledger: a filter that saved 4M estimated tokens and an `expand` that cost 0.5M
        // back (the claude session), and an `expand` alone on the codex one.
        for (session, plugin, kind, before, after) in [
            ("s1", "cmd", "filter", 5_000_000, 1_000_000),
            ("s1", "expand", "expand", 0, 500_000),
            ("s2", "expand", "expand", 0, 2_000),
        ] {
            store
                .insert_measurement(
                    session,
                    &crate::plugin::Measurement {
                        plugin,
                        kind,
                        before_bytes: 0,
                        after_bytes: 0,
                        est_before: before,
                        est_after: after,
                        ref_id: None,
                        call_id: None,
                    },
                )
                .unwrap();
            store
                .set_measurement_ts(session, at("2026-10-01T22:30:00Z"))
                .unwrap();
        }
        (cfg, store)
    }

    #[test]
    fn the_screen_totals_by_agent_and_month_in_the_zone() {
        let (cfg, store) = fixture();
        let r = report(&cfg, &store, 0).unwrap();
        assert_eq!(r.tz, "Europe/Kyiv");
        assert_eq!(r.through.as_deref(), Some("2026-10-02"));
        assert_eq!((r.totals.sessions, r.totals.daily_rows), (3, 3));
        assert_eq!(r.totals.row.tokens, 4_806_000);
        assert_eq!(r.totals.row.cost_usd, Some(6.202));
        assert_eq!(r.unpriced_models, 1);
        let text = r.to_text();
        let expected = "\
rtok agents usage: through rtok, up to 2026-10-02 (Europe/Kyiv)

  4.81M tokens
  $6.20 estimated cost
  3 sessions
  3 daily rows
  rtok saved 3.5M tokens (≈ $3.50)

! Cost is incomplete: 1 model has no price in [stats.prices], so its tokens are not in
  the estimate. `rtok agents usage --unpriced` lists them.

Agent                    Tokens Estimated cost Saved tokens Saved est.
Codex                      1.2M          $4.00        -2000      $0.00
Claude Code                3.6M          $2.20         3.5M      $3.50
unattributed (anthropic)     2K          $0.00            0      $0.00

Monthly totals
Month   Tokens Estimated cost
2026-09   1.2M          $4.00
2026-10   3.6M          $2.20
";
        assert_eq!(text, expected);
        assert_eq!(
            r.unpriced_text(),
            "Model   Agent Tokens\nmystery codex     4K\n"
        );
    }

    #[test]
    fn a_session_across_utc_midnight_splits_by_the_zone_not_by_utc() {
        let (mut cfg, store) = fixture();
        cfg.agents.usage.period = "daily".into();
        let days = |cfg: &Config| -> Vec<String> {
            let r = report(cfg, &store, 0).unwrap();
            r.periods.into_iter().map(|p| p.period).collect()
        };
        assert_eq!(days(&cfg), ["2026-09-15", "2026-10-02"]);
        cfg.agents.usage.tz = "UTC".into();
        assert_eq!(days(&cfg), ["2026-09-15", "2026-10-01", "2026-10-02"]);
    }

    /// Kyiv is UTC+3 on 1 August and UTC+2 on 1 January: the same 21:30 UTC lands on the
    /// next local day in summer and, an hour later, in winter.
    /// The on-disk fixture the trycmd cases read too: one Claude Code transcript (a streamed
    /// message repeated, one unpriced model) and one Codex rollout.
    fn logs_config(source: &str) -> Config {
        let root = format!(
            "{}/tests/trycmd/input/usage-logs",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut cfg = config_in(&tmp_dir("usage-logs"));
        cfg.stats.transcripts_dir = format!("{root}/claude").into();
        cfg.stats.codex_dir = format!("{root}/codex").into();
        cfg.stats.prices = BTreeMap::from([
            ("claude-test".into(), price(1.0, 1.25, 0.1, 5.0)),
            ("gpt-x".into(), price(2.0, 2.0, 0.0, 10.0)),
        ]);
        cfg.agents.usage.source = source.into();
        cfg.agents.usage.tz = "Europe/Kyiv".into();
        cfg
    }

    #[test]
    fn logs_total_each_agent_once_and_cut_days_in_the_zone() {
        let cfg = logs_config("logs");
        let store = Store::open_in_memory().unwrap();
        let r = report(&cfg, &store, 0).unwrap();
        assert_eq!(r.totals.row.tokens, 6_850_000);
        assert_eq!((r.totals.sessions, r.totals.daily_rows), (2, 3));
        // 1.7 (claude-test) + 6.0 (gpt-x); `mystery-model` is counted, not priced.
        assert_eq!(r.totals.row.cost_usd, Some(7.7));
        assert_eq!(
            (r.unpriced_models, r.unpriced[0].model.as_str()),
            (1, "mystery-model")
        );
        let names: Vec<_> = r.agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Codex", "Claude Code"]);
        assert!(r.agents.iter().all(|a| a.coverage.is_none()));
        // 23:30 UTC on 30 Sept is already 1 Oct in Kyiv.
        assert_eq!(
            r.periods
                .iter()
                .map(|p| p.period.as_str())
                .collect::<Vec<_>>(),
            ["2026-10"]
        );
        assert!(
            r.to_text().starts_with(
                "rtok agents usage: logs from 2 agents, up to 2026-10-02 (Europe/Kyiv)"
            )
        );
        let mut utc = logs_config("logs");
        utc.agents.usage.tz = "UTC".into();
        let r = report(&utc, &store, 0).unwrap();
        assert_eq!(
            r.periods
                .iter()
                .map(|p| p.period.as_str())
                .collect::<Vec<_>>(),
            ["2026-09", "2026-10"]
        );
        let mut one = logs_config("logs");
        one.agents.usage.hosts = vec!["codex".into()];
        assert_eq!(
            report(&one, &store, 0).unwrap().totals.row.tokens,
            3_200_000
        );
    }

    #[test]
    fn both_reports_how_much_of_each_agent_passed_through_rtok() {
        let cfg = logs_config("both");
        let store = Store::open_in_memory().unwrap();
        store
            .insert_usage_at(
                "s1",
                Some("claude"),
                "claude-test",
                at("2026-10-01T09:00:00Z"),
                [1_000_000, 0, 0, 0],
            )
            .unwrap();
        let r = report(&cfg, &store, 0).unwrap();
        // Totals and costs stay the logs'; the store only fills the coverage columns.
        assert_eq!(r.totals.row.tokens, 6_850_000);
        let claude = r.agents.iter().find(|a| a.host == "claude").unwrap();
        assert_eq!(claude.through_rtok_tokens, Some(1_000_000));
        assert_eq!(claude.coverage, Some(0.274));
        let codex = r.agents.iter().find(|a| a.host == "codex").unwrap();
        assert_eq!(
            (codex.through_rtok_tokens, codex.coverage),
            (Some(0), Some(0.0))
        );
        let text = r.to_text();
        assert!(
            text.contains("logs from 2 agents and through rtok"),
            "{text}"
        );
        assert!(text.contains("Through rtok Coverage"), "{text}");
        assert!(text.contains("27%"), "{text}");
    }

    #[test]
    fn saved_tokens_are_the_net_ledger_per_agent_priced_at_its_input_rate() {
        let (cfg, store) = fixture();
        let v = serde_json::to_value(report(&cfg, &store, 0).unwrap()).unwrap();
        // 5M - 1M saved by the filter, 0.5M given back by `expand`; claude-test input is $1/MTok.
        assert_eq!(v["totals"]["saved_tokens"], 3_498_000);
        let claude = v["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["host"] == "claude")
            .unwrap();
        assert_eq!(claude["saved_tokens"], 3_500_000);
        assert_eq!(claude["saved_usd"], 3.5);
        // The codex session saw only an `expand`: a negative saving, priced at gpt-x's $2.
        let codex = &v["agents"][0];
        assert_eq!(
            (codex["saved_tokens"].as_i64(), codex["saved_usd"].as_f64()),
            (Some(-2000), Some(-0.004))
        );
        // Outside the window the ledger rows are not counted.
        let mut late = cfg;
        late.agents.usage.since = "2026-10-03".into();
        let r = report(&late, &store, 0).unwrap();
        assert_eq!(r.totals.saved.map(|s| s.tokens), Some(0));
    }

    #[test]
    fn by_model_groups_the_middle_table_by_model() {
        let (mut cfg, store) = fixture();
        cfg.agents.usage.by = "model".into();
        let r = report(&cfg, &store, 0).unwrap();
        let models: Vec<_> = r
            .models
            .as_ref()
            .unwrap()
            .iter()
            .map(|m| m.model.as_str())
            .collect();
        // Dearest first; the unpriced `mystery` is last, and the raw ids are kept.
        assert_eq!(models, ["openai/gpt-x-20260901", "claude-test", "mystery"]);
        let text = r.to_text();
        assert!(text.contains("Model "), "{text}");
        assert!(!text.contains("Saved tokens"), "{text}");
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["models"][0]["model"], "openai/gpt-x-20260901");
        cfg.agents.usage.by = "agent".into();
        assert!(
            serde_json::to_value(report(&cfg, &store, 0).unwrap())
                .unwrap()
                .get("models")
                .is_none()
        );
        cfg.agents.usage.by = "day".into();
        assert!(report(&cfg, &store, 0).is_err());
    }

    #[test]
    fn both_lists_an_agent_that_only_passed_through_rtok() {
        let cfg = logs_config("both");
        let store = Store::open_in_memory().unwrap();
        store
            .insert_usage_at(
                "s9",
                Some("cursor"),
                "claude-test",
                at("2026-10-01T09:00:00Z"),
                [10, 0, 0, 0],
            )
            .unwrap();
        let r = report(&cfg, &store, 0).unwrap();
        let cursor = r.agents.iter().find(|a| a.host == "cursor").unwrap();
        assert_eq!((cursor.row.tokens, cursor.row.cost_usd), (0, None));
        assert_eq!(
            (cursor.through_rtok_tokens, cursor.coverage),
            (Some(10), None)
        );
        assert_eq!(r.agents.last().unwrap().host, "cursor");
        assert!(r.to_text().contains("Saved tokens"));
    }

    #[test]
    fn a_log_dir_that_holds_no_json_is_named_not_counted() {
        let dir = tmp_dir("usage-skipped");
        std::fs::create_dir_all(dir.join("claude/p")).unwrap();
        std::fs::create_dir_all(dir.join("codex")).unwrap();
        std::fs::write(dir.join("claude/p/s.jsonl"), "not json\nstill not\n").unwrap();
        std::fs::write(dir.join("codex/rollout.jsonl"), "{broken\n").unwrap();
        let mut cfg = logs_config("logs");
        cfg.stats.transcripts_dir = dir.join("claude");
        cfg.stats.codex_dir = dir.join("codex");
        let store = Store::open_in_memory().unwrap();
        let r = report(&cfg, &store, 0).unwrap();
        let named: Vec<_> = r
            .skipped
            .iter()
            .map(|s| (s.host.as_str(), s.reason))
            .collect();
        assert_eq!(
            named,
            [("claude", "unknown format"), ("codex", "unknown format")]
        );
        assert_eq!(r.totals.row.tokens, 0);
        cfg.agents.usage.hosts = vec!["codex".into()];
        assert_eq!(report(&cfg, &store, 0).unwrap().skipped.len(), 1);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["skipped"][0]["host"], "claude");
        assert!(
            v["skipped"][0]["path"]
                .as_str()
                .unwrap()
                .ends_with("s.jsonl")
        );
    }

    #[test]
    fn month_edges_follow_the_daylight_saving_offset() {
        let (mut cfg, _) = fixture();
        let store = Store::open_in_memory().unwrap();
        for (utc, session) in [
            ("2026-07-31T21:30:00Z", "a"),
            ("2026-12-31T21:30:00Z", "b"),
            ("2026-12-31T22:30:00Z", "c"),
        ] {
            store
                .insert_usage_at(
                    session,
                    Some("claude"),
                    "claude-test",
                    at(utc),
                    [1_000, 0, 0, 0],
                )
                .unwrap();
        }
        let months: Vec<(String, i64)> = report(&cfg, &store, 0)
            .unwrap()
            .periods
            .into_iter()
            .map(|p| (p.period, p.row.tokens))
            .collect();
        assert_eq!(
            months,
            [
                ("2026-08".to_string(), 1_000),
                ("2026-12".to_string(), 1_000),
                ("2027-01".to_string(), 1_000),
            ]
        );
        cfg.agents.usage.tz = "UTC".into();
        let r = report(&cfg, &store, 0).unwrap();
        assert_eq!(r.periods.len(), 2, "UTC: July and December");
    }

    #[test]
    fn host_and_window_filters_and_bad_values() {
        let (mut cfg, store) = fixture();
        cfg.agents.usage.hosts = vec!["claude".into()];
        cfg.agents.usage.since = "2026-10-02".into();
        let r = report(&cfg, &store, 0).unwrap();
        // Kyiv midnight of the 2nd: the 22:30 UTC request on the 1st is already inside.
        assert_eq!(r.totals.row.tokens, 3_600_000);
        cfg.agents.usage.until = "2026-10-01".into();
        assert_eq!(report(&cfg, &store, 0).unwrap().totals.row.tokens, 0);
        // Droid is not in `HOSTS` yet is a host here: it is listed as unsupported, not refused.
        cfg.agents.usage.hosts = vec!["droid".into()];
        assert!(report(&cfg, &store, 0).is_ok());
        for (key, value) in [
            ("hosts", "nope"),
            ("tz", "Mars/Base"),
            ("period", "weekly"),
            ("source", "bogus"),
        ] {
            let (mut bad, store) = fixture();
            match key {
                "hosts" => bad.agents.usage.hosts = vec![value.into()],
                "tz" => bad.agents.usage.tz = value.into(),
                "period" => bad.agents.usage.period = value.into(),
                _ => bad.agents.usage.source = value.into(),
            }
            assert!(report(&bad, &store, 0).is_err(), "{key}={value}");
        }
    }

    /// The `--json` field names are the contract scripts read; the numbers stay exact.
    #[test]
    fn json_field_names_are_stable() {
        let (cfg, store) = fixture();
        let v = serde_json::to_value(report(&cfg, &store, 0).unwrap()).unwrap();
        let keys = |v: &serde_json::Value| -> Vec<String> {
            v.as_object().unwrap().keys().cloned().collect()
        };
        assert_eq!(
            keys(&v),
            [
                "agents",
                "periods",
                "skipped",
                "source",
                "through",
                "totals",
                "tz",
                "unpriced",
                "unpriced_models"
            ]
        );
        assert_eq!(
            keys(&v["totals"]),
            [
                "cache_read",
                "cache_write",
                "cost_usd",
                "daily_rows",
                "input",
                "output",
                "saved_tokens",
                "saved_usd",
                "sessions",
                "tokens"
            ]
        );
        assert_eq!(
            keys(&v["agents"][0]),
            [
                "cache_read",
                "cache_write",
                "cost_usd",
                "host",
                "input",
                "name",
                "output",
                "saved_tokens",
                "saved_usd",
                "tokens"
            ]
        );
        assert_eq!(keys(&v["periods"][0])[5], "period");
        assert_eq!(keys(&v["unpriced"][0]), ["host", "model", "tokens"]);
        assert_eq!(v["totals"]["input"], 2_506_000);
        assert_eq!(v["agents"][0]["host"], "codex");
    }

    /// T358's check: for the same rows the total is what `rtok stats --price` prints, since
    /// both price through `row_cost` and `[stats.prices]`.
    #[test]
    fn the_total_matches_stats_price_for_the_same_rows() {
        let (cfg, _) = fixture();
        let store = Store::open_in_memory().unwrap();
        for (session, utc) in [("a", "2026-10-01T10:00:00Z"), ("b", "2026-10-02T10:00:00Z")] {
            let legs = [1_234_567, 10, 2_000_000, 98_765];
            store
                .insert_usage_at(session, Some("claude"), "claude-test", at(utc), legs)
                .unwrap();
        }
        let by_model = store.usage_by_model().unwrap();
        let stats_price: f64 = by_model
            .iter()
            .map(|m| {
                let p = &cfg.stats.prices[&m.model];
                row_cost(m.input, m.cache_create, m.cache_read, m.output, p).0
            })
            .sum();
        let total = report(&cfg, &store, 0)
            .unwrap()
            .totals
            .row
            .cost_usd
            .unwrap();
        assert!(
            (stats_price - total).abs() < 1e-5,
            "{stats_price} vs {total}"
        );
    }

    #[test]
    fn units_and_prices_normalise() {
        assert_eq!(
            [
                units(999),
                units(22_100),
                units(155_450_000),
                units(26_520_000_000),
                units(1_000)
            ],
            ["999", "22.1K", "155.45M", "26.52B", "1K"]
        );
        assert_eq!(usd(Some(52_020.9)), "$52,020.90");
        let prices = BTreeMap::from([("claude-test".to_string(), price(1.0, 1.0, 1.0, 1.0))]);
        for id in [
            "claude-test",
            "anthropic/claude-test",
            "claude-test-20260901",
        ] {
            assert!(super::price(&prices, id).is_some(), "{id}");
        }
        assert!(super::price(&prices, "claude-test-2").is_none());
    }
}
