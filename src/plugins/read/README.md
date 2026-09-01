# `read`

Five MCP tools instead of seventy-eight, and no per-turn banner.

| | |
|---|---|
| Kind | native |
| Surfaces | MCP `read`, `search`, `tree`; PreToolUse(Read) advice |
| Replaces | lean-ctx ctx_read/ctx_search/ctx_tree, token-optimizer read_cache/structure_map |
| Default | on |

## Tools

- `read(path, mode=full|lines|map|signatures, range?)` — numbered lines; `map` and
  `signatures` come from tree-sitter tags queries (Rust, TS, JS, Python, Dart, C, Go);
  unknown language → first 60 lines + note. Output over 20 K chars → head/tail + archive id.
- `search(pattern, path, max=50)` — regex over files respecting `.gitignore`;
  `path:line: snippet` (≤ 120 chars).
- `tree(path, depth=2)` — compact listing with sizes.
- Re-read dedup: same session, same path, same sha256, same mode/range →
  `unchanged since <archive_id> (N lines)`. Invalidated by PostToolUse(Edit|Write).
- PreToolUse(Read) advice: native `Read` of a file > 32 K that was not edited in the last
  5 turns is denied with "use rtok read(mode=map) first". Never for files under 32 K.

Root guard: paths must be under cwd or `allow_paths`.

## Config

```toml
[plugins.read]
enabled = true
native_max_bytes = 32768     # PreToolUse(Read) deny threshold
allow_paths = []             # extra roots outside cwd
```

## Tasks

T4.2 full/lines · T4.3 map/signatures · T4.4 dedup · T4.5 search + tree · T4.6 Read advice.

## Status

Manifest only. First task: T4.2 (after T4.1 `rtok mcp`).
