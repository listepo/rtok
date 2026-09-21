# Toolchain

Project programs and direct packages from the manifests.

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| mise | brew / curl, then `mise install` | Pinned tool versions | https://github.com/jdx/mise |
| cargo-cache | mise | `just cache` / `just cache-autoclean` (T17.2); the shared cargo home fills up | https://github.com/matthiaskrgr/cargo-cache |
| binaryen | brew | wasm-opt for T60.7 webui bundle | https://github.com/WebAssembly/binaryen |
| cargo-nextest | global (cargo install) | Parallel test runner | https://github.com/nextest-rs/nextest |
| git-cliff | mise | Changelog | https://github.com/orhun/git-cliff |
| go | mise | hugo resolves the hextra theme as a Go module (site/go.mod) | https://github.com/golang/go |
| hugo | mise | Documentation site | https://github.com/gohugoio/hugo |
| just | mise | Command recipes | https://github.com/casey/just |
| node | mise | jscpd, oxlint, oxfmt and vitest run on it; nothing in the binary does | https://github.com/nodejs/node |
| jscpd | mise | `just dup` (T26.0): copy-paste detector, config in .jscpd.json | https://github.com/kucherenko/jscpd |
| oxlint | mise (`npm:oxlint`) | `just js` (T110): lint for the TS host plugins and tests/node, `--deny-warnings` | https://github.com/oxc-project/oxc |
| oxfmt | mise (`npm:oxfmt`) | `just js` / `just js-fmt` (T110): formatter for the same JS/TS files | https://github.com/oxc-project/oxc |
| vitest | mise (`npm:vitest`) | T111: runs the TS host plugin tests (`vitest.config.mjs`, globals, inline snapshots); driven by `tests/filter.rs` and `tests/pi_plugin.rs` | https://github.com/vitest-dev/vitest |
| rust | mise | CI otherwise installs the minimal profile | https://github.com/rust-lang/rust |
| rustc | mise (pin rust) | Rust compiler | https://github.com/rust-lang/rust |
| cargo | mise (pin rust) | Rust build and dependencies | https://github.com/rust-lang/cargo |
| colima | mise | Container runtime that runs Docker (and others) inside a Lima VM — lightweight alternative to Docker Desktop for agents | https://github.com/abiosoft/colima |
| lima | mise | Linux VM Colima drives; install via mise, usually started only through `colima start` | https://github.com/lima-vm/lima |
| docker-cli | mise | `docker` client; points at Colima's Docker context when Colima is running | https://github.com/docker/cli |
| docker-compose | mise | Standalone `docker-compose` against the same Colima daemon (~Docker-compatible; not Podman) | https://github.com/docker/compose |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| anyhow | local | https://crates.io/crates/anyhow | CLI errors |
| assert_cmd | local | https://crates.io/crates/assert_cmd | CLI e2e tests |
| axum | local | https://crates.io/crates/axum | Rust dependency |
| clap | local | https://crates.io/crates/clap | CLI |
| clap_complete | local | https://crates.io/crates/clap_complete | `rtok completions` shell scripts (T53.2) |
| clap_mangen | local | https://crates.io/crates/clap_mangen | `rtok man` roff page (T53.2) |
| crossterm | local | https://crates.io/crates/crossterm | Terminal |
| dialoguer | local | https://crates.io/crates/dialoguer | Rust dependency |
| diesel | local | https://crates.io/crates/diesel | SQLite ORM |
| divan | local | https://crates.io/crates/divan | Divan benches in benches/ |
| dotenvy | local | https://crates.io/crates/dotenvy | Rust dependency |
| dunce | local | https://crates.io/crates/dunce | Canonicalize without Windows UNC prefixes |
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
| pathdiff | local | https://crates.io/crates/pathdiff | Relative path between two paths |
| printpdf | local | https://crates.io/crates/printpdf | Rust dependency |
| ratatui | local | https://crates.io/crates/ratatui | TUI |
| regex | local | https://crates.io/crates/regex | Rust dependency |
| reqwest | local | https://crates.io/crates/reqwest | HTTP |
| rmcp | local | https://crates.io/crates/rmcp | Rust dependency |
| rstest | local | https://crates.io/crates/rstest | Rust dependency |
| rustix | local | https://crates.io/crates/rustix | Rust dependency |
| rustls | local | https://crates.io/crates/rustls | Preconfigured webpki TLS client config (T53.3) |
| rustls-pemfile | local | https://crates.io/crates/rustls-pemfile | `SSL_CERT_FILE` bundle parsing (T53.3) |
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
| tree-sitter-c-sharp | local | https://crates.io/crates/tree-sitter-c-sharp | C# grammar tags (T52.2) |
| tree-sitter-dart | local | https://crates.io/crates/tree-sitter-dart | Rust dependency |
| tree-sitter-go | local | https://crates.io/crates/tree-sitter-go | Rust dependency |
| tree-sitter-java | local | https://crates.io/crates/tree-sitter-java | Java grammar tags (T52.2) |
| tree-sitter-javascript | local | https://crates.io/crates/tree-sitter-javascript | Rust dependency |
| tree-sitter-kotlin-ng | local | https://crates.io/crates/tree-sitter-kotlin-ng | Kotlin grammar tags (T52.2) |
| tree-sitter-php | local | https://crates.io/crates/tree-sitter-php | PHP grammar tags (T52.2) |
| tree-sitter-python | local | https://crates.io/crates/tree-sitter-python | Rust dependency |
| tree-sitter-ruby | local | https://crates.io/crates/tree-sitter-ruby | Ruby grammar tags (T52.2) |
| tree-sitter-rust | local | https://crates.io/crates/tree-sitter-rust | Rust dependency |
| tree-sitter-swift | local | https://crates.io/crates/tree-sitter-swift | Swift grammar tags (T52.2) |
| tree-sitter-tags | local | https://crates.io/crates/tree-sitter-tags | Rust dependency |
| tree-sitter-typescript | local | https://crates.io/crates/tree-sitter-typescript | Rust dependency |
| trycmd | local | https://crates.io/crates/trycmd | Full CLI command-output fixtures in tests/trycmd/ |
| tokio-tungstenite | local | https://crates.io/crates/tokio-tungstenite | WebSocket client for the `rtok web` e2e (tests/web_e2e.rs) |
| wasmi | local | https://crates.io/crates/wasmi | Rust dependency |
| watchman_client | local | https://crates.io/crates/watchman_client | Rust dependency |
| webpki-roots | local | https://crates.io/crates/webpki-roots | Mozilla roots without the platform verifier (T53.3) |
