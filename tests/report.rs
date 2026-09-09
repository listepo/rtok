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

    // Recommendations is the T22.5 placeholder: the heading renders, the list says so.
    assert!(out.contains("No recommendations."));

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
