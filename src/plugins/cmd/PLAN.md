# cmd — design note (D15)

## Problem

Bash is ~1.0 M estimated tokens in the 17-session slice (`research.md` §2) and three filters already fight over the same stdout (rtk, lean-ctx `ctx_shell`, token-optimizer `bash_compress`). Lossy drops hide errors; expand rate is unmeasured.

## Alternatives

| Tool | Version | Date | Gets right | Gets wrong |
|------|---------|------|------------|------------|
| rtk filters | rtk-ai 2026-09-01 | 2026-09-01 | ~80 family filters; real bash-byte cut | not lossless; custom TOML; no archive id |
| lean-ctx ctx_shell | 2026-09-01 | 2026-09-01 | 95+ shell patterns; MCP-shaped | +3.1 K/turn injection; 0 output saved here |
| token-optimizer bash_compress | 2026-09-01 | 2026-09-01 | 111 cmds; archives raw | Python subprocess on every Bash; 27 hooks |
| cargo --message-format=json | rustc 1.90 | 2026-09-02 | structured compiler output; errors stay typed | only cargo; not a general shell filter |

## Mechanism

Per-family formatters (cargo/pytest/jest/git/…) with a generic head/tail/dedupe fallback. Archive the raw result first; put `expand <id>` in the shortened text. Crossover: generic rule is enough under ~4 KiB; family formatters pay off above that. On non-zero exit, never drop stderr, the exit line, or the last failing command.

The property that beats the table: lossless by default (D2) with a measured expand rate, not a silent drop.

## Rejected

- One generic head/tail for all families — cargo JSON and pytest `-q` are cheaper and keep errors.
- Delegating to rtk at runtime (D6) — rtk is the spec, not a subprocess.

Target: ≥ 40 % byte cut on a named Bash corpus (rtk's measured 40 % of bash bytes, `research.md` §4) at expand rate < 5 % (P3 gate).

Falsified by: expand rate ≥ 5 % on a working day, or a failed command whose error line was stripped.
