# `toon`

Tabular JSON → TOON (Token-Oriented Object Notation). Off by default until an A/B run shows it
pays for itself on this workload.

| | |
|---|---|
| Kind | native |
| Surfaces | proxy `compress` mode; MCP tool results |
| Replaces | caveman toon, TOON |
| Default | **off** |

## Mechanism

Arrays of uniform objects (same keys, scalar values) are re-encoded as a header row plus one
line per record. Vendor benchmark: −42.6 % tokens on tabular data; unmeasured here, hence off.
Non-tabular JSON is left untouched. The original is archived; the encoded block carries an
`expand` id.

## Config

```toml
[plugins.toon]
enabled = false
min_rows = 5
```

## Tasks

See `roadmap.md` § `toon`. Checks in `plan.md`.

T11.7 `proxy_filter` TOON on tabular JSON (default off). Enable for traffic after T9.1 can A/B it.

## Status

Manifest only.
