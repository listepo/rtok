// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import {
  BoxGeometry,
  Group,
  Mesh,
  MeshBasicMaterial,
  Sprite,
  SpriteMaterial,
  Texture,
} from "three";
import { expect, test, vi } from "vitest";
import { disposeObject } from "./dispose";

test("geometries, materials and textures are freed, shared geometry is not", () => {
  const shared = new BoxGeometry();
  const own = new BoxGeometry();
  const texture = new Texture();
  const spies = [shared, own, texture].map((o) => vi.spyOn(o, "dispose"));
  const material = new MeshBasicMaterial();
  const materialSpy = vi.spyOn(material, "dispose");
  const sprite = new Sprite(new SpriteMaterial({ map: texture }));
  const spriteSpy = vi.spyOn(sprite.material, "dispose");
  const root = new Group().add(new Mesh(shared, material), new Mesh(own, material), sprite);

  disposeObject(root, new Set([shared]));

  expect(spies[0]).not.toHaveBeenCalled();
  expect(spies[1]).toHaveBeenCalled();
  expect(spies[2]).toHaveBeenCalled();
  expect(materialSpy).toHaveBeenCalled();
  expect(spriteSpy).toHaveBeenCalled();
});
