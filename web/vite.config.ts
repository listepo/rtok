// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// `rtok web` (justfile) serves the API + WS on 127.0.0.1:3333; the dev server proxies
// both so `npm run dev` can point at a running rtok host without a rebuild.
export default defineConfig({
  base: "/",
  plugins: [react(), tailwindcss()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/ws": { target: "http://127.0.0.1:3333", ws: true },
      "/health": { target: "http://127.0.0.1:3333" },
    },
  },
});
