//! The `--ai` rendering of [`Document`](super::Document) (T22.4): the same document,
//! shaped for a model instead of a person.
//!
//! Dense plain lines, stable `#id` anchors, tables in the `toon` plugin's encoding
//! (dogfood: the encoder lives in `crate::plugins::toon` and is used here through its
//! `pub(crate)` entry points; without the `toon` feature the same tables fall back to
//! `k=v` lines). Every number keeps its unit and its row count. One budget
//! (`[report] budget_tokens`): whole sections are dropped past it and named in
//! `dropped:` — never truncated mid-table. It formats; it computes nothing (D24): the
//! only arithmetic is measuring rendered sections against the budget.

#[cfg(feature = "toon")]
use serde_json::Value;

use super::Document;
use super::markdown::ms;
use crate::config::Config;
use crate::tokens::{self, Class};

#[cfg(feature = "toon")]
use crate::plugins::toon;

/// Stable section anchors, in the fixed P22 order — a model can be pointed at one.
const IDS: [&str; 8] = [
    "window",
    "savings",
    "calls",
    "cache",
    "expand",
    "config",
    "doctor",
    "recommendations",
];

/// Render the whole document under `cfg.report.budget_tokens`. Sections that do not fit
/// are dropped whole and named in `dropped:`; recommendations still close the document.
pub fn render(doc: &Document, cfg: &Config) -> String {
    use std::fmt::Write as _;
    let mut budget = cfg.report.budget_tokens;
    let mut kept = Vec::with_capacity(IDS.len());
    let mut dropped = Vec::new();
    for id in IDS {
        let body = section(id, doc);
        let cost = tokens::estimate(&body, Class::Prose, &cfg.estimator);
        if cost <= budget {
            budget -= cost;
            kept.push((id, body));
        } else {
            dropped.push(id);
        }
    }
    let w = &doc.ledgers.window;
    let mut s = String::new();
    let _ = writeln!(s, "rtok-report ai");
    let _ = writeln!(
        s,
        "range {}..{} window={} store={}",
        w.from_date, w.to_date, w.since, w.db_path
    );
    let _ = writeln!(
        s,
        "dropped: {}",
        if dropped.is_empty() {
            "none".to_string()
        } else {
            dropped.join(",")
        }
    );
    for (id, body) in kept {
        let _ = writeln!(s, "#{id}");
        s.push_str(&body);
    }
    s
}

fn section(id: &str, doc: &Document) -> String {
    match id {
        "window" => window(doc),
        "savings" => savings(doc),
        "calls" => calls(doc),
        "cache" => cache(doc),
        "expand" => expand(doc),
        "config" => config(doc),
        "doctor" => doctor(doc),
        "recommendations" => recommendations(doc),
        _ => unreachable!("report section ids are the fixed P22 set"),
    }
}

fn window(doc: &Document) -> String {
    use std::fmt::Write as _;
    let w = &doc.ledgers.window;
    let mut s = String::from("units: row counts per ledger\n");
    s.push_str(&table(
        &["ledger", "in_window", "total", "coverage"],
        &[
            vec![
                "calls".into(),
                w.calls_in_window.to_string(),
                w.calls_total.to_string(),
                format!("window {}", w.since),
            ],
            vec![
                "measurements".into(),
                String::new(),
                w.measurements.to_string(),
                "whole ledger no row times".into(),
            ],
            vec![
                "usage".into(),
                String::new(),
                w.usage.to_string(),
                "whole ledger no row times".into(),
            ],
        ],
    ));
    let _ = writeln!(s, "since={}", w.since);
    s
}

