//! T22.1: `rtok report --format md` renders the D23 model (D24). This file pins the
//! document against a fixture store the test builds row by row, so every number in the
//! output is traceable to a row it also knows: 7 calls (2 hook, 1 mcp timed, 4 proxy
//! untimed), 3 measurements (cmd filter, archive pointer, archive expand), 3 usage rows
//! (one tools-cause cache bust), 3 archive decisions (1 expanded). The empty-store case
//! asserts the report says so rather than printing zeros.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t221-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn rtok(args: &[&str], home: &Path) -> String {
    let out = Command::new(bin())
        .args(args)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rtok {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

const BODY_A: &str = r#"{"system":"s","tools":[{"name":"A"}],"messages":[]}"#;
const BODY_MORE_TOOLS: &str = r#"{"system":"s","tools":[{"name":"A"},{"name":"B"}],"messages":[]}"#;

/// The store every asserted number is summed from. Returns the archive id the expand
/// rows name, so the test asserts the id the rows actually carry.
fn seed(home: &Path) -> String {
    // Doctor reads `~/.claude/settings.json`: two hook entries → "hooks 2".
    let claude = home.join(".claude");
    fs::create_dir_all(&claude).unwrap();
    fs::write(
        claude.join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"rtok hook PreToolUse"}]}],
             "PostToolUse":[{"hooks":[{"type":"command","command":"rtok hook PostToolUse"}]}]}}"#,
    )
    .unwrap();

    let cfg = rtok::config::Config::load_from(home).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    store
        .upsert_session("s1", None, None, None, Some("proxy"))
        .unwrap();

    // 2 hook calls (timed 4.0/2.0 → p50 2.0, p95 4.0), 1 mcp call (10.0), 1 untimed proxy.
    for (surface, kind, ms) in [
        ("hook", "hook", 4.0),
        ("hook", "hook", 2.0),
        ("mcp", "mcp_call", 10.0),
    ] {
        let id = store
            .insert_call("s1", surface, kind, None, None, None, None, None)
            .unwrap();
        store.set_call_ms(id, ms).unwrap();
    }
    store
        .insert_call("s1", "proxy", "api_request", None, None, None, None, None)
        .unwrap();

    // 3 more proxy calls whose usage rows make one tools-cause cache bust (the
    // `stats --cache` pattern: read drop + cache_create > 20k, tools array changed).
    for (body, create, read) in [
        (BODY_A, 0, 0),
        (BODY_A, 500, 30_000),
        (BODY_MORE_TOOLS, 31_000, 200),
    ] {
        let id = store
            .insert_call("s1", "proxy", "api_request", None, None, None, None, None)
            .unwrap();
        store
            .insert_call_io(id, Some(body.as_bytes()), None, 1 << 20, None)
            .unwrap();
        store
            .insert_usage("s1", Some("m"), "anthropic", 100, create, read, 5, id)
            .unwrap();
    }

    // 3 live-zone pointers over real archived payloads, one frozen by `rtok expand` —
    // the 33.3% expand rate. The archive row must exist first: the decision row
    // references it.
    let mut first_archive_id = String::new();
    for (tool, body) in [
        ("t1", &b"first payload\n"[..]),
        ("t2", &b"second"[..]),
        ("t3", &b"third"[..]),
    ] {
        let archive_id = store
            .put_archive("s1", body, &cfg.core.archive_dir)
            .unwrap();
        if tool == "t1" {
            first_archive_id = archive_id.clone();
        }
        store
            .put_archive_decision(tool, &archive_id, "s1", "head…")
            .unwrap();
    }
    store.mark_expanded(&first_archive_id).unwrap();
    // cmd: 25→10. archive: pointer 100→20 and expand 0→5 (retrieval costs tokens);
    // the expand row names the frozen id.
    for (plugin, kind, est_before, est_after, ref_id) in [
        ("cmd", "filter", 25, 10, None),
        (
            "archive",
            "pointer",
            100,
            20,
            Some(first_archive_id.clone()),
        ),
        ("archive", "expand", 0, 5, Some(first_archive_id.clone())),
    ] {
        store
            .insert_measurement(
                "s1",
                &rtok::Measurement {
                    plugin,
                    kind,
                    before_bytes: 100,
                    after_bytes: 40,
                    est_before,
                    est_after,
                    ref_id,
                    call_id: None,
                },
            )
            .unwrap();
    }
    first_archive_id
}

