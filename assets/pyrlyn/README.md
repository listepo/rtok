# Pyrlyn logo

Parent brand of [github.com/pyrlyn](https://github.com/pyrlyn). Concept "Prompt": the shell prompt
`>_` reduced to a chevron and an amber cursor bar. Pyrlyn's tools are CLIs, so the parent mark is
the place they all run. Product logos stay primary; this one is for "by Pyrlyn" credits and
org-level use.

## Files

| File | Use |
|---|---|
| `pyrlyn-mark-on-light.svg` / `-1000.png` | Mark, ink chevron + amber cursor, for light backgrounds |
| `pyrlyn-mark-on-dark.svg` / `-1000.png` | Mark, paper chevron + amber cursor, for dark backgrounds |
| `pyrlyn-mark-mono.svg` | Mark in `currentColor` (one path): inline SVG, masks, one-colour print |
| `pyrlyn-lockup-on-light.svg` / `-2000.png` | Mark + `pyrlyn` wordmark, for light backgrounds |
| `pyrlyn-lockup-on-dark.svg` / `-2000.png` | Mark + `pyrlyn` wordmark, for dark backgrounds |
| `pyrlyn-lockup-mono.svg` | Lockup in `currentColor` (one path) |
| `pyrlyn-favicon.svg` | Favicon; switches ink/paper with `prefers-color-scheme` |
| `pyrlyn-favicon-32.png` / `-64.png` | PNG favicons: paper mark on an ink rounded tile |
| `pyrlyn-apple-touch-icon.png` | 180×180 opaque ink square (iOS adds the rounding) |

Mark and lockup PNGs are RGBA with a transparent background: marks 1000×1000, lockups 2000 px
wide. The wordmark is outlined to paths, so no font is needed.

## Colours

| Token | Hex | Use |
|---|---|---|
| Ink | `#0C0E11` | Mark and wordmark on light backgrounds; dark background |
| Paper | `#F4F2ED` | Mark and wordmark on dark backgrounds; light background |
| Amber (on dark) | `#F2B33D` | Cursor bar on dark backgrounds |
| Amber (on light) | `#B97C06` | Cursor bar on light backgrounds |

## Usage

- The amber cursor is the only accent. Do not recolour the chevron or animate the cursor in the
  logo itself.
- Clear space: at least the cursor height (4 grid units, 1/8 of the mark box) on every side; the
  lockup SVGs already include 2 units.
- Minimum size: mark 16 px; lockup 16 px high.
- Themed pages: switch `on-light` / `on-dark` with the theme, use `<picture>` with
  `prefers-color-scheme`, or inline the `mono` file and set `color`.

## Geometry

32×32 grid, 2 units = 1 px at 16 px. Chevron `M4 6H10L20 16L10 26H4L14 16Z`: 45° arms, 6 units
thick horizontally, flat ends on even rows (y 6 and 26). Cursor 10×4 at (18, 22), bottom-aligned
with the chevron, so both land on whole pixels at 16 and 32 px.

Lockup: the `l` ascender sits on the chevron top and the baseline on the cursor top, so the cursor
reads as the underscore and the chevron centre matches the x-height centre. Wordmark: JetBrains
Mono (OFL) weight 600, hand-kerned for even gaps (font units, 1000 upm): py −21, yr −35, rl +12,
ly −5, yn −22. Cursor-to-wordmark gap: 7 units.

Regenerate with `python3 assets/pyrlyn/_build/build.py` (needs `fonttools`, `cairosvg`; point
`JBMONO_VF` at `JetBrainsMono-VariableFont_wght.ttf`).
