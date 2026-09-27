# rtok — HTML design

Full static HTML/CSS design of rtok: every screen of the web admin, the public site,
the terminal UI, the component kit, light/dark tokens and all brand assets. No server,
no network, no build step to view it.

## Open it

Double-click `index.html` (or `open design/html/index.html` on macOS). Every page works
from `file://`; fonts, CSS, JS and images are all local. The theme button in the top bar
switches dark/light and is remembered (`localStorage` key `rtok-theme`); the default
follows the OS.

## What's here

| Path | Contents |
|------|----------|
| `index.html` | page index with the source of each screen |
| `tokens.html` | colour, type, spacing, radius, elevation and motion tokens, both themes |
| `components.html` | buttons, inputs, select, switch, checkbox, chips, badges, table, cards, navigation, toasts, banners, modal, empty/loading/error — with default, hover, focus-visible, disabled and error states |
| `assets.html` | every logo, wordmark, favicon, icon, image and font, with its source path |
| `admin/*.html` | the 13 web admin pages in `model::pages()` order (overview … worktrees); `?state=empty\|loading\|error` shows the other states |
| `site/index.html`, `site/docs.html`, `site/404.html` | the Hugo + Hextra public site: landing, docs layout, not found |
| `tui.html` | `rtok tui` (ratatui) usage and calls tabs |
| `auth/sign-in.html` | **concept** — rtok has no sign-in today (loopback + Origin check only) |
| `assets/` | `logo/` (site/static, webui logo, brand PNG exports), `icons/ui` (Slint webui icons), `icons/feature` (site feature icons), `images/` (site/static/images) |
| `fonts/` | IBM Plex Mono woff2 + OFL licence (brand repo) |
| `css/tokens.css`, `css/fonts.css`, `css/tailwind-v4.css` | copied verbatim from the brand repo `dist/` |
| `css/app.css` | compiled Tailwind v4 output (committed so the pages open without a build) |
| `src/app.input.css` | Tailwind v4 entry: brand theme + the admin component classes |
| `js/` | `design.js` (theme), `admin.js` (admin screens + sample data), `orb.js` (background orb), `components.js` (component demos) |

## Data

All numbers are sample data shaped like the real payloads (`Snapshot` in `src/web/model.rs`,
the CLI text of `rtok stats`, `graph status`, `agents list`, `config show --sources`,
`demon status`, `otel status`, `worktree list`). Nothing talks to a running rtok.

## Rebuild the CSS

Only needed after editing `src/app.input.css` or adding classes to the HTML/JS:

```bash
cd design/html
npx @tailwindcss/cli@4 -i src/app.input.css -o css/app.css --minify
```

Tailwind v4 scans `*.html`, `admin/`, `site/`, `auth/` and `js/` (see `@source` in the
entry file). With a warm npm cache this runs offline.

## Known gaps

- `graph`, `hosts`, `config`, `services` and `worktrees` have no product icon yet (the
  Slint webui falls back to the overview icon); the design uses two-letter monograms.
- The TUI's first tab is titled `usage`; the web route is `overview`.
- External links (GitHub, releases) are shown as text only so the design stays offline.
