# web/

The rtok admin SPA: Vite + React + TypeScript, styled from the design tokens in
`design/html/css/` (copied into `src/styles/`). It talks to `rtok web` over `/ws`
and `/health` (see `src/web/model.rs` for the snapshot contract).

This does not replace `crates/rtok-webui` (the Slint/WASM UI) yet; both build
independently until the SPA covers the same screens.

## Commands

Run from the repo root:

- `just spa-install` — install dependencies (`npm ci`)
- `just spa-dev` — Vite dev server, proxying `/ws` and `/health` to a `rtok web`
  instance on `127.0.0.1:3333`
- `just spa-build` — production build into `web/dist/`
- `just spa-typecheck` — `tsc --noEmit`
