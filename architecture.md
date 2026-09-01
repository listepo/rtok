# rtok architecture

One static Rust binary. Every token-reduction method is a plugin behind one trait. Three
surfaces reach the plugins: Claude Code hooks, an MCP server, and an API proxy. One SQLite
file records what every plugin did, before and after, so a saving is a row or it does not
exist.

This document describes the shape; `plan.md` holds the decisions (D1–D14) and the tasks;
`research.md` holds the evidence. `roadmap.md` is the per-plugin build plan. `ideas.md` holds propositions not yet in `plan.md`.

## 1. Principles the code enforces

| Principle | Where it lives |
|-----------|----------------|
| Fail open: a hook exits 0 in ≤ 10 ms even on error, output `{}` | `hooks::types::HookOutput::default()` serialises to `{}`; the dispatcher (T2.1) wraps plugins in `catch_unwind` |
| Lossless by default: anything shortened is retrievable via `expand <id>` | `archive` table + `~/.rtok/archive/`; every capped output carries an id |
| A saving that is not a `Measurement` row does not exist | `plugin::Measurement` is the only type `Ctx::record` accepts; `measurements` table |
| Injected context is budgeted and byte-stable | single `inject` plugin; `core.inject_budget_tokens` |
| PostToolUse can only add context | `Plugin::post_tool` returns `Option<String>` (additionalContext), nothing else |
| v0.1: no daemon, no subprocess plugins, no WASM (v0.2+ may add daemon/WASM; D6 unchanged) | plugins are in-tree modules behind Cargo features |
| Every plugin is written here from scratch; no third-party tool on any code path (D6) | `Manifest` has no adapter kind; T0.8 Check greps `src/plugins` for retired tool names |
| Every CLI flag is a config key; one precedence rule (D12, D14) | clap 4 derive; figment layers + provenance; toml_edit for `config set`; `tests/config_coverage.rs` walks the clap tree |

## 2. Layers

```
┌────────────────────────────── surfaces ───────────────────────────────┐
│  rtok hook <event>        rtok mcp              rtok proxy            │
│  (stdin JSON → stdout)    (stdio JSON-RPC)      (ANTHROPIC_BASE_URL,  │
│                                                  OPENAI_BASE_URL)      │
│  src/hooks/               src/mcp.rs            src/proxy/            │
└──────────────┬───────────────────┬───────────────────────┬────────────┘
               │ HookInput         │ tools/list, call      │ MessagesRequest
               ▼                   ▼                       ▼
┌──────────────────────────── plugins::Registry ────────────────────────┐
│  enabled plugins in dispatch order, from Cargo features ∩ config      │
│  measure  cmd  read  archive  proxy  inject  guard  memory  graph toon│
│  each: src/plugins/<id>/{mod.rs, README.md, AGENTS.md}                │
└──────────────┬────────────────────────────────────────────────────────┘
               │ &Ctx
               ▼
┌──────────────────────────────── core ─────────────────────────────────┐
│  plugin.rs   trait Plugin, Manifest, Ctx, Measurement, event views    │
│  config.rs   ~/.rtok/config.toml, RTOK_HOME, CATALOGUE                │
│  store/      Diesel ORM + bundled SQLite (WAL, FTS5), migrations/     │
│  tokens.rs   chars-per-token estimator per class (±15 %)              │
└───────────────────────────────────────────────────────────────────────┘
               │
               ▼
        ~/.rtok/rtok.db            ~/.rtok/archive/<id>
```

Dependencies point downward only. Surfaces know about the registry; plugins know about
`Ctx`; core knows about nothing above it. A surface never calls another surface.

## 3. Module map

