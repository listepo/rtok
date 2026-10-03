// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import {
  AmbientLight,
  BufferGeometry,
  CanvasTexture,
  Color,
  ConeGeometry,
  CylinderGeometry,
  DirectionalLight,
  Group,
  InstancedMesh,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  PerspectiveCamera,
  Quaternion,
  Raycaster,
  Scene as ThreeScene,
  Sprite,
  SpriteMaterial,
  SphereGeometry,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { disposeObject } from "./dispose";
import type { Scene, SceneEdge, SceneNode } from "./scene";
import type { Positions } from "./useLayout";
import type { ViewApi, ViewEvents } from "./webgl";

/** A dashed edge is this many dashes, whatever its length: the gaps scale with the edge. */
const DASHES = 10;
/** Labels farther from the camera than this are hidden, so a big scene stays readable. */
const LABEL_FAR = 600;
/** Pointer travel in pixels beyond which a press was an orbit, not a click. */
const CLICK_SLOP = 4;
const FLY_MS = 450;
const DEFAULT_DIR = new Vector3(0, 0.3, 1).normalize();
const UP = new Vector3(0, 1, 0);

interface NodeObj {
  node: SceneNode;
  mesh: Mesh;
  ring?: Mesh;
  label: Sprite;
}

/**
 * The Three.js side of the overview. It owns the renderer, camera, controls, every object and
 * every listener it adds, and `dispose` gives all of them back; nothing outside holds a Three
 * object. Rendering is on demand: the loop runs only while something changed.
 */
export class Stage implements ViewApi {
  private renderer: WebGLRenderer;
  private camera = new PerspectiveCamera(50, 1, 0.1, 5000);
  private controls: OrbitControls;
  private three = new ThreeScene();
  private group = new Group();
  private sphere = new SphereGeometry(1, 24, 16);
  private cone = new ConeGeometry(1, 1, 12);
  private cylinder = new CylinderGeometry(1, 1, 1, 6, 1, true);
  private nodes = new Map<number, NodeObj>();
  private edges: SceneEdge[] = [];
  private lines?: InstancedMesh;
  private heads?: InstancedMesh;
  /** For each line instance, the index of its edge. */
  private owner: number[] = [];
  private raf = 0;
  private userMoved = false;
  private fly?: { from: Vector3; to: Vector3; t0: number; target: Vector3; targetFrom: Vector3 };
  private down?: { x: number; y: number };
  private hover?: { x: number; y: number };
  private lostTimer?: ReturnType<typeof setTimeout>;
  private disposed = false;
  private unsubscribe: () => void;
  private observer: ResizeObserver;
  private ray = new Raycaster();
  private scratch = { m: new Matrix4(), q: new Quaternion(), v: new Vector3(), s: new Vector3() };

  constructor(
    private host: HTMLElement,
    private events: ViewEvents,
    private positions: Positions,
    private reducedMotion: boolean,
    private onLost: (reason: string) => void,
  ) {
    // `preserveDrawingBuffer` lets a test read the canvas back; the cost is small at this size.
    this.renderer = new WebGLRenderer({
      antialias: true,
      alpha: true,
      preserveDrawingBuffer: true,
    });
    const canvas = this.renderer.domElement;
    canvas.style.cssText = "display:block;width:100%;height:100%;touch-action:none";
    canvas.setAttribute("role", "img");
    canvas.setAttribute("aria-label", "3D graph of the registered projects");
    host.appendChild(canvas);
    this.camera.position.copy(DEFAULT_DIR).multiplyScalar(260);
    const sun = new DirectionalLight(0xffffff, 2.2);
    sun.position.set(60, 120, 90);
    this.three.add(new AmbientLight(0xffffff, 1.6), sun, this.group);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.addEventListener("change", this.invalidate);
    this.controls.addEventListener("start", () => (this.userMoved = true));
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(host);
    canvas.addEventListener("pointerdown", this.onDown);
    canvas.addEventListener("pointerup", this.onUp);
    canvas.addEventListener("pointermove", this.onMove);
    canvas.addEventListener("pointerleave", this.onLeave);
    canvas.addEventListener("dblclick", this.onDouble);
    canvas.addEventListener("contextmenu", this.onMenu);
    canvas.addEventListener("webglcontextlost", this.onContextLost);
    canvas.addEventListener("webglcontextrestored", this.onContextRestored);
    this.unsubscribe = positions.subscribe(this.onPositions);
    this.resize();
  }

  // --- scene ---------------------------------------------------------------------------

