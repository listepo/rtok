// Background orb, ported from design/html/js/orb.js: one WebGL fragment shader at half
// resolution and ~30 fps, painted behind the glass panels at low opacity.
const FPS = 30;
const VERT = "attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}";
const FRAG = `precision mediump float;
uniform vec2 r;uniform float t;uniform vec3 ca;uniform vec3 cd;
float h(vec2 p){return fract(sin(dot(p,vec2(12.9898,78.233)))*43758.5453);}
void main(){
 float m=min(r.x,r.y);vec2 uv=(gl_FragCoord.xy-.5*r)/m;
 vec2 c=vec2(.5*r.x/m-.3,.5*r.y/m-.22)+.025*vec2(sin(t*.21),cos(t*.17));
 vec2 d=uv-c;float l=length(d);float R=.16+.008*sin(t*.5);
 float a=atan(d.y,d.x);
 float wob=.012*sin(a*3.+t*.6)+.008*sin(a*5.-t*.4);
 float core=smoothstep(R+.02+wob,R-.09+wob,l);
 float shade=clamp(1.-dot(normalize(d+1e-4),normalize(vec2(-.6,.8)))*.5,0.,1.);
 float b1=exp(-pow(max(l-R,0.)/.05,2.));
 float b2=exp(-max(l-R,0.)/.14);
 float b3=exp(-max(l-R,0.)/.30)*.4;
 float rim=smoothstep(.07,0.,abs(l-R-wob))*(.5+.5*sin(a*2.-t*.8));
 vec3 col=ca*(core*(.35+.45*shade)+b1*.55+b2*.35+b3*.25)+cd*rim*.18;
 col+=(h(gl_FragCoord.xy+t)-.5)*.015;
 gl_FragColor=vec4(col,clamp(max(max(col.r,col.g),col.b),0.,1.));
}`;

function cssRgb(name: string): number[] {
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
    .split(/\s+/)
    .map(Number);
  return v.length === 3 ? v.map((x) => x / 255) : [0.36, 0.88, 1];
}

const t0 = performance.now();

function orbEnabled(): boolean {
  try {
    return localStorage.getItem("rtok-orb") !== "off";
  } catch {
    return true;
  }
}

function compile(gl: WebGLRenderingContext, canvas: HTMLCanvasElement) {
  const shader = (type: number, src: string) => {
    const s = gl.createShader(type);
    if (!s) return null;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
  };
  const vs = shader(gl.VERTEX_SHADER, VERT);
  const fs = shader(gl.FRAGMENT_SHADER, FRAG);
  const prog = gl.createProgram();
  if (!vs || !fs || !prog) return null;
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
  gl.useProgram(prog);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, "p");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  const at = (n: string) => gl.getUniformLocation(prog, n);
  const [r, t, ca, cd] = [at("r"), at("t"), at("ca"), at("cd")];
  return (now: number) => {
    // Half resolution: the orb is all blur, sharpness buys nothing.
    const w = Math.max(1, Math.floor(innerWidth / 2));
    const h = Math.max(1, Math.floor(innerHeight / 2));
    if (canvas.width !== w || canvas.height !== h) [canvas.width, canvas.height] = [w, h];
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform2f(r, w, h);
    // Seconds since load: mediump `sin` loses precision on large arguments.
    gl.uniform1f(t, (now - t0) / 1000);
    gl.uniform3fv(ca, cssRgb("--rtok-accent-rgb"));
    gl.uniform3fv(cd, cssRgb("--rtok-delta-rgb"));
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };
}

/** Runs the orb on `canvas`; `onMode(false)` tells the caller to show the CSS fallback. */
export function startOrb(canvas: HTMLCanvasElement, onMode: (webgl: boolean) => void): () => void {
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  let draw: ReturnType<typeof compile> | undefined;
  let raf = 0;
  let last = 0;
  const loop = (now: number) => {
    raf = requestAnimationFrame(loop);
    if (now - last < 1000 / FPS) return;
    last = now;
    draw?.(now);
  };
  const apply = () => {
    cancelAnimationFrame(raf);
    raf = 0;
    const enabled = orbEnabled() && !motion.matches;
    if (enabled && draw === undefined) {
      const gl = canvas.getContext("webgl", {
        premultipliedAlpha: false,
        antialias: false,
        alpha: true,
        powerPreference: "low-power",
      });
      draw = gl ? compile(gl, canvas) : null;
    }
    onMode(enabled && !!draw);
    if (!enabled || !draw) return;
    // A hidden tab keeps one still frame instead of burning frames nobody sees.
    if (document.hidden) draw(performance.now());
    else raf = requestAnimationFrame(loop);
  };
  document.addEventListener("visibilitychange", apply);
  const onResize = () => {
    if (draw && !raf) draw(performance.now());
  };
  addEventListener("resize", onResize);
  motion.addEventListener("change", apply);
  apply();
  return () => {
    cancelAnimationFrame(raf);
    document.removeEventListener("visibilitychange", apply);
    removeEventListener("resize", onResize);
    motion.removeEventListener("change", apply);
  };
}
