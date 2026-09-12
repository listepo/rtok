//! Gate P29 — optional embed search beside FTS5 (T29.2).

use rtok::config::Config;
use rtok::plugin::{Measurement, Runtime};
use rtok::plugins::memory::{mem_save, mem_search};

struct Fixture {
    notes: Vec<(String, String, String, Option<String>)>,
    fts: String,
    embed: String,
    hybrid: String,
    embed_off: String,
    planted: String,
}

fn fixture() -> Fixture {
    let raw = include_str!("fixtures/p29_memory.toml");
    let doc: toml_edit::DocumentMut = raw.parse().expect("fixture parses");
    let planted = doc["expect"]["planted_title"]
        .as_str()
        .unwrap()
        .to_string();
    let notes = doc["note"]
        .as_array_of_tables()
        .expect("[[note]]")
        .iter()
        .map(|t| {
            (
                t["kind"].as_str().unwrap().to_string(),
                t["title"].as_str().unwrap().to_string(),
                t["body"].as_str().unwrap().to_string(),
                t.get("project")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            )
        })
        .collect();
    let q = doc["query"].as_table().expect("[query]");
    Fixture {
        notes,
        fts: q["fts"].as_str().unwrap().into(),
        embed: q["embed"].as_str().unwrap().into(),
        hybrid: q["hybrid"].as_str().unwrap().into(),
        embed_off: q["embed_off"].as_str().unwrap().into(),
        planted,
    }
}

fn open(tag: &str, embed_enabled: bool, hybrid: bool) -> (Runtime, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rtok-p29-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::default();
    cfg.core.db_path = dir.join("rtok.db");
    cfg.core.archive_dir = dir.join("archive");
    cfg.plugins.memory.embed.enabled = embed_enabled;
    cfg.plugins.memory.embed.hybrid = hybrid;
    (Runtime::open(cfg, tag).unwrap(), dir)
}

fn seed(rt: &Runtime, fix: &Fixture) {
    for (kind, title, body, project) in &fix.notes {
        mem_save(rt, kind, title, body, project.as_deref()).unwrap();
    }
}

fn top_title(rt: &Runtime, query: &str) -> Option<String> {
    mem_search(rt, query, 5)
        .ok()?
        .into_iter()
        .next()
        .map(|h| h.title)
}

#[test]
fn gate_p29_fts5_default_and_embed_paths() {
    let fix = fixture();

    let (rt, dir) = open("p29-off", false, true);
    seed(&rt, &fix);
    assert_eq!(
        top_title(&rt, &fix.fts).as_deref(),
        Some(fix.planted.as_str()),
        "FTS5 with embed off"
    );
    assert!(
        top_title(&rt, &fix.embed_off).is_none(),
        "embed off: FTS must not rank the planted note on the embed query"
    );
    drop(rt);
    let _ = std::fs::remove_dir_all(&dir);

    let (rt, dir) = open("p29-embed", true, false);
    seed(&rt, &fix);
    assert_eq!(
        top_title(&rt, &fix.embed).as_deref(),
        Some(fix.planted.as_str()),
        "vector leg alone"
    );
    drop(rt);
    let _ = std::fs::remove_dir_all(&dir);

    let (rt, dir) = open("p29-hybrid", true, true);
    seed(&rt, &fix);
    assert_eq!(
        top_title(&rt, &fix.hybrid).as_deref(),
        Some(fix.planted.as_str()),
        "hybrid RRF"
    );
    let hit = top_title(&rt, &fix.hybrid).is_some();
    rt.record(&Measurement {
        plugin: "memory",
        kind: "p29_hybrid_recall",
        before_bytes: fix.hybrid.len() as u64,
        after_bytes: u64::from(hit as u8),
        est_before: 0,
        est_after: 0,
        ref_id: None,
        call_id: None,
    })
    .unwrap();
    assert_eq!(rt.store.measurement_count("memory").unwrap(), 1);
    drop(rt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flag_off_search_matches_fts5_only() {
    let fix = fixture();
    let (mut rt, dir) = open("p29-bytes", false, true);
    seed(&rt, &fix);
    let off = mem_search(&rt, &fix.fts, 3).unwrap();
    rt.config.plugins.memory.embed.enabled = true;
    rt.config.plugins.memory.embed.hybrid = false;
    let on_fts = rt.store.search_notes(&fix.fts, 3).unwrap();
    assert_eq!(off.len(), on_fts.len());
    for (a, b) in off.iter().zip(&on_fts) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.title, b.title);
        assert_eq!(a.snippet, b.snippet);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
