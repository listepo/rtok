---
title: Getting started
weight: 1
---

## Install

macOS (Apple silicon or Intel) and Linux x86-64 have prebuilt binaries. The script is POSIX
`sh`, and it puts `rtok` — plus `rtok-update` — in `~/.cargo/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/listepo/rtok/releases/latest/download/rtok-installer.sh | sh
```

Or with [ketch](https://github.com/listepo/ketch), straight from the same release archives:

```bash
ketch install listepo/rtok
```

Building from source works anywhere Rust does. The toolchain is pinned in `mise.toml`; never
install or switch a global toolchain.

```bash
git clone https://github.com/listepo/rtok && cd rtok
mise install
mise exec -- cargo install --path .
rtok --version
```

## First run

```bash
rtok config init      # writes ~/.rtok/config.toml
rtok plugins          # id, enabled, surfaces
rtok doctor           # inspect hooks, MCP servers, proxy chain
```

`rtok config init` writes the embedded default config verbatim. Set `RTOK_HOME` to move the
whole directory (database, archive, config) somewhere else.

`rtok doctor` is worth running *before* you install anything: it prices the hooks and MCP
servers you already have, including the description tokens each one costs on every turn.

{{< callout type="info" >}}
A subcommand whose plugin was compiled out prints `not implemented` and exits 0, so a
stripped or half-installed rtok never blocks the host agent.
{{< /callout >}}

## Wire it into Claude Code

```bash
rtok agent setup claude --dry-run   # print the seven hook entries it would write
rtok agent setup claude             # installs hooks / MCP / proxy, with backups
rtok agent remove claude      # takes hooks, MCP and the proxy variable back out
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
  Only the proxy rows are the actual bill.
- The committed A/B bench has only been run offline, so it reports zeros for both
  configurations. Rerun it against live traffic before adopting a configuration on its word.

## Next

{{< cards >}}
  {{< card link="../commands" title="Commands" subtitle="What each subcommand does." >}}
  {{< card link="../reference/configuration" title="Configuration" subtitle="Precedence rules and the full key reference." >}}
  {{< card link="../reference/comparison" title="Comparison" subtitle="How rtok differs from the tools it replaces — and where it is behind." >}}
{{< /cards >}}
