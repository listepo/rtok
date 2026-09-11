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
4. **`.env` files** — `RTOK_*` lines (same names as the environment layer) from the nearest
   `.env` at or above the working directory, then `~/.rtok/.env`; the project file wins.
   Parsed only, never exported: commands run through `rtok run` do not inherit them, and other
   keys in a project's `.env` are ignored. A malformed file is reported on stderr and skipped.
5. **Environment** — `RTOK_<SECTION>_<KEY>` in upper snake case, e.g. `RTOK_PROXY_PORT=8791`,
   `RTOK_PLUGINS_CMD_REWRITE=false`, `RTOK_STATS_SINCE=7d`. Lists are comma-separated.
   Existing short names stay as aliases: `RTOK_UPSTREAM`, `RTOK_OPENAI_UPSTREAM`.
6. **Command-line flags** — `rtok proxy --port 8791`.

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

Credentials never print: `otel.headers` (it carries OTLP ingestion keys) shows as `<redacted>`
once set, in `show`, `get`, `set` and `rtok report` — its source still shows. Read the file itself
to see the value.

## Reference file

This is `config/default.toml` verbatim. Every value shown is the default; a fresh
`rtok config init` writes exactly this. Logging is `[log]` only (D26). An old file's
`core.log_file` / `log_level` / `log_to_db` still loads once with a warning and folds into
`[log]`; `rtok config validate` rejects those keys because they are absent from the reference
schema.

