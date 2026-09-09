# Writing a plugin

A plugin implements one trait: `Plugin`, from the published
[`rtok-plugin-sdk`](https://crates.io/crates/rtok-plugin-sdk) crate. That is true whether the
plugin ships inside this repository or lives in someone else's (D25) — there is one contract
and no in-tree shortcut. `crates/rtok-plugin-sdk/examples/shrink.rs` is a complete plugin in
one file; `examples/hello_plugin.rs` is the same thing wired to the real host.

## 0. Should it exist?

Check `plan.md` §1 (catalogue) and §0 (decisions), and `roadmap.md` for that plugin's task
order. If the method is not there, add it to `ideas.md` (source tool + plugin; Later if it is
v0.2+) and propose a plan change first. Every plugin is written from scratch here: it must not
shell out to, link, or read the data of a third-party tool (D6). The tool it replaces is the
spec; `research.md` is the evidence.

## 1. The trait

```rust
use rtok_plugin_sdk::{
    Ctx, DashboardPage, Manifest, Plugin, PreToolDecision, PreToolUse, Surface,
};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    // Required: what this plugin is.
    fn manifest(&self) -> Manifest {
        Manifest { id: "<id>", surfaces: &[Surface::Hook], default_on: true }
    }

    // Required: the page `rtok web` and `rtok tui` both render (D23).
    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new("<Title>", "One sentence for the operator.", true)
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        let cfg = cx.plugin_config::<crate::config::MySection>("<id>");
        let _ = (ev, cfg);
        None
    }
}
```

The two required methods are required at compile time: a plugin that will not say what it is
and what page it shows does not build. Every event method — `pre_tool`, `post_tool`,
`session_start`, `prompt_submit`, `pre_compact`, `mcp_tools`, `proxy_filter` — has a no-op
default, so implement only the surfaces the manifest declares. The crate docs carry the table
of when the host asks for each.

`cx` is the host: `cx.estimate(text, Class::Code)`, `cx.record(&Measurement { .. })`,
`cx.log(..)`, `cx.plugin_config::<T>("<id>")` for your own `[plugins.<id>]` section, and
`cx.config::<T>("proxy")` for the rare host-wide value. Stored state is reached the same way —
`cx.put_archive(..)`, `cx.search_notes(..)`, `cx.symbol_defs(..)` — through the capability
traits `Ctx` derefs to. A plugin never touches the host's database directly; if a capability
is missing, it is added to the SDK deliberately, not worked around.

## 2. Inside this repository: wire it in (three one-liners)

- `Cargo.toml` → `[features]`: add `<id> = []` and append it to `default`.
- `src/plugins/mod.rs`: `#[cfg(feature = "<id>")] pub mod <id>;` and the matching
  `v.push(Box::new(<id>::MyPlugin));` in `all()`. Position = dispatch order.
- `src/config.rs` → `CATALOGUE`: `("<id>", default_on)` in the same position.

The `registry_matches_catalogue` test fails until all three agree. Import from
`rtok_plugin_sdk` directly — `crate::plugin` re-exports the same types for the host's own
dispatch code, but a plugin names the crate it implements.

## 3. Outside this repository

```toml
[dependencies]
rtok-plugin-sdk = "0.0.1"
```

Implement `Plugin` against the crate — three dependencies, no SQLite, no tree-sitter, no web
server. To run it, build a binary that depends on `rtok` and hands your plugin to the
registry: `Registry::from_plugins(vec![Box::new(Mine)], &Config::load()?)`. This repository
ships no third-party plugins and no adapters.

## 4. Documentation next to the code

- `src/plugins/<id>/PLAN.md` — D15 design note (copy `docs/plugin-plan-template.md`). Lands in
  the commit before the plugin's first implementation task. Required headings, ≥ 3 surveyed
  alternatives, one `Target:` and one `Falsified by:`. Checked by `cargo test plugin_plans`.
- `src/plugins/<id>/README.md` — what it does, what it replaces, surfaces, mechanism, config
  keys, plan tasks, status. For users.
- `src/plugins/<id>/AGENTS.md` — files it owns, invariants that must not break, allowed
  dependencies, the Checks. For whoever (human or model) edits it next.

Copy the structure from an existing plugin; keep README and AGENTS.md under a screen.

## 5. Rules every plugin obeys

1. **Fail open.** Return `None`/empty on any error. Never panic on the hook path; the
   dispatcher catches panics, but a panic is still a bug.
2. **Lossless.** If you shorten something, `cx.put_archive` the original first and put the id
   in `Measurement::ref_id` and in the text the model sees. It must be able to `expand` it.
3. **Measure.** Every action that changes what the model sees calls `cx.record(&Measurement)`
   with before/after bytes and estimated tokens. A saving without a row does not exist.
4. **Budget.** Anything for SessionStart/UserPromptSubmit is returned as an `Injection`, never
   written into `additionalContext` directly. Text must be byte-stable across turns.
5. **Speed.** Hook path ≤ 10 ms p95 in release. One indexed DB query is fine; a filesystem
   walk or a network call is not.
6. **No new dependency** without a one-line reason in the commit message, and only from the
   baseline list in `plan.md` §2.
7. **Don't duplicate.** Reuse an existing helper, or extract one shared helper at the layer
   that owns the behaviour.

## 6. Test

Unit tests live in the module. Out of tree, or for a plugin that needs no store, use
`rtok_plugin_sdk::testing::MemoryHost` — it holds what the plugin records and archives:

```rust
use rtok_plugin_sdk::testing::MemoryHost;
use rtok_plugin_sdk::Ctx;

#[test]
fn rewrites_recursive_ls() {
    let host = MemoryHost::new();
    let input = serde_json::json!({ "command": "ls -R src" });
    let ev = PreToolUse { tool_name: "Bash", tool_input: &input };
    assert!(MyPlugin.pre_tool(&ev, &Ctx::new(&host)).is_some());
    assert_eq!(host.recorded().len(), 1);
}
```

In this repository, `crate::plugin::Runtime::in_memory("test")` is the real host without a
file — use it when the assertion is about what actually landed in the store, and the fixtures
in `tests/fixtures/hooks/` for realistic events. Then the task's Check from `plan.md` (see
`roadmap.md` for order), then `just check`.

## 7. Ship

Commit `<task-id>: <title>` on `main` (no feature branch). Same commit: mark the task done and
move it from `plan.md` to `done.md` with the date and the Check output. Implemented work still
listed in `plan.md` is unfinished.
