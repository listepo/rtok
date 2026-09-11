//! The HTML rendering of [`Document`](super::Document) (T22.2): the same eight sections
//! in the same order, one self-contained file — inline CSS, inline SVG charts, no network
//! fetch, openable from a `file://` URL. It formats; it computes nothing (D24).
//!
//! Charts cover the series the model has — saved tokens per plugin, calls per surface,
//! busts per cause. The model carries no per-turn series, so the rest stays tables.

use super::Document;

const CSS: &str = "body{font-family:sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#222}\ntable{border-collapse:collapse;margin:1rem 0}\nth,td{border:1px solid #ccc;padding:.25rem .5rem;text-align:left}\npre{background:#f6f6f6;padding:1rem;overflow-x:auto}\nsvg.chart{background:#fafafa;margin:1rem 0}\n";

/// Render the whole document: one `<h2>` per Markdown `##` heading, same names, same order.
pub fn render(doc: &Document) -> String {
    let mut s = String::from(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>rtok report</title>\n<style>\n",
    );
    s.push_str(CSS);
    s.push_str("</style>\n</head>\n<body>\n<h1>rtok report</h1>\n");
    let w = &doc.ledgers.window;
    s.push_str(&format!(
        "<p>Window {} → {} ({}) · store {}</p>\n<h2 id=\"window\">Window</h2>\n",
        esc(&w.from_date),
        esc(&w.to_date),
        esc(&w.since),
        esc(&w.db_path)
    ));
    table(
        &mut s,
        &["ledger", "rows", "covers"],
        vec![
            vec![
                "calls".into(),
                format!("{} of {}", w.calls_in_window, w.calls_total),
                format!("window ({})", esc(&w.since)),
            ],
            vec![
                "measurements".into(),
                w.measurements.to_string(),
                "whole ledger (no row times)".into(),
            ],
            vec![
                "usage".into(),
                w.usage.to_string(),
                "whole ledger (no row times)".into(),
            ],
        ],
    );
    if w.calls_total == 0 && w.measurements == 0 && w.usage == 0 {
        s.push_str("<p><strong>No rows in window.</strong> The store has no rows to report.</p>\n");
    }

    let sav = &doc.ledgers.savings;
    s.push_str("<h2 id=\"savings\">Savings</h2>\n<p>Estimated tokens from <code>Measurement</code> rows only (est_before − est_after, net; floors, not context-token-turns).</p>\n");
    if sav.rows.is_empty() {
        s.push_str("<p>No rows in window.</p>\n");
    } else {
        s.push_str(&bars(
            "saved tokens per plugin",
            &sav.rows
                .iter()
                .map(|r| (r.plugin.clone(), r.saved))
                .collect::<Vec<_>>(),
        ));
        table(
            &mut s,
            &["plugin", "rows", "est before", "est after", "saved"],
            sav.rows
                .iter()
                .map(|r| {
                    vec![
                        esc(&r.plugin),
                        r.rows.to_string(),
                        r.est_before.to_string(),
                        r.est_after.to_string(),
                        r.saved.to_string(),
                    ]
                })
                .collect(),
        );
        s.push_str(&format!(
            "<p>Total: {} est tokens over {} <code>Measurement</code> rows (whole ledger: no row times).</p>\n",
            sav.total_saved, sav.total_rows
        ));
    }

    let calls = &doc.ledgers.calls;
    s.push_str("<h2 id=\"calls\">Calls</h2>\n<p>Latency per surface, nearest-rank p50/p95 over the calls that recorded one.</p>\n");
    if calls.total == 0 {
        s.push_str("<p>No rows in window.</p>\n");
    } else {
        s.push_str(&bars(
            "calls per surface",
            &calls
                .rows
                .iter()
                .map(|r| (r.surface.clone(), r.calls as i64))
                .collect::<Vec<_>>(),
        ));
        table(
            &mut s,
            &["surface", "calls", "timed", "p50 ms", "p95 ms"],
            calls
                .rows
                .iter()
                .map(|r| {
                    vec![
                        esc(&r.surface),
                        r.calls.to_string(),
                        r.timed.to_string(),
                        ms(r.p50_ms),
                        ms(r.p95_ms),
                    ]
                })
                .collect(),
        );
        s.push_str(&format!(
            "<p>{} of {} <code>calls</code> rows in window ({}); surfaces beyond hook/mcp/proxy are not in the section set.</p>\n",
            calls.in_window, calls.total, esc(&w.since)
        ));
    }

    let cache = &doc.ledgers.cache;
    s.push_str("<h2 id=\"cache\">Cache</h2>\n<p>Prompt-cache busts by cause, from the proxy's <code>usage</code> rows (whole ledger).</p>\n");
    if cache.sessions == 0 {
        s.push_str("<p>No rows in window.</p>\n");
    } else {
        s.push_str(&bars(
            "busts per cause",
            &cache
                .by_cause
                .iter()
                .map(|(c, n)| (c.clone(), *n as i64))
                .collect::<Vec<_>>(),
        ));
        table(
            &mut s,
            &["cause", "busts"],
            cache
                .by_cause
                .iter()
                .map(|(c, n)| vec![esc(c), n.to_string()])
                .collect(),
        );
        s.push_str(&format!(
            "<p>Busts: {} over {} turns in {} session(s).</p>\n",
            cache.busts, cache.turns, cache.sessions
        ));
    }

    let exp = &doc.ledgers.expand;
    s.push_str("<h2 id=\"expand\">Expand</h2>\n<p>How often a live-zone pointer had to be expanded (<code>archive_decisions</code> rows, T5.4).</p>\n");
    if exp.decisions == 0 {
        s.push_str("<p>No rows in window.</p>\n");
    } else {
        let what = if exp.expanded_ids.is_empty() {
            "none".to_string()
        } else {
            exp.expanded_ids
                .iter()
                .map(|id| format!("<code>{}</code>", esc(id)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        s.push_str(&format!(
            "<p><code>rtok expand</code> froze {} of {} live-zone pointers ({:.1}%). Expanded: {}.</p>\n",
            exp.expanded, exp.decisions, 100.0 * exp.rate, what
        ));
    }

    s.push_str("<h2 id=\"config\">Config</h2>\n<p>Every effective key with its origin — the <code>config show --sources</code> page.</p>\n");
    // Values ride in `<code>` so a URL (`proxy.upstream`) is a code span, never an
    // external reference — the document must open from `file://` fetching nothing.
    table(
        &mut s,
        &["key", "value", "source"],
        doc.config
            .iter()
            .map(|e| {
                vec![
                    esc(&e.key),
                    format!("<code>{}</code>", esc(&e.value)),
                    esc(&e.source),
                ]
            })
            .collect(),
    );
    s.push_str(&format!("<p>{} keys.</p>\n", doc.config.len()));

    s.push_str("<h2 id=\"doctor\">Doctor</h2>\n<p>Live probes (the <code>rtok doctor</code> page), not store rows.</p>\n<pre><code>");
    s.push_str(&esc(&doc.doctor.to_text()));
    s.push_str("</code></pre>\n<h2 id=\"recommendations\">Recommendations</h2>\n");
    if doc.recommendations.is_empty() {
        s.push_str("<p>No recommendations.</p>\n");
    } else {
        s.push_str("<ul>\n");
        for r in &doc.recommendations {
            s.push_str(&format!(
                "<li><strong>{}</strong>: {} ({})</li>\n",
                esc(&r.rule),
                esc(&r.finding),
                esc(&r.evidence)
            ));
        }
        s.push_str("</ul>\n");
    }
    s.push_str("</body>\n</html>\n");
    s
}

/// One `<table>`; cells arrive pre-escaped (or plain numbers).
fn table(s: &mut String, heads: &[&str], rows: Vec<Vec<String>>) {
    s.push_str("<table>\n<tr>");
    for h in heads {
        s.push_str("<th>");
        s.push_str(h);
        s.push_str("</th>");
    }
    s.push_str("</tr>\n");
    for r in &rows {
        s.push_str("<tr>");
        for c in r {
            s.push_str("<td>");
            s.push_str(c);
            s.push_str("</td>");
        }
        s.push_str("</tr>\n");
    }
    s.push_str("</table>\n");
}

/// One inline-SVG bar chart over label → value pairs: no script, no external
/// reference. Empty input is a sentence, not an empty chart.
fn bars(title: &str, pairs: &[(String, i64)]) -> String {
    if pairs.is_empty() {
        return format!("<p>No data for {}.</p>\n", esc(title));
    }
    let shares = super::bar_shares(pairs);
    let mut o = format!(
        "<svg class=\"chart\" width=\"600\" height=\"{}\" role=\"img\" aria-label=\"{}\">\n",
        pairs.len() * 24 + 8,
        esc(title)
    );
    for (i, (label, v)) in pairs.iter().enumerate() {
        let y = i * 24 + 4;
        let wd = (shares[i] * 440.0) as i64;
        o.push_str(&format!(
            "<text x=\"0\" y=\"{}\" font-size=\"12\">{}</text><rect x=\"140\" y=\"{y}\" width=\"{wd}\" height=\"14\"/><text x=\"{}\" y=\"{}\" font-size=\"12\">{}</text>\n",
            y + 12,
            esc(label),
            146 + wd,
            y + 12,
            v
        ));
    }
    o.push_str("</svg>\n");
    o
}

/// `4.0` for a recorded latency, `—` for none — the Markdown table's spelling.
fn ms(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into())
}

/// Escape text for element content and attribute values.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
