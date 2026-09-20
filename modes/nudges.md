# nudges
Do not re-read a file you already read this session; a re-read answers `unchanged since <id>` and spends a turn.
Prefer `outline` or `read mode=map` before `read mode=full`, `search` before native Grep, `tree` before native Glob, `symbol`/`callers` before grepping for a definition.
Long command output carries `expand <id>`; expand a slice, never re-run the command for the same bytes.
A guard deny names the prior result — `expand` it instead of retrying the call.
