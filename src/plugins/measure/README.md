# `measure`

Ground truth for everything else: nothing in rtok ships until this plugin can show a
before/after number (decision D3).

| | |
|---|---|
| Kind | native |
| Surfaces | `rtok stats`, `rtok bench`, proxy `usage` capture |
| Replaces | rtk gain, headroom savings, lean-ctx gain, token-optimizer dashboard |
| Default | on |

## What it does

- Parses Claude Code transcripts (`~/.claude/projects/**/*.jsonl`): tool calls, tool
  results, `usage` blocks, turn index.
- Reads `usage` rows written by the proxy.
- Reports per-tool result sizes, Bash by command family, MCP server groups, cache hit
  rate, and **context-token-turns** (tokens × turns they stay in context).
- Saves and compares baselines (`rtok stats --save-baseline`, `--compare`).
- Optional calibration of the estimator against `count_tokens` (`--calibrate`).

## Config

None beyond `[estimator]` (chars per token per class), which `--calibrate` rewrites.

## Tasks

T1.1 JSONL parser · T1.2 `rtok stats` · T1.3 baseline · T1.5 calibration · T5.5 cache health · T9.1 `rtok bench`.

## Status

Manifest only. First task: T1.1.
