//! The Markdown rendering of [`Document`](super::Document) — tables, no charts (T22.1).
//!
//! It formats; it computes nothing (D24): every number is a model value, every section
//! names the rows and the window behind it, and a section with no rows says so instead of
//! printing zeros.

use super::Document;

/// Render the whole document. The section order is the fixed P22 set:
/// Window · Savings · Calls · Cache · Expand · Config · Doctor · Recommendations.
pub fn render(doc: &Document) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let w = &doc.ledgers.window;

    let _ = writeln!(
        s,
        "# rtok report\n\nWindow {} → {} ({}) · store {}\n",
        w.from_date, w.to_date, w.since, w.db_path
    );

    // ── Window ──────────────────────────────────────────────────────────────
    let _ = writeln!(s, "## Window\n");
    let _ = writeln!(s, "| ledger | rows | covers |\n| --- | --- | --- |");
    let _ = writeln!(
        s,
        "| calls | {} of {} | window ({}) |",
        w.calls_in_window, w.calls_total, w.since
    );
    let _ = writeln!(
        s,
        "| measurements | {} | whole ledger (no row times) |",
        w.measurements
    );
    let _ = writeln!(s, "| usage | {} | whole ledger (no row times) |", w.usage);
    if w.calls_total == 0 && w.measurements == 0 && w.usage == 0 {
        let _ = writeln!(
            s,
            "\n**No rows in window.** The store has no rows to report."
        );
    }

    // ── Savings ─────────────────────────────────────────────────────────────
    let sav = &doc.ledgers.savings;
    let _ = writeln!(
        s,
        "\n## Savings\n\nEstimated tokens from `Measurement` rows only: est_before − est_after, \
net — an `expand` row counts negative, because retrieval costs tokens. A `Measurement` row \
carries no turn count, so these are floors, not context-token-turns.\n"
    );
    if sav.rows.is_empty() {
        let _ = writeln!(s, "No rows in window.");
    } else {
        let _ = writeln!(
            s,
            "| plugin | rows | est before | est after | saved |\n| --- | ---: | ---: | ---: | ---: |"
        );
        for r in &sav.rows {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} |",
                cell(&r.plugin),
                r.rows,
                r.est_before,
                r.est_after,
                r.saved
            );
        }
        let _ = writeln!(
            s,
            "\nTotal: {} est tokens over {} `Measurement` rows (whole ledger: no row times).",
            sav.total_saved, sav.total_rows
        );
    }

    // ── Calls ───────────────────────────────────────────────────────────────
    let calls = &doc.ledgers.calls;
    let _ = writeln!(
        s,
        "\n## Calls\n\nLatency per surface, nearest-rank p50/p95 over the calls that recorded \
one.\n"
    );
    if calls.total == 0 {
        let _ = writeln!(s, "No rows in window.");
    } else {
        let _ = writeln!(
            s,
            "| surface | calls | timed | p50 ms | p95 ms |\n| --- | ---: | ---: | ---: | ---: |"
        );
        for r in &calls.rows {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} |",
                cell(&r.surface),
                r.calls,
                r.timed,
                ms(r.p50_ms),
                ms(r.p95_ms)
            );
        }
        let _ = writeln!(
            s,
            "\n{} of {} `calls` rows in window ({}); surfaces beyond hook/mcp/proxy are not in \
the section set.",
            calls.in_window, calls.total, w.since
        );
    }

    // ── Cache ───────────────────────────────────────────────────────────────
    let cache = &doc.ledgers.cache;
    let _ = writeln!(
        s,
        "\n## Cache\n\nPrompt-cache busts by cause, from the proxy's `usage` rows (whole \
ledger).\n"
    );
    if cache.sessions == 0 {
        let _ = writeln!(s, "No rows in window.");
    } else {
        let _ = writeln!(s, "| cause | busts |\n| --- | ---: |");
        for (cause, n) in &cache.by_cause {
            let _ = writeln!(s, "| {} | {} |", cell(cause), n);
        }
        let _ = writeln!(
            s,
            "\nBusts: {} over {} turns in {} session(s).",
            cache.busts, cache.turns, cache.sessions
        );
    }

    // ── Expand ──────────────────────────────────────────────────────────────
    let exp = &doc.ledgers.expand;
    let _ = writeln!(
        s,
        "\n## Expand\n\nHow often a live-zone pointer had to be expanded (`archive_decisions` \
rows, T5.4).\n"
    );
    if exp.decisions == 0 {
        let _ = writeln!(s, "No rows in window.");
    } else {
        let what = if exp.expanded_ids.is_empty() {
            "none".to_string()
        } else {
            exp.expanded_ids
                .iter()
                .map(|id| cell(id))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            s,
            "`rtok expand` froze {} of {} live-zone pointers ({:.1}%). Expanded: {}.",
            exp.expanded,
            exp.decisions,
            100.0 * exp.rate,
            what
        );
    }

    // ── Config ──────────────────────────────────────────────────────────────
    let _ = writeln!(
        s,
        "\n## Config\n\nEvery effective key with its origin — the `config show --sources` \
page.\n"
    );
    let _ = writeln!(s, "| key | value | source |\n| --- | --- | --- |");
    for e in &doc.config {
        let _ = writeln!(
            s,
            "| {} | {} | {} |",
            cell(&e.key),
            cell(&e.value),
            cell(&e.source)
        );
    }
    let _ = writeln!(s, "\n{} keys.", doc.config.len());

    // ── Doctor ──────────────────────────────────────────────────────────────
    let _ = writeln!(
        s,
        "\n## Doctor\n\nLive probes (the `rtok doctor` page), not store rows.\n\n```text\n{}```\n",
        doc.doctor.to_text()
    );

    // ── Recommendations ─────────────────────────────────────────────────────
    let _ = writeln!(s, "## Recommendations\n");
    if doc.recommendations.is_empty() {
        let _ = writeln!(s, "No recommendations.");
    } else {
        for r in &doc.recommendations {
            let _ = writeln!(
                s,
                "- **{}**: {} ({})",
                cell(&r.rule),
                cell(&r.finding),
                cell(&r.evidence)
            );
        }
    }
    s
}

/// `4.0` for a recorded latency, `—` for none — an untimed surface is not a zero-ms one.
fn ms(v: Option<f64>) -> String {
    v.map(|ms| format!("{ms:.1}")).unwrap_or_else(|| "—".into())
}

/// A table cell: `|` would end the cell early, so it is escaped.
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}
