# Configuration

One file, `~/.rtok/config.toml`, holds every setting rtok has. **Every CLI flag is a config
key**, so anything you can pass on the command line you can also make permanent, and
`rtok config show --sources` always tells you where a value came from.

Status: T12.1–T12.2 are done — every table below is a typed section in `config/mod.rs` (unknown
key = error), `config/default.toml` is embedded and written verbatim by `rtok config init`,
`core.inject_budget_tokens` has moved to `plugins.inject.budget_tokens`, and layering
(user < project < env < flags, `figment`-based) plus `config show [--sources] [--json]` and
`config get <key>` are live. `config set`/`validate` and the flag-coverage test land in T12.3–T12.4.

## Precedence

Lowest to highest. Later layers override earlier ones key by key.

1. **Built-in defaults** — the values in the reference file below (`config/default.toml`,
   embedded in the binary).
2. **User file** — `~/.rtok/config.toml` (directory overridable with `RTOK_HOME`; file
   overridable with `RTOK_CONFIG=<path>` or `--config <path>`).
3. **Project file** — `<git root>/.rtok.toml`, if present. Same schema; typically only
   `[plugins.read] allow_paths`, `[plugins.cmd] rules`, `[plugins.inject] modes`.
4. **Environment** — `RTOK_<SECTION>_<KEY>` in upper snake case, e.g. `RTOK_PROXY_PORT=8791`,
   `RTOK_PLUGINS_CMD_REWRITE=false`, `RTOK_STATS_SINCE=7d`. Lists are comma-separated.
   Existing short names stay as aliases: `RTOK_UPSTREAM`, `RTOK_OPENAI_UPSTREAM`.
5. **Command-line flags** — `rtok proxy --port 8791`.

Rules:

- Positional per-call arguments (`hook <event>`, `expand <id>`, `run -- <cmd>`,
  `setup <host>`, `memory import <file>`, `--save-baseline <name>`) are not settings and have no key. Everything
  else does.
- Flag `--foo-bar` on subcommand `baz` ↔ key `baz.foo_bar`. Plugin settings live under
  `plugins.<id>.<key>`.
- Unknown keys are an error in `rtok config validate` and a warning (stderr, once) elsewhere.
  Hooks never fail on config problems: they log and use defaults (fail open).
- Paths accept `~`. Relative paths are relative to the file they appear in.
- Durations: `30d`, `12h`, `15m`. Sizes: plain integers in bytes or tokens as named.

## `rtok config`

| Command | Does |
|---------|------|
| `rtok config show [--sources] [--json]` | effective config after all layers; `--sources` annotates each key with `default / user / project / env / flag` |
| `rtok config init [--force]` | write the reference file (below) to `~/.rtok/config.toml`; never overwrites without `--force` |
| `rtok config validate [path]` | parse, reject unknown keys and out-of-range values with line numbers; exit 1 on error |
| `rtok config path` | print the resolved user file path (and project file if any) |
| `rtok config get <key>` | print one effective value, e.g. `rtok config get proxy.port` |
| `rtok config set <key> <value>` | edit the user file in place, preserving comments (uses `toml_edit`) |

## Reference file

This is `config/default.toml` verbatim. Every value shown is the default; a fresh
`rtok config init` writes exactly this.