/// The header's dates come from the clock, so the test accepts the date it computed
/// before the run or the one after (they differ only across UTC midnight).
fn header_dates(out: &str) -> bool {
    let d = |secs: u64| rtok::log::stamp(secs)[..10].to_string();
    [rtok::log::now(), rtok::log::now()]
        .into_iter()
        .any(|to| out.contains(&format!("→ {} (30d)", d(to))))
        && out.contains(" · store ")
}

/// T22.1 Check: every number the document prints equals what the seeded rows sum to.
#[test]
fn fixture_numbers_are_traceable_to_rows() {
    let h = home("fixture");
    let first_id = seed(&h);
    let out = rtok(&["report"], &h);
    println!("{out}");

    assert!(out.starts_with("# rtok report\n\nWindow "), "header");
    assert!(header_dates(&out), "window dates + store path: {out}");

    // Window: 7 calls (all in the 30d window), 3 measurements, 3 usage rows.
    assert!(out.contains("| calls | 7 of 7 | window (30d) |"));
    assert!(out.contains("| measurements | 3 | whole ledger (no row times) |"));
    assert!(out.contains("| usage | 3 | whole ledger (no row times) |"));

    // Savings: cmd 25→10 saves 15; archive 100→25 saves 75 (pointer 80 − expand 5);
    // total 90 over 3 rows.
    assert!(out.contains("| cmd | 1 | 25 | 10 | 15 |"));
    assert!(out.contains("| archive | 2 | 100 | 25 | 75 |"));
    assert!(out.contains("Total: 90 est tokens over 3 `Measurement` rows"));

    // Calls: hook p50 2.0 / p95 4.0 over 2 timed; mcp 10.0/10.0 over 1; proxy 4 untimed.
    assert!(out.contains("| hook | 2 | 2 | 2.0 | 4.0 |"));
    assert!(out.contains("| mcp | 1 | 1 | 10.0 | 10.0 |"));
    assert!(out.contains("| proxy | 4 | 0 | — | — |"));
    assert!(out.contains("7 of 7 `calls` rows in window (30d)"));

    // Cache: one bust, cause tools, over 3 turns in 1 session.
    assert!(out.contains("| tools | 1 |"));
    assert!(out.contains("Busts: 1 over 3 turns in 1 session(s)."));

    // Expand: 1 of 3 decisions frozen, the id the measurement names.
    assert!(out.contains("`rtok expand` froze 1 of 3 live-zone pointers (33.3%)"));
    assert!(out.contains(&format!("Expanded: {first_id}.")));

    // Config: the report's own keys, with their origin.
    assert!(out.contains("| report.since | 30d | user |"));
    assert!(out.contains("| report.format | md | user |"));

    // Doctor: the fixture's two hooks. The section set's fixed order, headings included.
    assert!(out.contains("## Doctor"));
    assert!(out.contains("hooks 2"));
    let order: Vec<usize> = [
        "## Window",
        "## Savings",
        "## Calls",
        "## Cache",
        "## Expand",
        "## Config",
        "## Doctor",
        "## Recommendations",
    ]
    .iter()
    .map(|sec| {
        out.find(sec)
            .unwrap_or_else(|| panic!("missing section {sec}"))
    })
    .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "the section set is fixed and ordered");

    // Recommendations (T22.5): the fixture's 33.3% expand rate, one tools bust and one
    // re-read each fire exactly once, most recoverable tokens first; nothing else does
    // (cmd/archive net positive, no inject rows, 2 unnamed hook calls below the often
    // threshold, no non-positive plugin).
    for rule in ["cache-bust", "expand-rate", "archive-window"] {
        assert_eq!(
            out.matches(&format!("- **{rule}**:")).count(),
            1,
            "{rule} fires once: {out}"
        );
    }
    for rule in ["retire-plugin", "inject-budget", "idle-hook"] {
        assert!(
            !out.contains(&format!("- **{rule}**:")),
            "{rule} stays silent: {out}"
        );
    }
    assert!(out.contains("33.3%"), "rate with its row counts: {out}");
    assert!(
        out.contains("3 archive_decisions rows"),
        "row counts: {out}"
    );
    assert!(
        out.contains("session s1 turn 3"),
        "the bust names its turn: {out}"
    );
    let pos = |rule: &str| out.find(&format!("- **{rule}**:")).unwrap();
    assert!(
        pos("cache-bust") < pos("expand-rate") && pos("expand-rate") < pos("archive-window"),
        "ordered by recoverable tokens: {out}"
    );
    assert!(
        !out.contains("No recommendations."),
        "the fixture has findings to report"
    );

    let _ = fs::remove_dir_all(&h);
}

