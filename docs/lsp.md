# LSP backend for `graph`

By default the `graph` plugin answers `symbol`, `callers`, `impact` and `outline`
from its tree-sitter-tags index in SQLite (`backend = "tags"`). Setting
`backend = "lsp"` routes those same four MCP tools — same names, same callers —
through a language server spoken to over stdio instead. The tags index is then
not consulted for that call. Each LSP answer records one `Measurement` row with
`plugin = "graph"` and `kind = "lsp.symbol" | "lsp.callers" | "lsp.impact" |
"lsp.outline"`, so `rtok stats` attributes it like any other saving.

The server is picked from the workspace root — rtok never links or shells out to
anything else (D6); it spawns one of these from `PATH`:

| Root marker | Server command |
|-------------|----------------|
| `Cargo.toml` | `rust-analyzer` |
| `compile_commands.json` | `clangd` |
| `tsconfig.json` | `typescript-language-server --stdio` |
| `pubspec.yaml` | `dart language-server` |

The walkthrough below covers Rust and Dart.

## Rust: rust-analyzer

Install once via rustup, then check the binary answers:

```bash
rustup component add rust-analyzer
rust-analyzer --version
```

`rustup` puts `rust-analyzer` on `PATH`; rtok additionally resolves it through
`rustup which rust-analyzer` when rustup is present.

## Dart: Dart SDK

Install the SDK (https://dart.dev/get-dart), then check it answers:

```bash
dart --version
```

The server is the SDK's own `language-server` subcommand — rtok spawns
`dart language-server`, so no extra install step exists beyond the SDK itself.

## Point a project at it

The marker file decides the workspace root: `Cargo.toml` for a Rust crate,
`pubspec.yaml` for a Dart package. Either set it per project in
`<git root>/.rtok.toml`:

```toml
[plugins.graph]
backend = "lsp"
```

or per invocation from the environment:

```bash
RTOK_PLUGINS_GRAPH_BACKEND=lsp rtok config get plugins.graph.backend
```

which prints `lsp` (default is `tags`). With `--sources` the row reads
`plugins.graph.backend = lsp (env)`:

```bash
rtok config show --sources | grep plugins.graph.backend
```

## Confirm it works

Call MCP `outline` on a sample file, then `callers` on one of its symbols.
`tests/graph_lsp_gate.rs` is the runnable version of this check: it gets
`outline` of a two-symbol `lib/main.dart` through `dart language-server`, asserts
the `OnlyTyped` reference hits through rust-analyzer (it hits through tags too
since T52.5; the tags-miss pin is now a macro-body `macro_callee` reference),
and asserts one `lsp*` measurement row:

```bash
cargo test --test graph_lsp_gate
```

All six tests pass when both servers are on `PATH`; the Dart and LSP tests
skip otherwise.

## Without the server

When the binary is missing, or no marker file is found above the queried file,
the tool returns an error instead of an answer — for example
`lsp: rust-analyzer not on PATH`, or
`lsp: no Cargo.toml / compile_commands.json / tsconfig.json / pubspec.yaml in <root>`.
Nothing is indexed as a substitute and nothing is recorded. To go back to the
built-in index, set `backend = "tags"` again (the default); the tags answers
are byte-identical with the flag off, per the `graph_lsp_gate` contract test.
