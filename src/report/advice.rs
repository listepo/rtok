//! Recommendations (T22.5, D24): rules over the ledgers, never a model call. Each
//! finding prints what triggered it and the rows it read, ordered by the tokens it would
//! recover; a finding with no number attached does not ship. Input is the D23 model
//! only — like every other `src/report/` reader, this module runs no query.

use super::Recommendation;
use crate::config::Config;
use crate::web::model::ReportLedgers;

/// "Fires often": in-window `calls` rows at or above this with no `Measurement` kind on
/// the event's path get a finding.
pub const OFTEN_HOOK_CALLS: u64 = 10;

/// The `Measurement` kinds an event can produce. `PreToolUse` denies through `guard`;
/// injections record `inject` via `apply`. `PostToolUse` does side-effects that are not
/// Measurement kinds (guard cache, read invalidation) — busy, not idle.
fn kinds_for_hook(event: &str) -> &'static [&'static str] {
    match event {
        "PreToolUse" => &["guard"],
        "SessionStart" | "UserPromptSubmit" | "PostCompact" => &["inject"],
        _ => &[],
    }
}

/// Hooks that are idle by design: they should not fire often and never record a
/// `Measurement`. `PostToolUse` is deliberately excluded — a busy healthy session
/// fires it constantly without a Measurement kind on that path (T22.6).
fn idle_by_design(event: &str) -> bool {
    matches!(event, "PreCompact" | "Stop" | "SessionEnd")
}