```toml
# rtok configuration. Every CLI flag has a key here; flags and RTOK_* env vars override.
# Precedence: defaults < this file < <git root>/.rtok.toml < env < flags.
# Docs: docs/config.md. Check: `rtok config validate`. Where a value came from: `rtok config show --sources`.

[core]
db_path     = "~/.rtok/rtok.db"       # one SQLite file, WAL (decision D8)
archive_dir = "~/.rtok/archive"       # raw payloads for `rtok expand <id>` (decision D4)
log_level   = "warn"                  # error | warn | info | debug
log_file    = "~/.rtok/rtok.log"      # hooks never write to stderr; they log here
session_env = "CLAUDE_SESSION_ID"     # env var consulted for the session id when stdin has none
call_io_inline_bytes = 65536          # MCP/API bodies larger than this go to archive (hooks never archive)
retain_calls_days    = 30             # 0 = keep `calls` forever
log_to_db            = true           # also write `logs` rows; log_file is always written

[estimator]                           # chars per token per class, ±15 %; `rtok stats --calibrate` rewrites
code  = 3.5
prose = 4.2
json  = 3.0
cjk   = 1.0

# ── surfaces ────────────────────────────────────────────────────────────────

[hook]                                # rtok hook <event>
host      = "claude"                  # claude | cursor — payload field mapping (T10.1)
max_ms    = 10                        # soft budget; over it, the event is logged as slow
fail_open = true                      # any error → `{}` and exit 0; false only for debugging

[mcp]                                 # rtok mcp
tools                   = []          # [] = all tools from enabled plugins; else an allow-list
max_description_tokens  = 60          # enforced by a test (T4.1)
max_result_chars        = 20000       # above this, head/tail + archive id

[proxy]                               # rtok proxy
bind            = "127.0.0.1"
port            = 8790
mode            = "passthrough"       # passthrough | compress
upstream        = "https://api.anthropic.com"      # RTOK_UPSTREAM; chain behind another proxy for A/B
openai_upstream = "https://api.openai.com"         # RTOK_OPENAI_UPSTREAM (D11)
timeout_s       = 600                 # upstream request timeout
include_usage   = true                # OpenAI streaming: add stream_options.include_usage when missing (T11.2)
dry_run         = false               # --dry-run: print effective [proxy] settings and exit, don't serve

[stats]                               # rtok stats
since           = "30d"
format          = "table"             # table | json      (--json)
plugin          = ""                  # "" = all         (--plugin <id>)
transcripts_dir = "~/.claude/projects"
calibrate_samples = 30                # per class        (--calibrate)
baseline        = ""                  # default name for --compare; "" = none

[bench]                               # rtok bench
tasks    = "bench/tasks.toml"
runs     = 3
dry_run  = false
timeout_s = 900                       # per task run
[bench.configs]                       # name = settings file passed to `claude --settings`
a = "bench/configs/legacy.json"
b = "bench/configs/rtok.json"

[doctor]                              # rtok doctor
settings_path   = "~/.claude/settings.json"
claude_json     = "~/.claude.json"
mcp_json        = ".mcp.json"
probe_timeout_ms = 500                # per proxy hop /health probe
mcp_timeout_ms  = 15000               # per MCP server tools/list (uvx/npx servers start slowly)
instruction_warn_tokens = 1000        # --instructions: flag files above this
instructions    = false               # run the instruction audit by default (--instructions)

[setup]                               # rtok setup <host>
dry_run      = false
yes          = false                  # required by --replace
backup       = true                   # settings.json.bak-<ts> before every write
hook_timeout_s = 5                    # timeout written into each hook entry
modes        = []                     # e.g. ["terse", "yagni"]   (--mode)
mcp          = true                   # also register the MCP server   (--mcp)
proxy        = false                  # also set the base URL          (--proxy)
[setup.claude]
settings_path = "~/.claude/settings.json"
[setup.cursor]
hooks_path    = "~/.cursor/hooks.json"
[setup.codex]
config_path   = "~/.codex/config.toml"
[setup.opencode]
config_path   = "~/.config/opencode/opencode.json"

[expand]                              # rtok expand <id>
max_lines = 0                         # 0 = unlimited   (--lines a-b is per call)

[filter]                              # rtok filter --stdin (T10.2)
cmd = ""                              # command family hint when the caller knows it (--cmd)

# ── plugins ─────────────────────────────────────────────────────────────────

[plugins.measure]
enabled = true

[plugins.cmd]
enabled  = true
rewrite  = true                       # PreToolUse(Bash) → `rtok run -- …`
shell    = ""                         # "" = $SHELL
rules    = "~/.rtok/rules.toml"       # extra filter rules; missing → built-in rules/default.toml
trailer_min_lines = 40                # add `[rtok <id> · N lines · expand …]` above this
fail_tail_lines   = 80                # non-zero exit → last N lines verbatim
never_wrap = ["rtok", "sudo"]         # first-word deny list; heredocs, `&`, -i are always skipped

[plugins.read]
enabled          = true
default_mode     = "full"             # full | lines | map | signatures
max_chars        = 20000              # above this, head/tail + archive id
native_max_bytes = 32768              # PreToolUse(Read) deny threshold; never below this
advice           = true               # false = never deny native Read
allow_paths      = []                 # extra roots outside cwd
search_max       = 50
tree_depth       = 2
languages        = ["rust", "ts", "js", "python", "dart", "c", "go"]

[plugins.archive]
enabled    = true
keep_turns = 4                        # never touch the last N turns
min_tokens = 1500                     # only rewrite tool results above this (estimated)
head_lines = 8
tail_lines = 4

[plugins.proxy]
enabled = true                        # the proxy plugin (usage capture); the server itself is [proxy]

[plugins.inject]
enabled       = true
budget_tokens = 800                   # per turn, all injections together (decision D5)
modes_dir     = "~/.rtok/modes"
modes         = []                    # same as [setup].modes; setup writes here

[plugins.guard]
enabled      = true
window_turns = 8

[plugins.memory]
enabled        = true
recall_titles  = 5                    # SessionStart: last N titles + ids
recall_tokens  = 200
checkpoint_tokens = 400               # PreCompact → SessionStart(compact)
search_limit   = 5

[plugins.graph]
enabled    = true
max_tokens = 2000                     # per response; beyond it: head + "N more, expand <id>"

[plugins.toon]
enabled  = false
min_rows = 5
```

