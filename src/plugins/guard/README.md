# `guard`

Stops the model from paying twice for the same result.

| | |
|---|---|
| Surfaces | PreToolUse hook |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Mechanism

An identical Read or Bash call (same tool, same normalised input) within the last N turns of
the same session is denied with a reason pointing at the earlier result:

```
guard: identical to toolu_… 2 turns ago — use rtok expand <archive_id> or change the input
```

Denials are lossless because the earlier result is archived; the model can always `expand`.

Opt-in (T62.1, Claude Code): with `skills = true` a `Skill` call whose `SKILL.md` exceeds
`skill_max_bytes` is denied before the host injects the body. The reason is the file's
heading map (each heading with its first line) plus `[rtok <id> · N lines · expand: rtok
expand <id>]`, so the model pulls one section with `rtok expand <id> --grep <heading>`
instead of carrying the whole body in every later request. Skills whose frontmatter names
`allowed-tools`, `model`, `context` or `agent` always load whole (a denied skill does not
apply them). Measured 2026-09-17 on this machine: 17 bodies, median 8,863 B, max 248,175 B
(`research.md` §10.2).

## Config

```toml
[plugins.guard]
enabled = true
window_turns = 8
skills = false          # T62.1: digest oversized SKILL.md on PreToolUse(Skill)
skill_max_bytes = 8192
```

## Tasks

See `roadmap.md` § `guard`. Checks in `plan.md`.

T2.6 deny duplicate Read/Bash within `window_turns` when an archive id exists.
T62.1 digest a skill body over `skill_max_bytes` (opt-in).

## Status

Manifest only.
