# `inject`

The only door into SessionStart and UserPromptSubmit context. Everything that wants to say
something to the model at those points goes through here, under one budget.

| | |
|---|---|
| Surfaces | SessionStart, UserPromptSubmit hooks |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Mechanism

1. Collect `Injection { plugin, text, priority }` from every enabled plugin.
2. Sort by priority (higher first).
3. Emit until `core.inject_budget_tokens` (default 800) is reached; drop the rest.
4. Record `Measurement { kind: "inject" }` with what was emitted and what was dropped.

SessionStart output is byte-identical across runs with unchanged state — no timestamps, no
counters — so the host's prompt cache keeps hitting.

Modes (`terse`, `yagni`) are markdown files under `~/.rtok/modes/`, ≤ 250 tokens each,
enabled with `rtok setup --mode terse,yagni`, injected once per session at priority 5.

## Config

```toml
[core]
inject_budget_tokens = 800

[plugins.inject]
enabled = true
modes = []          # e.g. ["terse", "yagni"]
```

## Tasks

See `roadmap.md` § `inject`. Checks in `plan.md`.

T2.4 plugin + budget · T7.1 modes as data.

## Status

Manifest only. First task: T2.4 (after T2.1 dispatcher).
