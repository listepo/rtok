# Getting started with rtok

`rtok` reduces the context AI coding agents must carry. One Rust binary, three
surfaces: Claude Code hooks, an MCP server, and an API proxy. Each reduction is
measured; shortened payloads stay retrievable by id.

## Install

macOS (Apple silicon or Intel) and Linux x86-64 have prebuilt binaries. The
installer is POSIX `sh` and puts `rtok` (plus `rtok-update`) in `~/.cargo/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/listepo/rtok/releases/latest/download/rtok-installer.sh | sh
```

Or with [ketch](https://github.com/listepo/ketch), from the same release archives:

```bash
ketch install listepo/rtok
```

From source (toolchain pinned in `mise.toml`):

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

`rtok doctor` is worth running *before* you install anything: it prices the
hooks and MCP servers you already have, including description tokens re-sent
on every turn.

A subcommand whose plugin was compiled out prints `not implemented` and exits
0, so a stripped or half-installed rtok never blocks the host agent.

## Wire into Claude Code

```bash
rtok agents install claude --dry-run
rtok agents install claude
rtok agents uninstall claude
```

`--dry-run` prints the hook entries and touches nothing. Install backs up every
file it writes. See the root [README](../README.md) for other hosts
(`cursor`, `codex`, `opencode`, …).

## Four invariants

1. **Fail open.** A hook exits 0 in ≤ 10 ms even on error, with unmodified input.
2. **Lossless by default.** Anything shortened is retrievable via `rtok expand <id>`.
3. **A saving that is not a `Measurement` row does not exist.**
4. **Injected context stays under budget and byte-stable**, so it never busts the prompt cache.

## Where things live

```text
~/.rtok/config.toml     configuration (`RTOK_HOME` overrides the directory)
~/.rtok/rtok.db         SQLite — measurements, archive index, memory
~/.rtok/archive/        raw payloads, addressed by expand id
<git root>/.rtok.toml   optional per-project overrides
```

## Caveats

- Token counts from `rtok stats` are estimates (see `src/tokens.rs`) plus real `usage` rows
  from the proxy. Only the proxy rows are the actual bill.
- The committed A/B bench has only been run offline, so it reports zeros for
  both configurations. Rerun it against live traffic before adopting a
  configuration on its word.

## Next

- [config.md](config.md) — full key reference and precedence
- [comparison.md](comparison.md) — against the tools rtok replaces (and where it is behind)
- [otel.md](otel.md) — OTLP export of the ledger
- [plugin-authoring.md](plugin-authoring.md) — external plugins
- [agents.md](agents.md) — host × module matrix
