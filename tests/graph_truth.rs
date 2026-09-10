//! T8.8: recall and precision of `symbol` / `callers` against hand-labelled ground truth.
//!
//! The labels in `fixtures/graph_truth.toml` come from a plain-text scan of this repo, not from
//! the tags index they score: a graph measured against its own edges is an upper bound, not an
//! accuracy number. `defs` is complete per symbol, so precision is real; `refs` is a
//! must-appear subset, so recall is a lower bound and the file survives ordinary edits.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rtok::config::Config;
use rtok::plugin::{Ctx, Runtime};
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

/// A fresh store in its own temp dir, so the tests here can run in parallel.
fn open(tag: &str) -> (Runtime, PathBuf) {
    let dir = std::env::temp_dir().join(format!("rtok-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::default();
    cfg.core.db_path = dir.join("rtok.db");
    cfg.core.archive_dir = dir.join("archive");
    (Runtime::open(cfg, tag).unwrap(), dir)
}

/// `name` as a whole identifier outside `//` lines, the rule the labels were scanned with:
/// `int_field` inside `int_field_accepts` or a comment does not count.
fn mentions(src: &str, name: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .any(|l| {
            l.match_indices(name).any(|(i, _)| {
                !l[..i].chars().next_back().is_some_and(ident)
                    && !l[i + name.len()..].chars().next().is_some_and(ident)
            })
        })
}

/// Every labelled file still exists and still names its symbol. A label the tree dropped
/// scores as an index miss below: T34.6 moved every `int_field` call into `wire.rs` and ref
/// recall fell 0.318 → 0.290 with the index unchanged. This names the label instead. It reads
/// the ~60 labelled files and parses nothing, so it costs milliseconds; the scoring test pays
/// for a cold index of the whole repo.
#[test]
fn every_label_names_a_file_that_mentions_the_symbol() {
    let mut stale = Vec::new();
    for t in truth() {
        for f in t.defs.iter().chain(&t.refs) {
            let src = std::fs::read_to_string(repo().join(f)).unwrap_or_default();
            if !mentions(&src, &t.name) {
                stale.push(format!("{} {f}", t.name));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "labels the tree no longer backs — repair tests/fixtures/graph_truth.toml and say why \
         in its header: {stale:?}"
    );
}

/// The constructs `src/plugins/graph/PLAN.md` lists under "Known misses", pinned on a one-file
/// repo: a change in what the tags query captures fails here by name instead of drifting a
/// repo-wide ratio. One parse, milliseconds. `scoped_callee` is caught by rtok's own
/// `RUST_SCOPED_CALL` pattern (`outline.rs`); the grammar's query alone would miss it.
#[test]
fn reference_capture_matches_the_known_misses() {
    let (cx, dir) = open("truth-constructs");
    let root = dir.join("repo");
    std::fs::create_dir_all(&root).unwrap();
    let src = "\
pub struct OnlyTyped;
pub struct Recv;
impl Recv {
    pub fn method_callee(&self) {}
}
pub fn plain_callee() {}
pub fn scoped_callee() {}
pub fn macro_callee() -> bool { true }
pub fn user(r: Recv, t: Vec<OnlyTyped>) {
    plain_callee();
    self::scoped_callee();
    r.method_callee();
    assert!(macro_callee());
}
";
    std::fs::write(root.join("lib.rs"), src).unwrap();
    index::run(&Ctx::new(&cx), &root, false).unwrap();
    let key = index::canon(&root);
    let seen = |n: &str| !cx.store.symbol_refs(&key, n).unwrap().is_empty();
    let changed: Vec<(&str, bool)> = [
        ("plain_callee", true),
        ("scoped_callee", true),
        ("method_callee", true),
        ("OnlyTyped", false),    // type position
        ("macro_callee", false), // macro arguments are an opaque token_tree
    ]
    .into_iter()
    .filter(|(n, want)| seen(n) != *want)
    .collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        changed.is_empty(),
        "reference capture changed for {changed:?} (name, expected); update PLAN.md \
         \"Known misses\" and this table"
    );
}

/// Definitions must clear the P8b bar of 0.9 and do, at 1.0. References do not: the Rust tags
/// query sees no type positions and nothing inside a macro body, which is 73 of the 147
/// labelled sites. The floor below is a regression guard on the measured 0.305 (0.351 at T8.8,
/// 0.339 at T8.19, 0.318 before T34.6; repo drift, not index drift), not a target;
/// `src/plugins/graph/PLAN.md` names the constructs under "Known misses".
///
/// Why it is the slowest test here: it indexes the whole repo cold, in a debug build, one file
/// after another — walk, stat query, read, sha256, a tags parse that first compiles the Rust
/// tags query afresh (`outline::config` builds a `TagsConfiguration` per file), then one
/// transaction per file with one INSERT per row. The printed line splits the cost.
#[test]
fn labelled_symbols_are_found() {
    let (cx, dir) = open("truth");
    let root = repo();
    let t0 = Instant::now();
    let cold = index::run(&Ctx::new(&cx), &root, false).unwrap();
    let cold_t = t0.elapsed();
    let t1 = Instant::now();
    let warm = index::run(&Ctx::new(&cx), &root, false).unwrap();
    println!(
        "index: cold {} files, {} rows in {cold_t:.2?}; warm re-walk (each watcher settle) \
         read {} files in {:.2?}",
        cold.indexed,
        cold.inserted,
        warm.read,
        t1.elapsed()
    );
    let key = index::canon(&root);

    let (mut dwant, mut dgot) = (0usize, 0usize);
    let (mut rwant, mut rgot) = (0usize, 0usize);
    let (mut def_returned, mut def_right) = (0usize, 0usize);
    let mut misses: Vec<String> = Vec::new();
    let mut false_positives: Vec<(&Truth, Vec<String>)> = Vec::new();
    let truth = truth();
    for t in &truth {
        let defs: HashSet<String> = cx
            .store
            .symbol_defs(&key, &t.name)
            .unwrap()
            .into_iter()
            .map(|(p, ..)| p)
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
        let fp: Vec<String> = defs
            .iter()
            .filter(|p| !t.defs.contains(p))
            .cloned()
            .collect();
        if !fp.is_empty() {
            false_positives.push((t, fp));
        }
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
        "graph_truth: precision {precision:.3} recall {recall:.3} over {} labels\n  sites {got}/{want}\n  definitions {dgot}/{dwant} recall {def_recall:.3} precision {precision:.3} ({def_right}/{def_returned})\n  references {rgot}/{rwant} recall {ref_recall:.3}",
        truth.len()
    );
    for m in &misses {
        println!("  miss: {m}");
    }
    for (t, defs) in false_positives {
        for p in defs {
            println!("  def {} in {p} is not a labelled definition", t.name);
        }
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
        "reference recall {ref_recall:.3} fell below the 0.30 floor; run \
         every_label_names_a_file_that_mentions_the_symbol first — a stale label reads as a miss"
    );
    assert!(
        Path::new(&root).join("src").is_dir(),
        "ground truth was scored against this repo"
    );
}