/// The empty-store Check: the report says so rather than printing zeros.
#[test]
fn empty_store_says_so_rather_than_zeros() {
    let h = home("empty");
    let out = rtok(&["report"], &h);
    println!("{out}");

    assert!(out.contains("## Recommendations"));
    assert!(out.contains("**No rows in window.** The store has no rows to report."));
    assert!(out.contains("No rows in window."));
    assert!(out.contains("No recommendations."));
    assert!(out.contains("| calls | 0 of 0 | window (30d) |"));
    assert!(
        !out.contains("| cmd |"),
        "no savings table on an empty store"
    );
    assert!(
        !out.contains("| hook |"),
        "no calls table on an empty store"
    );
    // Config and Doctor still render: they do not depend on store rows.
    assert!(out.contains("| report.format | md | user |"));
    assert!(out.contains("hooks 0"));

    let _ = fs::remove_dir_all(&h);
}

/// `--out` writes the document instead of printing it, and says where (D12: `report.out`).
#[test]
fn out_flag_writes_the_file() {
    let h = home("out");
    seed(&h);
    let path = h.join("report.md");
    let stdout = rtok(&["report", "--out", path.to_str().unwrap()], &h);
    assert_eq!(stdout.trim(), path.display().to_string());
    let written = fs::read_to_string(&path).unwrap();
    assert!(written.starts_with("# rtok report"));
    assert!(written.contains("| cmd | 1 | 25 | 10 | 15 |"));
    let _ = fs::remove_dir_all(&h);
}

/// Maximal digit-led runs (`30d` → `30`, `33.3%` → `33.3`): the Check's "every number".
fn numbers(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in md.chars() {
        if ch.is_ascii_digit() || (ch == '.' && !cur.is_empty()) {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(cur.trim_end_matches('.').to_string());
            cur = String::new();
        }
    }
    if !cur.is_empty() {
        out.push(cur.trim_end_matches('.').to_string());
    }
    out.retain(|n| !n.is_empty());
    out
}

/// T22.2 Check: `--format html` holds every number `--format md` holds (walk both over
/// the same fixture store), the eight sections in the same order, a chart per model
/// series — and no `http` outside code blocks, so the file opens from `file://`
/// fetching nothing.
#[test]
fn html_holds_every_markdown_number_and_fetches_nothing() {
    let h = home("html");
    let first_id = seed(&h);
    let md = rtok(&["report"], &h);
    let html = rtok(&["report", "--format", "html"], &h);
    for n in numbers(&md) {
        assert!(html.contains(&n), "markdown number {n} missing from html");
    }
    assert!(html.contains(&first_id), "expanded id missing from html");
    let order: Vec<usize> = [
        "Window",
        "Savings",
        "Calls",
        "Cache",
        "Expand",
        "Config",
        "Doctor",
        "Recommendations",
    ]
    .iter()
    .map(|sec| {
        html.find(&format!(">{sec}</h2>"))
            .unwrap_or_else(|| panic!("missing section {sec}"))
    })
    .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "the section set is fixed and ordered");
    assert_eq!(
        html.matches("<svg").count(),
        3,
        "one chart per series: savings, calls, cache"
    );
    let mut bare = html.clone();
    while let Some(a) = bare.find("<code>") {
        let end = bare[a..]
            .find("</code>")
            .map(|i| a + i + 7)
            .unwrap_or(bare.len());
        bare.replace_range(a..end, "");
    }
    assert!(
        !bare.contains("http"),
        "external reference outside code: {}",
        &bare[..bare.len().min(200)]
    );
    let _ = fs::remove_dir_all(&h);
}