```toml
# rtok configuration. Every CLI flag has a key here; flags and RTOK_* env vars override.
# Precedence: defaults < this file < <git root>/.rtok.toml < env < flags.
# Docs: docs/config.md. Check: `rtok config validate`. Where a value came from: `rtok config show --sources`.

[core]
enabled     = true                    # false = plain proxy (no business logic); HTTP stays up until process exit
db_path     = "~/.rtok/rtok.db"       # one SQLite file, WAL (decision D8)
archive_dir = "~/.rtok/archive"       # raw payloads for `rtok expand <id>` (decision D4)
session_env = "CLAUDE_SESSION_ID"     # env var consulted for the session id when stdin has none
call_io_inline_bytes = 65536          # MCP/API bodies larger than this go to archive (hooks never archive)
retain_calls_days    = 30             # 0 = keep `calls` forever

[log]                                 # rtok's own log (D26); `rtok logs` reads it
path      = "~/.rtok/logs/rtok.log"   # rotated siblings live beside it: rtok.log.1 … .5
max_bytes = 1048576                   # rotate past 1 MiB
files     = 5                         # generations kept; older ones are deleted, never archived
lines     = 200                       # what `rtok logs` prints when --lines is not given
level     = "info"                    # error | warn | info | debug
to_db     = true                      # also write a `logs` row for `rtok otel`

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
enabled         = true                # false = plain reverse proxy (bypass compress/bookkeeping); does NOT stop HTTP
bind            = "127.0.0.1"
port            = 8790
mode            = "passthrough"       # passthrough | compress
upstream        = "https://api.anthropic.com"      # RTOK_UPSTREAM; chain behind another proxy for A/B
openai_upstream = "https://api.openai.com"         # RTOK_OPENAI_UPSTREAM (D11)
timeout_s       = 600                 # upstream request timeout
include_usage   = true                # OpenAI streaming: add stream_options.include_usage when missing (T11.2)
dry_run         = false               # --dry-run: print effective [proxy] settings and exit, don't serve

[web]                                 # rtok web (same data as rtok tui)
host = "127.0.0.1"                    # --host
port = 3333                           # --port

[tui]                                 # rtok tui (same data as rtok web)
tab       = ""                        # "" = first tab          (--tab <page>)
tick_secs = 2                         # model re-read cadence, same as the web 2 s tick (--tick-secs)

[stats]                               # rtok stats
since           = "30d"
format          = "table"             # table | json      (--json)
plugin          = ""                  # "" = all         (--plugin <id>)
transcripts_dir = "~/.claude/projects"
calibrate_samples = 30                # per class        (--calibrate)
baseline        = ""                  # default name for --compare; "" = none

[report]                              # rtok report (D24: renders the operator model, computes nothing)
format = "md"                         # md; html (T22.2), pdf (T22.3), --ai (T22.4)
out    = ""                           # "" = stdout     (--out <path>)
since  = "30d"                        # how far back the report reads
ai     = false                        # model-shaped rendering instead of --format (--ai)
budget_tokens = 8000                  # --ai drops whole sections past this (T22.4)

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

[setup]                               # rtok agent setup / remove <host>
dry_run      = false
yes          = false                  # required by --replace
backup       = true                   # <name>.bak-<ts> beside each file, before setup and remove touch it
hook_timeout_s = 5                    # timeout written into each hook entry
modes        = []                     # e.g. ["terse", "yagni"]   (--mode)
mcp          = true                   # also register the MCP server   (--mcp)
proxy        = false                  # also set the base URL          (--proxy)
[setup.claude]
settings_path = "~/.claude/settings.json"
[setup.cursor]
hooks_path    = "~/.cursor/hooks.json"  # today: beforeShellExecution only (T10.11 wires PostToolUse)
[setup.codex]
config_path   = "~/.codex/config.toml"
[setup.opencode]
config_path   = "~/.config/opencode/opencode.json"
[setup.pi]
extensions_path = "~/.pi/agent/extensions"

[expand]                              # rtok expand <id>
max_lines = 0                         # 0 = unlimited   (--lines a-b is per call)
max_rate  = 0.05                      # re-read ceiling; above it the report flags lossy compression (T22.5)

[filter]                              # rtok filter --stdin (T10.2)
cmd = ""                              # command family hint when the caller knows it (--cmd)

[otel]                                # OpenTelemetry export (D19); off until endpoint resolves
endpoint      = ""                    # OTLP/HTTP base URL, e.g. "http://localhost:4318"; "" = $OTEL_EXPORTER_OTLP_ENDPOINT
headers       = ""                    # "k=v,k2=v2", e.g. "signoz-ingestion-key=…"; "" = $OTEL_EXPORTER_OTLP_HEADERS
service_name  = "rtok"                # resource service.name
content       = true                  # gen_ai.input/output.messages, tool arguments and results on spans
content_bytes = 65536                 # per attribute; beyond it rtok.archive.id → `rtok expand <id>`
flush_secs    = 5                     # proxy / mcp flush interval, and the POST timeout

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

[plugins.archive]
enabled    = true
keep_turns = 4                        # never touch the last N turns
min_tokens = 1500                     # only rewrite tool results above this (estimated)
head_lines = 8
tail_lines = 4
tiers      = false                    # opt-in tiered loading (default off); OpenViking L0/L1/L2 behaviour spec is AGPL-3.0 — rtok does not vendor, link, or subprocess it (D6); gates native impl in T33.2

[plugins.proxy]
enabled = true                        # the proxy plugin (usage capture); the server itself is [proxy]

#### `[plugins.proxy.semantic_cache]` — opt-in response cache (P31)

Off by default until Gate P31 (zero false hits on the P9 set). When enabled (T31.2), the proxy may
serve a prior response when a normalized prompt is similar enough; a false hit is a wrong answer, so
this stays opt-in. Env: `RTOK_PLUGINS_PROXY_SEMANTIC_CACHE_ENABLED=true`.

| Key | Default | Meaning |
|-----|---------|---------|
| `enabled` | `false` | Master switch; proxy bytes stay identical when off |
| `threshold` | `0.99` | Cosine similarity floor for the semantic tier |
| `ttl_s` | `300` | Entry TTL in seconds |
| `max_messages` | `1` | Skip cache when `messages` length exceeds this |
| `require_empty_tools` | `true` | Do not cache turns with non-empty `tools[]` |
| `embed_backend` | `"hash"` | `"hash"` = direct tier only until P29 embeddings |
| `cache_by_model` | `true` | Partition cache entries by model |
| `cache_by_provider` | `true` | Partition cache entries by provider |

```toml
[plugins.proxy.semantic_cache]
enabled = false
threshold = 0.99
ttl_s = 300
max_messages = 1
require_empty_tools = true
embed_backend = "hash"
cache_by_model = true
cache_by_provider = true
```

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

[plugins.memory.embed]
enabled    = false                    # P29: FTS5-only when false; vector search is opt-in
provider   = "local"                  # "local" | "openai"
model      = "all-MiniLM-L6-v2"
dimensions = 384
hybrid     = true                     # when enabled: RRF(fts5, knn); false = knn only

[plugins.graph]
enabled    = true
max_tokens = 2000                     # per response; beyond it: head + "N more, expand <id>"
body_lines = 40                       # symbol(): source lines shown per definition
auto_index = true                     # true = every call walks the tree; false = index once, then `rtok graph index` or the watcher (a hook-staled file reads as missing until then)
backend    = "tags"                   # tags | lsp: index backend; default tags-only; lsp is opt-in and unused until T30.2 (P30)
watch      = "off"                    # off | notify | watchman: background re-index inside `rtok mcp` (P8d); watchman needs `--features graph-watchman` (opt-in, Gate P8d)

[plugins.toon]
enabled  = false
min_rows = 5

[plugins.compress]
enabled = false                       # optional LLM compression / memory extractor (P28); off until Gate P28

[plugins.wasm]
enabled = false                      # off by default; no .wasm loaded until T32.2 host + `wasm-host` feature
dir     = "~/.rtok/plugins"          # scan one level for *.wasm; D6 — this repo never vendors third-party plugins
```


