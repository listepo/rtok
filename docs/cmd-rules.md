# Command filter rules

`rtok run` (and `rtok filter --stdin`, and the PreToolUse Bash rewrite that
calls `rtok run`) shortens command output with per-family rules before the
model sees it. The raw output is always archived first, so every shortened
result stays retrievable with `rtok expand <id>` (lossless by default, D4).
Rules never redact: a line carrying an error, warning or secret passes through
unchanged.

Three layers merge in this order; a later layer wins per command family:

1. **Built-ins** — `rules/default.toml` in the repo (grep, rg, sed, cat, make,
   curl, npm, pnpm, node, plus the cargo/git/test/tree formatters in code).
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
| `group` | `"dir"` \| `"diag"` | off | `dir` rewrites path-per-line output as `dir/ (N files): a, b, c …`; `diag` rewrites coded diagnostics as `E0308 ×N: first message (file:line, …)`; runs before the head/tail cut (T64.1) |
| `json_items` | integer ≥ 0 | 20 | JSON arrays keep this many elements; the rest is one `… +K more` (T65.2) |
| `json_string` | integer ≥ 0 | 200 | JSON strings longer than this are cut with their character length (T65.2) |

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

## Families (T50.1)

Measured on the golden fixtures in `tests/cmd_golden/` (`rtok filter --stdin`, `Rule` vs
`Rule::default()` on the same body; est tokens = bytes/4).

| Family | `keep` highlights | before B | after B | est saved |
| --- | --- | ---: | ---: | ---: |
| `gh` | `error`, `HTTP 4`, `gh:` | 325 | 277 | 12 |
| `pip` | `ERROR`, `Could not` | 389 | 341 | 12 |
| `uv` | `error`, `Failed` | 292 | 244 | 12 |
| `python` / `python3` | `Traceback`, `Error` | 367 | 299 | 17 |
| `go` | `error`, `panic`, `cannot` | 290 | 242 | 12 |
| `aws` | `An error occurred`, `AccessDenied` | 349 | 301 | 12 |
| `mvn` | `BUILD FAILURE`, `[ERROR]` | 419 | 395 | 6 |
| `gradle` | `BUILD FAILED`, `What went wrong` | 357 | 302 | 13 |
| `dotnet` | `error CS`, `Build FAILED` | 285 | 237 | 12 |
| `tsc` | `error TS`, `Found N error` | 373 | 325 | 12 |
| `eslint` | `error`, `warning`, `problems` | 298 | 250 | 12 |
| `brew` | `Error:`, `failed` | 310 | 262 | 12 |
| `apt` | `^E:`, `Unable to` | 293 | 245 | 12 |
| `cmake` | `CMake Error`, `FAILED` | 317 | 255 | 15 |

## Formatters (T58.5)

Measured on the same golden bodies (`formatters::compress`, `Measurement.kind = formatter`
vs `Rule::default()` on the same fixture; est tokens = bytes/4). Each formatter keeps one
row per object and returns `None` on unrecognized output so the rule path stays the fallback.

| Family | keeps | before B | rule after B | formatter after B | est saved vs rule |
| --- | --- | ---: | ---: | ---: | ---: |
| `docker ps` | one row per container | 3147 | 1311 | 1190 | 30 |
| `kubectl get` | one row per object (`NAME READY STATUS IP`) | 4542 | 1770 | 1731 | 9 |
| `ps aux` | one row per process | 2341 | 990 | 870 | 30 |

## Grouping (T64.1)

Measured on the golden fixtures in `tests/cmd_golden/` (`formatters::compress` with the
built-in rule vs the same rule with `group` off; est tokens = bytes/4). A family is
switched on only when the grouped body is smaller. `tree`, `git status`, `pytest`, and
`python` stay as they are: tree art and the git/pytest formatters are not path-per-line
or coded-diagnostic streams, and the T50.1 `python` traceback does not shrink as `NameError ×1`.

| Family | `group` | ungrouped B | grouped B | est saved |
| --- | --- | ---: | ---: | ---: |
| `ls` | `dir` | 248 | 49 | 49 |
| `find` | `dir` | 539 | 83 | 114 |
| `rg` (`-l`) | `dir` | 459 | 83 | 94 |
| `tsc` | `diag` | 1619 | 107 | 378 |
| `eslint` | `diag` | 749 | 48 | 175 |
| `cargo` (`check`) | `diag` | 749 | 34 | 178 |
| `dotnet` | `diag` | 849 | 71 | 194 |

## JSON (T65.2)

A body that `serde_json` parses as an object or array is rewritten after grouping and
before the head/tail cut: null / empty-string / empty-container fields dropped, arrays
beyond `json_items` shown as `… +K more`, object keys kept, strings longer than
`json_string` cut with their length, one line per top-level key. Unparseable bodies are
untouched. Table formatters (`kubectl get`, `docker ps`) stand down when the body is JSON
so `kubectl get -o json` reaches this pass. `toon` stays off the hook path. Raw bytes stay
in the archive; a shortened result still carries the expand trailer.

Measured on `tests/cmd_golden/{gh,aws,kubectl}_json.in` (`formatters::compress` vs a
head/tail cut of the same pretty body; est tokens = bytes/4).

| Source | raw B | line-cut B | compact B | est saved vs raw |
| --- | ---: | ---: | ---: | ---: |
| `gh pr list --json` | 10275 | 908 | 4530 | 1436 |
| `aws ec2 describe-instances` | 19516 | 519 | 5890 | 3406 |
| `kubectl get pods -o json` | 26271 | 353 | 8297 | 4493 |