/// T22.5/T22.6 Check, first clause: one store in which every rule fires exactly once —
/// expand 1-of-3 (33.3% > 5%), a negative-net plugin, one tools bust, 12 PreCompact
/// calls (idle-by-design; PostToolUse is excluded — T22.6), inject over budget, one re-read.
fn seed_advice(home: &Path) {
    let cfg = rtok::config::Config::load_from(home).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    store
        .upsert_session("adv", None, None, None, Some("proxy"))
        .unwrap();

    // Hooks: 12 PreCompact (idle-by-design, often), 2 PreToolUse and 1 SessionStart
    // below the threshold; also 12 PostToolUse so a busy productive path stays quiet.
    for (event, n) in [
        ("PreCompact", 12),
        ("PostToolUse", 12),
        ("PreToolUse", 2),
        ("SessionStart", 1),
    ] {
        for _ in 0..n {
            store
                .insert_call("adv", "hook", "hook", None, None, None, None, Some(event))
                .unwrap();
        }
    }

    // Usage: the `stats --cache` bust pattern — one tools-cause bust at turn 3.
    for (body, create, read) in [
        (BODY_A, 0, 0),
        (BODY_A, 500, 30_000),
        (BODY_MORE_TOOLS, 31_000, 200),
    ] {
        let id = store
            .insert_call("adv", "proxy", "api_request", None, None, None, None, None)
            .unwrap();
        store
            .insert_call_io(id, Some(body.as_bytes()), None, 1 << 20, None)
            .unwrap();
        store
            .insert_usage("adv", Some("m"), "anthropic", 100, create, read, 5, id)
            .unwrap();
    }

    // Archive: 3 pointers, one frozen — the 33.3% rate and the one re-read.
    let mut first_archive_id = String::new();
    for (i, body) in [b"first payload\n".as_slice(), b"second", b"third"]
        .into_iter()
        .enumerate()
    {
        let archive_id = store
            .put_archive("adv", body, &cfg.core.archive_dir)
            .unwrap();
        if i == 0 {
            first_archive_id = archive_id.clone();
        }
        store
            .put_archive_decision(&format!("u{i}"), &archive_id, "adv", "head…")
            .unwrap();
    }
    store.mark_expanded(&first_archive_id).unwrap();

    // Measurements: cmd and archive net positive, inject 1500/turn over the 800 budget,
    // toon net −15 (the one plugin to retire); the expand row names the frozen id.
    for (plugin, kind, est_before, est_after, ref_id) in [
        ("cmd", "filter", 25, 10, None),
        (
            "archive",
            "pointer",
            100,
            20,
            Some(first_archive_id.clone()),
        ),
        ("archive", "expand", 0, 5, Some(first_archive_id.clone())),
        ("inject", "inject", 2000, 1500, None),
        ("toon", "table", 10, 25, None),
    ] {
        store
            .insert_measurement(
                "adv",
                &rtok::Measurement {
                    plugin,
                    kind,
                    before_bytes: 100,
                    after_bytes: 40,
                    est_before,
                    est_after,
                    ref_id,
                    call_id: None,
                },
            )
            .unwrap();
    }
}