  setScene(scene: Scene): void {
    this.clear();
    const fg = getComputedStyle(this.host).color || "#888";
    for (const node of scene.nodes) {
      const color = new Color(node.color);
      const material = new MeshStandardMaterial({
        color,
        emissive: node.inScope ? color : new Color(0),
        emissiveIntensity: 0.25,
        wireframe: node.hollow,
        transparent: node.dim || node.hollow,
        opacity: node.dim ? 0.3 : node.hollow ? 0.7 : 1,
      });
      const mesh = new Mesh(this.sphere, material);
      mesh.scale.setScalar(node.radius);
      mesh.userData.id = node.id;
      const label = labelSprite(node.label, fg, node.dim);
      let ring: Mesh | undefined;
      if (node.selected) {
        ring = new Mesh(this.sphere, new MeshBasicMaterial({ color: fg, wireframe: true }));
        ring.scale.setScalar(node.radius * 1.5);
      }
      this.group.add(mesh, label, ...(ring ? [ring] : []));
      this.nodes.set(node.id, { node, mesh, ring, label });
    }
    this.edges = scene.edges.filter((e) => this.nodes.has(e.from) && this.nodes.has(e.to));
    this.owner = this.edges.flatMap((e, i) => Array<number>(e.dashed ? DASHES : 1).fill(i));
    const tint = (e: SceneEdge) =>
      new Color(e.inScope ? fg : "#888").multiplyScalar(e.inScope ? 1 : 0.6);
    const lineMat = new MeshBasicMaterial({ color: 0xffffff });
    this.lines = new InstancedMesh(this.cylinder, lineMat, Math.max(1, this.owner.length));
    this.heads = new InstancedMesh(
      this.cone,
      new MeshBasicMaterial({ color: 0xffffff }),
      Math.max(1, this.edges.length),
    );
    this.lines.count = this.owner.length;
    this.heads.count = this.edges.length;
    this.owner.forEach((i, n) => this.lines!.setColorAt(n, tint(this.edges[i]!)));
    this.edges.forEach((e, i) => this.heads!.setColorAt(i, tint(e)));
    // No edge means no setColorAt call, and three.js leaves instanceColor null until the first.
    for (const m of [this.lines, this.heads]) {
      if (m.instanceColor) m.instanceColor.needsUpdate = true;
    }
    for (const m of [this.lines, this.heads]) m.frustumCulled = false;
    this.group.add(this.lines, this.heads);
    this.onPositions();
  }

  private clear() {
    for (const c of this.group.children.slice()) {
      this.group.remove(c);
      disposeObject(c, new Set<BufferGeometry>([this.sphere, this.cone, this.cylinder]));
    }
    this.nodes.clear();
    this.lines = this.heads = undefined;
  }

  // --- positions -----------------------------------------------------------------------

  private onPositions = () => {
    if (this.disposed) return;
    const { m, q, v, s } = this.scratch;
    for (const [id, o] of this.nodes) {
      const p = this.positions.map.get(id);
      if (!p) continue;
      o.mesh.position.set(...p);
      o.ring?.position.set(...p);
      o.label.position.set(p[0], p[1] + o.node.radius + 5, p[2]);
    }
    const a = new Vector3();
    const b = new Vector3();
    let line = 0;
    this.edges.forEach((e, i) => {
      const pa = this.positions.map.get(e.from);
      const pb = this.positions.map.get(e.to);
      const ra = this.nodes.get(e.from)!.node.radius;
      const rb = this.nodes.get(e.to)!.node.radius;
      const n = e.dashed ? DASHES : 1;
      if (!pa || !pb) {
        s.set(0, 0, 0);
        for (let k = 0; k < n; k++) this.lines!.setMatrixAt(line++, m.compose(v, q.identity(), s));
        this.heads!.setMatrixAt(i, m.compose(v, q.identity(), s));
        return;
      }
      a.set(...pa);
      b.set(...pb);
      const dir = b.clone().sub(a);
      const len = dir.length() || 1;
      dir.divideScalar(len);
      // The edge runs surface to surface; the arrowhead sits on the target's surface.
      const start = a.clone().addScaledVector(dir, ra);
      const head = Math.min(6, len / 3);
      const end = b.clone().addScaledVector(dir, -(rb + head));
      const span = Math.max(0.01, end.clone().sub(start).length());
      q.setFromUnitVectors(UP, dir);
      const r = e.width * 0.18;
      for (let k = 0; k < n; k++) {
        // Dashed: ten dashes, each half of its slot. Solid: one cylinder over the span.
        const slot = span / n;
        const from = e.dashed ? k * slot : 0;
        const size = e.dashed ? slot / 2 : span;
        v.copy(start).addScaledVector(dir, from + size / 2);
        s.set(r, size, r);
        this.lines!.setMatrixAt(line++, m.compose(v, q, s));
      }
      v.copy(b).addScaledVector(dir, -(rb + head / 2));
      s.set(head * 0.45, head, head * 0.45);
      this.heads!.setMatrixAt(i, m.compose(v, q, s));
    });
    for (const mesh of [this.lines, this.heads]) {
      if (!mesh) continue;
      mesh.instanceMatrix.needsUpdate = true;
      // The raycaster caches a bounding sphere of the instances, which just moved.
      mesh.boundingSphere = null;
    }
    if (!this.userMoved && !this.fly) this.fit(false);
    this.invalidate();
  };

