// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { type Frame, Layout, type LayoutInput } from "./layout";

export interface LayoutClient {
  push(input: LayoutInput): void;
  /** Ends the worker (or the fallback timer); nothing runs afterwards. */
  stop(): void;
}

/**
 * Runs the layout in a web worker so the page never waits on it. Without `Worker` (a test
 * environment) or when the worker cannot start (a blocking CSP), the same `Layout` steps on the
 * main thread in small slices.
 */
export function startLayout(onFrame: (f: Frame) => void): LayoutClient {
  let stopped = false;
  let worker: Worker | undefined;
  let local: Layout | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let last: LayoutInput | undefined;

  const slice = () => {
    timer = undefined;
    if (stopped || !local) return;
    const frame = local.tick(4);
    onFrame(frame);
    if (!frame.settled) timer = setTimeout(slice, 16);
  };
  const runLocal = (input: LayoutInput) => {
    if (local) local.update(input);
    else local = new Layout(input);
    timer ??= setTimeout(slice, 0);
  };
  try {
    if (typeof Worker !== "undefined") {
      worker = new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" });
      worker.onmessage = (e: MessageEvent<Frame>) => !stopped && onFrame(e.data);
      worker.onerror = () => {
        worker?.terminate();
        worker = undefined;
        if (last && !stopped) runLocal(last);
      };
    }
  } catch {
    worker = undefined;
  }
  return {
    push(input) {
      last = input;
      if (worker) worker.postMessage(input);
      else runLocal(input);
    },
    stop() {
      stopped = true;
      worker?.terminate();
      if (timer !== undefined) clearTimeout(timer);
    },
  };
}