fn savings(doc: &Document) -> String {
    use std::fmt::Write as _;
    let sav = &doc.ledgers.savings;
    let mut s = String::from(
        "units: est tokens, floors from Measurement rows (no turn counts); expand rows negative\n",
    );
    if sav.rows.is_empty() {
        s.push_str("no rows.\n");
        return s;
    }
    s.push_str(&table(
        &["plugin", "rows", "est_before", "est_after", "saved"],
        &sav.rows
            .iter()
            .map(|r| {
                vec![
                    r.plugin.clone(),
                    r.rows.to_string(),
                    r.est_before.to_string(),
                    r.est_after.to_string(),
                    r.saved.to_string(),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    let _ = writeln!(
        s,
        "total_saved={}tok rows={}",
        sav.total_saved, sav.total_rows
    );
    s
}

fn calls(doc: &Document) -> String {
    use std::fmt::Write as _;
    let calls = &doc.ledgers.calls;
    let mut s = String::from(
        "units: counts; p50_ms/p95_ms nearest-rank over timed calls, blank when untimed\n",
    );
    if calls.total == 0 {
        s.push_str("no rows.\n");
        return s;
    }
    s.push_str(&table(
        &["surface", "calls", "timed", "p50_ms", "p95_ms"],
        &calls
            .rows
            .iter()
            .map(|r| {
                vec![
                    r.surface.clone(),
                    r.calls.to_string(),
                    r.timed.to_string(),
                    ms(r.p50_ms),
                    ms(r.p95_ms),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    let _ = writeln!(
        s,
        "in_window={} total={} window={}",
        calls.in_window, calls.total, doc.ledgers.window.since
    );
    s
}

fn cache(doc: &Document) -> String {
    use std::fmt::Write as _;
    let cache = &doc.ledgers.cache;
    let mut s = String::from("units: counts from usage rows, whole ledger\n");
    if cache.sessions == 0 {
        s.push_str("no rows.\n");
        return s;
    }
    // Two columns are not tabular for the toon encoder (it needs ≥ 3), and read
    // denser as lines than as an encoded block — `table` falls back on its own.
    s.push_str(&table(
        &["cause", "busts"],
        &cache
            .by_cause
            .iter()
            .map(|(cause, n)| vec![cause.clone(), n.to_string()])
            .collect::<Vec<_>>(),
    ));
    let _ = writeln!(
        s,
        "turns={} sessions={} busts={}",
        cache.turns, cache.sessions, cache.busts
    );
    s
}

fn expand(doc: &Document) -> String {
    use std::fmt::Write as _;
    let exp = &doc.ledgers.expand;
    let mut s = String::from("units: counts; rate = expanded/decisions\n");
    if exp.decisions == 0 {
        s.push_str("no rows.\n");
        return s;
    }
    let _ = writeln!(
        s,
        "decisions={} expanded={} rate={:.1}% expanded_ids={}",
        exp.decisions,
        exp.expanded,
        100.0 * exp.rate,
        if exp.expanded_ids.is_empty() {
            "none".to_string()
        } else {
            exp.expanded_ids.join(",")
        }
    );
    s
}

fn config(doc: &Document) -> String {
    use std::fmt::Write as _;
    let mut s = String::from("units: effective keys with origin\n");
    s.push_str(&table(
        &["key", "value", "source"],
        &doc.config
            .iter()
            .map(|e| vec![e.key.clone(), e.value.clone(), e.source.clone()])
            .collect::<Vec<_>>(),
    ));
    let _ = writeln!(s, "keys={}", doc.config.len());
    s
}

fn doctor(doc: &Document) -> String {
    let mut s = String::from("units: live probes, not store rows\n");
    s.push_str(&doc.doctor.to_text());
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn recommendations(doc: &Document) -> String {
    use std::fmt::Write as _;
    let mut s = String::from("units: rules over ledgers, most recoverable tokens first\n");
    if doc.recommendations.is_empty() {
        s.push_str("no recommendations.\n");
        return s;
    }
    for (i, r) in doc.recommendations.iter().enumerate() {
        let _ = writeln!(s, "{}. [{}] {} ({})", i + 1, r.rule, r.finding, r.evidence);
    }
    s
}

/// A uniform string table: through the toon encoder when the shape is tabular,
/// `k=v` lines otherwise (ragged, non-scalar, or under 3 columns). Ends with `\n`.
fn table(keys: &[&str], rows: &[Vec<String>]) -> String {
    #[cfg(feature = "toon")]
    if let Some(block) = toon_block(keys, rows) {
        let mut block = block;
        block.push('\n');
        return block;
    }
    let mut s = flat_lines(keys, rows);
    s.push('\n');
    s
}

/// The dogfood path: the same encoder `rtok proxy` runs, over the caller's column
/// order (the encoder's own key discovery only validates the shape).
#[cfg(feature = "toon")]
fn toon_block(keys: &[&str], rows: &[Vec<String>]) -> Option<String> {
    let json_rows: Vec<Value> = rows
        .iter()
        .map(|r| {
            Value::Object(
                keys.iter()
                    .zip(r)
                    .map(|(k, v)| (k.to_string(), Value::String(flat(v))))
                    .collect(),
            )
        })
        .collect();
    let arr = Value::Array(json_rows);
    toon::tabular_keys(&arr, 1)?;
    let order: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    Some(toon::encode(arr.as_array()?, &order))
}

fn flat_lines(keys: &[&str], rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|r| {
            keys.iter()
                .zip(r)
                .map(|(k, v)| format!("{k}={}", flat(v)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One unfolded line per cell: a raw newline would break the toon block structure.
fn flat(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn doc() -> Document {
        use crate::web::model::*;
        Document {
            ledgers: ReportLedgers {
                window: ReportWindow {
                    since: "30d".into(),
                    from_unix: 1,
                    to_unix: 2,
                    from_date: "2026-08-10".into(),
                    to_date: "2026-09-09".into(),
                    db_path: "/tmp/rtok.db".into(),
                    calls_in_window: 7,
                    calls_total: 7,
                    measurements: 3,
                    usage: 3,
                },
                savings: ReportSavingsSection {
                    rows: vec![ReportSavings {
                        plugin: "cmd".into(),
                        rows: 1,
                        est_before: 25,
                        est_after: 10,
                        saved: 15,
                    }],
                    total_rows: 1,
                    total_saved: 15,
                },
                calls: ReportCallsSection {
                    rows: vec![ReportCalls {
                        surface: "hook".into(),
                        calls: 2,
                        timed: 2,
                        p50_ms: Some(2.0),
                        p95_ms: Some(4.0),
                    }],
                    in_window: 2,
                    total: 2,
                },
                cache: ReportCache {
                    sessions: 1,
                    turns: 3,
                    busts: 1,
                    by_cause: [("tools".to_string(), 1)].into(),
                },
                expand: ReportExpand {
                    decisions: 3,
                    expanded: 1,
                    rate: 1.0 / 3.0,
                    expanded_ids: vec!["abc".into()],
                },
            },
            config: vec![crate::web::model::ConfigEntry {
                key: "report.since".into(),
                value: "30d".into(),
                source: "user".into(),
            }],
            doctor: crate::doctor::Report {
                hooks_total: 2,
                hooks_by_event: BTreeMap::from([
                    ("PreToolUse".to_string(), 1),
                    ("PostToolUse".to_string(), 1),
                ]),
                mcp: vec![],
                proxy: "8787".into(),
                proxy_openai: String::new(),
                mcp_tool_search_disabled: false,
                bash_max_output_length: None,
                auto_compact_window: None,
                instructions: None,
            },
            recommendations: vec![
                crate::report::Recommendation {
                    rule: "expand-rate".into(),
                    finding: "pointers frozen".into(),
                    evidence: "3 rows".into(),
                },
                crate::report::Recommendation {
                    rule: "cache-bust".into(),
                    finding: "tools rewritten".into(),
                    evidence: "1 turn".into(),
                },
            ],
        }
    }

    fn cfg_with_budget(budget: u32) -> Config {
        let mut cfg = Config::default();
        cfg.report.budget_tokens = budget;
        cfg
    }

    /// The Check's shape clause: one ordered, explicit task list — and the empty
    /// rule set says so instead of inventing tasks (Gate P22).
    #[test]
    fn recommendations_close_as_an_explicit_task_list() {
        let out = render(&doc(), &cfg_with_budget(8000));
        let tail = &out[out.find("#recommendations").expect("id")..];
        assert!(tail.contains("1. [expand-rate] pointers frozen (3 rows)"));
        assert!(tail.contains("2. [cache-bust] tools rewritten (1 turn)"));
        assert!(
            out.ends_with("2. [cache-bust] tools rewritten (1 turn)\n"),
            "nothing follows the task list"
        );

        let mut empty = doc();
        empty.recommendations.clear();
        let out = render(&empty, &cfg_with_budget(8000));
        assert!(out.contains("no recommendations."));
    }

    /// The Check's budget clause: whole sections dropped and named — kept tables
    /// intact, ids of dropped sections gone.
    #[test]
    fn tight_budget_drops_whole_sections_and_names_them() {
        let full = render(&doc(), &cfg_with_budget(8000));
        assert!(full.contains("dropped: none"));
        let out = render(&doc(), &cfg_with_budget(120));
        assert!(!out.contains("dropped: none"), "{out}");
        let dropped: Vec<&str> = out
            .lines()
            .find(|l| l.starts_with("dropped: "))
            .expect("dropped line")[9..]
            .split(',')
            .collect();
        assert!(!dropped.is_empty());
        for id in &dropped {
            assert!(
                IDS.contains(id),
                "named drops are section ids: {id} in {out}"
            );
            assert!(
                !out.contains(&format!("#{id}\n")),
                "a dropped section leaves no heading: {id}"
            );
        }
        // A kept toon block is whole: its header row count matches its body rows.
        for line in out
            .lines()
            .filter(|l| l.starts_with('[') && l.contains("]{"))
        {
            let n: usize = line[1..line.find(']').unwrap()].parse().unwrap();
            assert!(n > 0, "kept block is non-empty: {line}");
        }
    }

    /// The Check's stability clause: two runs over the same data, same bytes.
    #[test]
    fn heading_ids_are_stable_across_runs() {
        let (a, b) = (
            render(&doc(), &cfg_with_budget(8000)),
            render(&doc(), &cfg_with_budget(8000)),
        );
        assert_eq!(a, b);
        let ids: Vec<String> = a
            .lines()
            .filter(|l| l.starts_with('#'))
            .map(str::to_string)
            .collect();
        assert_eq!(
            ids,
            IDS.iter().map(|id| format!("#{id}")).collect::<Vec<_>>(),
            "every section carries its stable id, in order"
        );
    }

    /// Dogfood + density: tabular sections use the toon encoding (no Markdown pipe
    /// tables), every number keeps its unit and row count.
    #[test]
    fn tables_are_toon_and_numbers_keep_units() {
        let out = render(&doc(), &cfg_with_budget(8000));
        assert!(!out.contains("| ---"), "no Markdown pipe tables");
        assert!(out.contains("[1]{plugin,rows,est_before,est_after,saved}:"));
        assert!(out.contains("total_saved=15tok rows=1"));
        assert!(out.contains("units:"));
    }
}
