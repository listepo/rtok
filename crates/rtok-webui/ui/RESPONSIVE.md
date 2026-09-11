# Bitset B — responsive shell

Layout/density only. Brand colors, F icons, and IBM Plex Mono stay fixed.

## Shared breakpoints

| Token | Range | Site CSS | Slint `RtokTheme` |
|-------|-------|----------|-------------------|
| `sm` / mobile | &lt; 640px | `--rtok-bp-sm` | `bp-sm` |
| `md` / tablet | 640–1023px | `--rtok-bp-md` | `bp-md` |
| `lg` / desktop | 1024–1439px | — | (≥ `bp-md`) |
| `xl` / wide | 1440–1919px | `--rtok-bp-lg` | `bp-lg` |
| `2xl` / ultrawide+TV | ≥ 1920px (to ~3840) | `--rtok-bp-xl` | `bp-xl` |

## Site (`site/assets/css/custom.css`)

- Hero shot fluid: full bleed + smaller radius/shadow on mobile; `min(100%, 1280px)` from lg; `min(100%, 1440px)` on 2xl.
- Feature grid capped ~1320–1400px centered.
- Prose/subtitle max-width ~42rem; docs tables keep horizontal scroll with tighter `td/th` min-width on narrow.
- Coarse pointer: feature icon hit area ≥ 44px.
- `.rtok-home-hero` uses `env(safe-area-inset-*)`.
- OLED: keep near-black `#06101A`; `@media (dynamic-range: high)` left alone.

## Slint webui (`theme.slint` + `app.slint`)

Derived from tracked `win-w` / `win-h` (updated via `changed width`/`height` + `init` to avoid Slint layoutinfo binding loops):

| Width | Nav | Content |
|-------|-----|---------|
| &lt; 640 (`compact`) | Top horizontal icon bar (icons only, 44px hits) | Full width; plugins/calls **stacked** (list then detail) |
| 640–1023 (`rail`) | 56px rail, icons only | Side-by-side panes |
| ≥ 1024 (`full-nav`) | 200px sidebar with labels + status | Side-by-side |
| ≥ 1440 (`wide`) | Full sidebar | More padding; plugin list ~240–280; calls list `min(480, 42% width)` |
| ≥ 1920 (`ultrawide`) | Full sidebar | `space-5` padding; title ~26px; body +1px; content `max-width` ~1600px |

- Window: `min-width: 320px`, `min-height: 480px`; preferred ~960×640.
- `TokenStatsWidget`: wraps to 2×3 when strip width &lt; 520px.
- Exported structs and MainWindow in/in-out property names unchanged for Rust bindings.

## Verify

```bash
slint-viewer --check ui/app.slint
```

(from `crates/rtok-webui`)
