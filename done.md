# rtok — completed tasks

Tasks move here from `plan.md` when their Check passed, `make check` is green, and the work is
committed as `<task-id>: <title>`. Newest phase first. Task text is kept verbatim so the
history of what was asked stays readable next to what was delivered.

## P0 — Scaffold · done 2026-09-01

Goal: `rtok --version`, DB, plugin registry, hook I/O types. All seven tasks done; gate
review (Sonnet) pending.

Verified at the head of the P0 series: `make check` (fmt, clippy `-D warnings` on all targets
and features, 13 unit tests, single-feature build) and `make example` pass on macOS,
Rust 1.97.1.

**T0.1 cargo project** · haiku · — · `Cargo.toml`, `src/main.rs`
Do: `cargo init --name rtok`; binary name `rtok` via `[[bin]]`; clap derive with subcommands `hook`, `mcp`, `proxy`, `stats`, `bench`, `doctor`, `setup`, `run`, `expand`, `plugins` (all stubs printing "not implemented", exit 0). Rust pinned via `mise.toml` (no `rust-toolchain.toml`). `.gitignore`, `git init`, first commit.
Check: `cargo run -q -- --version` → `rtok 0.1.0`; `cargo run -q -- plugins` → exits 0.
Status: done 2026-09-01 · `rtok 0.1.0`, `plugins` exit 0 · commit `T0.1: cargo project`.

**T0.2 config + paths** · haiku · T0.1 · `src/config.rs`
Do: `~/.rtok/config.toml` (create with defaults if missing): `[core] db_path, archive_dir, estimator_chars_per_token = 3.5, inject_budget_tokens = 800`; `[plugins.<id>] enabled = bool` per catalogue id. Env override `RTOK_HOME`. Function `Config::load()`.
Check: `RTOK_HOME=$(mktemp -d) cargo run -q -- plugins` creates `config.toml`; a unit test asserts default budget 800.
Status: done 2026-09-01 · config.toml created with all 10 plugin ids; `config::tests::creates_defaults_and_budget_is_800` passes.
Deviation: estimator rates live in an `[estimator] code/prose/json/cjk` table (one key per class, as T0.5 needs) instead of a single `core.estimator_chars_per_token`. Per-plugin settings are free-form keys under `[plugins.<id>]` next to `enabled`.

**T0.3 SQLite store** · haiku · T0.2 · `src/store.rs`, `migrations/0001.sql`
Do: rusqlite with `bundled` + `fts5`. WAL mode. Tables: `events(id, ts, session, event, tool, plugin, ms)`, `measurements(id, ts, session, plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id)`, `archive(id TEXT PK, ts, session, tool, bytes, path, sha256)`, `read_cache(session, path, sha256, ts, archive_id)`, `notes(id, ts, project, kind, title, body)` + `notes_fts` (FTS5 content table), `usage(id, ts, session, model, input, cache_create, cache_read, output)`. Migration runner keyed by filename.
Check: `cargo test store::` → migration applies twice idempotently; FTS5 `MATCH` query returns an inserted note.
Status: done 2026-09-01 · 3 tests (`migration_is_idempotent`, `fts5_match_finds_inserted_note`, `open_on_disk_uses_wal`). FTS5 comes with rusqlite's `bundled` build; no separate feature needed. `Store::insert_measurement` added here for T0.4's `Ctx::record`.

**T0.4 plugin trait + registry** · haiku · T0.3 · `src/plugin.rs`, `src/plugins/mod.rs`
Do: trait from §1; `Manifest { id, kind, surfaces: Vec<Surface>, default_on }`; registry built from Cargo features (`--features cmd,read,...`, default = all); `rtok plugins` prints a table (id, kind, enabled, surfaces) reading config.
Check: `cargo run -q -- plugins` lists ≥ 10 ids; `cargo build --no-default-features --features measure` succeeds.
Status: done 2026-09-01 · table lists 10 ids (9 on, `toon` off); single-feature build passes. Trait, `Ctx`, `Measurement`, `PreToolDecision`, `Injection`, `ToolDef` and the five event views are in `src/plugin.rs`. Ten modules `src/plugins/<id>/mod.rs` hold manifests only; each ships `README.md` + `AGENTS.md`. `config::CATALOGUE` is the canonical id list and a test pins the registry to it. Crate split into lib + thin bin so tests, examples and future surfaces share one API.

**T0.5 token estimator** · haiku · T0.2 · `src/tokens.rs`
Do: `estimate(text, class) -> u32` with classes `Code`, `Prose`, `Json`, `Cjk`; defaults 3.5 / 4.2 / 3.0 / 1.0 chars per token, loaded from config; `tokens_saved(before, after)`. Document ±15 % error in a doc comment.
Check: unit tests on 3 fixtures; `estimate("", _) == 0`.
Status: done 2026-09-01 · 3 tests; rates come from `config::Estimator`; counts chars, not bytes (CJK fixture).

**T0.6 hook I/O types** · haiku · T0.4 · `src/hooks/types.rs`, `tests/fixtures/hooks/*.json`
Do: serde structs for Claude Code hook input (`session_id, transcript_path, cwd, hook_event_name, tool_name, tool_input, tool_response, prompt, source, trigger`) and output (`hookSpecificOutput { hookEventName, permissionDecision, permissionDecisionReason, updatedInput, additionalContext }`). Fixtures for PreToolUse(Bash), PreToolUse(Read), PostToolUse, UserPromptSubmit, SessionStart, PreCompact, PostCompact.
Check: `cargo test hooks::types` round-trips every fixture unchanged (`serde_json::Value` equality).
Status: done 2026-09-01 · 7 fixtures round-trip byte-for-byte as `Value`; unknown fields survive via a flattened `extra` map; `HookOutput::default()` serialises to `{}` (fail-open output). Event views (`pre_tool()`, `post_tool()`, …) bridge `HookInput` to the trait's event types.

**T0.7 CI** · haiku · T0.1 · `.github/workflows/ci.yml`, `Makefile`
Do: `make check` = fmt --check, clippy -D warnings, test. CI runs it on macOS + Linux.
Check: `make check` exits 0 locally.
Status: done 2026-09-01 · `make check` also runs the single-feature build; CI (`ubuntu-latest`, `macos-latest`) installs the toolchain from `mise.toml` via `jdx/mise-action` and runs `make check` + `make example`. Lints pinned in `Cargo.toml` `[lints]` (`unsafe_code = forbid`, clippy `all` as warnings → errors under `-D warnings`); `rustfmt.toml` sets `style_edition = "2024"`; `.editorconfig` added.

Also delivered with P0 (not numbered tasks): `architecture.md`, `README.md`, `docs/plugin-authoring.md`, `examples/hello_plugin.rs` (asserts a `Measurement` row is written), this file.
