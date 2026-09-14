//! trycmd CLI snapshots: full command-output fixtures in `tests/trycmd/*.toml`.
//! Cases are hermetic by construction: `--help` / `--version` never load `Config`,
//! and `bench --dry-run` reads only the `--config` fixture plus `bench/tasks.toml`.
//! Insta stays out of here: it covers structured renderings
//! (`tests/compress_snapshot.rs`), trycmd covers the binary's stdout.

#[test]
fn cli() {
    trycmd::TestCases::new().case("tests/trycmd/*.toml");
}
