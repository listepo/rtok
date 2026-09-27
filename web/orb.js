// rtok web admin — background orb with bloom (vanilla WebGL, no deps).
// Low opacity, ~30 fps cap, half-resolution buffer, pauses when the tab is
// hidden, off under prefers-reduced-motion and via the settings toggle.
// Falls back to the static `.orb-fallback` CSS gradient.
(function () {
  "use strict";
  const FPS = 30;
  const canvas = document.getElementById("orb");
  const fallback = document.getElementById("orb-fallback");
  const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  let gl = null,
    prog = null,
    raf = 0,
    last = 0,
    t0 = performance.now(),
    uni = {};
  let enabled = localStorage.getItem("rtok-orb") !== "off";

  const VERT = "attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}";
  // Orb = soft core + three bloom lobes (exp falloff ≈ gaussian blur of a bright
  // disc), faint scanline-free grain. Coral Δ only as a slow rim glint.
  const FRAG = [
    "precision mediump float;",
    "uniform vec2 r;uniform float t;uniform vec3 ca;uniform vec3 cd;uniform float dark;",
    "float h(vec2 p){return fract(sin(dot(p,vec2(12.9898,78.233)))*43758.5453);}",
    "void main(){",
    " float m=min(r.x,r.y);vec2 uv=(gl_FragCoord.xy-.5*r)/m;",
    " vec2 c=vec2(.5*r.x/m-.3,.5*r.y/m-.22)+.025*vec2(sin(t*.21),cos(t*.17));",
    " vec2 d=uv-c;float l=length(d);float R=.16+.008*sin(t*.5);",
    " float a=atan(d.y,d.x);",
    " float wob=.012*sin(a*3.+t*.6)+.008*sin(a*5.-t*.4);",
    " float core=smoothstep(R+wob,R-.06+wob,l);",
    " float shade=clamp(1.-dot(normalize(d+1e-4),normalize(vec2(-.6,.8)))*.5,0.,1.);",
    " float b1=exp(-pow(max(l-R,0.)/.05,2.));",
    " float b2=exp(-max(l-R,0.)/.14);",
    " float b3=exp(-max(l-R,0.)/.30)*.4;",
    " float rim=smoothstep(.04,0.,abs(l-R-wob))*(.5+.5*sin(a*2.-t*.8));",
    " vec3 col=ca*(core*(.35+.45*shade)+b1*.55+b2*.35+b3*.25)+cd*rim*.35;",
    " col+=(h(gl_FragCoord.xy+t)-.5)*.015;",
    " float alpha=clamp(max(max(col.r,col.g),col.b),0.,1.);",
    " gl_FragColor=vec4(col,alpha);",
    "}",
  ].join("\n");

  function cssRGB(name) {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim()
      .split(/\s+/)
      .map(Number);
    return v.length === 3 ? v.map((x) => x / 255) : [0.36, 0.88, 1];
  }

  function init() {
    try {
      gl = canvas.getContext("webgl", {
        premultipliedAlpha: false,
        antialias: false,
        alpha: true,
        powerPreference: "low-power",
      });
    } catch {
      gl = null;
    }
    if (!gl) return false;
    const sh = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
    };
    const vs = sh(gl.VERTEX_SHADER, VERT),
      fs = sh(gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) return false;
    prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return false;
    gl.useProgram(prog);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p");
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    ["r", "t", "ca", "cd", "dark"].forEach((n) => {
      uni[n] = gl.getUniformLocation(prog, n);
    });
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    return true;
  }

  function resize() {
    const scale = 0.5; // half-res: the orb is all blur, sharpness buys nothing
    const w = Math.max(1, Math.floor(innerWidth * scale)),
      h = Math.max(1, Math.floor(innerHeight * scale));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
  }

  function draw(now) {
    resize();
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform2f(uni.r, canvas.width, canvas.height);
    gl.uniform1f(uni.t, (now - t0) / 1000);
    gl.uniform3fv(uni.ca, cssRGB("--accent"));
    gl.uniform3fv(uni.cd, cssRGB("--delta"));
    gl.uniform1f(uni.dark, document.documentElement.classList.contains("dark") ? 1 : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  function loop(now) {
    raf = requestAnimationFrame(loop);
    if (now - last < 1000 / FPS) return; // ~30 fps cap
    last = now;
    draw(now);
  }

  function shouldRun() {
    return enabled && !motionQuery.matches && !document.hidden;
  }

  function apply() {
    cancelAnimationFrame(raf);
    raf = 0;
    const want = enabled && !motionQuery.matches;
    const ok = want && (gl || init());
    canvas.hidden = !ok;
    fallback.hidden = !!ok;
    if (ok && shouldRun()) raf = requestAnimationFrame(loop);
    else if (ok) draw(performance.now()); // one still frame while hidden/paused
    document.documentElement.dataset.orb = ok ? "webgl" : "fallback";
  }

  document.addEventListener("visibilitychange", apply);
  if (motionQuery.addEventListener) motionQuery.addEventListener("change", apply);
  else motionQuery.addListener(apply);
  addEventListener("resize", () => {
    if (gl && !raf) draw(performance.now());
  });

  window.rtokOrb = {
    get enabled() {
      return enabled;
    },
    set(on) {
      enabled = !!on;
      localStorage.setItem("rtok-orb", on ? "on" : "off");
      apply();
    },
    refresh() {
      if (gl && !canvas.hidden) draw(performance.now());
    },
  };
  apply();
})();
