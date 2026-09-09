<picture>
  <source media="(prefers-color-scheme: dark)" srcset="site/static/logo-wordmark-dark.svg">
  <img src="site/static/logo-wordmark.svg" alt="rtok" width="200">
</picture>

`rtok` reduces the context that AI coding agents must carry. It is one Rust binary with
three surfaces: Claude Code hooks, an MCP server, and an API proxy. Each reduction is
measured, and shortened payloads stay retrievable by id.

Ten methods, one process, one ledger. A saving that is not a row in that ledger does not
exist — including rtok's own.

## Install

macOS on Apple silicon or Intel, and Linux x86-64. The script is POSIX `sh`, so it behaves the
same whether your shell is bash or zsh; it puts `rtok` in `~/.cargo/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/listepo/rtok/releases/latest/download/rtok-installer.sh | sh
```

Or with [ketch](https://github.com/listepo/ketch) — it installs straight from these
GitHub Release archives, no tap or formula:

```bash
curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash
ketch install listepo/rtok
```

The dist installer also gives you `rtok-update`; run it to move to the newest release.

The binaries are not codesigned or notarised. Downloading an archive through a browser will make
macOS quarantine it; the installer above and ketch are not affected. What turning that on would
involve is written down in [docs/release.md](docs/release.md).

Building from source works on any platform Rust supports:

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
rtok agent setup claude --dry-run
rtok agent setup claude
rtok doctor
```

`--dry-run` prints exactly what it would write and touches nothing:

```text
+ PreToolUse Bash rtok hook PreToolUse
+ PreToolUse Read rtok hook PreToolUse
+ PostToolUse * rtok hook PostToolUse
+ UserPromptSubmit rtok hook UserPromptSubmit
+ SessionStart rtok hook SessionStart
+ PreCompact rtok hook PreCompact
+ PostCompact rtok hook PostCompact
7 additions
```

`rtok agent remove claude` takes it all back out: hook entries, the MCP registration and the proxy variable. Both commands copy every file they touch to `<name>.bak-<ts>` beside it first.

Run the proxy separately when you want provider usage rows and archive compression:

```bash
rtok proxy --mode passthrough
# In another shell, point the host at http://127.0.0.1:8790.
```

## Examples

### Price the stack you already have

`rtok doctor` reads your host's settings and reports what every installed tool costs per
turn — before you install anything of rtok's. Abridged real output:

```text
hooks 88
  PreToolUse 14
  PostToolUse 12
  SessionStart 14
  UserPromptSubmit 9
  …
mcp
  code-review-graph (30 tools, ~2295 desc tokens)
  serena (22 tools, ~1494 desc tokens)
  lean-ctx (12 tools, ~697 desc tokens)
  rtok (11 tools, ~143 desc tokens)
proxy 8788→8787
mcp_tool_search likely disabled (ANTHROPIC_BASE_URL is set)
autoCompactWindow 300000
```

Those description tokens are re-sent on every request of every session. That is the number
most tools do not count against themselves.

### Run a command without paying for its output

```bash
rtok run -- cargo test
```

`rtok run` executes the command through your shell, keeps the exit code, writes the raw
output to `~/.rtok/archive/<id>`, and prints a per-family summary. Anything over 40 lines
gets a trailer naming its id:

```text
[rtok 7f3a91 · 412 lines · expand: rtok expand 7f3a91]
```

Nothing is redacted, a non-zero exit prints the last 80 lines verbatim, and the original is
one command away:

```bash
rtok expand 7f3a91                    # the whole thing
rtok expand 7f3a91 --lines 120-180    # just that range
rtok expand 7f3a91 --grep "panicked"  # just the matches
```

With the hooks installed this happens on its own: `PreToolUse` rewrites the agent's Bash
call to `rtok run -- <command>`.

### Ask the code graph instead of grepping

```bash
rtok graph index .
```

```text
indexed 5 files · 551 rows · 0 skipped · 5 read
```

The agent then reaches it over MCP as `symbol`, `callers`, `impact` and `outline` — four
tools whose descriptions cost 62 tokens, in place of a grep-and-read chain. Definition
lookups are exact (recall and precision 1.000 over a hand-labelled set); reference lookups
find about a third of the sites, which [docs/comparison.md](docs/comparison.md) explains
rather than hides.

### See what is on and where it runs

```bash
rtok plugins
```

```text
id       enabled  surfaces
measure  on       cli,proxy
cmd      on       hook,cli
read     on       mcp,hook
archive  on       proxy,mcp
proxy    on       proxy
inject   on       hook
guard    on       hook
memory   on       mcp,hook
graph    on       mcp
toon     off      proxy,mcp
```

Turn one off with `rtok config set plugins.cmd.enabled false`, or turn `toon` on with
`rtok config set plugins.toon.enabled true` — same thing as a `[plugins.<id>]` table in the
config file.

### Find out where a setting came from

Every flag is a config key, and every key knows its layer:

```bash
rtok config show --sources
```

```text
core.db_path = ~/.rtok/rtok.db (user)
core.log_level = warn (user)
dashboard.port = 3333 (user)
estimator.code = 3.5 (user)
…
```

`rtok proxy --port 8791` and `[proxy] port = 8791` are the same setting reached two ways;
`--sources` says which one won.

### Send the ledger to your observability stack

```bash
rtok otel status
```

```text
endpoint: none (set [otel] endpoint or OTEL_EXPORTER_OTLP_ENDPOINT)
calls: mark 0 · 0 pending
logs: mark 0 · 0 pending
sessions: mark 0
```

Point `[otel] endpoint` at Jaeger, Grafana, SigNoz or Maple and `rtok otel flush` posts
calls, logs and metrics as OTLP/HTTP JSON — see [`docs/otel.md`](docs/otel.md).

## Measure before keeping a reduction

`rtok stats` reads transcript estimates and proxy usage. The proxy's provider-reported
usage is ground truth; transcript estimates are useful directionally but have a ±15% error
margin.

```bash
rtok stats --since 7d
rtok stats --save-baseline before-rtok
rtok stats --compare before-rtok
```

A fresh install has nothing to report yet, and says so rather than inventing a number:

```text
sessions 0  lines 0  malformed 0
usage input=0 cache_create=0 cache_read=0 output=0  hit=0.0%  median_context=0
```

## Commands

| Command | Purpose |
|---|---|
| `rtok agent setup claude` | install Claude Code hooks and MCP registration (`--dry-run`) |
| `rtok agent remove claude` | take hooks, MCP registration and proxy variable back out (`--dry-run`) |
| `rtok agent setup cursor` / `codex` / `opencode` / `pi` | register the other supported host integrations |
| `rtok hook <event>` | hook entry point (JSON on stdin, JSON on stdout) |
| `rtok mcp` | serve read, memory, graph, and expansion tools over stdio |
| `rtok proxy` | capture API usage; optionally archive older tool results |
| `rtok dashboard` | local Slint/WASM UI + WebSocket API (`--host`, `--port`) |
| `rtok stats` | report transcript and proxy measurements |
| `rtok bench` | run the fixed A/B schedule |
| `rtok doctor` | inspect hooks, MCP servers and the proxy chain |
| `rtok run -- <cmd>` | run, archive, and format a command result |
| `rtok filter --stdin` | filter a payload without executing it (OpenCode) |
| `rtok expand <id>` | retrieve an archived original (`--lines`, `--grep`) |
| `rtok plugins` | list plugins: id, enabled, surfaces |
| `rtok config show --sources` | show effective configuration and its source |
| `rtok graph index [path]` | build the symbol index for a tree |
| `rtok memory import <file>` | import notes as JSONL |
| `rtok otel flush` / `status` | export the ledgers over OTLP, or report the watermarks |

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

## How it compares

Ten tools that each shrink one slice of the context, stacked on one machine, produce a stack
nobody measures end to end. The author's machine before rtok: 88 hook entries, nine MCP
servers, ~8 600 tool-description tokens on every turn, two chained proxies — and every one of
those tools reported a saving while the bill did not move.

| | The field | rtok |
|---|---|---|
| Processes per tool call | up to ~30 subprocesses, several Python | one Rust process, p95 8.25 ms |
| Hook entries | 88 across 16 events | 7 |
| MCP description tokens/turn | ~8 600 across nine servers | ~143 across 11 tools |
| Injection per turn | 3.1 K and up, per tool | one 800-token budget, byte-stable |
| Reversibility | partial | every rewrite has `rtok expand <id>` |
| Measurement | five meters, none of them the bill | one ledger + provider `usage` |
| Failure mode | varies | fail open: exit 0, unmodified input, ≤ 10 ms |

The other side of it: rtok has no live end-to-end cost win yet, its reference lookups find
0.351 of the sites where serena's LSP finds all of them, and it has no LLM compression or
embeddings. Tool-by-tool detail, evidence for every number, and the cases where you should
use something else: **[docs/comparison.md](docs/comparison.md)**.

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
rtok agent setup claude --dry-run
rtok stats --since 1h
rtok plugins
rtok proxy --dry-run
rtok otel status
```

Read [`plan.md`](plan.md) for the current implementation plan, [`done.md`](done.md) for
completed tasks, and [`docs/plugin-authoring.md`](docs/plugin-authoring.md) to build an
external plugin.