| Path | Role | Plan task |
|------|------|-----------|
| `src/main.rs` | clap CLI; each subcommand is a thin call into the library | T0.1 |
| `src/lib.rs` | crate root; declares the modules below | — |
| `src/config.rs` | `Config::load()`, defaults, `[plugins.<id>]`, `CATALOGUE` | T0.2 |
| `src/config/layers.rs`, `validate.rs`, `config/default.toml` | figment providers (D14); `rtok config show/validate/set` (see `docs/config.md`) | P12 |
| `src/store/` + `migrations/` | Diesel models; `Store::open`; `insert_call`/`tokens`/`log`; `insert_measurement` | T0.3, P13 |
| `src/plugin.rs` | the contract (§4) | T0.4 |
| `src/plugins/mod.rs` | feature-gated module list, `all()`, `Registry` | T0.4 |
| `src/plugins/<id>/` | one plugin: `mod.rs` + `README.md` (what/why) + `AGENTS.md` (how to work on it) | per plugin |
| `src/tokens.rs` | `estimate(text, Class, &Estimator)`, `tokens_saved` | T0.5 |
| `src/hooks/types.rs` | `HookInput`, `HookOutput`, event views | T0.6 |
| `src/hooks/mod.rs` | dispatcher: merge plugin outputs, log `events`, fail open | T2.1 |
| `src/mcp.rs` | rmcp stdio server built from `Plugin::mcp_tools()` | T4.1 |
| `src/proxy/` | axum passthrough + `compress` mode via `Plugin::proxy_filter()` | T5.1 |
| `src/proxy/wire.rs`, `anthropic.rs`, `openai_chat.rs`, `openai_responses.rs` | `Wire` adapters: one per API format, exposing tool results and `usage` in one normalised shape (D11) | P11 |
| `src/measure/` | JSONL ingest, `rtok stats`, baselines, cache report | P1 |
| `src/setup/` | host installers (claude, cursor, codex) with backups and `--dry-run` | T2.3, P10 |
| `examples/hello_plugin.rs` | smallest complete plugin, run by CI | — |
| `tests/fixtures/hooks/*.json` | one real payload per hook event | T0.6 |

