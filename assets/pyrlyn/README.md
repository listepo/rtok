# Pyrlyn logo

Parent brand of [github.com/pyrlyn](https://github.com/pyrlyn). Concept "Monolith P": a solid
geometric P with an ember cursor block at its foot. rtok keeps its own logo; this one is for
"by Pyrlyn" credits and org-level use.

## Files

| File | Use |
|---|---|
| `pyrlyn-mark-on-light.svg` / `-1000.png` | Mark, ink P + ember cursor, for light backgrounds |
| `pyrlyn-mark-on-dark.svg` / `-1000.png` | Mark, paper P + ember cursor, for dark backgrounds |
| `pyrlyn-mark-mono.svg` | Mark in `currentColor` (one path): inline SVG, masks, one-colour print |
| `pyrlyn-lockup-on-light.svg` / `-2000.png` | Mark + `pyrlyn` wordmark, for light backgrounds |
| `pyrlyn-lockup-on-dark.svg` / `-2000.png` | Mark + `pyrlyn` wordmark, for dark backgrounds |
| `pyrlyn-lockup-mono.svg` | Lockup in `currentColor` (one path) |

PNGs are RGBA with a transparent background: marks 1000×1000, lockups 2000 px wide.
The wordmark is outlined to paths, so no font is needed.

## Colours

| Token | Hex | Use |
|---|---|---|
| Ink | `#0C0E11` | P and wordmark on light backgrounds; dark background |
| Paper | `#F4F2ED` | P and wordmark on dark backgrounds; light background |
| Ember (on dark) | `#FF5A36` | Cursor block on dark backgrounds |
| Ember (on light) | `#E2431E` | Cursor block on light backgrounds |

## Usage

- The ember cursor is the only accent. Do not recolour the P, do not animate or move the cursor.
- Clear space: at least the cursor height (4 grid units, 1/8 of the mark box) on every side; the
  lockup SVGs already include 2 units.
- Minimum size: mark 16 px (built on a 32-unit grid, so every edge lands on a whole pixel at
  16, 32 and 64 px); lockup 18 px high.
- Themed pages: switch the `on-light` / `on-dark` file with the theme (see
  `site/layouts/_partials/custom/footer.html`), use `<picture>` with `prefers-color-scheme`, or
  inline the `mono` file and set `color`.

## Geometry

32×32 grid, 2 units = 1 px at 16 px. Stem x 6–12, bowl y 4–22 (outer radius 9, inner radius 3,
stroke 6), counter 8×6, cursor 6×4 at (16, 24), 2 units below the bowl.

Lockup: the P top sits on the `l` ascender and the bowl foot on the baseline, so the cursor
falls in the descender zone of `p` and `y`. Wordmark: Inter (OFL) at weight 600, x-height
12.76 units, tracking −16/2048 em, hand-kerned pairs (font units): py −58, yr −14, rl −34,
ly −22, yn −18. Mark-to-wordmark gap: 8 units.

Regenerate with `python3 assets/pyrlyn/_build/build.py` (needs `fonttools`,
`cairosvg`; point `INTER_VF` at `Inter-VariableFont_opsz,wght.ttf`).
