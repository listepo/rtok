# `rtok-plugin-sdk` — where the crate line goes (T23.0, D15, D25)

Design note for the published plugin contract. No code lands from this document; T23.1 does.

## Problem

D6 says third parties extend rtok from outside, through `rtok::plugin` and
`Registry::from_plugins`. Today that costs them the whole binary crate. Measured on this tree 2026-09-09
(`awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-z]/' Cargo.toml | wc -l` → 35 entries, 24
of them not optional): **24 direct dependencies always, 33 in a default build** — the default
feature set pulls 9 of the 11 optional ones, and only `lbug` and `watchman_client` stay out. diesel with a bundled SQLite C library, axum,
tokio, reqwest, rmcp, clap, figment, seven tree-sitter grammars. A crate that wants to implement
one trait with two methods compiles all of it, and takes rtok's MSRV, its feature flags and its
release cadence with it.

The second half of the problem is that the contract has no edge. `Plugin` takes `&Ctx`, `Ctx`
exposes `pub store: Store`, and `Store` is 1 192 lines of whatever the last task needed. Measured
the same day (`grep -rhoE "cx\.store\.[a-z_0-9]+" src/plugins/ | sort -u`): the ten plugins call
**26 distinct `Store` methods across 5 areas** — symbols 11 (graph), ledger 6 (archive, guard,
inject, read, graph), archive 3, read cache 3, notes 4 (memory). Nothing says which of the other
~40 `Store` methods a plugin may call; nothing breaks if one starts. That is not an API, it is a
habit, and it cannot be versioned.

Sizes that price the options below, `wc -l` on 2026-09-09: `src/store/` 2 219 (+ 387 for the
optional lbug backend), `src/config/` 1 631, `src/tokens.rs` 112, `src/proxy/wire.rs` 150.

## Alternatives

How three projects outside this stack drew the same line between "the host" and "a plugin".
Versions and dates from the crates.io API, looked up 2026-09-09.

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| `bevy_app::Plugin` | 0.19.1 | 2026-08-13 | One required method (`build(&self, app: &mut App)`), everything else defaulted; the contract crate is small enough to depend on alone rather than pulling the whole engine. | The contract crate *is* the runtime type: `App` lives there, so its 17 direct dependencies are the plugin author's, and every change to the runtime is a change to the published contract. |
| `tower-layer` / `tower-service` | 0.3.3 | 2024-08-13 | **0 direct dependencies each.** The trait is its own crate, so a middleware author compiles nothing but the trait, and the traits have been stable for years precisely because they carry nothing. | The traits are so abstract they grant no capability — every useful thing lives in `tower` (utilities) or the host, so an author reads two crates to write one thing. A pure-contract SDK here would land in the same place: `Plugin` alone cannot record a `Measurement`. |
| `nu-plugin` + `nu-protocol` | 0.115.1 | 2026-08-23 | A process boundary means the host cannot leak types at all; the contract is a wire protocol and is therefore explicit by construction. | `nu-protocol` carries 38 direct dependencies and `nu-plugin` 8; a call costs a process hop. D1 gives the whole hook 10 ms, so a hop per dispatch is out before the ergonomics are discussed. |

The lesson the three agree on: the contract crate must carry *capabilities*, not the runtime that
implements them (bevy's mistake) and not nothing at all (tower's), and it must do it in-process
(nu's cost).

## Mechanism

**Chosen: the middle line (option C).** `rtok-plugin-sdk` carries the trait, the events, the
value types and *traits describing what the host can do for a plugin*. It carries no storage
engine, no config schema and no surface. Its dependency list is **`serde`, `serde_json`,
`anyhow`** — three, none of them C.

What crosses the line:

- **What a plugin is**: `Plugin`, `Manifest`, `Surface`.
- **Events, borrowed**: `PreToolUse`, `PostToolUse`, `SessionStart`, `PromptSubmit`, `PreCompact`,
  and the proxy request view.
- **What a plugin returns**: `PreToolDecision`, `Injection`, `ToolDef`, `DashboardPage`,
  `Measurement`.
- **What a plugin may ask of the host**: `Host` (estimate, record a `Measurement`, record a call,
  record tokens, log, read its own config section) plus one trait per store area a plugin actually
  uses — `Archive`, `Notes`, `ReadCache`, `Symbols`, `Ledger`. Each is a named list, so "what a
  plugin may touch" is a document, not a habit.

What does not cross: `Store` and its schema, `Config` and its 100-odd keys, `rtok hook` / `mcp` /
`proxy` / `web`. `rtok` implements the traits on `Ctx` and stays the only dispatcher.

The 26 methods map onto the five capability traits one for one; no method becomes public that no
plugin calls today, and adding one later is a semver-minor addition with a default body where that
makes sense.

**Config.** A plugin reads its own section, `[plugins.<id>]`, through
`Host::plugin_config::<T>() -> Result<T>` with `T: serde::de::DeserializeOwned` — the plugin owns
the struct, which is what lets an out-of-tree plugin have settings at all. Core values a plugin
legitimately needs (the archive directory) are named accessors. The estimator stays behind
`Host::estimate(text, class)` because its rates are config the host owns (D12).

**Required methods.** `manifest()` — a plugin that will not say its id, surfaces and default state
cannot be registered. `dashboard_page()` — D23 says every plugin has a page on both operator
surfaces, and today the copy for all ten lives in one `DashboardPage::from_id` match in the host,
which is exactly backwards: an out-of-tree plugin falls through it to "External plugin". Making it
required moves each plugin's own description to the plugin. Everything else keeps a no-op default,
so a plugin still implements only the surfaces its `Manifest` declares.

**Versioning.** `cargo-semver-checks` in CI on the SDK only; the `rtok` binary crate stays off
crates.io (dist ships it), so the published surface is exactly this crate.

Target: `rtok stats --json` byte-identical before and after

## Rejected

- **(A) The runtime moves into the SDK** — `Config`, `Store`, `tokens` and the wire views go with
  the trait, `rtok` becomes surfaces on top. Every internal plugin would compile unchanged, which
  is its only real merit. It publishes 4 112 lines of host internals as public API: every `Store`
  method and every config key becomes a semver promise, and a plugin author compiles diesel plus a
  bundled SQLite to implement a trait. This is bevy's line, and bevy pays for it with 17
  dependencies on the contract crate.
- **(B) Contract only, no capabilities** — the tower line: publish `Plugin` and the event types and
  nothing else. Zero dependencies and a surface that never breaks, but a plugin cannot record a
  `Measurement`, which makes it useless under D3: a saving that is not a `Measurement` row does not
  exist. Every plugin would reach back into `rtok` for the one thing it must do, and the crate line
  would be decorative.
- **(C′) Out-of-process plugins** — nu's line, and genuinely the only way to make the boundary
  airtight. D1 gives a hook 10 ms end to end; a process hop per dispatch spends most of that
  budget. Kept where it belongs: the v0.2+ WASM plugin host, which is the same idea without the
  process.

Falsified by: if the first two plugin tasks after the migration each need a new method on a
capability trait, the traits are tracking `Store` rather than describing a contract — and the
honest fix is then option A, publishing the store as the API and paying its cost openly.