  // --- camera --------------------------------------------------------------------------

  private bounds(): { center: Vector3; radius: number } | null {
    const pts = [...this.nodes.values()]
      .map((o) => ({ p: this.positions.map.get(o.node.id), r: o.node.radius }))
      .filter((x): x is { p: [number, number, number]; r: number } => !!x.p);
    if (!pts.length) return null;
    const center = new Vector3();
    pts.forEach(({ p }) => center.add(new Vector3(...p)));
    center.divideScalar(pts.length);
    const radius = Math.max(20, ...pts.map(({ p, r }) => center.distanceTo(new Vector3(...p)) + r));
    return { center, radius };
  }

  private place(target: Vector3, distance: number, dir: Vector3, animate: boolean) {
    const to = target.clone().addScaledVector(dir, distance);
    if (!animate || this.reducedMotion) {
      this.camera.position.copy(to);
      this.controls.target.copy(target);
      this.controls.update();
      return;
    }
    this.fly = {
      from: this.camera.position.clone(),
      to,
      targetFrom: this.controls.target.clone(),
      target,
      t0: performance.now(),
    };
    this.invalidate();
  }

  private direction(): Vector3 {
    const d = this.camera.position.clone().sub(this.controls.target);
    return d.lengthSq() > 1e-6 ? d.normalize() : DEFAULT_DIR.clone();
  }

  private distanceFor(radius: number): number {
    const half = (this.camera.fov * Math.PI) / 360;
    // A narrow view needs the sphere to fit its width, not its height.
    const fov = Math.atan(Math.tan(half) * Math.min(1, this.camera.aspect));
    return (radius * 1.25) / Math.sin(fov);
  }

  fit(animate = true): void {
    const b = this.bounds();
    if (b)
      this.place(b.center, this.distanceFor(b.radius), this.direction(), animate && this.userMoved);
  }

  reset(): void {
    this.userMoved = false;
    const b = this.bounds();
    if (b) this.place(b.center, this.distanceFor(b.radius), DEFAULT_DIR.clone(), true);
  }

  focus(id: number): void {
    const o = this.nodes.get(id);
    const p = this.positions.map.get(id);
    if (!o || !p) return;
    this.userMoved = true;
    this.place(new Vector3(...p), Math.max(60, o.node.radius * 9), this.direction(), true);
  }

  screenOf(id: number): { x: number; y: number } | null {
    const p = this.positions.map.get(id);
    if (!p) return null;
    const v = new Vector3(...p).project(this.camera);
    const r = this.renderer.domElement.getBoundingClientRect();
    return { x: r.left + ((v.x + 1) / 2) * r.width, y: r.top + ((1 - v.y) / 2) * r.height };
  }

  // --- picking -------------------------------------------------------------------------

  private pick(e: { clientX: number; clientY: number }): { node?: number; edge?: SceneEdge } {
    const r = this.renderer.domElement.getBoundingClientRect();
    const ndc = new Vector2(
      ((e.clientX - r.left) / r.width) * 2 - 1,
      -(((e.clientY - r.top) / r.height) * 2 - 1),
    );
    this.ray.setFromCamera(ndc, this.camera);
    const node = this.ray.intersectObjects(
      [...this.nodes.values()].map((o) => o.mesh),
      false,
    )[0];
    if (node) return { node: node.object.userData.id as number };
    const line = this.lines ? this.ray.intersectObject(this.lines, false)[0] : undefined;
    if (line?.instanceId !== undefined) return { edge: this.edges[this.owner[line.instanceId]!]! };
    return {};
  }

  private onDown = (e: PointerEvent) => (this.down = { x: e.clientX, y: e.clientY });

  private onUp = (e: PointerEvent) => {
    const d = this.down;
    this.down = undefined;
    if (!d || e.button !== 0 || Math.hypot(e.clientX - d.x, e.clientY - d.y) > CLICK_SLOP) return;
    const hit = this.pick(e);
    if (hit.node !== undefined) this.events.select(hit.node);
  };

  private onMove = (e: PointerEvent) => {
    this.hover = { x: e.clientX, y: e.clientY };
    this.invalidate();
  };

