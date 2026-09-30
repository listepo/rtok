//! Divan benches for the graph index: cold index + warm walk + warm symbol query.
//!
//! Tags vs LSP token/latency comparison (tool-output into context) lives in
//! `tests/graph_backend_bench.rs` — run:
//! `mise exec -- cargo test --release --test graph_backend_bench -- --ignored --nocapture --test-threads=1`
//!
//!
//! Each case builds its own store and repo outside the timed region (`with_inputs`) and
//! drops them outside it too (`bench_refs` — `bench_values` would move the fixture into the
//! closure and time the removal of the files), so a number is one cold index or one warm
//! walk, never an average over a warmed cache. The fixture is 100 files on purpose: the
//! 3 000-file release numbers stay in `tests/graph_bench.rs` (ignored, release only), while
//! this bench must finish in reasonable time on a debug build too. No network, no binary.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use divan::Bencher;
use rtok::config::Config;
use rtok::plugin::{Ctx, Runtime};
use rtok::plugins::graph::{index, symbol};

fn main() {
    divan::main();
}

/// Files per bench repo. Small enough for a debug `cargo bench` to stay fast.
const FILES: u32 = 100;

/// A store and a repo that vanish when the sample is done.
struct Fixture {
    cx: Runtime,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn home(tag: &str) -> (Runtime, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rtok-divan-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Config::default();
    c.core.db_path = dir.join("rtok.db");
    c.core.archive_dir = dir.join("archive");
    (Runtime::open(c, tag).unwrap(), dir)
}

fn write_repo(dir: &std::path::Path) {
    for i in 0..FILES {
        let a = (i + 1) % FILES;
        let b = (i + 2) % FILES;
        std::fs::write(
            dir.join(format!("f{i}.rs")),
            format!("fn f{i}() {{\n    f{a}();\n    f{b}();\n}}\n"),
        )
        .unwrap();
    }
}

fn repo(tag: &str) -> Fixture {
    let (cx, dir) = home(tag);
    write_repo(&dir);
    Fixture { cx, dir }
}

fn indexed(tag: &str) -> Fixture {
    let f = repo(tag);
    index::run(&Ctx::new(&f.cx), &f.dir, false).unwrap();
    f
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn cold_index(b: Bencher) {
    b.with_inputs(|| repo("cold")).bench_refs(|f| {
        index::run(&Ctx::new(&f.cx), &f.dir, false)
            .unwrap()
            .inserted
    });
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn warm_walk(b: Bencher) {
    b.with_inputs(|| indexed("warm"))
        .bench_refs(|f| index::run(&Ctx::new(&f.cx), &f.dir, false).unwrap().skipped);
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn warm_symbol(b: Bencher) {
    b.with_inputs(|| indexed("symbol"))
        .bench_refs(|f| symbol(&Ctx::new(&f.cx), &f.dir, "f0").unwrap().len());
}
