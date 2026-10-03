// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  forceZ,
  type SimNode,
  type Simulation,
} from "d3-force-3d";
import type { Scene } from "./scene";

export interface LayoutInput {
  nodes: { id: number; radius: number; group: number }[];
  links: { from: number; to: number }[];
  /** Group anchors are used only when set: the scene is clustered. */
  groups: number;
}

/** Positions in `ids` order, `x y z` per node. */
export interface Frame {
  ids: number[];
  pos: Float32Array;
  settled: boolean;
}

export const layoutInput = (s: Scene): LayoutInput => ({
  nodes: s.nodes.map((n) => ({ id: n.id, radius: n.radius, group: n.group })),
  links: s.edges.map((e) => ({ from: e.from, to: e.to })),
  groups: s.clustered ? s.groups.length : 0,
});

/** Distance of the cluster anchors from the centre, in scene units. */
const RING = 140;

interface Body extends SimNode {
  id: number;
  radius: number;
  group: number;
}

/**
 * The 3D force simulation, with no worker or DOM so a test and the main-thread fallback run the
 * same code. It starts from d3's deterministic initial spread, so equal input settles equally.
 */
export class Layout {
  private sim: Simulation<Body>;
  private groups = 0;

  constructor(input: LayoutInput) {
    this.sim = forceSimulation<Body>([], 3).stop();
    this.update(input);
  }

  /** New input keeps the position of every node that stays, and reheats the simulation. */
  update(input: LayoutInput): void {
    const old = new Map(this.sim.nodes().map((n) => [n.id, n]));
    const nodes = input.nodes.map((n) => {
      const was = old.get(n.id);
      return { ...n, x: was?.x, y: was?.y, z: was?.z } as Body;
    });
    this.groups = input.groups;
    const links = input.links.map((l) => ({ source: l.from, target: l.to }));
    const ring = (g: number, axis: "x" | "y") => {
      const a = (g / Math.max(1, this.groups)) * Math.PI * 2;
      return RING * (axis === "x" ? Math.cos(a) : Math.sin(a));
    };
    const sim = this.sim.nodes(nodes);
    sim.force(
      "link",
      forceLink(links)
        .id((n) => (n as Body).id)
        .distance(45),
    );
    sim.force("charge", forceManyBody().strength(-70));
    sim.force(
      "collide",
      forceCollide((n) => (n as Body).radius + 3),
    );
    if (this.groups > 0) {
      sim.force("center", null);
      sim.force("x", forceX((n) => ring((n as Body).group, "x")).strength(0.08));
      sim.force("y", forceY((n) => ring((n as Body).group, "y")).strength(0.08));
      sim.force("z", forceZ(0).strength(0.08));
    } else {
      for (const f of ["x", "y", "z"]) sim.force(f, null);
      sim.force("center", forceCenter(0, 0, 0));
    }
    sim.alpha(old.size ? 0.5 : 1);
  }

  get settled(): boolean {
    return this.sim.alpha() < this.sim.alphaMin();
  }

  tick(steps: number): Frame {
    if (!this.settled) this.sim.tick(steps);
    const nodes = this.sim.nodes();
    const pos = new Float32Array(nodes.length * 3);
    nodes.forEach((n, i) => pos.set([n.x, n.y, n.z], i * 3));
    return { ids: nodes.map((n) => n.id), pos, settled: this.settled };
  }
}
