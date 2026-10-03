// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

/**
 * Whether a WebGL context can be made here. Checked before the 3D chunk is downloaded, so a
 * blocked or absent GPU never pays for Three.js; the probe context is released at once.
 */
export function webglAvailable(): boolean {
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2") ?? canvas.getContext("webgl");
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
    return gl !== null && gl !== undefined;
  } catch {
    return false;
  }
}

/** `prefers-reduced-motion`: no camera fly-to, the view jumps. */
export function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/** What both views offer the page's buttons and the tests. */
export interface ViewApi {
  fit(): void;
  reset(): void;
  focus(id: number): void;
  /** Client coordinates of a node's centre, or null while it has no position. */
  screenOf(id: number): { x: number; y: number } | null;
}

export interface Hover {
  text: string;
  x: number;
  y: number;
}

export interface ViewEvents {
  select(id: number): void;
  menu(id: number, x: number, y: number): void;
  hover(h: Hover | null): void;
}