  private onLeave = () => {
    this.hover = undefined;
    this.events.hover(null);
  };

  private onDouble = (e: MouseEvent) => {
    const hit = this.pick(e);
    if (hit.node !== undefined) this.focus(hit.node);
  };

  private onMenu = (e: MouseEvent) => {
    const hit = this.pick(e);
    if (hit.node === undefined) return;
    e.preventDefault();
    this.events.menu(hit.node, e.clientX, e.clientY);
  };

  private updateHover() {
    const h = this.hover;
    this.hover = undefined;
    if (!h) return;
    const hit = this.pick({ clientX: h.x, clientY: h.y });
    if (hit.node !== undefined) {
      const n = this.nodes.get(hit.node)!.node;
      this.events.hover({ text: `${n.label} · ${n.state}`, x: h.x, y: h.y });
    } else if (hit.edge) {
      const names = (id: number) => this.nodes.get(id)?.node.label ?? String(id);
      const how =
        hit.edge.kind === "auto"
          ? `auto${hit.edge.reason ? `: ${hit.edge.reason}` : ""}`
          : "manual";
      this.events.hover({
        text: `${names(hit.edge.from)} → ${names(hit.edge.to)} (${how})`,
        x: h.x,
        y: h.y,
      });
    } else this.events.hover(null);
  }

  // --- context loss --------------------------------------------------------------------

  private onContextLost = (e: Event) => {
    // Preventing the default asks the browser to restore the context; give it a moment.
    e.preventDefault();
    this.lostTimer = setTimeout(() => this.onLost("the WebGL context was lost"), 2000);
  };

  private onContextRestored = () => {
    clearTimeout(this.lostTimer);
    this.invalidate();
  };

  // --- loop ----------------------------------------------------------------------------

  private resize() {
    const w = Math.max(1, this.host.clientWidth);
    const h = Math.max(1, this.host.clientHeight);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.invalidate();
  }

  private invalidate = () => {
    if (this.raf || this.disposed) return;
    this.raf = requestAnimationFrame(this.frame);
  };

  private frame = () => {
    this.raf = 0;
    if (this.disposed) return;
    if (this.fly) {
      const t = Math.min(1, (performance.now() - this.fly.t0) / FLY_MS);
      const k = t * (2 - t);
      this.camera.position.lerpVectors(this.fly.from, this.fly.to, k);
      this.controls.target.lerpVectors(this.fly.targetFrom, this.fly.target, k);
      this.controls.update();
      if (t >= 1) this.fly = undefined;
      else this.invalidate();
    }
    for (const o of this.nodes.values()) {
      o.label.visible = this.camera.position.distanceTo(o.mesh.position) < LABEL_FAR;
    }
    this.updateHover();
    this.renderer.render(this.three, this.camera);
  };

  // --- end -----------------------------------------------------------------------------

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    cancelAnimationFrame(this.raf);
    clearTimeout(this.lostTimer);
    this.unsubscribe();
    this.observer.disconnect();
    const canvas = this.renderer.domElement;
    canvas.removeEventListener("pointerdown", this.onDown);
    canvas.removeEventListener("pointerup", this.onUp);
    canvas.removeEventListener("pointermove", this.onMove);
    canvas.removeEventListener("pointerleave", this.onLeave);
    canvas.removeEventListener("dblclick", this.onDouble);
    canvas.removeEventListener("contextmenu", this.onMenu);
    canvas.removeEventListener("webglcontextlost", this.onContextLost);
    canvas.removeEventListener("webglcontextrestored", this.onContextRestored);
    this.controls.dispose();
    this.clear();
    this.sphere.dispose();
    this.cone.dispose();
    this.cylinder.dispose();
    this.renderer.dispose();
    // Dropping the context now returns GPU memory without waiting for the garbage collector.
    this.renderer.forceContextLoss();
    canvas.remove();
  }
}

/** A label that always faces the camera: text on a canvas, drawn as a sprite. */
function labelSprite(text: string, color: string, dim: boolean): Sprite {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d")!;
  const font = "600 28px ui-monospace, monospace";
  ctx.font = font;
  canvas.width = Math.ceil(ctx.measureText(text).width) + 8;
  canvas.height = 40;
  ctx.font = font;
  ctx.fillStyle = color;
  ctx.textBaseline = "middle";
  ctx.fillText(text, 4, 21);
  const sprite = new Sprite(
    new SpriteMaterial({
      map: new CanvasTexture(canvas),
      transparent: true,
      opacity: dim ? 0.4 : 1,
      depthWrite: false,
    }),
  );
  sprite.scale.set(canvas.width * 0.16, canvas.height * 0.16, 1);
  return sprite;
}
