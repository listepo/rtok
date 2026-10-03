// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { BufferGeometry, Material, Object3D, Texture } from "three";

interface Drawable extends Object3D {
  geometry?: BufferGeometry;
  material?: Material | Material[];
}

/**
 * Frees what `root` holds on the GPU: geometries, materials and their textures. `keep` are
 * geometries shared across rebuilds, freed once by their owner.
 */
export function disposeObject(root: Object3D, keep: Set<BufferGeometry> = new Set()): void {
  root.traverse((o) => {
    const d = o as Drawable;
    if (d.geometry && !keep.has(d.geometry)) d.geometry.dispose();
    for (const m of [d.material ?? []].flat()) {
      (m as Material & { map?: Texture | null }).map?.dispose();
      m.dispose();
    }
  });
}
