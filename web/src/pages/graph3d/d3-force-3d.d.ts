// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// d3-force-3d ships no types; this is the part the layout uses.
declare module "d3-force-3d" {
  export interface SimNode {
    index?: number;
    x: number;
    y: number;
    z: number;
    [key: string]: unknown;
  }
  export interface Force {
    strength(s: number | ((n: never) => number)): this;
    [key: string]: unknown;
  }
  export interface Simulation<N extends SimNode> {
    nodes(): N[];
    nodes(nodes: N[]): this;
    force(name: string): Force | undefined;
    force(name: string, force: Force | null): this;
    alpha(): number;
    alpha(a: number): this;
    alphaMin(): number;
    alphaDecay(d: number): this;
    stop(): this;
    tick(iterations?: number): this;
  }
  export function forceSimulation<N extends SimNode>(nodes?: N[], dims?: number): Simulation<N>;
  export interface LinkForce extends Force {
    id(f: (n: never) => unknown): this;
    distance(d: number | ((l: never) => number)): this;
  }
  export function forceLink<L>(links?: L[]): LinkForce;
  export function forceManyBody(): Force & { distanceMax(d: number): Force };
  export function forceCenter(x?: number, y?: number, z?: number): Force;
  export function forceCollide(r?: number | ((n: never) => number)): Force;
  export function forceX(x?: number | ((n: never) => number)): Force;
  export function forceY(y?: number | ((n: never) => number)): Force;
  export function forceZ(z?: number | ((n: never) => number)): Force;
}
