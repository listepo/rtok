# `toon`

Tabular JSON → TOON (Token-Oriented Object Notation). On by default (T127); a block is
rewritten only when the encoding is smaller than the JSON it replaces.

| | |
|---|---|
| Surfaces | proxy `compress` mode; MCP tool results |
| Spec | the `spec (replaces)` column of the catalogue in `plan.md` §1 |
| Default | **off** |

## Mechanism

Arrays of uniform objects (same keys, scalar values) are re-encoded as a header row plus one
line per record. Vendor benchmark: −42.6 % tokens on tabular data; unmeasured here, hence off.
Non-tabular JSON is left untouched. The original is archived; the encoded block carries an
`expand` id.

## Config

```toml
[plugins.toon]
enabled = true
min_rows = 5
```

## Tasks

See `roadmap.md` § `toon`. Checks in `plan.md`.

T11.7 `proxy_filter` TOON on tabular JSON (default on). A table that would not shrink is left as is.

## Status

Manifest only.
