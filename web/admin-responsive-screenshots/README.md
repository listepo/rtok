# rtok admin: responsive screenshot set

These are reference screenshots of the rtok web admin (plain HTML + Tailwind + WebGL orb). They show the real UI at every supported breakpoint, in both themes. The admin app itself is in draft PR #447 (branch `design/web-admin`, captured at commit `1e14425`).

104 PNGs. Files are named `<screen>-<width>-<theme>.png` and grouped by screen:

| Folder | What it shows |
|---|---|
| `overview/` | Usage totals, Δ saved tokens, cache hit, calls over time, tokens, top plugins by savings, doctor summary, recent sessions |
| `plugins/` | Plugin catalogue with filters and enable toggles, plus the Measurement stats of the selected plugin |
| `calls/` | Ledger rows, newest first, with surface/status filters and the call detail panel. Cards on phones, a table from 768px |
| `sessions/` | One row per session (live/ended), with the detail and per-session calls of the selected one |
| `doctor/` | Hooks, MCP servers, proxy chains, environment and instruction audit, agents × modules matrix |
| `logs/` | Log lines, newest first (the lines `rtok logs` shows), with level filter |
| `dialogs/` | The Settings dialog (theme, orb, opaque panels, preview state) and the Help dialog (keyboard shortcuts), at 360 and 1024 |

- Breakpoints: 360, 390, 430 (phones: bottom tab bar), 768 (tablet), 1024 (compact sidebar), and 1280, 1440, 1920 (desktop: sidebar plus detail column).
- Themes: `dark` and `light` for every screen and dialog.
- Data: the app's built-in deterministic sample snapshot (`index.html?sample`), labelled "sample data" in the UI.

## How they were captured

- Headless Google Chrome via Playwright, SwiftShader WebGL, `--force-color-profile=srgb`, scrollbars hidden, device pixel ratio 1.
- The theme is set the way the app reads it (`localStorage['rtok-theme']` + `prefers-color-scheme`). CSS transitions and animations are off during capture. Each shot waits for web fonts and for the orb's first WebGL frame. The orb clock is pinned, so every image shows the same orb frame, and two runs came out bit-identical.
- Screens are captured full-page: the window grows to the document height, so the phone tab bar sits at the bottom. Base window heights: 844 (phones), 1024 (768–1279), 900 (1280–1440) and 1080 (1920).
- Exception: `calls` at 768 and above. There the table is a scroll pane sized to the window, so a full page doesn't exist and these are shot at the standard window height.
- Dialogs are opened the way a user opens them (the Settings button, the `?` key), with focus cleared, at the standard window height.

## What was polished

- **Consistent framing and alignment:** every image is the exact viewport width for its breakpoint, with a uniform 32px outer frame in the page background colour (dark `rgb(6,16,26)`, light `rgb(244,248,251)`). Nothing is cropped.
- **Artifact cleanup by re-capture, not retouching:** dialogs that had come out in the wrong theme or state were re-captured in dark (light versions were added). The same went for a page cut off at a height cap, "live" dots caught mid-pulse, orb frames that differed from shot to shot, a stray focus ring, and inconsistent heights.
- **Colour normalization:** every file carries one sRGB profile (the `sRGB` chunk, no gAMA/iCCP), so they look the same in every viewer. **UI colours were not altered.** Inside the frame, every pixel is exactly what Chrome rendered, checked byte-for-byte against the raw capture.
- Lossless PNG optimisation (oxipng).

`CHANGES.md` lists each defect found in the previous set and how it was fixed.
