// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { readFileSync } from "node:fs";
import { PAGES } from "../src/pages";
import { expect, test } from "./fixtures";
import { ARCHIVED_MARKER } from "./rtok";

// `PAGES` mirrors `model::pages()`; `tests/surface_parity.rs` pins the two together, so this
// loop covers every page the server knows.
for (const { id } of PAGES) {
  test(`${id} page renders from the live snapshot`, async ({ page }) => {
    const problems: string[] = [];
    page.on("pageerror", (e) => problems.push(`pageerror: ${e.message}`));
    // A CSP violation or a failed asset load reaches the console as an error.
    page.on("console", (m) => m.type() === "error" && problems.push(`console: ${m.text()}`));

    await page.goto(`/#/${id}`);

    await expect(page.getByRole("heading", { level: 1 })).toHaveText(id);
    await expect(page).toHaveTitle(`${id} · rtok`);
    await expect(
      page.getByRole("navigation", { name: "Admin screens" }).getByRole("link", { name: id }),
    ).toHaveAttribute("aria-current", "page");
    await expect(page.getByRole("banner").getByText("open", { exact: true })).toBeVisible();
    const main = page.getByRole("main");
    await expect(main.locator("[aria-busy]")).toHaveCount(0);
    await expect(main.getByText("Something went wrong")).toHaveCount(0);
    await expect(main.getByText("Offline")).toHaveCount(0);
    await expect(main.getByText(`No ${id} data yet`)).toHaveCount(0);
    expect(problems).toEqual([]);
  });
}

test("plugin toggle round-trips through /ws and the config file", async ({ page, rtok }) => {
  await page.goto("/#/plugins");
  const toon = page.getByRole("switch", { name: "toggle toon" });
  await expect(toon).toHaveAttribute("aria-checked", "true");

  await toon.click();
  // The switch only asks: it moves once the next snapshot from the server says so.
  await expect(toon).toHaveAttribute("aria-checked", "false");
  await expect
    .poll(() => readFileSync(rtok.configPath, "utf8"))
    .toMatch(/\[plugins\.toon\][^[]*enabled\s*=\s*false/);

  await toon.click();
  await expect(toon).toHaveAttribute("aria-checked", "true");
  await expect
    .poll(() => readFileSync(rtok.configPath, "utf8"))
    .toMatch(/\[plugins\.toon\][^[]*enabled\s*=\s*true/);
});

test("expand returns the archived payload of a call", async ({ page }) => {
  await page.goto("/#/calls");
  await page.getByRole("row", { name: /mem_save/ }).click();

  const expand = page.getByRole("button", { name: /^expand [0-9a-f]{64}$/ });
  await expand.click();

  const payload = page.locator("pre");
  await expect(payload).toContainText(ARCHIVED_MARKER);
  await expect(page.getByText(/\d+ lines/)).toBeVisible();
});

test("offline banner shows when the server stops and clears when it returns", async ({
  page,
  rtok,
}) => {
  await page.goto("/#/overview");
  const connection = page.getByRole("banner").getByText(/^(open|closed|connecting)$/);
  await expect(connection).toHaveText("open");
  const offline = page.getByRole("main").getByText("Offline", { exact: true });
  await expect(offline).toHaveCount(0);

  await rtok.stop();
  await expect(offline).toBeVisible();
  await expect(connection).not.toHaveText("open");

  await rtok.start();
  // The client retries with backoff capped at 10 s (web/src/api/ws.ts).
  await expect(connection).toHaveText("open", { timeout: 20_000 });
  await expect(offline).toHaveCount(0);
});
