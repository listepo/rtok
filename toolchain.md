# Toolchain

Project programs and direct packages from the manifests.

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| mise | brew / curl, then `mise install` | Pinned tool versions | https://github.com/jdx/mise |
| cargo-cache | mise | `just cache` / `just cache-autoclean` (T17.2); the shared cargo home fills up | https://github.com/matthiaskrgr/cargo-cache |
| git-cliff | mise | Changelog | https://github.com/orhun/git-cliff |
| go | mise | hugo resolves the hextra theme as a Go module (site/go.mod) | https://github.com/golang/go |
| hugo | mise | Documentation site | https://github.com/gohugoio/hugo |
| just | mise | Command recipes | https://github.com/casey/just |
| node | mise | jscpd runs on it; nothing in the binary does | https://github.com/nodejs/node |
| jscpd | mise | `just dup` (T26.0): copy-paste detector, config in .jscpd.json | https://github.com/kucherenko/jscpd |
| rust | mise | CI otherwise installs the minimal profile | https://github.com/rust-lang/rust |
| rustc | mise (pin rust) | Rust compiler | https://github.com/rust-lang/rust |
| cargo | mise (pin rust) | Rust build and dependencies | https://github.com/rust-lang/cargo |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| anyhow | local | https://crates.io/crates/anyhow | CLI errors |
| assert_cmd | local | https://crates.io/crates/assert_cmd | CLI e2e tests |
| axum | local | https://crates.io/crates/axum | Rust dependency |
| clap | local | https://crates.io/crates/clap | CLI |
| crossterm | local | https://crates.io/crates/crossterm | Terminal |
| dialoguer | local | https://crates.io/crates/dialoguer | Rust dependency |
| diesel | local | https://crates.io/crates/diesel | SQLite ORM |
| divan | local | https://crates.io/crates/divan | Divan benches in benches/ |
| dotenvy | local | https://crates.io/crates/dotenvy | Rust dependency |
| figment | local | https://crates.io/crates/figment | Config |
| futures-util | local | https://crates.io/crates/futures-util | Rust dependency |
| httpmock | local | https://crates.io/crates/httpmock | Rust dependency |
| i-slint-backend-testing | local | https://crates.io/crates/i-slint-backend-testing | Headless backend for Slint UI e2e tests |
| ignore | local | https://crates.io/crates/ignore | Rust dependency |
| indicatif | local | https://crates.io/crates/indicatif | Rust dependency |
| insta | local | https://crates.io/crates/insta | Snapshot tests for stable text output |
| libsqlite3-sys | local | https://crates.io/crates/libsqlite3-sys | Rust dependency |
| notify | local | https://crates.io/crates/notify | Rust dependency |
| owo-colors | local | https://crates.io/crates/owo-colors | Rust dependency |
| printpdf | local | https://crates.io/crates/printpdf | Rust dependency |
| ratatui | local | https://crates.io/crates/ratatui | TUI |
| regex | local | https://crates.io/crates/regex | Rust dependency |
| reqwest | local | https://crates.io/crates/reqwest | HTTP |
| rmcp | local | https://crates.io/crates/rmcp | Rust dependency |
| rstest | local | https://crates.io/crates/rstest | Rust dependency |
| rustix | local | https://crates.io/crates/rustix | Rust dependency |
| serde | local | https://crates.io/crates/serde | Serialization |
| serde_json | local | https://crates.io/crates/serde_json | JSON |
| sha2 | local | https://crates.io/crates/sha2 | Rust dependency |
| similar | local | https://crates.io/crates/similar | Rust dependency |
| slint | local | https://crates.io/crates/slint | Rust dependency |
| slint-build | local | https://crates.io/crates/slint-build | Rust dependency |
| tokio | local | https://crates.io/crates/tokio | Async runtime |
| toml_edit | local | https://crates.io/crates/toml_edit | Rust dependency |
| tower-http | local | https://crates.io/crates/tower-http | Rust dependency |
| tree-sitter | local | https://crates.io/crates/tree-sitter | Rust dependency |
| tree-sitter-c | local | https://crates.io/crates/tree-sitter-c | Rust dependency |
| tree-sitter-dart | local | https://crates.io/crates/tree-sitter-dart | Rust dependency |
| tree-sitter-go | local | https://crates.io/crates/tree-sitter-go | Rust dependency |
| tree-sitter-javascript | local | https://crates.io/crates/tree-sitter-javascript | Rust dependency |
| tree-sitter-python | local | https://crates.io/crates/tree-sitter-python | Rust dependency |
| tree-sitter-rust | local | https://crates.io/crates/tree-sitter-rust | Rust dependency |
| tree-sitter-tags | local | https://crates.io/crates/tree-sitter-tags | Rust dependency |
| tree-sitter-typescript | local | https://crates.io/crates/tree-sitter-typescript | Rust dependency |
| trycmd | local | https://crates.io/crates/trycmd | Full CLI command-output fixtures in tests/trycmd/ |
| wasmi | local | https://crates.io/crates/wasmi | Rust dependency |
| watchman_client | local | https://crates.io/crates/watchman_client | Rust dependency |
