# web/

The rtok admin SPA: Vite + React + TypeScript, styled from the design tokens in
`src/styles/tokens.css`. It talks to `rtok web` over `/ws`
and `/health` (see `src/web/model.rs` for the snapshot contract).

`rtok web` serves it (T310.9): `build.rs` embeds `web/dist/` in the binary, so an installed
`rtok web` needs no files on disk. `RTOK_WEB_DIST=<dir>` makes it read a built `dist/` at run
time instead. A binary compiled before the first SPA build embeds a "UI not built" page and
still serves the API. Assets under `dist/assets/` are content-hashed and cached for a year,
`index.html` is revalidated, and the `.br`/`.gz` files `scripts/precompress.mjs` writes are
served to clients that accept them. The CSP forbids inline scripts, so the pre-paint theme
script is `public/theme-init.js`, not an inline `<script>`.

## Commands

Run from the repo root:

- `just spa-install` — install dependencies (`npm ci`)
- `just spa-dev` — Vite dev server, proxying `/ws` and `/health` to a `rtok web`
  instance on `127.0.0.1:3333`
- `just spa-build` — production build into `web/dist/` (plus `.br`/`.gz`), then `touch build.rs`
- `just web` — build the SPA and run `rtok web` on it (`RTOK_WEB_DIST=web/dist`)
- `just spa-typecheck` — `tsc --noEmit`
