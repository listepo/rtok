//! `rtok agents usage` (T358): tokens and estimated cost per agent, day and month. T358.1
//! reads only what passed through rtok (`usage` rows in the store); the agents' own logs are
//! T358.2. Costs come from `[stats.prices]` through `measure::stats::row_cost`, the same
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
}

#[derive(Debug, Serialize)]
pub struct Agent {
    pub host: String,
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
    pub periods: Vec<Period>,
    #[serde(skip)]
    daily: bool,
}

/// The report `[agents.usage]` describes, over the store's `usage` rows. `now` anchors a
/// `since = "30d"` window.
pub fn report(cfg: &Config, store: &Store, now: i64) -> Result<Report> {
    let o = &cfg.agents.usage;
    if o.source != "rtok" {
        bail!(
            "agents.usage.source `{}` is not available yet; only `rtok` is (the agents' own logs are T358.2)",
            o.source
        );
    }
    let daily = match o.period.as_str() {
        "monthly" => false,
        "daily" => true,
        other => bail!("agents.usage.period `{other}`: expected `monthly` or `daily`"),
    };
    if let Some(bad) = o.hosts.iter().find(|h| !super::HOSTS.contains(&h.as_str())) {
        bail!(
            "unknown host `{bad}` in agents.usage.hosts; known: {}",
            super::HOSTS.join(", ")
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

    let (mut totals, mut sessions, mut days) = (Row::default(), BTreeSet::new(), BTreeSet::new());
    let (mut agents, mut periods): (BTreeMap<String, Row>, BTreeMap<String, Row>) =
        (BTreeMap::new(), BTreeMap::new());
    let mut unpriced: BTreeMap<(String, String), i64> = BTreeMap::new();
    for b in store.usage_slices(since, until)? {
        let host = b
            .host
            .clone()
            .unwrap_or_else(|| format!("unattributed ({})", b.api));
        if !o.hosts.is_empty() && !o.hosts.contains(&host) {
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
        sessions.insert(b.session);
        days.insert((host, day));
    }

    let through = days.iter().map(|(_, d)| d.clone()).max();
    let mut agents: Vec<Agent> = agents
        .into_iter()
        .map(|(host, row)| Agent {
            host,
            row: row.rounded(),
        })
        .collect();
    // Dearest first; an unpriced agent (`None`) sorts below every priced one.
    agents.sort_by(|a, b| {
        let by_cost = b
            .row
            .cost_usd
            .unwrap_or(-1.0)
            .total_cmp(&a.row.cost_usd.unwrap_or(-1.0));
        by_cost.then(b.row.tokens.cmp(&a.row.tokens))
    });
    Ok(Report {
        source: "rtok",
        tz: tz.iana_name().unwrap_or("local").to_string(),
        through,
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
        },
        agents,
        periods: periods
            .into_iter()
            .map(|(period, row)| Period {
                period,
                row: row.rounded(),
            })
            .collect(),
        daily,
    })
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
    let cents = (c * 100.0).round() as u64;
    format!("${}.{:02}", grouped(cents / 100), cents % 100)
}

impl Report {
    /// The default screen: header, summary, the unpriced warning, per-agent table, then the
    /// month (or day) rows.
    pub fn to_text(&self) -> String {
        let Some(through) = &self.through else {
            return format!(
                "rtok agents usage: through rtok, no usage recorded ({})\n",
                self.tz
            );
        };
        let t = &self.totals;
        let mut out = format!(
            "rtok agents usage: through rtok, up to {through} ({})\n\n  {} tokens\n  {} estimated cost\n  {} sessions\n  {} daily rows\n",
            self.tz,
            units(t.row.tokens),
            usd(t.row.cost_usd),
            grouped(t.sessions as u64),
            grouped(t.daily_rows as u64),
        );
        if self.unpriced_models > 0 {
            let n = self.unpriced_models;
            out.push_str(&format!(
                "\n! Cost is incomplete: {n} model{} no price in [stats.prices], so {} tokens are not in\n  the estimate. `rtok agents usage --unpriced` lists them.\n",
                if n == 1 { " has" } else { "s have" },
                if n == 1 { "its" } else { "their" },
            ));
        }
        let cols = [Col::left(0), Col::right(0), Col::right(0)];
        let mut rows = vec![vec![
            "Agent".into(),
            "Tokens".into(),
            "Estimated cost".into(),
        ]];
        rows.extend(
            self.agents
                .iter()
                .map(|a| vec![a.host.clone(), units(a.row.tokens), usd(a.row.cost_usd)]),
        );
        out.push_str(&format!("\n{}", table(&cols, &rows)));
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

! Cost is incomplete: 1 model has no price in [stats.prices], so its tokens are not in
  the estimate. `rtok agents usage --unpriced` lists them.

Agent                    Tokens Estimated cost
codex                      1.2M          $4.00
claude                     3.6M          $2.20
unattributed (anthropic)     2K          $0.00

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
        for (key, value) in [
            ("hosts", "nope"),
            ("tz", "Mars/Base"),
            ("period", "weekly"),
            ("source", "logs"),
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
                "output",
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
