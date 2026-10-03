// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// @vitest-environment happy-dom
import { expect, test, vi } from "vitest";
import { project } from "../../api/sampleRows";
import type { Frame } from "./layout";
import { layoutInput } from "./layout";
import { startLayout } from "./layoutClient";
import { buildScene } from "./scene";

const input = layoutInput(
  buildScene(
    [
      project(1, "a", {
        selected: true,
        links: [{ kind: "manual", name: "b", reason: null, to: 2 }],
      }),
      project(2, "b"),
    ],
    {
      query: "",
      scopeOnly: false,
    },
  ),
);

test("without a worker the layout steps on the main thread, settles and stops", async () => {
  vi.useFakeTimers();
  const frames: Frame[] = [];
  const client = startLayout((f) => frames.push(f));
  client.push(input);
  await vi.advanceTimersByTimeAsync(10_000);
  expect(frames.at(-1)?.settled).toBe(true);
  const n = frames.length;
  await vi.advanceTimersByTimeAsync(10_000);
  expect(frames).toHaveLength(n);
  client.stop();
  vi.useRealTimers();
});

test("stop ends a running layout before it settles", async () => {
  vi.useFakeTimers();
  const frames: Frame[] = [];
  const client = startLayout((f) => frames.push(f));
  client.push(input);
  await vi.advanceTimersByTimeAsync(40);
  client.stop();
  const n = frames.length;
  await vi.advanceTimersByTimeAsync(10_000);
  expect(frames).toHaveLength(n);
  vi.useRealTimers();
});
