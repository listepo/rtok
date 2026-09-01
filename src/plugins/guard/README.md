# `guard`

Stops the model from paying twice for the same result.

| | |
|---|---|
| Kind | native |
| Surfaces | PreToolUse hook |
| Replaces | token-optimizer refetch_guard / loop detection |
| Default | on |

## Mechanism

An identical Read or Bash call (same tool, same normalised input) within the last N turns of
the same session is denied with a reason pointing at the earlier result:

```
guard: identical to toolu_… 2 turns ago — use rtok expand <archive_id> or change the input
```

Denials are lossless because the earlier result is archived; the model can always `expand`.

## Config

```toml
[plugins.guard]
enabled = true
window_turns = 5
```

## Tasks

See `roadmap.md` § `guard`. Checks in `plan.md`.

T2.6 deny duplicate Read/Bash within `window_turns` when an archive id exists.

## Status

Manifest only.
