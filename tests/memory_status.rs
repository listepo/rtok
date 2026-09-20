//! T69.4: `rtok memory status` and the Memory plugin page fields.

use rtok::config::Config;
use rtok::plugin::{Measurement, Runtime};
use rtok::web::model;

fn home(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-mem-status-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn memory_page_carries_store_counts() {
    let dir = home("page");
    let cfg = Config::load_from(&dir).expect("config");
    let cx = Runtime::open(cfg.clone(), "mem-page").unwrap();
    cx.store
        .upsert_note(Some("rtok"), "decision", "one", "alpha")
        .unwrap();
    cx.store
        .upsert_note(Some("rtok"), "note", "two", "beta")
        .unwrap();
    cx.store
        .upsert_note(Some("other"), "note", "three", "gamma")
        .unwrap();
    let mem = model::Model::new(&cfg, Some(&cx.store))
        .plugins()
        .into_iter()
        .find(|p| p.id == "memory")
        .unwrap();
    assert!(
        mem.fields
            .iter()
            .any(|(k, v)| k == "notes live" && v == "3"),
        "{:?}",
        mem.fields
    );
}

#[test]
fn memory_status_json_matches_model_type() {
    let dir = home("json");
    let cfg = Config::load_from(&dir).expect("config");
    let cx = Runtime::open(cfg.clone(), "mem-json").unwrap();
    cx.record(&Measurement {
        plugin: "memory",
        kind: "recall",
        before_bytes: 12,
        after_bytes: 4,
        est_before: 3,
        est_after: 1,
        ref_id: Some("sess".into()),
        call_id: None,
    })
    .unwrap();
    let status = model::memory_status(&cfg, None, Some("9999d")).unwrap();
    assert_eq!(status.recall.recalls, 1);
    let v: serde_json::Value = serde_json::to_value(&status).unwrap();
    assert_eq!(v["recall"]["recalls"], 1);
}
