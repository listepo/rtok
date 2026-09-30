# Fuzzing rtok

[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer) targets for the parsers that
take untrusted input. `fuzz/` is its own cargo workspace, excluded from the root one, so
`cargo build`, `just check` and CI never compile it. It needs a nightly toolchain for the
sanitizer build only; the Rust pinned in `mise.toml` is untouched.

## Setup

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

## Run

```sh
just fuzz                   # list the targets
just fuzz cli-argv          # one target, 60 s
just fuzz cli-argv 300      # one target, 300 s
just fuzz all 90            # every target in turn, 90 s each

# or directly, with any libFuzzer flag after `--`
cargo +nightly fuzz run cmd-filter -- -max_total_time=120 -max_len=16384
```

A crash leaves its input in `fuzz/artifacts/<target>/`; replay and shrink it with

```sh
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<sha>
cargo +nightly fuzz tmin <target> fuzz/artifacts/<target>/crash-<sha>
cargo +nightly fuzz fmt <target> fuzz/artifacts/<target>/crash-<sha>   # the Arbitrary value
```

A fixed crash becomes a unit test next to the code, not a file under `fuzz/`. `corpus/`,
`artifacts/` and `target/` are git-ignored.

## Targets

| Target | Covers |
| --- | --- |
| `cli-argv` | Arbitrary argv (tree words, `--flag=value`, free and non-UTF-8 words) through `Cli::try_parse_from`, plus the error/help rendering and clap's `debug_assert` of the tree |
| `config-toml` | `config.toml` text through `rtok config validate` and the file-free config stack (defaults < TOML < legacy fold, `~` expansion, `config show` rows) |
| `cmd-filter` | A user `rules` TOML merged over the built-ins, then the `cmd` stdout compactor for an argv and output; the "already bounded" command lexer |
| `jsonc-edit` | The JSONC editor that installs rtok into host settings; asserts an edit keeps the file parseable, lands the entry, and that removing it round-trips |
| `hook-io` | Hook stdin JSON, every host adapter, the typed event views, host tool-name mapping + guard key, and the per-host reply encoders |
| `proxy-wire` | Proxy request/response bodies per wire: session, model and usage (body and SSE), request preparation, tool-result/skill/blob views, context edits, semantic-cache key, skills-listing measure, upstream URL join |
| `transcript-jsonl` | Session transcript ingest behind `rtok stats` |
| `text-ops` | Terse prose compression, `rtok expand` ranges/grep/cut, the memory block splice, TOON encoding |
| `outline` | `read` outlines and comment stripping (tree-sitter tags per language, markdown headings) |

Everything runs in-process with no network, no process spawn and no file writes: hook
dispatch, the store and the servers stay out. Crate-private parsers are reached through
`src/fuzzing.rs`, compiled only under the `--cfg fuzzing` that `cargo fuzz` sets.

## Adding a target

`cargo +nightly fuzz add <name>` (kebab-case), then give it a `Covers` row above. If the code
under test is not public, add a small entry point to `src/fuzzing.rs` rather than widening the
crate API.