### WASM plugin host (`[plugins.wasm]`)

Out-of-tree `.wasm` plugins (P32, decision D6). This repo writes every catalogue plugin from
scratch and **never vendors third-party `.wasm` blobs** — operators install them under
`plugins.wasm.dir` on their machine. Default `enabled = false`: in-tree builds and the default
config do not load WASM. The Wasmi host and Cargo feature `wasm-host` land in T32.2; until then
the flag is visible in `rtok config show --sources` but has no loader.

## OpenTelemetry

`[otel]` turns on the exporter (`docs/otel.md`). Off until `endpoint` or
`OTEL_EXPORTER_OTLP_ENDPOINT` names a collector; nothing runs on the hook path.

## Mapping table (flags → keys)

| Subcommand | Flag | Key |
|-----------|------|-----|
| global | `--config <path>` | (selects the file; not a key) |
| global | `RTOK_HOME` | (selects the directory; env only, not a clap flag) |
| `hook` | `--host` | `hook.host` |
| `proxy` | `--port`, `--upstream`, `--mode`, `--dry-run` | `proxy.port`, `proxy.upstream`, `proxy.mode`, `proxy.dry_run` |
| `web` | `--host`, `--port` | `web.host`, `web.port` (`rtok dashboard` is the deprecated spelling) |
| `tui` | `--tab`, `--tick-secs` | `tui.tab`, `tui.tick_secs` |
| `stats` | `--since`, `--json`, `--plugin`, `--compare`, `--calibrate`, `--cache` | `stats.since`, `stats.format`, `stats.plugin`, `stats.baseline`, (`--calibrate`, `--cache` are actions; their knobs are `stats.calibrate_samples`) |
| `report` | `--format`, `--out`, `--since`, `--ai` | `report.format`, `report.out`, `report.since`, `report.ai` (`report.budget_tokens` caps `--ai`) |
| `bench` | `--tasks`, `--runs`, `--dry-run`, `--timeout` | `bench.*` |
| `doctor` | `--instructions` | `doctor.instructions` |
| `agent setup` | `--dry-run`, `--yes`, `--mode`, `--mcp`, `--proxy`, `--remove`, `--replace` | `setup.*` (`--remove`, `--replace` are actions) |
| `agent remove` | `--dry-run` | `setup.dry_run` (the command itself is the `--remove` action) |
| `expand` | `--lines`, `--grep` | per call (no key); `expand.max_lines` caps; `expand.max_rate` is the report ceiling (T22.5) |
| `filter` | `--cmd` | `filter.cmd` |
| `config init`, `config set`, `memory import`, `graph index` | `--dry-run` | (action: renders the change as a git diff and writes nothing) |

The coverage test (T12.4) walks the clap command tree and fails if a non-positional flag
appears without a key in `config/default.toml`, so this table cannot silently drift.

## Env var examples

```bash
RTOK_PROXY_MODE=compress rtok proxy
RTOK_PLUGINS_READ_ALLOW_PATHS=/opt/src,/srv/lib rtok mcp
RTOK_PLUGINS_WASM_ENABLED=true rtok config show --sources
RTOK_STATS_SINCE=7d rtok stats
RTOK_CONFIG=./ci-config.toml rtok bench --dry-run
```

## Why one file and not flags-only

Hooks are spawned by the host with a fixed command line; the only way to tune them is a
file. The proxy and MCP server run for hours; restarting them to change a flag is a
regression. And a bench needs two complete, reproducible configurations — which is a file
per configuration, not a shell history.

## Legacy keys

`[dashboard]` folds into `[web]` (T21.3). `core.inject_budget_tokens` folds into
`plugins.inject.budget_tokens` (T12.1). `core.log_file` / `log_level` / `log_to_db` fold into
`[log].path` / `level` / `to_db` (T24.5, D26). Each prints one warning on load and is then
dropped; `rtok config validate` rejects them because they are absent from the reference schema.
