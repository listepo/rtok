# rtok

`rtok` reduces the context that AI coding agents must carry. It is one Rust binary with
three surfaces: Claude Code hooks, an MCP server, and an API proxy. Each reduction is
measured, and shortened payloads stay retrievable by id.

## Install

Install the Rust version pinned by this repository, then build the binary:

```bash
git clone https://github.com/listepo/rtok && cd rtok
mise install
mise exec -- cargo install --path .
rtok --version
```

## Start with Claude Code

Install rtok's seven hooks and MCP entry. The installer backs up the settings file before
writing it; inspect its changes first if preferred.

```bash
rtok setup claude --dry-run
rtok setup claude
rtok doctor
```

Run the proxy separately when you want provider usage rows and archive compression:

```bash
rtok proxy --mode passthrough
# In another shell, point the host at http://127.0.0.1:8790.
```

## Measure before keeping a reduction

`rtok stats` reads transcript estimates and proxy usage. The proxy's provider-reported
usage is ground truth; transcript estimates are useful directionally but have a ±15% error
margin.

```bash
rtok stats --since 7d
rtok stats --save-baseline before-rtok
rtok stats --compare before-rtok
```

## Commands

| Command | Purpose |
|---|---|
| `rtok setup claude` | install Claude Code hooks and MCP registration |
| `rtok setup cursor` / `rtok setup codex` | register the supported host integrations |
| `rtok hook <event>` | hook entry point (JSON on stdin, JSON on stdout) |
| `rtok mcp` | serve read, memory, graph, and expansion tools over stdio |
| `rtok proxy` | capture API usage; optionally archive older tool results |
| `rtok stats` | report transcript and proxy measurements |
| `rtok bench` | run the fixed A/B schedule |
| `rtok run -- <cmd>` | run, archive, and format a command result |
| `rtok expand <id>` | retrieve an archived original |
| `rtok config show --sources` | show effective configuration and its source |

## Plugins

| Plugin | Surface | What it does |
|---|---|---|
| `measure` | stats, bench, proxy | records before/after tokens and provider usage |
| `cmd` | PreToolUse Bash | wraps commands, formats output, and archives originals |
| `read` | MCP, PreToolUse Read | provides bounded reads, searches, trees, and re-read deduplication |
| `archive` | proxy, expand | replaces eligible old tool results with stable archive pointers |
| `proxy` | API proxy | passes traffic through and captures usage |
| `inject` | SessionStart, UserPromptSubmit | emits byte-stable context within a token budget |
| `guard` | PreToolUse | prevents repeated reads and commands within a turn window |
| `memory` | MCP, PreCompact | stores agent-written notes with progressive disclosure |
| `graph` | MCP | indexes symbols and references with bounded responses |
| `toon` | proxy, MCP | optionally encodes tabular JSON; disabled by default |

Plugin details and configuration live in [`docs/config.md`](docs/config.md),
[`architecture.md`](architecture.md), and each `src/plugins/<id>/README.md`. Every call rtok
records can be exported as OpenTelemetry traces, logs and metrics to Jaeger, Grafana, SigNoz or
Maple — see [`docs/otel.md`](docs/otel.md).

## Measured results

The committed T9.2 A/B harness schedules six tasks × three runs. Its first run was offline
(`RTOK_BENCH_LIVE` was unset), so both configurations have zero provider usage and equal
task checks. This is a reproducible baseline, **not** evidence of a bill reduction; rerun it
with live traffic before adopting a configuration.

| config | mean input | mean cache | mean output | mean cost USD | passed |
|---|---:|---:|---:|---:|---:|
| A — legacy baseline | 0 | 0 | 0 | 0.0000 | 6/6 |
| B — rtok | 0 | 0 | 0 | 0.0000 | 6/6 |
| delta (B − A) | 0 | 0 | 0 | 0.0000 | 0 |

Sources: [`bench/results/a.json`](bench/results/a.json),
[`bench/results/b.json`](bench/results/b.json), and [`research.md`](research.md).

## Guarantees and caveats

- Hooks fail open: errors return unmodified input and exit successfully.
- Archive and proxy reductions are lossless: `rtok expand <id>` retrieves the original
  payload. A regenerable command result may instead be re-run.
- A token saving only counts when a `Measurement` row records it.
- Proxy compression preserves the cached prefix and never rewrites system instructions,
  tool definitions, or the newest tool-result turns.
- Estimates are ±15% until matched with provider usage. No live A/B cost reduction has been
  established yet.

## Development

```bash
just check
just example
just readme-check
just dist-plan
```

The README smoke examples below are executed by `just readme-check`.

```bash
# check
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
rtok() {
  RTOK_HOME="$tmp/home" \
  RTOK_STATS_TRANSCRIPTS_DIR="$tmp/transcripts" \
  RTOK_SETUP_CLAUDE_SETTINGS_PATH="$tmp/settings.json" \
  mise exec -- cargo run -q -- "$@"
}
rtok --version
rtok config init
rtok config validate
rtok setup claude --dry-run
rtok stats --since 1h
```

Read [`plan.md`](plan.md) for the current implementation plan, [`done.md`](done.md) for
completed tasks, and [`docs/plugin-authoring.md`](docs/plugin-authoring.md) to build an
external plugin.
