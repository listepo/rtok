# Writing a plugin

A plugin is one module, one manifest, one feature flag, one line in the registry, one line in
the catalogue, two markdown files, and at least one test. The whole thing for a real plugin
fits in a single ≤ 200 LOC task. `examples/hello_plugin.rs` is the runnable reference.

## 0. Should it exist?

Check `plan.md` §1 (catalogue) and §0 (decisions), and `roadmap.md` for that plugin's task order. If the method is not there, add it to `ideas.md` (source tool + plugin; Later if it is v0.2+) and propose a plan
change first. Every plugin is written from scratch here: it must not shell out to, link, or
read the data of a third-party tool (D6). The tool it replaces is the spec; `research.md`
is the evidence.

## 1. Module

`src/plugins/<id>/mod.rs`:

```rust
//! `<id>` — one line on what it does and what it replaces.

use crate::plugin::{Ctx, Manifest, Plugin, PreToolDecision, PreToolUse, Surface};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn manifest(&self) -> Manifest {
        Manifest { id: "<id>", surfaces: &[Surface::Hook], default_on: true }
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        // Read config: cx.config.plugins.<id>.some_key   (typed section, T12.1)
        // Estimate:    cx.estimate(text, Class::Code)
        // Claim:       cx.record(&Measurement { .. }).ok()?
        None
    }
}
```

Implement only the trait methods your surfaces need; every method has a no-op default.

## 2. Wire it in (three one-liners)

- `Cargo.toml` → `[features]`: add `<id> = []` and append it to `default`.
- `src/plugins/mod.rs`: `#[cfg(feature = "<id>")] pub mod <id>;` and the matching
  `v.push(Box::new(<id>::MyPlugin));` in `all()`. Position = dispatch order.
- `src/config.rs` → `CATALOGUE`: `("<id>", default_on)` in the same position.

The `registry_matches_catalogue` test fails until all three agree.

## 2b. Outside this repo

A third-party plugin lives in its own crate: depend on the `rtok` library
(`use rtok::{Ctx, Manifest, Plugin, Surface};` — the contract is re-exported at the crate
root), implement `Plugin`, and build your own binary with
`Registry::from_plugins(vec![Box::new(Mine)], &Config::load()?)`. This repo ships no
third-party plugins and no adapters; `examples/` shows the two minimal shapes (a hook plugin
and an MCP-tool plugin).

## 3. Documentation next to the code

- `src/plugins/<id>/PLAN.md` — D15 design note (copy `docs/plugin-plan-template.md`). Lands
  in the commit before the plugin's first implementation task. Required headings, ≥ 3
  surveyed alternatives, one `Target:` and one `Falsified by:`. Checked by `cargo test plugin_plans`.
- `src/plugins/<id>/README.md` — what it does, what it replaces, surfaces, mechanism, config
  keys, plan tasks, status. For users.
- `src/plugins/<id>/AGENTS.md` — files it owns, invariants that must not break, allowed
  dependencies, the Checks. For whoever (human or model) edits it next.

Copy the structure from an existing plugin; keep README and AGENTS.md under a screen.

## 4. Rules every plugin obeys

1. **Fail open.** Return `None`/empty on any error. Never panic on the hook path; the
   dispatcher catches panics, but a panic is still a bug.
2. **Lossless.** If you shorten something, archive the original first and put the archive id
   in the output. The model must be able to `expand` it.
3. **Measure.** Every action that changes what the model sees calls `cx.record(&Measurement)`
   with before/after bytes and estimated tokens. A saving without a row does not exist.
4. **Budget.** Anything for SessionStart/UserPromptSubmit goes through `inject` as an
   `Injection`, never directly into `additionalContext`. Text must be byte-stable across turns.
5. **Speed.** Hook path ≤ 10 ms p95 in release. One indexed DB query is fine; a filesystem
   walk or a network call is not.
6. **No new dependency** without a one-line reason in the commit message, and only from the
   baseline list in `plan.md` §2.
7. **Don't duplicate.** Reuse an existing helper, or extract one shared helper at the
   layer that owns the behaviour.

## 5. Test

Put unit tests in the module. Use `Ctx::in_memory("test")` for a real store without a file,
and the fixtures in `tests/fixtures/hooks/` for realistic events:

```rust
#[test]
fn denies_rm_rf() {
    let cx = Ctx::in_memory("t").unwrap();
    let input: HookInput = serde_json::from_str(include_str!("../../../tests/fixtures/hooks/pre_tool_bash.json")).unwrap();
    let ev = input.pre_tool().unwrap();
    assert!(MyPlugin.pre_tool(&ev, &cx).is_none());
}
```

Then the task's Check from `plan.md` (see `roadmap.md` for order), then `make check`.

## 6. Ship

Commit `<task-id>: <title>` on `main` (no feature branch). Same commit: mark the task done
and move it from `plan.md` to `done.md` with the date and the Check output. Implemented
work still listed in `plan.md` is unfinished.
