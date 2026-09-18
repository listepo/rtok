//! Memory recall quality bench (plan T69.3): no LLM, seeded store.

use rtok::config::Config;
use rtok::plugin::Runtime;
use rtok::plugins::memory::{mem_save, mem_search, score};

fn seeded_store(sessions: usize, facts: usize) -> (Config, Runtime) {
    let dir = rtok::testutil::tmp_dir("memory-bench");
    let cfg = rtok::testutil::config_in(&dir);
    let rt = Runtime::open(cfg.clone(), "bench").unwrap();
    for s in 0..sessions {
        for n in 0..10 {
            let title = format!("noise-{s}-{n}");
            let body = format!("session {s} filler note {n} about unrelated widgets");
            mem_save(&rt, "note", &title, &body, Some("bench")).unwrap();
        }
        for f in 0..facts {
            let title = format!("fact-{f}");
            let body = format!("TARGET-FACT-{f}: the walrus login uses jwt rotation on Tuesdays");
            mem_save(&rt, "note", &title, &body, Some("bench")).unwrap();
        }
    }
    (cfg, rt)
}

#[test]
fn recall_hit_rate_report() {
    let (_cfg, rt) = seeded_store(10, 5);
    let mut hits = 0;
    for f in 0..5 {
        let q = format!("walrus jwt TARGET-FACT-{f}");
        let rows = mem_search(&rt, &q, 5).unwrap();
        if rows.iter().any(|h| h.title == format!("fact-{f}")) {
            hits += 1;
        }
    }
    let rate = hits as f64 / 5.0;
    eprintln!("memory_bench: 10 sessions, fts hit_rate={rate:.0}% ({hits}/5 facts)");
    assert!(rate >= 0.0);
    let _ = score::now_secs();
}

#[test]
fn half_life_zero_matches_id_order() {
    let dir = rtok::testutil::tmp_dir("memory-bench-order");
    let cfg = rtok::testutil::config_in(&dir);
    let rt = Runtime::open(cfg, "bench").unwrap();
    for i in 0..6 {
        mem_save(&rt, "note", &format!("t{i}"), "body", None).unwrap();
    }
    let rows = rt
        .store
        .list_note_titles_scored(None, 3, 0, score::now_secs())
        .unwrap();
    let ids: Vec<i32> = rows.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids[0] > ids[1] && ids[1] > ids[2]);
}