## 4. The plugin contract

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> Manifest;                                   // id, surfaces, default_on
    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision>;   // Deny | Rewrite
    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String>;          // additionalContext only
    fn session_start(&self, ev: &SessionStart, cx: &Ctx) -> Option<Injection>;
    fn prompt_submit(&self, ev: &PromptSubmit, cx: &Ctx) -> Option<Injection>;
    fn pre_compact(&self, ev: &PreCompact, cx: &Ctx);
    fn mcp_tools(&self) -> Vec<ToolDef>;
    fn proxy_filter(&self, req: &mut MessagesRequest, cx: &Ctx) -> Vec<Measurement>;
}
```

Every method has a no-op default, so a plugin implements only the surfaces its manifest
declares. Event types are borrowed views over `HookInput`, so no copying happens on the hook
path.

`Ctx` is the whole world a plugin sees:

```rust
pub struct Ctx { pub config: Config, pub store: Store, pub session: String }
impl Ctx {
    fn estimate(&self, text: &str, class: Class) -> u32;   // tokens, ±15 %
    fn plugin_cfg(&self, id: &str) -> Option<&PluginCfg>;  // [plugins.<id>] table
    fn record(&self, m: &Measurement) -> Result<()>;       // the only way to claim a saving
}
```

The archive store (`~/.rtok/archive/`) joins `Ctx` in T3.1.

### Merge rules in the dispatcher (T2.1)

- PreToolUse: the first `Deny` wins; `Rewrite` is last-writer; anything else passes through.
- PostToolUse: `additionalContext` strings are concatenated, then capped by the inject budget.
- SessionStart / UserPromptSubmit: all `Injection`s go to `inject`, which sorts by priority
  and emits until the budget.
- Any panic or error inside a plugin → that plugin's output is dropped, the event is logged
  with the error, and the hook still exits 0 with whatever the other plugins produced.

## 5. One kind of plugin

Every plugin is native Rust written from scratch in this repo (decision D6). No plugin
spawns, links, imports, or reads the data of a third-party tool. The tools rtok replaces are
specs (`research.md`); their names appear in code only where rtok inspects them (`doctor`),
retires them (`setup --replace`) or benches against them.

Third parties extend rtok from outside: depend on the `rtok` library, implement `Plugin`,
and build a binary with `Registry::from_plugins(vec![Box::new(Mine)], &config)` (T0.8).
`docs/plugin-authoring.md` and `examples/` are the whole public surface; this repo ships no
third-party plugins.

## 6. Compile-time and run-time selection

- **Compile time**: one Cargo feature per plugin id, `default = all`. `plugins::all()` pushes
  each plugin under `#[cfg(feature = "<id>")]`. `cargo build --no-default-features
  --features measure` must always succeed (T0.4 Check) — this keeps every plugin decoupled
  from every other plugin.
- **Run time**: `[plugins.<id>] enabled = bool` in config; unset → `Manifest::default_on`.
  `Registry::new(&config)` resolves both and exposes `enabled()` in dispatch order.
- `config::CATALOGUE` is the single list of `(id, default_on)`; a test asserts the registry's
  manifests match it, so a new plugin cannot be half-registered.

## 7. Data

One SQLite file, WAL mode, opened per invocation (hooks are short-lived processes; SQLite
handles the concurrency). Migrations are `migrations/NNNN.sql`, embedded with
`include_str!`, applied once each and recorded in `schema_migrations`. Editing an applied
migration is forbidden; add the next file.

| Table | Written by | Read by |
|-------|-----------|---------|
| `hosts`, `providers`, `models`, `sessions` | `Store` upsert | every `calls` row |
| `calls` + `call_io` | dispatcher, mcp, proxy | `rtok stats`, doctor |
| `tokens` | same surfaces (`before`/`after`/`mcp`) | `rtok stats --plugin <id>` |
| `logs` | core + plugins via `Ctx::log` | doctor, debug |
| `events` | (superseded; 0001 leftover) | — |
| `measurements` | `Ctx::record` | `rtok stats --plugin <id>`, bench |
| `archive` | `cmd`, `read`, `archive`, `call_io` spill | `rtok expand`, `guard` |
| `read_cache` | `read` | `read` (dedup) |
| `notes` + `notes_fts` | `memory` | `memory` |
| `usage` | `proxy` | `measure` |

Raw payloads live on disk under `archive_dir/<id>`; the DB holds size, sha256 and path.

## 8. Measurement is the product

The metric is **context-token-turns**: a tool result of T tokens produced at turn t of an
N-turn session costs T × (N − t), because it is re-sent (cached or not) on every later turn.
Output tokens are counted separately. Estimates come from `tokens::estimate` and are
labelled as such; real counts come only from proxy `usage` rows.

The honesty metric for any lossless shortening is the **expand rate**: how often the model
had to ask for the original. Gates in `plan.md` keep a plugin only if the expand rate stays
under 5 %.

## 9. Extending

- **New plugin**: `docs/plugin-authoring.md` — module, manifest, feature, registry push,
  `CATALOGUE` entry, README + AGENTS, one test, one measurement path.
- **New hook event**: add fields to `HookInput`, a view struct in `plugin.rs`, an accessor,
  a fixture, and a trait method with a default body.
- **New host** (Cursor, OpenCode, Codex): a `src/setup/<host>.rs` installer plus, if the
  payload differs, a field mapping into `HookInput`. The plugins do not change.
- **New surface**: a new module under `src/` that builds a `Registry` and calls the trait;
  add a `Surface` variant so manifests can declare it.
- **New API wire format** (e.g. Gemini): implement `Wire` in `src/proxy/<name>.rs` — route
  match, tool-result accessor, usage parser for body and SSE — plus fixtures. Plugins do not
  change; `usage.api` gets a new value.

## 10. Testing strategy

- Unit tests next to the code (`cargo test`); every task in `plan.md` has one machine Check.
- Fixture-driven: hook payloads in `tests/fixtures/hooks/`, golden filter cases in
  `tests/cmd_golden/`, per-language outline fixtures for `read`.
- Latency harness (`tests/latency.rs`, T2.2) asserts p95 < 10 ms for a hook round trip.
- `make check` = `fmt --check` + `clippy --all-targets --all-features -D warnings` + tests +
  the single-feature build; CI runs it on macOS and Linux with the toolchain from `mise.toml`.
- `examples/hello_plugin.rs` runs in CI and asserts a measurement row was written.

## 11. Not in v0.1

v0.1 has no LLM-based compression, embeddings, type-resolved call graph (the `graph` plugin
is a tree-sitter-tags index), semantic response cache, daemon, or WASM plugin host. Those
are **v0.2+** (`plan.md` Later versions, `ideas.md` Later, `roadmap.md` Later), not discarded.
Adapters over third-party tools stay out of *this repo* at every version (D6); a later WASM
host loads plugins that live outside this repo.
