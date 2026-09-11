/**
 * rtok Nebula — locked A · Nebula landing background (Bitset B).
 * Auto-inits when #rtok-nebula exists. Canvas uses pointer-events: none;
 * pointer tracking is on window so navbar/links stay clickable.
 */
import * as THREE from "three";
import { EffectComposer } from "three/addons/postprocessing/EffectComposer.js";
import { RenderPass } from "three/addons/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/addons/postprocessing/UnrealBloomPass.js";
import { OutputPass } from "three/addons/postprocessing/OutputPass.js";

const NAVY = 0x06101a;
const CYAN = 0x5ce1ff;
const CORAL = 0xff6b4a;

function prefersReducedMotion() {
  try {
    return matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function webglAvailable() {
  try {
    const c = document.createElement("canvas");
    return !!(c.getContext("webgl2") || c.getContext("webgl"));
  } catch {
    return false;
  }
}

function particleTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d");
  const grd = g.createRadialGradient(32, 32, 0, 32, 32, 32);
  grd.addColorStop(0, "rgba(255,255,255,1)");
  grd.addColorStop(0.25, "rgba(255,255,255,0.85)");
  grd.addColorStop(0.55, "rgba(255,255,255,0.2)");
  grd.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = grd;
  g.fillRect(0, 0, 64, 64);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/**
 * Mount Nebula into a container element.
 * @param {HTMLElement} container
 * @returns {() => void} dispose
 */
export function mountNebula(container) {
  if (!container || prefersReducedMotion() || !webglAvailable()) {
    return () => {};
  }

  let renderer;
  let scene;
  let camera;
  let composer;
  let bloom;
  let pointsCyan;
  let pointsCoral;
  let glassOrb;
  let raf = 0;
  let disposed = false;

  const clock = new THREE.Clock();
  const pointer = new THREE.Vector2(0, 0);
  const pointerNDC = new THREE.Vector2(0, 0);
  const targetCam = new THREE.Vector3(0, 0.15, 4.2);
  const orbTarget = new THREE.Vector3(0, 0, 0);
  const raycaster = new THREE.Raycaster();
  let over = false;

  try {
    renderer = new THREE.WebGLRenderer({
      antialias: true,
      powerPreference: "high-performance",
      alpha: false,
    });
  } catch {
    return () => {};
  }

  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.setClearColor(NAVY, 1);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  container.appendChild(renderer.domElement);

  scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(NAVY, 0.085);

  camera = new THREE.PerspectiveCamera(50, 1, 0.05, 40);
  camera.position.copy(targetCam);

  scene.add(new THREE.AmbientLight(0x8aa8bc, 0.35));
  const key = new THREE.PointLight(CYAN, 2.2, 20);
  key.position.set(2, 2, 3);
  scene.add(key);
  const fill = new THREE.PointLight(CORAL, 1.1, 16);
  fill.position.set(-3, -1, 2);
  scene.add(fill);

  const floorGeo = new THREE.PlaneGeometry(30, 30);
  const floorMat = new THREE.MeshBasicMaterial({
    color: 0x040a12,
    transparent: true,
    opacity: 0.65,
    depthWrite: false,
  });
  const floor = new THREE.Mesh(floorGeo, floorMat);
  floor.rotation.x = -Math.PI / 2;
  floor.position.y = -1.35;
  scene.add(floor);

  const glassMat = new THREE.MeshPhysicalMaterial({
    color: 0xb8ecff,
    metalness: 0,
    roughness: 0.05,
    transmission: 0.92,
    thickness: 1.4,
    ior: 1.4,
    transparent: true,
    opacity: 0.85,
    reflectivity: 0.4,
    clearcoat: 1,
    clearcoatRoughness: 0.05,
    attenuationColor: new THREE.Color(CYAN),
    attenuationDistance: 2.5,
    envMapIntensity: 1.2,
  });
  glassOrb = new THREE.Mesh(new THREE.SphereGeometry(0.42, 64, 64), glassMat);
  glassOrb.visible = false;
  scene.add(glassOrb);

  const shell = new THREE.Mesh(
    new THREE.SphereGeometry(0.425, 32, 32),
    new THREE.MeshBasicMaterial({
      color: CYAN,
      transparent: true,
      opacity: 0.08,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    })
  );
  glassOrb.add(shell);

  const ring = new THREE.Mesh(
    new THREE.TorusGeometry(0.55, 0.008, 12, 80),
    new THREE.MeshBasicMaterial({
      color: CORAL,
      transparent: true,
      opacity: 0.55,
    })
  );
  ring.rotation.x = Math.PI / 2.4;
  glassOrb.add(ring);

  composer = new EffectComposer(renderer);
  composer.addPass(new RenderPass(scene, camera));
  bloom = new UnrealBloomPass(new THREE.Vector2(1, 1), 0.85, 0.7, 0.18);
  composer.addPass(bloom);
  composer.addPass(new OutputPass());

  function clearField() {
    [pointsCyan, pointsCoral].forEach((o) => {
      if (!o) return;
      scene.remove(o);
      o.geometry?.dispose();
      if (Array.isArray(o.material)) o.material.forEach((m) => m.dispose());
      else o.material?.dispose();
    });
    pointsCyan = pointsCoral = null;
  }

  function rebuildField() {
    clearField();
    const w = container.clientWidth || window.innerWidth;
    const h = container.clientHeight || window.innerHeight;
    const area = w * h;
    const count = Math.min(14000, Math.max(5000, Math.floor(area / 90)));
    const tex = particleTexture();

    const cyanPos = [];
    const coralPos = [];
    const cyanSize = [];
    const coralSize = [];

    for (let i = 0; i < count; i++) {
      const u = Math.random();
      const v = Math.random();
      const r = Math.pow(Math.random(), 0.55) * 2.6;
      const theta = u * Math.PI * 2;
      const phi = Math.acos(2 * v - 1);
      let x = r * Math.sin(phi) * Math.cos(theta);
      let y = r * Math.sin(phi) * Math.sin(theta) * 0.55;
      let z = r * Math.cos(phi);
      const a = Math.atan2(z, x) + r * 0.55;
      const rr = Math.hypot(x, z);
      x = Math.cos(a) * rr;
      z = Math.sin(a) * rr;
      const cut = Math.random() < 0.08;
      const s = 0.6 + Math.random() * 1.6;
      if (cut) {
        coralPos.push(x, y, z);
        coralSize.push(s);
      } else {
        cyanPos.push(x, y, z);
        cyanSize.push(s);
      }
    }

    function makePoints(positions, sizes, color, sizeMul) {
      if (!positions.length) return null;
      const geo = new THREE.BufferGeometry();
      geo.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
      geo.setAttribute("aSize", new THREE.Float32BufferAttribute(sizes, 1));
      const mat = new THREE.PointsMaterial({
        color,
        map: tex,
        size: sizeMul,
        sizeAttenuation: true,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        opacity: 0.9,
      });
      mat.onBeforeCompile = (shader) => {
        shader.vertexShader = shader.vertexShader
          .replace("uniform float size;", "attribute float aSize;\nuniform float size;")
          .replace("gl_PointSize = size;", "gl_PointSize = size * aSize;");
      };
      const pts = new THREE.Points(geo, mat);
      pts.frustumCulled = false;
      scene.add(pts);
      return pts;
    }

    pointsCyan = makePoints(cyanPos, cyanSize, CYAN, 0.035);
    pointsCoral = makePoints(coralPos, coralSize, CORAL, 0.04);
  }

  function onResize() {
    const w = container.clientWidth || window.innerWidth;
    const h = container.clientHeight || window.innerHeight;
    camera.aspect = w / Math.max(1, h);
    camera.updateProjectionMatrix();
    renderer.setSize(w, h, false);
    composer.setSize(w, h);
    bloom.setSize(w, h);
  }

  function updateOrb(dt) {
    if (!over) {
      glassOrb.visible = false;
      return;
    }
    glassOrb.visible = true;
    raycaster.setFromCamera(pointerNDC, camera);
    const dir = raycaster.ray.direction.clone();
    const hit = new THREE.Vector3();
    hit.copy(camera.position).add(dir.multiplyScalar(3.1));
    orbTarget.lerp(hit, 1 - Math.exp(-dt * 10));
    glassOrb.position.copy(orbTarget);
    glassOrb.lookAt(camera.position);
    const pulse = 1 + Math.sin(clock.elapsedTime * 2.2) * 0.03;
    glassOrb.scale.setScalar(pulse);
    bloom.strength = 0.75 + 0.25 * Math.sin(clock.elapsedTime * 1.5);
  }

  function animateField(t) {
    const spin = t * 0.08;
    if (pointsCyan) {
      pointsCyan.rotation.y = spin;
      pointsCyan.rotation.x = Math.sin(t * 0.15) * 0.08;
    }
    if (pointsCoral) {
      pointsCoral.rotation.y = spin * 1.05;
      pointsCoral.rotation.x = Math.sin(t * 0.15 + 1) * 0.1;
    }
  }

  function tick() {
    if (disposed) return;
    const dt = Math.min(0.05, clock.getDelta());
    const t = clock.elapsedTime;

    const want = new THREE.Vector3(
      pointer.x * 0.65,
      0.15 + pointer.y * 0.35,
      targetCam.z
    );
    camera.position.lerp(want, 1 - Math.exp(-dt * 2.2));
    camera.lookAt(0, 0, 0);

    animateField(t);
    updateOrb(dt);
    renderer.toneMappingExposure = 1.0 + Math.sin(t * 0.4) * 0.04;

    composer.render();
    raf = requestAnimationFrame(tick);
  }

  function onPointerMove(e) {
    const r = container.getBoundingClientRect();
    const x = (e.clientX - r.left) / Math.max(1, r.width);
    const y = (e.clientY - r.top) / Math.max(1, r.height);
    over = x >= 0 && x <= 1 && y >= 0 && y <= 1;
    pointer.set((x - 0.5) * 2, -(y - 0.5) * 2);
    pointerNDC.set(x * 2 - 1, -(y * 2 - 1));
  }

  function onPointerLeaveDoc() {
    over = false;
  }

  rebuildField();
  onResize();
  raf = requestAnimationFrame(tick);

  window.addEventListener("pointermove", onPointerMove, { passive: true });
  window.addEventListener("blur", onPointerLeaveDoc);
  document.addEventListener("pointerleave", onPointerLeaveDoc);
  window.addEventListener("resize", onResize);

  return function dispose() {
    disposed = true;
    cancelAnimationFrame(raf);
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("blur", onPointerLeaveDoc);
    document.removeEventListener("pointerleave", onPointerLeaveDoc);
    window.removeEventListener("resize", onResize);
    clearField();
    if (glassOrb) {
      scene.remove(glassOrb);
      glassOrb.geometry?.dispose();
      glassOrb.traverse((ch) => {
        ch.geometry?.dispose?.();
        if (ch.material) {
          if (Array.isArray(ch.material)) ch.material.forEach((m) => m.dispose());
          else ch.material.dispose?.();
        }
      });
    }
    composer?.dispose?.();
    renderer?.dispose?.();
    if (renderer?.domElement?.parentNode === container) {
      container.removeChild(renderer.domElement);
    }
  };
}

function autoInit() {
  const el = document.getElementById("rtok-nebula");
  if (!el) return;
  if (prefersReducedMotion() || !webglAvailable()) return;
  try {
    mountNebula(el);
  } catch (err) {
    console.warn("[rtok-nebula] init failed; CSS fallback only", err);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", autoInit, { once: true });
} else {
  autoInit();
}

export default mountNebula;
