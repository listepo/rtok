// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useCallback, useEffect, useRef } from "react";
import { type Frame, layoutInput } from "./layout";
import { startLayout } from "./layoutClient";
import { type Scene, topology } from "./scene";

export type Vec3 = [number, number, number];

export interface Positions {
  /** Updated in place by the layout; read it, never hold it across frames. */
  map: Map<number, Vec3>;
  /** Calls `fn` after each layout frame; returns the unsubscribe. */
  subscribe(fn: () => void): () => void;
}

/** One layout per overview, shared by whichever view is showing; it restarts only when the topology changes. */
export function useLayout(scene: Scene): Positions {
  const map = useRef(new Map<number, Vec3>()).current;
  const subs = useRef(new Set<() => void>()).current;
  const client = useRef<ReturnType<typeof startLayout>>(undefined);
  const latest = useRef(scene);
  latest.current = scene;

  useEffect(() => {
    const c = startLayout((f: Frame) => {
      map.clear();
      f.ids.forEach((id, i) => map.set(id, [f.pos[i * 3]!, f.pos[i * 3 + 1]!, f.pos[i * 3 + 2]!]));
      subs.forEach((fn) => fn());
    });
    client.current = c;
    c.push(layoutInput(latest.current));
    return () => {
      c.stop();
      client.current = undefined;
    };
  }, [map, subs]);

  const key = topology(scene);
  const first = useRef(key);
  useEffect(() => {
    // The mount effect already pushed the first topology.
    if (first.current === key) return;
    first.current = key;
    client.current?.push(layoutInput(latest.current));
  }, [key]);

  const subscribe = useCallback(
    (fn: () => void) => {
      subs.add(fn);
      return () => void subs.delete(fn);
    },
    [subs],
  );
  return { map, subscribe };
}
