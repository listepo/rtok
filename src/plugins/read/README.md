# `read`

Five MCP tools instead of seventy-eight, and no per-turn banner.

| | |
|---|---|
| Surfaces | MCP `read`, `search`, `tree`; PreToolUse(Read) advice |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | on |

## Tools

- `read(path, mode=full|lines|map|signatures|diff|stripped, range?)` — numbered lines; `map` and
  `signatures` come from tree-sitter tags queries (Rust, TS, JS, Python, Dart, C, Go, Java, Kotlin, Swift, C#, Ruby, PHP) or a
  Markdown heading scan (`.md`, `.mdx`: level, line number, first body line; fenced blocks
  skipped); `stripped` drops comment nodes (same grammars; unknown language or parse fail → `full`);
  unknown language for `map`/`signatures` → first 60 lines + note. Output over 20 K chars → head/tail +
  archive id. `diff` is the edit → verify form of the automatic delta below.
- `search(pattern, path, max=50)` — regex over files respecting `.gitignore`;
  `path:line: snippet` (≤ 120 chars).
- `tree(path, depth=2)` — compact listing with sizes.
- Re-read dedup: same session, same path, same sha256, same mode/range →
  `unchanged since <archive_id> (N lines)` (≤ 13 estimated tokens on the T58.1 fixture).
- Changed re-read (T58.1): a later `read` of a file that changed since the last archive
  returns a unified diff plus `previous <id>` / `expand <id>` of the full file, when the
  diff is below `delta_max_ratio` of the file (default 0.6). Measured 2026-09-18
  (`rtok stats --since 90d`, 959 sessions): 593 such re-reads, **7.3 %** of Read bytes.
- PreToolUse(Read) advice: native `Read` of a file > 32 K that was not edited in the last
  5 turns is denied with "use rtok read(mode=map) first". After an Edit of a file already
  in the read cache, the deny points at `read(mode=diff)` instead. Never for files under 32 K.

Root guard: paths must be under cwd or `allow_paths`.

## Config

```toml
[plugins.read]
enabled = true
native_max_bytes = 32768     # PreToolUse(Read) deny threshold
allow_paths = []             # extra roots outside cwd
delta = true                 # T58.1: changed re-read → unified diff (7.3 % of Read bytes)
delta_max_ratio = 0.6        # full file when the diff is not below this fraction
```

## Tasks

See `roadmap.md` § `read`. Checks in `plan.md`.

T4.2 full/lines · T4.3 map/signatures · T4.4 dedup · T4.5 search + tree · T4.6 Read advice · T50.3 stripped.

## Status

Manifest only. First task: T4.2 (after T4.1 `rtok mcp`).