/// T22.5 Check, first clause: each rule fires exactly once, ordered by recoverable
/// tokens (31000 bust, 700 inject excess, 15 toon loss, 5 + 5 expand, 0 idle hook),
/// and every finding names the row count behind it.
#[test]
fn each_rule_fires_once_in_recoverable_order() {
    let h = home("advice");
    seed_advice(&h);
    let out = rtok(&["report"], &h);
    println!("{out}");

    for rule in [
        "expand-rate",
        "retire-plugin",
        "cache-bust",
        "idle-hook",
        "inject-budget",
        "archive-window",
    ] {
        assert_eq!(
            out.matches(&format!("- **{rule}**:")).count(),
            1,
            "{rule} fires exactly once: {out}"
        );
    }
    // Every finding names the row count behind it.
    assert!(
        out.contains("3 archive_decisions rows"),
        "expand rows: {out}"
    );
    assert!(out.contains("1 toon Measurement row"), "retire rows: {out}");
    assert!(
        out.contains("session adv turn 3"),
        "the bust names its turn: {out}"
    );
    assert!(
        out.contains("12 hook `PreCompact` rows in window"),
        "hook rows: {out}"
    );
    // PostToolUse appears in Calls, but must not be the idle-hook subject.
    let idle = out
        .split("- **idle-hook**:")
        .nth(1)
        .and_then(|s| {
            s.split(
                "
- **",
            )
            .next()
        })
        .unwrap_or("");
    assert!(
        !idle.contains("PostToolUse"),
        "busy PostToolUse must not trigger idle-hook: {idle}"
    );
    assert!(
        out.contains("1 inject Measurement row"),
        "inject rows: {out}"
    );
    assert!(out.contains("budget_tokens 800"), "the budget: {out}");
    assert!(out.contains("keep_turns 4"), "the window setting: {out}");
    // Ordered by the tokens each would recover.
    let pos = |rule: &str| out.find(&format!("- **{rule}**:")).unwrap();
    let order = [
        "cache-bust",
        "inject-budget",
        "retire-plugin",
        "expand-rate",
        "archive-window",
        "idle-hook",
    ];
    let positions: Vec<usize> = order.iter().map(|r| pos(r)).collect();
    let mut sorted = positions.clone();
    sorted.sort_unstable();
    assert_eq!(positions, sorted, "most recoverable tokens first: {out}");

    let _ = fs::remove_dir_all(&h);
}

/// T22.5 Check, second clause: a healthy store — positive savings, no busts, no
/// re-reads, injections under budget, quiet hooks — gets an empty section that says
/// so, not filler advice.
#[test]
fn healthy_store_has_no_recommendations() {
    let h = home("healthy");
    let cfg = rtok::config::Config::load_from(&h).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    store
        .upsert_session("ok", None, None, None, Some("proxy"))
        .unwrap();
    for _ in 0..2 {
        store
            .insert_call(
                "ok",
                "hook",
                "hook",
                None,
                None,
                None,
                None,
                Some("PreToolUse"),
            )
            .unwrap();
    }
    // Busy healthy PostToolUse must not false-trigger idle-hook (T22.6).
    for _ in 0..12 {
        store
            .insert_call(
                "ok",
                "hook",
                "hook",
                None,
                None,
                None,
                None,
                Some("PostToolUse"),
            )
            .unwrap();
    }
    let id = store
        .insert_call("ok", "proxy", "api_request", None, None, None, None, None)
        .unwrap();
    store
        .insert_call_io(id, Some(BODY_A.as_bytes()), None, 1 << 20, None)
        .unwrap();
    store
        .insert_usage("ok", Some("m"), "anthropic", 100, 0, 0, 5, id)
        .unwrap();
    for (plugin, kind, est_before, est_after) in [
        ("cmd", "filter", 25, 10),
        ("archive", "pointer", 100, 20),
        ("inject", "inject", 500, 400),
    ] {
        store
            .insert_measurement(
                "ok",
                &rtok::Measurement {
                    plugin,
                    kind,
                    before_bytes: 100,
                    after_bytes: 40,
                    est_before,
                    est_after,
                    ref_id: None,
                    call_id: None,
                },
            )
            .unwrap();
    }
    let out = rtok(&["report"], &h);
    println!("{out}");

    assert!(out.contains("## Recommendations"));
    assert!(out.contains("No recommendations."));
    assert!(
        !out.contains("- **"),
        "no findings on a healthy store: {out}"
    );

    let _ = fs::remove_dir_all(&h);
}
