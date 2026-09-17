# Command filter rules

`rtok run` (and `rtok filter --stdin`, and the PreToolUse Bash rewrite that
calls `rtok run`) shortens command output with per-family rules before the
model sees it. The raw output is always archived first, so every shortened
result stays retrievable with `rtok expand <id>` (lossless by default, D4).
Rules never redact: a line carrying an error, warning or secret passes through
unchanged.

Three layers merge in this order; a later layer wins per command family:

1. **Built-ins** — `rules/default.toml` in the repo (grep, rg, sed, cat, make,
   curl, npm, pnpm, node, plus the cargo/git/test/ls formatters in code).
2. **User file** — `[plugins.cmd] rules` (default `~/.rtok/rules.toml`).
3. **Drop-ins** — every `*.toml` in `[plugins.cmd] rules_dir` (default
   `~/.rtok/rules.d/`), merged in file-name order.

A missing file or dir is not an error — that layer is skipped. A malformed
file is skipped at runtime (fail open) and reported by
`rtok config validate`, which names every bad file.

## Schema

One TOML table per command family. The table name is the command's first word
(`[grep]`, `[pytest]`); unknown families fall back to the default rule
(`max_lines = 40`, `head = 10`, `tail = 10`, dedupe on).

The same engine cuts foreign MCP results behind `rtok mcp -- <server argv>` (T59.4): a
`[mcp]` section is the rule for every wrapped server's `tools/call` text block, the default
rule when there is none. Only text blocks longer than `max_lines` of a non-`isError` result
are touched; the raw block is archived first and the cut ends with the same
`[rtok <id> · N lines · expand: rtok expand <id>]` trailer `rtok run` prints. The saving
lands as a `cmd` / `wrap` measurement with `ref_id = <server>/<tool>:<id>`.

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `max_lines` | integer ≥ 0 | 40 | capped output lines; the rest becomes one `… N lines omitted (expand <id>)` line |
| `head` | integer ≥ 0 | 10 | lines always kept from the start |
| `tail` | integer ≥ 0 | 10 | lines always kept from the end |
| `drop` | array of strings | `[]` | case-insensitive substrings to remove (`\|` separates alternatives) |
| `keep` | array of strings | `[]` | substrings that are never dropped and never cut by the cap; built-in keeps are `error`, `warning`, `panic`, `fail`, `traceback` |
| `dedupe` | bool | true | collapse runs of identical lines into `line (×N)` |

Any other field, a wrong type, or broken TOML is malformed. A non-zero exit
ignores all of this: the last `[plugins.cmd] fail_tail_lines` lines (default
80) print verbatim, because a failed command's error is the payload.

## Worked example

`~/.rtok/rules.d/pytest.toml` — keep pytest quiet runs to a pointer, but never
lose a failure:

```toml
[pytest]
max_lines = 30
head = 5
tail = 15
drop = ["PASSED", "passed"]
keep = ["FAILED|failed|ERROR|error"]
dedupe = true
```

Before (212 lines of dots and passes), after (12 lines):

```text
tests/test_auth.py::test_login PASSED
tests/test_auth.py::test_refresh FAILED
… 200 lines omitted (expand abc123)
E   assert False is True
```

Check it before relying on it:

```bash
rtok config validate   # names rules.d/*.toml files that do not parse
```

`validate` checks the file named on its command line (or the user config)
plus the single `rules` file and every `rules.d/*.toml` it resolves to. A
reported file is skipped at runtime until fixed; everything else keeps
filtering.
