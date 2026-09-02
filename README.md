# rtok

Token reduction for AI coding agents, in one Rust binary. Every method is a plugin; every
saving is measured; everything shortened can be expanded back.

> Status: **P0 scaffold done** (config, store, plugin registry, estimator, hook types, CI).
> No token is saved yet — by design, measurement comes first (`plan.md` D3). See `done.md`
> for what is finished, `plan.md` for what is next, `roadmap.md` for each plugin's build plan, and `ideas.md` for tool-inspired propositions (not yet tasks).

## Why

A typical Claude Code setup stacks several tools that each compress Bash output, each cache
reads, each inject context at session start, and each claim 60–95 % savings nobody measures
end to end. Measured, the stack saves 3–40 % and costs ~3 K injected tokens per turn
(`research.md`). rtok replaces the stack with one binary that:

- runs as **one hook per event** in < 10 ms and fails open,
- serves **five MCP tools** instead of seventy-eight,
- sits as **one proxy hop** for both the Anthropic and the OpenAI API (`ANTHROPIC_BASE_URL`
  / `OPENAI_BASE_URL`: Claude Code, Codex, OpenCode, aider) that records real `usage` and can
  shrink old tool results without breaking the prompt cache,
- keeps **one measurement table** so `rtok stats` can say what actually changed.

## Install

```bash
git clone https://github.com/listepo/rtok && cd rtok
mise install                # Rust 1.97.1, pinned in mise.toml
mise exec -- cargo install --path .
rtok --version              # rtok 0.1.0
```

## Commands

| Command | Does | Status |
|---------|------|--------|
| `rtok plugins` | list plugins: id, enabled, surfaces | done |
| `rtok config show\|init\|validate\|get\|set\|path` | the one config file; `show --sources` says where each value came from | P12 |
| `rtok hook <event>` | Claude Code hook entry point (stdin JSON → stdout JSON) | T2.1 |
| `rtok mcp` | MCP server over stdio: `read`, `search`, `tree`, `expand`, `mem_*`, graph tools | T4.1 |
| `rtok proxy` | `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` hop: usage capture, optional compress mode | T5.1, P11 |
| `rtok run -- <cmd>` | run a command, archive raw output, print the filtered version | T3.1 |
| `rtok expand <id>` | print an archived payload | T3.5 |
| `rtok stats` | measurements from session logs and the proxy | T1.2 |
| `rtok doctor` | inspect hooks, MCP servers, proxy chain | T1.4 |
| `rtok setup claude` | install hooks / MCP / proxy into Claude Code, with backups | T2.3 |
| `rtok bench` | A/B two host configurations on fixed tasks | T9.1 |

Unimplemented subcommands print `not implemented` and exit 0, so a half-installed rtok never
blocks the host.

## Plugins

| id | replaces (spec only, never a dependency) | surface |
|----|------------------------------------------|---------|
| [`measure`](src/plugins/measure/README.md) | rtk gain, headroom savings, lean-ctx gain | `stats`, `bench`, proxy |
| [`cmd`](src/plugins/cmd/README.md) | rtk hook, ctx_shell, bash_compress | PreToolUse(Bash) → `rtok run` |
| [`read`](src/plugins/read/README.md) | lean-ctx read/search/tree, read_cache | MCP `read`, `search`, `tree` |
| [`archive`](src/plugins/archive/README.md) | archive_result, headroom CCR | proxy compress, `expand` |
| [`proxy`](src/plugins/proxy/README.md) | headroom proxy, caveman-proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` |
| [`inject`](src/plugins/inject/README.md) | caveman/ponytail modes, lean-ctx banner | SessionStart, UserPromptSubmit |
| [`guard`](src/plugins/guard/README.md) | refetch_guard | PreToolUse |
| [`memory`](src/plugins/memory/README.md) | engram, claude-mem | MCP `mem_*`, PreCompact |
| [`graph`](src/plugins/graph/README.md) | codebase-memory-mcp, serena, … | MCP `symbol`, `callers`, `outline` |
| [`toon`](src/plugins/toon/README.md) (off by default) | caveman toon | proxy, MCP |

Every plugin is written from scratch in this repo; rtok never runs, links or reads the tools
it replaces (plan D6). Write your own against the public API:
[`docs/plugin-authoring.md`](docs/plugin-authoring.md).

Each plugin directory has a `README.md` (what and why) and an `AGENTS.md` (invariants and
tasks for whoever works on it). Turn plugins off in `~/.rtok/config.toml`
(`[plugins.<id>] enabled = false`) or at build time (`--no-default-features --features cmd,read`).

## Configuration

One file holds every setting, and every CLI flag is a key in it (decision D12; full
reference and precedence rules in [`docs/config.md`](docs/config.md)). Precedence:
defaults < `~/.rtok/config.toml` < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env <
flags. `~/.rtok/config.toml` is created on first run (`RTOK_HOME` overrides the directory).
What exists today:

```toml
[core]
db_path = "~/.rtok/rtok.db"
archive_dir = "~/.rtok/archive"
inject_budget_tokens = 800

[estimator]          # chars per token, ±15 %; refit with `rtok stats --calibrate`
code = 3.5
prose = 4.2
json = 3.0
cjk = 1.0

[plugins.cmd]
enabled = true
```

## Layout

```
src/main.rs            CLI
src/lib.rs             crate root
src/config.rs          config + plugin catalogue
src/store.rs           SQLite (WAL, FTS5), migrations/
src/tokens.rs          estimator
src/plugin.rs          Plugin trait, Ctx, Measurement
src/plugins/<id>/      one module per plugin (+ README.md, AGENTS.md)
src/hooks/             hook I/O types, dispatcher
examples/hello_plugin.rs
tests/fixtures/hooks/  real hook payloads
```

Full picture: [`architecture.md`](architecture.md). Writing a plugin:
[`docs/plugin-authoring.md`](docs/plugin-authoring.md). Tool-inspired ideas not yet in the plan:
[`ideas.md`](ideas.md) (v0.1 open ideas and v0.2+ Later).

## Development

```bash
make check      # fmt --check, clippy -D warnings, tests, single-feature build
make example    # run examples/hello_plugin.rs
make fmt        # apply rustfmt
make changelog  # regenerate CHANGELOG.md from git history (git-cliff, cliff.toml)
```

Commit subjects are `<task-id>: <title>` or `<area>: <title>` (`plan:`, `docs:`, `ci:`);
`cliff.toml` groups them into the changelog by that prefix. Run `make changelog` before
tagging a release.

Workflow (from `AGENTS.md`): take the next unblocked task in `plan.md`, stay on `main`
(no feature branches), stay within ≤ 200 LOC and ≤ 3 files, run the task's Check verbatim, then
`make check`, commit as `<task-id>: <title>` on `main`. Same commit: mark the task done and
move it from `plan.md` to `done.md`. Implemented work still listed in `plan.md` is unfinished.

## Caveats

- Token counts from `rtok stats` are estimates (±15 %) unless they come from the proxy's
  `usage` rows.
- Lossless means the original is on disk under `~/.rtok/archive/` and `rtok expand <id>`
  returns it.
- Nothing in this repo has produced a measured saving yet. The first number lands with P1.
