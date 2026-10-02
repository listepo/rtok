import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";
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
  test: {
    projects: [
      {
        extends: true,
        test: { name: "unit", include: ["src/**/*.test.{ts,tsx}"] },
      },
      {
        // Every story is a browser test (render, play function, axe). Needs Chromium:
        // `npx playwright install chromium`, or RTOK_BROWSER_CHANNEL=chrome for the system one.
        extends: true,
        plugins: [storybookTest({ configDir: ".storybook" })],
        test: {
          name: "storybook",
          setupFiles: [".storybook/vitest.setup.ts"],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright({
              launchOptions: { channel: process.env["RTOK_BROWSER_CHANNEL"] || undefined },
            }),
            instances: [{ browser: "chromium" }],
          },
        },
      },
    ],
  },
});