/// "1 row" / "N rows" — evidence strings name counts, so they decline them.
fn count(n: u64, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

/// Finding sink: recoverable tokens plus the rule triple.
type Push<'a> = &'a mut dyn FnMut(i64, &str, String, String);

/// The six T22.5 rules over one ledger read, most recoverable tokens first.
pub fn recommendations(ledgers: &ReportLedgers, cfg: &Config) -> Vec<Recommendation> {
    // Findings carry their recoverable tokens as the sort key, so a finding with no
    // number cannot be pushed.
    let mut out: Vec<(i64, Recommendation)> = Vec::new();
    let mut push = |tokens: i64, rule: &str, finding: String, evidence: String| {
        let r = Recommendation {
            rule: rule.into(),
            finding,
            evidence,
        };
        out.push((tokens, r));
    };
    expand_rate(ledgers, cfg, &mut push);
    retire_plugin(ledgers, &mut push);
    cache_busts(ledgers, &mut push);
    idle_hooks(ledgers, &mut push);
    inject_budget(ledgers, cfg, &mut push);
    archive_window(ledgers, cfg, &mut push);
    // Stable: ties keep the rule order above.
    out.sort_by_key(|&(tokens, _)| std::cmp::Reverse(tokens));
    out.into_iter().map(|(_, r)| r).collect()
}

/// (1) Expand rate above `[expand] max_rate`: the compression is lossier than it looks.
fn expand_rate(ledgers: &ReportLedgers, cfg: &Config, push: Push<'_>) {
    let e = &ledgers.expand;
    let max_rate = cfg.expand.max_rate;
    if e.decisions > 0 && e.rate > max_rate {
        push(
            e.cost,
            "expand-rate",
            format!(
                "expand rate {:.1}% ({} of {} live-zone pointers re-read) is above [expand] max_rate {:.0}% — re-expansions cost {} est tokens over {}",
                100.0 * e.rate,
                e.expanded,
                e.decisions,
                100.0 * max_rate,
                e.cost,
                count(e.cost_rows, "row")
            ),
            format!(
                "{}, {} of kind expand",
                count(e.decisions as u64, "archive_decisions row"),
                count(e.cost_rows, "archive Measurement row")
            ),
        );
    }
}

/// (2) Net saving ≤ 0 costs more than it returns (D10: retire, not stack); all-zero
/// rows are bookkeeping (idle `inject`, `guard` denies), not evidence.
fn retire_plugin(ledgers: &ReportLedgers, push: Push<'_>) {
    for r in &ledgers.savings.rows {
        if r.saved <= 0 && (r.est_before != 0 || r.est_after != 0) {
            push(
                -r.saved,
                "retire-plugin",
                format!(
                    "plugin {} net {} est tokens over {} (≤ 0) — D10 says retire, not stack",
                    r.plugin,
                    r.saved,
                    count(r.rows, "Measurement row")
                ),
                count(r.rows, &format!("{} Measurement row", r.plugin)),
            );
        }
    }
}

/// (3) Cache busts the host caused: a rewritten tool list or system prompt, naming turn.
fn cache_busts(ledgers: &ReportLedgers, push: Push<'_>) {
    let c = &ledgers.cache;
    for b in &c.detail {
        if b.cause == "tools" || b.cause == "system" {
            let what = ["tool list", "system prompt"][(b.cause == "system") as usize];
            push(
                b.cache_create,
                "cache-bust",
                format!(
                    "session {} turn {} busted the prompt cache (cause {}): {} cache-create tokens re-written with cache_read {} — pin the {what}; {} tokens would have stayed cached",
                    b.session, b.turn, b.cause, b.cache_create, b.cache_read, b.cache_create
                ),
                format!(
                    "{} over {} in {} with usage rows",
                    count(c.busts, "cache bust"),
                    count(c.turns, "turn"),
                    count(c.sessions, "session")
                ),
            );
        }
    }
}

/// (4) Idle-by-design hooks that fire often: latency on the 10 ms path for nothing.
fn idle_hooks(ledgers: &ReportLedgers, push: Push<'_>) {
    let kinds = &ledgers.savings.kinds;
    for h in &ledgers.calls.hooks {
        if !idle_by_design(&h.name) {
            continue;
        }
        let worked = kinds_for_hook(&h.name)
            .iter()
            .any(|want| kinds.iter().any(|have| have.as_str() == *want));
        if h.calls >= OFTEN_HOOK_CALLS && !worked {
            push(
                0,
                "idle-hook",
                format!(
                    "{} fired {} times in window with no Measurement kind recorded on its path — weight on the 10 ms hook path for nothing; 0 tokens saved",
                    h.name, h.calls
                ),
                format!(
                    "{} in window, {} in ledger",
                    count(h.calls, &format!("hook `{}` row", h.name)),
                    count(ledgers.window.measurements, "Measurement row")
                ),
            );
        }
    }
}

/// (5) Measured injection per turn against the budget.
fn inject_budget(ledgers: &ReportLedgers, cfg: &Config, push: Push<'_>) {
    let budget = i64::from(cfg.plugins.inject.budget_tokens);
    if let Some(r) = ledgers.savings.rows.iter().find(|r| r.plugin == "inject")
        && r.rows > 0
        && r.est_after > budget * r.rows as i64
    {
        let excess = r.est_after - budget * r.rows as i64;
        push(
            excess,
            "inject-budget",
            format!(
                "inject averages {} est tokens/turn over {} vs [plugins.inject] budget_tokens {} — {} tokens over budget; trim modes and memory recall",
                r.est_after / r.rows as i64,
                count(r.rows, "inject Measurement row"),
                budget,
                excess
            ),
            count(r.rows, "inject Measurement row"),
        );
    }
}

/// (6) `keep_turns` against observed re-reads: expanded pointers were archived young.
fn archive_window(ledgers: &ReportLedgers, cfg: &Config, push: Push<'_>) {
    let e = &ledgers.expand;
    if e.expanded > 0 {
        push(
            e.cost,
            "archive-window",
            format!(
                "archive keep_turns {} with {} of {} pointers re-read via rtok expand ({} est tokens over {}) — old results ARE re-read; raise keep_turns or min_tokens so volatile results stay live",
                cfg.plugins.archive.keep_turns,
                e.expanded,
                e.decisions,
                e.cost,
                count(e.cost_rows, "row")
            ),
            format!(
                "{}, {}",
                count(e.decisions as u64, "archive_decisions row"),
                count(e.cost_rows, "expand Measurement row")
            ),
        );
    }
}
