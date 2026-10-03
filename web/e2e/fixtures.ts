// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { test as base } from "@playwright/test";
import { Rtok } from "./rtok";

export { expect } from "@playwright/test";

// Test-scoped on purpose: the toggle test rewrites the store's config and the offline test
// stops the server, so no two tests may share one.
export const test = base.extend<{ rtok: Rtok }>({
  // Playwright reads the fixture dependencies from this pattern and rejects any other first argument.
  // oxlint-disable-next-line no-empty-pattern
  rtok: async ({}, use) => {
    const rtok = await Rtok.create();
    try {
      await rtok.start();
      await use(rtok);
    } finally {
      await rtok.dispose();
    }
  },
  baseURL: async ({ rtok }, use) => {
    await use(rtok.url);
  },
});
