// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { LinkKind, Origin, ProjectRow } from "../../api/snapshot.gen";

/** Above this many projects the layout groups them by origin (T329 §8a). */
export const CLUSTER_ABOVE = 50;

/**
 * Every edge has this width: the snapshot carries no cross-project reference counts yet, so
 * there is nothing to scale by. A count per link replaces it when the server reports one.
 */
export const EDGE_WIDTH = 2;

export const ORIGINS: Origin[] = ["manual", "session", "worktree", "mcp", "reference"];

export interface SceneNode {
  id: number;
  label: string;
  root: string;
  /** CSS colour, stable per root so a project keeps its colour across sessions. */
  color: string;
  radius: number;
  state: string;
  origin: Origin;
  /** Drawn hollow and never opened: the directory is gone. */
  hollow: boolean;
  selected: boolean;
  inScope: boolean;
  /** Outside the selected project's scope: shown, but faded. */
  dim: boolean;
  /** Index into `Scene.groups` when the layout is clustered, else 0. */
  group: number;
}

export interface SceneEdge {
  id: string;
  from: number;
  to: number;
  kind: LinkKind;
  dashed: boolean;
  width: number;
  reason: string | null;
  /** Both ends are in the selected scope. */
  inScope: boolean;
}

export interface SceneCounts {
  total: number;
  inScope: number;
  pairs: number;
  problems: number;
}

export interface Scene {
  nodes: SceneNode[];
  edges: SceneEdge[];
  counts: SceneCounts;
  clustered: boolean;
  /** The origins present, in `ORIGINS` order; a clustered node's `group` indexes this. */
  groups: Origin[];
}

export interface SceneOptions {
  /** Hide projects whose name or root lacks this text. */
  query: string;
  /** Hide everything outside the selected project's scope. */
  scopeOnly: boolean;
}

/** A hue from the root, so the colour needs no stored value and is the same on every page. */
export function colorOf(root: string): string {
  let h = 2166136261;
  for (let i = 0; i < root.length; i++) h = Math.imul(h ^ root.charCodeAt(i), 16777619);
  return `hsl(${(h >>> 0) % 360} 62% 56%)`;
}

/** Symbols are the size: a log scale keeps one huge project from hiding the rest. */
export function radiusOf(rows: number | undefined): number {
  return 3 + Math.min(7, Math.log10(1 + (rows ?? 0)) * 1.6);
}

/**
 * Everything reachable from `from` through links, `from` included. Cycles end at the first
 * revisit, and a link to an id the registry no longer holds is ignored.
 */
export function scopeOf(rows: ProjectRow[], from: number | undefined): Set<number> {
  const known = new Map(rows.map((p) => [p.id, p]));
  const seen = new Set<number>();
  const todo = from !== undefined && known.has(from) ? [from] : [];
  while (todo.length) {
    const id = todo.pop()!;
    if (seen.has(id)) continue;
    seen.add(id);
    for (const l of known.get(id)!.links) if (known.has(l.to) && !seen.has(l.to)) todo.push(l.to);
  }
  return seen;
}

const isProblem = (p: ProjectRow) => p.missing || p.state === "failed" || p.state === "missing";

function matches(p: ProjectRow, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || `${p.name} ${p.root}`.toLowerCase().includes(q);
}

/** Registry rows to what the views draw. No WebGL, no DOM: the same input always gives the same scene. */
export function buildScene(rows: ProjectRow[], opts: SceneOptions): Scene {
  const selected = rows.find((p) => p.selected)?.id;
  const scope = scopeOf(rows, selected);
  const hasScope = selected !== undefined;
  const shown = rows.filter(
    (p) => matches(p, opts.query) && (!opts.scopeOnly || !hasScope || scope.has(p.id)),
  );
  const ids = new Set(shown.map((p) => p.id));
  const clustered = rows.length > CLUSTER_ABOVE;
  const groups = ORIGINS.filter((o) => rows.some((p) => p.origin === o));
  const nodes = shown.map<SceneNode>((p) => ({
    id: p.id,
    label: p.name,
    root: p.root,
    color: colorOf(p.root),
    radius: radiusOf(p.index?.rows),
    state: p.state,
    origin: p.origin,
    hollow: p.missing,
    selected: p.selected,
    inScope: scope.has(p.id),
    dim: hasScope && !scope.has(p.id),
    group: clustered ? groups.indexOf(p.origin) : 0,
  }));
  const edges: SceneEdge[] = [];
  for (const p of shown) {
    for (const l of p.links) {
      if (!ids.has(l.to)) continue;
      edges.push({
        id: `${p.id}>${l.to}`,
        from: p.id,
        to: l.to,
        kind: l.kind,
        dashed: l.kind === "auto",
        width: EDGE_WIDTH,
        reason: l.reason,
        inScope: scope.has(p.id) && scope.has(l.to),
      });
    }
  }
  // Counters describe the registry, not the filtered view, so a filter never changes them.
  const pairs = new Set(
    rows.flatMap((p) =>
      p.links
        .filter((l) => rows.some((q) => q.id === l.to))
        .map((l) => [p.id, l.to].sort().join("-")),
    ),
  );
  return {
    nodes,
    edges,
    clustered,
    groups,
    counts: {
      total: rows.length,
      inScope: scope.size,
      pairs: pairs.size,
      problems: rows.filter(isProblem).length,
    },
  };
}

/**
 * What a rebuild must notice: the view changes when this text does, so a snapshot that only
 * moves `last_used_at` neither re-creates the 3D objects nor restarts the layout.
 */
export function signature(scene: Scene): string {
  return JSON.stringify([scene.nodes, scene.edges, scene.clustered]);
}

/** What the layout depends on: moving the selection or fading a node must not re-heat it. */
export function topology(scene: Scene): string {
  return JSON.stringify([
    scene.nodes.map((n) => [n.id, n.radius, n.group]),
    scene.edges.map((e) => [e.from, e.to]),
    scene.clustered,
  ]);
}
