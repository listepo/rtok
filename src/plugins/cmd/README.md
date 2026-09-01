# `cmd`

Every Bash output archived raw, filtered for the model, measured.

| | |
|---|---|
| Kind | native + adapter (delegates to `rtk` when installed) |
| Surfaces | PreToolUse(Bash) hook → `rtok run -- <cmd>`; `rtok expand <id>` |
| Replaces | rtk hook, lean-ctx ctx_shell, token-optimizer bash_compress |
| Default | on |

## How it works

1. The PreToolUse hook rewrites `command` to `rtok run -- <command>` unless the command
   already starts with `rtok`/`rtk`, contains a heredoc, `sudo`, `&` background, or
   `-i`/`--interactive`.
2. `rtok run` executes via `$SHELL -lc`, keeps the exit code, writes the raw output to
   `~/.rtok/archive/<id>` and an `archive` row.
3. If `rtk` is on PATH and supports the command family, `rtk <argv>` runs instead
   (measurement kind `rtk`). Otherwise TOML rules in `rules/default.toml` apply
   (kind `rule`): head/tail/dedupe/drop, always keeping `error|warning|panic|FAIL|Traceback`.
4. Output over 40 lines gets a trailer: `[rtok <id> · N lines · expand: rtok expand <id>]`.
5. Non-zero exit → last 80 lines verbatim. Nothing is redacted.

## Config

```toml
[plugins.cmd]
enabled = true
rewrite = true      # false: never rewrite Bash commands
```

## Tasks

T3.1 `rtok run` · T3.2 rtk delegation · T3.3 filter rules · T3.4 PreToolUse rewrite ·
T3.5 `rtok expand` · T3.6 measurement wiring · T10.2 `rtok filter --stdin` (OpenCode).

## Status

Manifest only. First task: T3.1.
