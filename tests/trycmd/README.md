# trycmd cases

One line per case: what the golden pins. `--help` / `--version` never load
config; reading cases set `inherit = false` and `RTOK_HOME` under `target/tmp/`.

- `help.toml` — top-level `rtok --help`
- `version.toml` — `rtok --version`
- `help-subcommands.trycmd` — `--help` for every subcommand and nested verb (`web`/`tui` help-only)
- `stats-price.toml` — `stats --price` on the empty fixture store
- `config-show.toml` — `config show` against `input/bench-config.toml`
- `completions-bash.toml` — bash completions
- `bench-dry-run.toml` — `bench --dry-run` schedule
- `doctor-json.toml` — `doctor --json` (T60.1)
- `plugins-json.toml` — `plugins --json` (T60.1)
- `agents-list-json.toml` — `agents list --json` (T60.1)
- `agents-sessions-json.toml` — `agents sessions --json` (T60.1)
- `logs-json.toml` — `logs --json` (T60.1)
- `demon-json.toml` — `demon status --json` (T60.1)
- `otel-json.toml` — `otel status --json` (T60.1)
- `stats.toml` — `stats` table on the empty fixture store
- `stats-json.toml` — `stats --json` on the empty fixture store
- `info-json.toml` — `info --json`; paths/version/status via `[..]`
- `doctor.toml` — `doctor` table; MCP binary and skills via `[..]` / `...`
- `plugins.toml` — `plugins` table: id, enabled, surfaces
- `config-init.toml` — `config init --dry-run` of a missing user file
- `config-path.toml` — `config path` for the `--config` fixture
- `config-get.toml` — `config get proxy.port`
- `config-validate.toml` — `config validate` on the bench fixture
- `config-set.toml` — `config set --dry-run` diff for `proxy.port`
- `completions-zsh.toml` — zsh completions
- `completions-fish.toml` — fish completions
- `completions-powershell.toml` — powershell completions
- `man.toml` — man page (roff); version via `[..]`
- `agents-list.toml` — `agents list` table (host paths via `...`)
- `agents-info.toml` — `agents info` table for one host (host paths via `...`)
- `agents-info-json.toml` — `agents info --json` for one host (host paths via `...`)
- `agents-sessions.toml` — `agents sessions` on an empty store
- `demon-status.toml` — `demon status` with nothing running
- `otel-status.toml` — `otel status` table
- `logs-print.toml` — `logs` on an empty log
- `report-md.toml` — `report --format md`; dates via `[..]`
- `proxy-dry-run.toml` — `proxy --dry-run` effective `[proxy]` settings
- `expand.trycmd` — `run cat` of the fixture body, then `expand --lines --grep`
- `hook.toml` — `hook SessionStart` with fixture stdin JSON
- `mcp.toml` — `mcp` `tools/list` frame on stdin
- `filter.toml` — `filter --cmd git status` of a short payload
