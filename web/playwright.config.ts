// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { defineConfig, devices } from "@playwright/test";

// Same switch the Storybook browser tests use (vite.config.ts): a machine without Playwright's
// own Chromium points it at an installed Chrome.
const channel = process.env["SPA_BROWSER_CHANNEL"] || undefined;

// Every test starts its own `rtok web` (e2e/rtok.ts), so there is no `webServer` here: one
// shared server could not be stopped for the offline test or mutated by the toggle test.
export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  // Each test owns an `rtok web` whose first snapshot scans the store and the host list; 13 of
  // them at once starved each other past the default 5 s expectation.
  workers: 4,
  expect: { timeout: 15_000 },
  forbidOnly: Boolean(process.env["CI"]),
  // A retry would hide a flaky round-trip; the suite must be green on the first run.
  retries: 0,
  reporter: process.env["CI"] ? [["list"], ["html", { open: "never" }]] : "list",
  use: { trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], channel } }],
});
