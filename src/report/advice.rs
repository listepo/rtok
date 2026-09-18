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
/// fires it constantly without a Measurement kind on that path (T22.6). So is `SessionEnd`:
/// `agent setup claude` installs it to close the session row, once per session.
fn idle_by_design(event: &str) -> bool {
    matches!(event, "PreCompact" | "Stop")
}

/// "1 row" / "N rows" — evidence strings name counts, so they decline them.
fn count(n: u64, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

/// Finding sink: recoverable tokens plus the rule triple.
type Push<'a> = &'a mut dyn FnMut(i64, &str, String, String);

/// The T22.5 rules over one ledger read, most recoverable tokens first.
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
    cache_busts(&ledgers.cache, &mut push);
    idle_hooks(ledgers, &mut push);
    inject_budget(ledgers, cfg, &mut push);
    archive_window(ledgers, cfg, &mut push);
    top_sinks(ledgers, &mut push);
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

/// Turns a grouped cache-bust finding names before it says "and K more".
const BUST_TURNS_NAMED: usize = 5;

/// (3) Cache busts the host caused: a rewritten tool list or system prompt, naming turn.
/// One finding per (session, cause): a session that busts on every turn used to produce
/// one finding per turn, and `--ai` dropped the whole section over budget.
fn cache_busts(c: &crate::web::model::ReportCache, push: Push<'_>) {
    let mut groups: Vec<(&str, &str, Vec<&crate::web::model::ReportBust>)> = Vec::new();
    for b in &c.detail {
        if b.cause != "tools" && b.cause != "system" {
            continue;
        }
        match groups
            .iter_mut()
            .find(|(s, k, _)| *s == b.session && *k == b.cause)
        {
            Some((_, _, busts)) => busts.push(b),
            None => groups.push((&b.session, &b.cause, vec![b])),
        }
    }
    for (session, cause, busts) in groups {
        let what = ["tool list", "system prompt"][(cause == "system") as usize];
        let create: i64 = busts.iter().map(|b| b.cache_create).sum();
        let finding = match busts.as_slice() {
            [b] => format!(
                "session {session} turn {} busted the prompt cache (cause {cause}): {create} cache-create tokens re-written with cache_read {} — pin the {what}; {create} tokens would have stayed cached",
                b.turn, b.cache_read
            ),
            _ => {
                let mut turns = busts
                    .iter()
                    .take(BUST_TURNS_NAMED)
                    .map(|b| b.turn.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                if busts.len() > BUST_TURNS_NAMED {
                    turns.push_str(&format!(" and {} more", busts.len() - BUST_TURNS_NAMED));
                }
                format!(
                    "session {session} busted the prompt cache {} (cause {cause}, turns {turns}): {create} cache-create tokens re-written — pin the {what}; {create} tokens would have stayed cached",
                    count(busts.len() as u64, "time")
                )
            }
        };
        push(
            create,
            "cache-bust",
            finding,
            format!(
                "{} over {} in {} with usage rows",
                count(c.busts, "cache bust"),
                count(c.turns, "turn"),
                count(c.sessions, "session")
            ),
        );
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


/// (7) Top token sinks: which paths, stems or MCP tools cost the most bytes.
fn top_sinks(ledgers: &ReportLedgers, push: Push<'_>) {
    for s in &ledgers.sinks.rows {
        if s.before_bytes < 1 {
            continue;
        }
        push(
            s.before_bytes,
            "top-sinks",
            format!(
                "{} {} cost {} bytes over {} — {}",
                s.class,
                s.sink,
                s.before_bytes,
                count(s.rows, "Measurement row"),
                s.switch
            ),
            format!(
                "{} {} Measurement rows, {} total in ledger",
                count(s.rows, &format!("{} row", s.sink)),
                s.class,
                count(ledgers.window.measurements, "Measurement row")
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::web::model::{ReportBust, ReportCache, ReportLedgers, ReportSink, ReportSinksSection};

    #[test]
    fn top_sinks_ranks_largest_sink_first() {
        let ledgers = ReportLedgers {
            window: crate::web::model::ReportWindow {
                since: "30d".into(),
                from_unix: 0,
                to_unix: 1,
                from_date: "1970-01-01".into(),
                to_date: "1970-01-01".into(),
                db_path: "x".into(),
                calls_in_window: 0,
                calls_total: 0,
                measurements: 2,
                usage: 0,
            },
            savings: crate::web::model::ReportSavingsSection {
                rows: vec![],
                total_rows: 0,
                total_saved: 0,
                kinds: vec![],
            },
            sinks: ReportSinksSection {
                rows: vec![
                    ReportSink {
                        class: "cmd".into(),
                        sink: "grep".into(),
                        before_bytes: 200,
                        rows: 2,
                        switch: "[grep] rule".into(),
                    },
                    ReportSink {
                        class: "read".into(),
                        sink: "src/a.rs".into(),
                        before_bytes: 50,
                        rows: 1,
                        switch: "[plugins.read] default_mode = full".into(),
                    },
                ],
            },
            calls: crate::web::model::ReportCallsSection {
                rows: vec![],
                in_window: 0,
                total: 0,
                hooks: vec![],
            },
            cache: ReportCache {
                sessions: 0,
                turns: 0,
                busts: 0,
                by_cause: vec![],
                detail: vec![],
            },
            expand: crate::web::model::ReportExpand {
                decisions: 0,
                expanded: 0,
                rate: 0.0,
                expanded_ids: vec![],
                cost: 0,
                cost_rows: 0,
            },
        };
        let mut found = Vec::new();
        super::top_sinks(&ledgers, &mut |tokens, rule, finding, _| {
            found.push((tokens, rule.to_string(), finding));
        });
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, "top-sinks");
        assert!(found[0].2.contains("grep"), "{}", found[0].2);
        assert!(found[0].0 >= found[1].0);
    }

    /// A session busting on every turn is one finding with a count, not one per turn.
    #[test]
    fn cache_busts_group_per_session_and_cause() {
        let bust = |session: &str, turn: u64, cause: &str| ReportBust {
            session: session.into(),
            turn,
            cause: cause.into(),
            cache_create: 100,
            cache_read: 0,
        };
        let detail: Vec<ReportBust> = (1..=8)
            .map(|t| bust("a", t, "tools"))
            .chain([bust("a", 9, "system"), bust("b", 1, "tools")])
            .collect();
        let cache = ReportCache {
            sessions: 2,
            turns: 10,
            busts: 10,
            by_cause: vec![],
            detail,
        };
        let mut found = Vec::new();
        super::cache_busts(&cache, &mut |tokens, _, finding, _| {
            found.push((tokens, finding));
        });
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(found[0].0, 800);
        assert!(
            found[0].1.contains("8 times") && found[0].1.contains("1, 2, 3, 4, 5 and 3 more"),
            "{}",
            found[0].1
        );
        assert!(found[1].1.contains("session a turn 9"), "{}", found[1].1);
    }

    /// Once setup installed `SessionEnd`, a busy week would have told the user to remove it.
    #[test]
    fn the_session_end_rtok_installs_is_not_idle() {
        assert!(!super::idle_by_design("SessionEnd"));
        assert!(super::idle_by_design("Stop"));
    }
}