## Mapping table (flags → keys)

| Subcommand | Flag | Key |
|-----------|------|-----|
| global | `--config <path>` | (selects the file; not a key) |
| global | `--home <dir>` / `RTOK_HOME` | (selects the directory; not a key) |
| global | `--log-level` | `core.log_level` |
| `hook` | `--host` | `hook.host` |
| `proxy` | `--port`, `--bind`, `--upstream`, `--openai-upstream`, `--mode`, `--timeout`, `--dry-run` | `proxy.*` |
| `stats` | `--since`, `--json`, `--plugin`, `--compare`, `--calibrate`, `--cache` | `stats.since`, `stats.format`, `stats.plugin`, `stats.baseline`, (`--calibrate`, `--cache` are actions; their knobs are `stats.calibrate_samples`) |
| `bench` | `--tasks`, `--runs`, `--dry-run`, `--timeout` | `bench.*` |
| `doctor` | `--instructions` | `doctor.instructions` |
| `setup` | `--dry-run`, `--yes`, `--mode`, `--mcp`, `--proxy`, `--remove`, `--replace` | `setup.*` (`--remove`, `--replace` are actions) |
| `run` | `--shell`, `--no-trailer` | `plugins.cmd.shell`, `plugins.cmd.trailer_min_lines` |
| `expand` | `--lines`, `--grep` | per call (no key); `expand.max_lines` caps |
| `filter` | `--cmd` | `filter.cmd` |
| `plugins` | `--json` | (output format only; follows `stats.format`) |

The coverage test (T12.4) walks the clap command tree and fails if a non-positional flag
appears without a key in `config/default.toml`, so this table cannot silently drift.

## Env var examples

```bash
RTOK_PROXY_MODE=compress rtok proxy
RTOK_PLUGINS_READ_ALLOW_PATHS=/opt/src,/srv/lib rtok mcp
RTOK_STATS_SINCE=7d rtok stats
RTOK_CONFIG=./ci-config.toml rtok bench --dry-run
```

## Why one file and not flags-only

Hooks are spawned by the host with a fixed command line; the only way to tune them is a
file. The proxy and MCP server run for hours; restarting them to change a flag is a
regression. And a bench needs two complete, reproducible configurations — which is a file
per configuration, not a shell history.
