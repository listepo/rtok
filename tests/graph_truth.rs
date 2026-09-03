//! T8.8: recall and precision of `symbol` / `callers` against hand-labelled ground truth.
//!
//! The labels in `fixtures/graph_truth.toml` come from a plain-text scan of this repo, not from
//! the tags index they score: a graph measured against its own edges is an upper bound, not an
//! accuracy number. `defs` is complete per symbol, so precision is real; `refs` is a
//! must-appear subset, so recall is a lower bound and the file survives ordinary edits.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rtok::config::Config;
use rtok::plugin::Ctx;
use rtok::plugins::graph::index;

struct Truth {
    name: String,
    defs: Vec<String>,
    refs: Vec<String>,
}

fn truth() -> Vec<Truth> {
    let raw = include_str!("fixtures/graph_truth.toml");
    let doc: toml_edit::DocumentMut = raw.parse().expect("fixture parses");
    let list = |t: &toml_edit::Table, k: &str| -> Vec<String> {
        t.get(k)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    doc["symbol"]
        .as_array_of_tables()
        .expect("[[symbol]] entries")
        .iter()
        .map(|t| Truth {
            name: t["name"].as_str().unwrap_or("").to_string(),
            defs: list(t, "defs"),
            refs: list(t, "refs"),
        })
        .collect()
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Definitions must clear the P8b bar of 0.9 and do, at 1.0. References do not: the Rust tags
/// query sees no type positions, nothing inside a macro body, and no path-qualified call, which
/// is 74 of the 144 labelled sites. The floor below is a regression guard on the measured 0.351,
/// not a target; `src/plugins/graph/PLAN.md` names the three constructs under "Known misses".
#[test]
fn labelled_symbols_are_found() {
    let dir = std::env::temp_dir().join(format!("rtok-truth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::default();
    cfg.core.db_path = dir.join("rtok.db");
    cfg.core.archive_dir = dir.join("archive");
    let cx = Ctx::open(cfg, "graph_truth").unwrap();
    let root = repo();
    index::run(&cx, &root).unwrap();
    let key = index::canon(&root);

    let (mut dwant, mut dgot) = (0usize, 0usize);
    let (mut rwant, mut rgot) = (0usize, 0usize);
    let (mut def_returned, mut def_right) = (0usize, 0usize);
    let mut misses: Vec<String> = Vec::new();
    for t in truth() {
        let defs: HashSet<String> = cx
            .store
            .symbol_defs(&key, &t.name)
            .unwrap()
            .into_iter()
            .map(|(p, _, _)| p)
            .collect();
        let refs: HashSet<String> = cx
            .store
            .symbol_refs(&key, &t.name)
            .unwrap()
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        def_returned += defs.len();
        def_right += defs.iter().filter(|p| t.defs.contains(p)).count();
        for d in &t.defs {
            dwant += 1;
            if defs.contains(d) {
                dgot += 1;
            } else {
                misses.push(format!("def {} {d}", t.name));
            }
        }
        for r in &t.refs {
            rwant += 1;
            if refs.contains(r) {
                rgot += 1;
            } else {
                misses.push(format!("ref {} {r}", t.name));
            }
        }
    }
    let def_recall = dgot as f64 / dwant.max(1) as f64;
    let ref_recall = rgot as f64 / rwant.max(1) as f64;
    let (got, want) = (dgot + rgot, dwant + rwant);
    let recall = got as f64 / want as f64;
    let precision = def_right as f64 / def_returned.max(1) as f64;
    println!(
        "graph truth: {got}/{want} sites, recall {recall:.3}\n  definitions {dgot}/{dwant} recall {def_recall:.3} precision {precision:.3}\n  references {rgot}/{rwant} recall {ref_recall:.3}"
    );
    for m in &misses {
        println!("  miss: {m}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        def_recall >= 0.9,
        "definition recall {def_recall:.3} below the P8b bar of 0.9; misses: {misses:?}"
    );
    assert!(
        precision >= 0.99,
        "definition precision {precision:.3}: a name resolved to a file it is not defined in"
    );
    assert!(
        ref_recall >= 0.30,
        "reference recall {ref_recall:.3} regressed below the measured 0.351"
    );
    assert!(
        Path::new(&root).join("src").is_dir(),
        "ground truth was scored against this repo"
    );
}
