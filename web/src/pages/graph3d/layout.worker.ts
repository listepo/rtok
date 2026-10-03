// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { Layout, type LayoutInput } from "./layout";

let layout: Layout | undefined;
let timer: ReturnType<typeof setInterval> | undefined;

function step() {
  if (!layout) return;
  const frame = layout.tick(4);
  (self as unknown as Worker).postMessage(frame, [frame.pos.buffer]);
  // A settled layout stops its timer: an idle page costs no CPU.
  if (frame.settled && timer !== undefined) {
    clearInterval(timer);
    timer = undefined;
  }
}

self.onmessage = (e: MessageEvent<LayoutInput>) => {
  if (layout) layout.update(e.data);
  else layout = new Layout(e.data);
  timer ??= setInterval(step, 16);
};
