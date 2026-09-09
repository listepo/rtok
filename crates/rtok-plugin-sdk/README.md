# rtok-plugin-sdk

The plugin contract for [rtok](https://github.com/listepo/rtok) — one Rust binary that
reduces the tokens an AI coding agent spends. Every method rtok uses is a plugin, and this
crate is the trait they all implement: the ten that ship inside rtok and any written
elsewhere.

Depending on this crate instead of the `rtok` binary means three dependencies
(`serde`, `serde_json`, `anyhow`) rather than SQLite, tree-sitter, axum and reqwest.

```toml
[dependencies]
rtok-plugin-sdk = "0.0.1"
```

```rust
use rtok_plugin_sdk::{DashboardPage, Manifest, Plugin, PreToolDecision, PreToolUse, Ctx, Surface};

struct Terse;

impl Plugin for Terse {
    fn manifest(&self) -> Manifest {
        Manifest { id: "terse", surfaces: &[Surface::Hook], default_on: true }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new("Terse", "Drops `-l` from `ls`.", true)
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        let cmd = ev.tool_input.get("command")?.as_str()?;
        cmd.starts_with("ls -l").then(|| PreToolDecision::Rewrite {
            input: serde_json::json!({ "command": cmd.replacen("ls -l", "ls", 1) }),
            reason: "terse: long listing is rarely what was wanted".into(),
        })
    }
}
```

`manifest` and `dashboard_page` are required — a plugin that will not say what it is and what
page it shows does not compile. Every event method has a no-op default, so a plugin writes
only the surfaces its manifest declares.

## What a plugin may do

`Ctx` is what the host hands you: estimate what text costs, record a `Measurement`, log, read
your own `[plugins.<id>]` section. Beyond that it derefs to the capability traits — `Archive`,
`Notes`, `ReadCache`, `Ledger`, `Symbols` — so `cx.put_archive(..)` and `cx.symbol_defs(..)`
are reachable without importing anything. A plugin never sees the host's database.

## The three rules

1. **Fail open.** A hook exits 0 in ≤ 10 ms even on error, with unmodified input.
2. **Lossless by default.** Archive the original before you shorten it and quote the id.
3. **A saving that is not a `Measurement` row does not exist.**

## Testing a plugin

`testing::MemoryHost` is a host with no database: it keeps what a plugin records and
archives, and answers every other capability empty — which a real host may also do, because
caches go cold and indexes are not always built.

```rust
use rtok_plugin_sdk::testing::MemoryHost;
let host = MemoryHost::new();
// let cx = Ctx::new(&host);  →  call your plugin, then assert on host.recorded()
```

`cargo run -p rtok-plugin-sdk --example shrink` runs a complete plugin against it.

## Documentation

- API: [docs.rs/rtok-plugin-sdk](https://docs.rs/rtok-plugin-sdk)
- Writing and shipping a plugin: [`docs/plugin-authoring.md`](https://github.com/listepo/rtok/blob/main/docs/plugin-authoring.md)
- What rtok is: <https://listepo.github.io/rtok/>

## Licence

Apache-2.0.
