---
title: Plugins
weight: 3
---

Every token-reduction method in rtok is a plugin behind one trait. Plugins are in-tree
modules behind Cargo features — no daemon, no subprocesses, no WASM in v0.1.

The **replaces** column is a specification target, never a dependency: rtok reimplements the
behaviour from scratch and never runs, links, or reads the tool named.

| Plugin | Replaces (spec only) | Surface |
|--------|----------------------|---------|
| [`measure`](measure) | rtk gain, headroom savings, lean-ctx gain | `stats`, `bench`, proxy |
| [`cmd`](cmd) | rtk hook, ctx_shell, bash_compress | PreToolUse(Bash) → `rtok run` |
| [`read`](read) | lean-ctx read/search/tree, read_cache | MCP `read` |
| [`archive`](archive) | CCR | `expand`, store |
| [`proxy`](proxy) | caveman-proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` |
| [`inject`](inject) | caveman/ponytail, lean-ctx | SessionStart, UserPromptSubmit |
| [`guard`](guard) | — | PreToolUse |
| [`memory`](memory) | claude-mem | MCP, PreCompact |
| [`graph`](graph) | codebase-memory-mcp | MCP |
| [`toon`](toon) | — | MCP |

## Turning plugins off

At runtime, in `~/.rtok/config.toml`:

```toml
[plugins.cmd]
enabled = false
```

Or at build time, so the code is not compiled in at all:

```bash
cargo build --no-default-features --features cmd,read
```

## Each plugin directory

| File | Holds |
|------|-------|
| `README.md` | what the plugin does and why — the pages below |
| `AGENTS.md` | invariants and task rules for whoever works on it |
| `PLAN.md` | the plugin's own build plan |

Writing your own: [plugin authoring](../reference/plugin-authoring).
