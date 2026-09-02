---
title: Getting started
weight: 1
---

## Install

The Rust toolchain is pinned in `mise.toml`; never install or switch a global toolchain.

```bash
git clone https://github.com/listepo/rtok && cd rtok
mise install                          # Rust 1.97.1, pinned in mise.toml
mise exec -- cargo install --path .
rtok --version                        # rtok 0.1.0
```

## First run

```bash
rtok config init      # writes ~/.rtok/config.toml
rtok plugins          # id, enabled, surfaces
rtok doctor           # inspect hooks, MCP servers, proxy chain
```

`rtok config init` writes the embedded default config verbatim. Set `RTOK_HOME` to move the
whole directory (database, archive, config) somewhere else.

{{< callout type="info" >}}
Unimplemented subcommands print `not implemented` and exit 0, so a half-installed rtok never
blocks the host agent.
{{< /callout >}}

## Wire it into Claude Code

```bash
rtok setup claude     # installs hooks / MCP / proxy, with backups
```

This writes the hook entries, the MCP server registration, and the proxy environment into
your Claude Code settings, backing up whatever was there before.

## The four invariants

Everything in rtok is built to hold these, and they are the reason to trust it in front of
your agent:

1. **Fail open.** A hook exits 0 in ≤ 10 ms even on error, with unmodified input.
2. **Lossless by default.** Anything shortened is retrievable via `rtok expand <id>`; the
   original stays on disk under `~/.rtok/archive/`.
3. **A saving that is not a `Measurement` row does not exist.**
4. **Injected context stays under budget and byte-stable across turns**, so it never
   invalidates the prompt cache.

A fifth constrains the hook surface: PostToolUse can only add context, it can never change a
tool result.

## Where things live

```text
~/.rtok/config.toml     configuration (RTOK_HOME overrides the directory)
~/.rtok/rtok.db         SQLite, WAL + FTS5 — measurements, archive index, memory
~/.rtok/archive/        raw payloads, addressed by expand id
<git root>/.rtok.toml   optional per-project overrides
```

## Caveats

- Token counts from `rtok stats` are estimates (±15 %) plus real `usage` rows from the proxy.
- No measured saving exists in the repo yet. The first number lands in P1.

## Next

{{< cards >}}
  {{< card link="../commands" title="Commands" subtitle="What each subcommand does and its status." >}}
  {{< card link="../reference/configuration" title="Configuration" subtitle="Precedence rules and the full key reference." >}}
{{< /cards >}}
