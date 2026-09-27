# rtok admin responsive screenshots: final set

Source: `web/` on branch `design/web-admin` at commit 1e14425 (plain HTML + Tailwind + WebGL orb), served statically, `index.html?sample` (built-in deterministic sample snapshot).
104 PNGs: 6 screens × 8 widths (360/390/430/768/1024/1280/1440/1920) × dark/light, plus Settings and Help dialogs at 360 and 1024 in dark and light (`dialogs/`).

## How they were captured (all 104 re-captured in one deterministic run)
- Headless Google Chrome via Playwright, SwiftShader WebGL, `--force-color-profile=srgb`, `--hide-scrollbars`, DPR 1 (same as the previous set).
- Theme set the way the app reads it: `localStorage['rtok-theme']` plus `prefers-color-scheme`. Every file was checked for the correct `html.dark` class.
- CSS transitions and animations turned off for capture (`transition:none; animation:none`, no caret). Screenshots wait for `document.fonts.ready`, for the orb to be in WebGL mode, and for 2 frames after every resize.
- The orb clock is frozen (`performance.now`/rAF pinned), so every shot shows the same orb frame (t = 0). Two runs came out bit-identical.
- Full-page capture: the viewport is resized to the document height, so the fixed mobile bottom nav sits at the bottom. The base window heights are 844 (<768), 1024 (768–1279), 900 (1280–1440) and 1080 (1920).
- Exception: `calls` at 768 and above. There the ledger table is a scroll pane sized to the viewport (`max-h-[calc(100vh-13rem)]`), so a full page doesn't exist. These are shot at the standard window height, and the pane scrolls as it does in the real app.
- Dialogs are opened the way a user opens them (the Settings header button, the `?` key), on a fresh page in the default "data" state, with focus cleared (no focus ring). They are shot at the standard window height.

## Defects found in the previous set and how each was fixed
| Batch | Defect | Fix |
|---|---|---|
| dialogs (settings/help 360+1024 dark) | Rendered in **light** theme (the capture context never set the theme) | Re-captured in dark. Light versions added (4 new files). |
| dialogs | Stale **"empty" preview state** carried over from earlier `?state=empty` visits (zeros everywhere; Settings showed "empty" selected) | Fresh page per dialog, default data state. |
| dialogs | Close button showed a keyboard **focus ring**. Viewport was 800px tall instead of the breakpoint height. | Focus cleared. Shot at 844 / 1024. |
| logs 360 dark/light | **Cut off**: capped at 4200px while the page is 4992px | Full height captured. |
| calls 768/1024 dark/light | Stretched to an arbitrary 1412px with the table still cut mid-row, so neither a window nor a full page | Standard 1024px window (viewport-bound scroll pane, see above). |
| overview, sessions (all) | "live" status dot **pulse animation caught mid-cycle** (dimmed dot) | Animations off: dot shows its static state. |
| all dark+light at every width | **Orb frame differed per shot** (random animation time) | Orb clock frozen: identical orb frame everywhere. |
| all | No outer frame; edges touched the file border | Uniform **32px frame** in the page background colour: dark `rgb(6,16,26)`, light `rgb(244,248,251)` (`--canvas`). |
| all | Untagged PNGs (no colour chunk) | Tagged **sRGB**. No gAMA/cHRM/iCCP/text chunks. |

Checked and not found: scrollbars, horizontal overflow (scrollWidth equals width on every shot), mixed DPR, page errors, failed fonts, WebGL fallback.

## What was NOT changed
- No retouching. UI pixels are exactly what Chrome rendered: each file's inner area was verified byte-for-byte against the raw capture. UI colours, brightness and contrast are untouched.
- Lossless optimisation only (oxipng level 4). The files are still PNG.
- Known by design, not a capture defect: some panes scroll inside the window (the calls table at md+, the sessions detail list `max-h-64`), so their extra rows aren't visible in a still image.
