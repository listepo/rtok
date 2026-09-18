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
