# Toolchain

Программы проекта и прямые пакеты из манифестов.

## Программы

| Программа | Как ставить | Зачем здесь | Источник |
| --- | --- | --- | --- |
| mise | brew / curl, затем `mise install` | Пины версий инструментов | https://github.com/jdx/mise |
| cargo-cache | mise | `just cache` / `just cache-autoclean` (T17.2); the shared cargo home fills up | https://github.com/matthiaskrgr/cargo-cache |
| git-cliff | mise | Changelog | https://github.com/orhun/git-cliff |
| go | mise | hugo resolves the hextra theme as a Go module (site/go.mod) | https://github.com/golang/go |
| hugo | mise | Сайт документации | https://github.com/gohugoio/hugo |
| just | mise | Рецепты команд | https://github.com/casey/just |
| node | mise | jscpd runs on it; nothing in the binary does | https://github.com/nodejs/node |
| jscpd | mise | `just dup` (T26.0): copy-paste detector, config in .jscpd.json | https://github.com/kucherenko/jscpd |
| rust | mise | CI otherwise installs the minimal profile | https://github.com/rust-lang/rust |
| rustc | mise (pin rust) | Компилятор Rust | https://github.com/rust-lang/rust |
| cargo | mise (pin rust) | Сборка и зависимости Rust | https://github.com/rust-lang/cargo |

## cargo

| Пакет | Где | Источник | Зачем здесь |
| --- | --- | --- | --- |
| anyhow | локально | https://crates.io/crates/anyhow | Ошибки CLI |
| axum | локально | https://crates.io/crates/axum | Зависимость Rust |
| clap | локально | https://crates.io/crates/clap | CLI |
| crossterm | локально | https://crates.io/crates/crossterm | Терминал |
| dialoguer | локально | https://crates.io/crates/dialoguer | Зависимость Rust |
| diesel | локально | https://crates.io/crates/diesel | SQLite ORM |
| divan | локально | https://crates.io/crates/divan | Зависимость Rust |
| dotenvy | локально | https://crates.io/crates/dotenvy | Зависимость Rust |
| figment | локально | https://crates.io/crates/figment | Конфиг |
| futures-util | локально | https://crates.io/crates/futures-util | Зависимость Rust |
| httpmock | локально | https://crates.io/crates/httpmock | Зависимость Rust |
| ignore | локально | https://crates.io/crates/ignore | Зависимость Rust |
| indicatif | локально | https://crates.io/crates/indicatif | Зависимость Rust |
| lbug | локально | https://crates.io/crates/lbug | Зависимость Rust |
| libsqlite3-sys | локально | https://crates.io/crates/libsqlite3-sys | Зависимость Rust |
| notify | локально | https://crates.io/crates/notify | Зависимость Rust |
| owo-colors | локально | https://crates.io/crates/owo-colors | Зависимость Rust |
| printpdf | локально | https://crates.io/crates/printpdf | Зависимость Rust |
| ratatui | локально | https://crates.io/crates/ratatui | TUI |
| regex | локально | https://crates.io/crates/regex | Зависимость Rust |
| reqwest | локально | https://crates.io/crates/reqwest | HTTP |
| rmcp | локально | https://crates.io/crates/rmcp | Зависимость Rust |
| rstest | локально | https://crates.io/crates/rstest | Зависимость Rust |
| rustix | локально | https://crates.io/crates/rustix | Зависимость Rust |
| serde | локально | https://crates.io/crates/serde | Сериализация |
| serde_json | локально | https://crates.io/crates/serde_json | JSON |
| sha2 | локально | https://crates.io/crates/sha2 | Зависимость Rust |
| similar | локально | https://crates.io/crates/similar | Зависимость Rust |
| slint | локально | https://crates.io/crates/slint | Зависимость Rust |
| slint-build | локально | https://crates.io/crates/slint-build | Зависимость Rust |
| tokio | локально | https://crates.io/crates/tokio | Асинхронность |
| toml_edit | локально | https://crates.io/crates/toml_edit | Зависимость Rust |
| tower-http | локально | https://crates.io/crates/tower-http | Зависимость Rust |
| tree-sitter | локально | https://crates.io/crates/tree-sitter | Зависимость Rust |
| tree-sitter-c | локально | https://crates.io/crates/tree-sitter-c | Зависимость Rust |
| tree-sitter-dart | локально | https://crates.io/crates/tree-sitter-dart | Зависимость Rust |
| tree-sitter-go | локально | https://crates.io/crates/tree-sitter-go | Зависимость Rust |
| tree-sitter-javascript | локально | https://crates.io/crates/tree-sitter-javascript | Зависимость Rust |
| tree-sitter-python | локально | https://crates.io/crates/tree-sitter-python | Зависимость Rust |
| tree-sitter-rust | локально | https://crates.io/crates/tree-sitter-rust | Зависимость Rust |
| tree-sitter-tags | локально | https://crates.io/crates/tree-sitter-tags | Зависимость Rust |
| tree-sitter-typescript | локально | https://crates.io/crates/tree-sitter-typescript | Зависимость Rust |
| wasmi | локально | https://crates.io/crates/wasmi | Зависимость Rust |
| watchman_client | локально | https://crates.io/crates/watchman_client | Зависимость Rust |
