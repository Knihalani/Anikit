// hall-world-ocean.js — world 3: the particle ocean.
//
// You stand on an endless sea of ~200k glowing points at dusk. Every point is placed by the
// vertex shader from gl_VertexID alone (no vertex buffers): seven nested square lattices
// (a clipmap of points), each snapped to its own spacing so nothing swims as you walk, and
// each skipping the square the finer level already covers. A handful of Gerstner waves swell
// the surface; up to 16 ripples (from footsteps and clicks) ride on top, their amplitudes
// driven by anikit spring()s. Colour comes from height, a fresnel-ish sky tint and a sun glint
// computed from the analytic wave normal.
//
// Two passes, both straight into the caller's framebuffer: a full-screen sky (gradient, sun,
// stars, the sea's base colour below the horizon) and the additive points. No offscreen
// buffers, no per-frame allocation.
//
//   createWorld(gl) → { name, tint, spawn, resize, enter, exit, preview, frame }

import { spring } from '../../../packages/anikit/dist/anikit.lean.js';

const N = 192;                 // lattice points per side, per level
const LEVELS = 7;
const S0 = 0.25;               // finest spacing (m)
const RIPPLES = 16;
const RIP_SPEED = 4.2;         // m/s

// Gerstner waves: dirX, dirZ, wavelength, amplitude, steepness. Shared by the shader and the
// camera bob, so you ride the swell you see.
const WAVES = [
  [1.0, 0.25, 74, 0.55, 0.55],
  [0.62, 0.78, 41, 0.32, 0.6],
  [-0.35, 0.94, 23, 0.16, 0.6],
  [0.93, -0.37, 12.5, 0.075, 0.55],
  [-0.8, -0.6, 6.3, 0.035, 0.5],
];
const TIME_SCALE = 0.75;       // slower than real water: calm

function wavesGLSL() {
  let s = '';
  WAVES.forEach(([dx, dz, L, A, Q]) => {
    const l = Math.hypot(dx, dz), k = (2 * Math.PI) / L, w = Math.sqrt(9.8 * k) * TIME_SCALE;
    s += `  W(vec2(${(dx / l).toFixed(5)}, ${(dz / l).toFixed(5)}), ${k.toFixed(5)}, ${A.toFixed(4)}, ${(Q / (k * A * WAVES.length)).toFixed(5)}, ${w.toFixed(5)});\n`;
  });
  return s;
}

const SKY_GLSL = /* glsl */ `
uniform vec3 uSun;
vec3 horizonCol(vec3 d) {
  vec2 h = normalize(d.xz + 1e-5);
  float toward = max(dot(h, normalize(uSun.xz)), 0.0);
  return mix(vec3(0.62, 0.36, 0.52), vec3(1.0, 0.58, 0.36), pow(toward, 3.0));
}
vec3 skyCol(vec3 d) {
  float y = d.y;
  vec3 hz = horizonCol(d);
  float mu = max(dot(d, uSun), 0.0);
  vec3 c;
  if (y >= 0.0) {
    vec3 mid = vec3(0.36, 0.25, 0.55), zen = vec3(0.05, 0.06, 0.19);
    c = mix(hz, mid, smoothstep(0.0, 0.22, y));
    c = mix(c, zen, smoothstep(0.12, 0.75, y));
    c += vec3(1.0, 0.55, 0.3) * (pow(mu, 6.0) * 0.35 + pow(mu, 60.0) * 0.5);
    c += vec3(1.0, 0.9, 0.75) * smoothstep(0.99955, 0.99975, mu) * 1.5;   // the sun's disc
  } else {
    // the sea's own body below the points: dark, lifting into the haze at the horizon,
    // with a broad warm path under the sun
    vec3 r = vec3(d.x, -d.y, d.z);
    float m = max(dot(r, uSun), 0.0);
    c = mix(hz * 0.85, vec3(0.03, 0.035, 0.1), smoothstep(0.0, 0.09, -y));
    c += vec3(1.0, 0.55, 0.3) * pow(m, 30.0) * 0.35 * smoothstep(0.2, 0.0, -y);
  }
  return c;
}`;

const SKY_VS = /* glsl */ `#version 300 es
uniform vec3 uF, uR, uU;  // camera basis
uniform vec2 uTan;        // tan(fov/2) * aspect, tan(fov/2)
out vec3 vDir;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)) * 2.0 - 1.0;
  vDir = uF + p.x * uTan.x * uR + p.y * uTan.y * uU;
  gl_Position = vec4(p, 0.0, 1.0);
}`;

const SKY_FS = /* glsl */ `#version 300 es
precision highp float;
in vec3 vDir;
uniform vec3 uClip;       // portal circle x, y, r (device px); r <= 0: off
uniform float uT;
out vec4 o;
${SKY_GLSL}
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main() {
  if (uClip.z > 0.0 && distance(gl_FragCoord.xy, uClip.xy) > uClip.z) discard;
  vec3 d = normalize(vDir);
  vec3 c = skyCol(d);
  if (d.y > 0.08) {
    // stars on a direction grid, twinkling slowly
    vec2 g = vec2(atan(d.z, d.x) * 120.0, d.y * 160.0);
    vec2 cell = floor(g), f = fract(g) - 0.5;
    float h = hash(cell);
    float s = smoothstep(0.12, 0.0, length(f)) * step(0.985, h) * smoothstep(0.08, 0.5, d.y);
    c += vec3(0.9, 0.92, 1.0) * s * (0.55 + 0.45 * sin(uT * (1.0 + h * 3.0) + h * 50.0));
  }
  // dither so the gradient doesn't band on 8-bit
  c += (hash(gl_FragCoord.xy + fract(uT)) - 0.5) / 255.0;
  o = vec4(c, 1.0);
}`;

const PT_VS = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform mat4 uVP;
uniform vec3 uEye;
uniform float uT;
uniform float uSizeK;     // px per (metre / depth)
uniform float uMaxPx;
uniform vec4 uLv[${LEVELS}];   // per level: snapped centre xz, spacing, half-width of the square it covers
uniform vec4 uRip[${RIPPLES}];  // x, z, start time, amplitude (signed, spring-driven)
uniform int uNRip;
uniform float uFar;
uniform float uKeep;     // quality: fraction of points kept beyond the finest levels
out vec3 vCol;
out float vA;
${SKY_GLSL}

uvec2 pcg2(uvec2 v) {
  v = v * 1664525u + 1013904223u;
  v.x += v.y * 1664525u; v.y += v.x * 1664525u;
  v ^= v >> 16u;
  v.x += v.y * 1664525u; v.y += v.x * 1664525u;
  v ^= v >> 16u;
  return v;
}

vec3 P; vec3 Nrm;  // accumulated position offset and normal terms
void W(vec2 D, float k, float A, float Q, float w) {
  float ph = k * dot(D, P.xz) - w * uT;
  float c = cos(ph), s = sin(ph);
  // Gerstner: horizontal crowding toward crests, vertical sine
  P.x += Q * A * D.x * c; P.z += Q * A * D.y * c; P.y += A * s;
  float wa = k * A;
  Nrm.x -= D.x * wa * c; Nrm.z -= D.y * wa * c; Nrm.y -= Q * wa * s;
}

void main() {
  int id = gl_VertexID;
  int L = id / ${N * N};
  int k = id - L * ${N * N};
  int ix = k % ${N}, iz = k / ${N};
  vec4 C = uLv[L];
  float sp = C.z;
  vec2 cellI = vec2(float(ix - ${N / 2}), float(iz - ${N / 2}));
  vec2 p = C.xy + cellI * sp;
  if (L > 0) {
    vec4 Cp = uLv[L - 1];
    vec2 d = abs(p - Cp.xy);
    if (max(d.x, d.y) < Cp.w) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; return; }
  }
  // stable per-point jitter and variety, hashed from the integer world cell
  ivec2 wc = ivec2(floor(p / sp + 0.5));
  uvec2 hv = pcg2(uvec2(wc + 32768) + uvec2(uint(L) * 7919u, uint(L) * 104729u));
  vec4 h = vec4(hv & 0xffffu, hv >> 16u) / 65535.0;
  if (L > 1 && h.w > uKeep) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; return; }
  p += (h.xy - 0.5) * sp * 0.85;

  P = vec3(p.x, 0.0, p.y);
  Nrm = vec3(0.0, 1.0, 0.0);
${wavesGLSL()}
  // ripples (only where they can reach: the finest four levels)
  float rip = 0.0;
  if (L < 4) {
    for (int i = 0; i < ${RIPPLES}; i++) {
      if (i >= uNRip) break;
      vec4 R = uRip[i];
      float age = uT - R.z;
      float front = age * ${RIP_SPEED.toFixed(2)};
      float x = distance(p, R.xy) - front;
      float wd = 0.7 + age * 0.45;
      if (abs(x) > wd * 3.0) continue;
      rip += R.w * exp(-x * x / (wd * wd)) * cos(x * 2.4) / (1.0 + 0.18 * front);
    }
  }
  P.y += rip;
  vec3 n = normalize(Nrm);

  vec3 toP = P - uEye;
  float dist = length(toP);
  vec3 v = toP / dist;
  vec4 clip = uVP * vec4(P, 1.0);
  gl_Position = clip;

  // soft size attenuation: a point covers ~45% of its cell; below a pixel, fade by area
  float px = uSizeK * sp * (0.38 + 0.25 * h.z) / max(clip.w, 0.05);
  // density-normalised: the ground patch a point stands for covers px² · (grazing) pixels, so
  // where the lattice crowds toward the horizon each point gets dimmer and the sum stays even
  float cover = min(1.0, px * px * max(abs(v.y), 0.0015) * 1.6);
  float clampFade = px > uMaxPx ? uMaxPx / px : 1.0;     // big near points: spread, dimmer
  gl_PointSize = clamp(px, 1.0, uMaxPx);

  // colour by height, sky tint by slope, sun glint from the wave normal
  float hn = clamp(P.y * 0.6 + 0.5, 0.0, 1.0);
  vec3 deep = vec3(0.16, 0.13, 0.5), midc = vec3(0.15, 0.55, 0.85), crest = vec3(1.25, 0.72, 0.62);
  vec3 col = mix(deep, midc, smoothstep(0.15, 0.55, hn));
  col = mix(col, crest, smoothstep(0.5, 0.9, hn));
  vec3 r = reflect(v, n);
  float fres = pow(1.0 - max(dot(-v, n), 0.0), 4.0);
  col += skyCol(normalize(vec3(r.x, abs(r.y), r.z))) * fres * 0.6;
  float glint = pow(max(dot(r, uSun), 0.0), 90.0);
  col += vec3(1.0, 0.74, 0.42) * glint * (2.5 + 4.0 * h.w);
  col += vec3(0.55, 0.9, 1.0) * min(abs(rip) * 2.2, 1.4);   // ripples glow as they pass
  float tw = 0.75 + 0.25 * sin(uT * (0.7 + h.w * 1.6) + h.z * 40.0);

  // fog toward the horizon; a round outer edge; near points dim a little (bokeh)
  float fog = exp(-dist * 0.0024);
  float edge = smoothstep(uFar, uFar * 0.7, dist);
  float near = smoothstep(0.6, 2.4, dist);
  vA = (0.75 + 0.6 * h.z) * tw * (L > 1 ? 1.0 / min(uKeep, 1.0) : 1.0) * cover * clampFade * fog * edge * near;
  vCol = col;
}`;

const PT_FS = /* glsl */ `#version 300 es
precision mediump float;
in vec3 vCol;
in float vA;
uniform vec3 uClip;
out vec4 o;
void main() {
  if (uClip.z > 0.0 && distance(gl_FragCoord.xy, uClip.xy) > uClip.z) discard;
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  float a = max(1.0 - dot(q, q), 0.0);
  a *= a;
  o = vec4(vCol * (vA * a), 1.0);
}`;

function compile(gl, vs, fs) {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error('ocean shader: ' + gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('ocean link: ' + gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const info = gl.getActiveUniform(p, i);
    u[info.name.replace(/\[0\]$/, '')] = gl.getUniformLocation(p, info.name);
  }
  return { p, u };
}

// height of the swell at (x, z), time t: the vertical part of the Gerstner sum
function swellHeight(x, z, t) {
  let y = 0;
  for (let i = 0; i < WAVES.length; i++) {
    const [dx, dz, L, A] = WAVES[i];
    const l = Math.hypot(dx, dz), k = (2 * Math.PI) / L, w = Math.sqrt(9.8 * k) * TIME_SCALE;
    y += A * Math.sin(k * ((dx / l) * x + (dz / l) * z) - w * t);
  }
  return y;
}

export const SHARED_GL = true;

export function createWorld(gl) {
  const sky = compile(gl, SKY_VS, SKY_FS);
  const pts = compile(gl, PT_VS, PT_FS);
  const vao = gl.createVertexArray();   // empty: everything comes from gl_VertexID

  const FOV = 1.1, NEAR = 0.1, FAR = 1700;
  const sun = new Float32Array([0, Math.sin(0.06), -Math.cos(0.06)]);   // low, straight ahead at spawn
  const eye = new Float32Array(3), f = new Float32Array(3), r = new Float32Array(3), u = new Float32Array(3);
  const vp = new Float32Array(16);
  const lv = new Float32Array(LEVELS * 4);
  const rip = new Float32Array(RIPPLES * 4);
  const clip = new Float32Array(3);
  let W = 1, H = 1, dpr = 1, quality = 0;

  // ripples: a ring of slots; each amplitude is a plain object an anikit spring() drives
  const slots = [];
  for (let i = 0; i < RIPPLES; i++) slots.push({ x: 0, z: 0, t0: -1e9, a: 0, s: null });
  let next = 0, nowT = 0;
  function ripple(x, z, amp) {
    const s = slots[next];
    next = (next + 1) % RIPPLES;
    if (s.s) s.s.kill();
    s.x = x; s.z = z; s.t0 = nowT; s.a = 0;
    // a velocity kick: rises fast, rings down (crest ↔ trough) as it spreads
    s.s = spring(s, { a: 0 }, { velocity: amp * 7, stiffness: 26, damping: 2.4 });
  }

  // camera: smoothed bob on the swell; footsteps make small rings
  let bob = 0, lastX = 0, lastZ = 0, walked = 0, entered = false, auto = 0;
  function camera(px, py, pz, yaw, pitch, t, dt) {
    const target = swellHeight(px, pz, t) * 0.8;
    bob += (target - bob) * Math.min(1, dt * 3);
    eye[0] = px; eye[1] = py + 0.5 + bob; eye[2] = pz;
    const cp = Math.cos(pitch), sp = Math.sin(pitch), cy = Math.cos(yaw), sy = Math.sin(yaw);
    f[0] = -sy * cp; f[1] = sp; f[2] = -cy * cp;
    r[0] = cy; r[1] = 0; r[2] = -sy;
    u[0] = r[1] * f[2] - r[2] * f[1]; u[1] = r[2] * f[0] - r[0] * f[2]; u[2] = r[0] * f[1] - r[1] * f[0];
  }
  function matrices(aspect, tanY) {
    const t = 1 / tanY;
    const a = t / aspect, b = t, c = (FAR + NEAR) / (NEAR - FAR), d = (2 * FAR * NEAR) / (NEAR - FAR);
    const ex = -(r[0] * eye[0] + r[1] * eye[1] + r[2] * eye[2]);
    const ey = -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]);
    const ez = f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2];
    // vp = P * V with V rows (r, u, -f), column-major
    for (let i = 0; i < 3; i++) {
      vp[i * 4 + 0] = a * r[i];
      vp[i * 4 + 1] = b * u[i];
      vp[i * 4 + 2] = -c * f[i];
      vp[i * 4 + 3] = f[i];
    }
    // row 2: c·(−f·p + f·e) + d, row 3: f·p − f·e
    vp[12] = a * ex; vp[13] = b * ey; vp[14] = c * ez + d; vp[15] = -ez;
  }

  function levels() {
    let s = S0;
    for (let l = 0; l < LEVELS; l++) {
      const snap = s * 2;
      lv[l * 4] = Math.round(eye[0] / snap) * snap;
      lv[l * 4 + 1] = Math.round(eye[2] / snap) * snap;
      lv[l * 4 + 2] = s;
      lv[l * 4 + 3] = (N / 2 - 1.5) * s;
      s *= 2;
    }
  }

  function packRipples() {
    let n = 0;
    for (let i = 0; i < RIPPLES; i++) {
      const s = slots[i];
      if (nowT - s.t0 > 14 || Math.abs(s.a) < 1e-4 && nowT - s.t0 > 1) continue;
      rip[n * 4] = s.x; rip[n * 4 + 1] = s.z; rip[n * 4 + 2] = s.t0; rip[n * 4 + 3] = s.a;
      n++;
    }
    return n;
  }

  function draw(t, x, y, w, h, clipOn) {
    gl.viewport(x, y, w, h);
    if (!clipOn) clip[2] = 0;
    passes(t, w / h, Math.tan(FOV / 2), h);
    gl.disable(gl.BLEND);
  }

  // the two passes into whatever is bound; camera in eye/f/r/u; leaves blend on
  function passes(t, aspect, tanY, h) {
    matrices(aspect, tanY);
    levels();
    const nr = packRipples();
    gl.bindVertexArray(vao);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.BLEND);

    gl.useProgram(sky.p);
    gl.uniform3fv(sky.u.uF, f); gl.uniform3fv(sky.u.uR, r); gl.uniform3fv(sky.u.uU, u);
    gl.uniform2f(sky.u.uTan, tanY * aspect, tanY);
    gl.uniform3fv(sky.u.uClip, clip);
    gl.uniform3fv(sky.u.uSun, sun);
    gl.uniform1f(sky.u.uT, t);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.useProgram(pts.p);
    gl.uniformMatrix4fv(pts.u.uVP, false, vp);
    gl.uniform3fv(pts.u.uEye, eye);
    gl.uniform1f(pts.u.uT, t);
    gl.uniform1f(pts.u.uSizeK, (0.5 * h) / tanY);
    gl.uniform1f(pts.u.uMaxPx, Math.min((quality ? 12 : 22) * dpr, h * 0.04, (quality ? 0.008 : 0.014) * h + 4));
    gl.uniform1f(pts.u.uKeep, quality > 1 ? 0.5 : 1.01);
    gl.uniform4fv(pts.u.uLv, lv);
    gl.uniform4fv(pts.u.uRip, rip);
    gl.uniform1i(pts.u.uNRip, nr);
    gl.uniform1f(pts.u.uFar, S0 * Math.pow(2, LEVELS - 1) * (N / 2 - 2));
    gl.uniform3fv(pts.u.uSun, sun);
    gl.uniform3fv(pts.u.uClip, clip);
    gl.drawArrays(gl.POINTS, 0, LEVELS * N * N);
    gl.blendFunc(gl.ONE, gl.ZERO);
    gl.bindVertexArray(null);
    gl.useProgram(null);
  }

  // ---- the hall's world interface (hall.js): the hall owns the camera, stencil and post
  let hallActive = false, lastHX = NaN, lastHZ = 0, hallWalked = 0;
  function update(t, dt, pos, active) {
    nowT = t;
    if (active) {
      if (!hallActive) { lastHX = NaN; hallWalked = 0; }
      const step = Number.isNaN(lastHX) ? 0 : Math.hypot(pos[0] - lastHX, pos[2] - lastHZ);
      if (step < 5) hallWalked += step;
      lastHX = pos[0]; lastHZ = pos[2];
      if (hallWalked > 1.5) { hallWalked = 0; ripple(pos[0], pos[2], 0.09); }
    } else {
      auto -= dt;   // rain while seen through the portal
      if (auto <= 0) {
        auto = 1.1 + Math.random() * 0.9;
        const d = 5 + Math.random() * 16, a = (Math.random() - 0.5) * 1.2;
        ripple(pos[0] - Math.sin(a) * d, pos[2] - Math.cos(a) * d, 0.25 + Math.random() * 0.25);
      }
    }
    hallActive = active;
  }
  function use(kind, pos, fw) {
    let d = 30;
    if (fw[1] < -0.02) d = Math.min(45, pos[1] / -fw[1]);
    ripple(pos[0] + fw[0] * d, pos[2] + fw[2] * d, kind === 'pull' ? -0.6 : 0.75);
  }
  function hallDraw(cam) {
    for (let i = 0; i < 3; i++) { eye[i] = cam.eye[i]; f[i] = cam.f[i]; r[i] = cam.r[i]; u[i] = cam.u[i]; }
    clip[2] = 0;
    const depth = gl.isEnabled(gl.DEPTH_TEST);
    gl.disable(gl.DEPTH_TEST);
    passes(cam.time, cam.aspect, cam.tanH, 2 * cam.px * cam.tanH);
    gl.disable(gl.BLEND);
    if (depth) gl.enable(gl.DEPTH_TEST);
  }

  const world = {
    id: 'ocean',
    name: 'Particle ocean',
    feature: 'spring() · 200k points, 16 ripples',
    hint: 'Walk on the water · click to drop a stone',
    fly: false, walk: true, radius: 600,
    tint: [1.0, 0.62, 0.42],
    fog: [0.62, 0.36, 0.52],
    spawn: [0, 1.7, 0, 0],
    spawnYaw: 0,
    start() {}, update, use, draw: hallDraw,

    resize(cssW, cssH, d) {
      dpr = Math.min(d || 1, 2);
      W = Math.max(1, Math.round(cssW * dpr));
      H = Math.max(1, Math.round(cssH * dpr));
    },

    enter() {
      entered = true;
      walked = 0; lastX = 0; lastZ = 0;
    },
    exit() { entered = false; },

    // the world seen through a portal: a slow look around, with rain-like drops showing the ripples
    // rect and portal: device px, top-left origin (CONTRACT.md); GL wants bottom-left
    preview(t, dt, rect, portal) {
      nowT = t;
      const yaw = Math.sin(t * 0.07) * 0.35, pitch = -0.1 + Math.sin(t * 0.11) * 0.03;
      camera(0, 1.7, 0, yaw, pitch, t, dt);
      auto -= dt;
      if (auto <= 0) {
        auto = 1.1 + Math.random() * 0.9;
        const d = 5 + Math.random() * 16, a = yaw + (Math.random() - 0.5) * 0.9;
        ripple(eye[0] - Math.sin(a) * d, eye[2] - Math.cos(a) * d, 0.25 + Math.random() * 0.25);
      }
      const bh = gl.drawingBufferHeight;
      const x = Math.floor(rect.x), w = Math.ceil(rect.w), h = Math.ceil(rect.h), y = bh - Math.floor(rect.y) - h;
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(x, y, w, h);
      clip[0] = portal.x; clip[1] = bh - portal.y; clip[2] = portal.r;
      draw(t, x, y, w, h, true);
      gl.disable(gl.SCISSOR_TEST);
      gl.viewport(0, 0, gl.drawingBufferWidth, bh);
    },

    frame(t, dt, input) {
      nowT = t;
      const p = input.pos;
      camera(p[0], p[1], p[2], input.yaw, input.pitch, t, dt);
      // footsteps
      const dx = p[0] - lastX, dz = p[2] - lastZ;
      const step = Math.hypot(dx, dz);
      if (step > 5) walked = 0;           // teleport (enter / respawn): no ripple trail
      else walked += step;
      lastX = p[0]; lastZ = p[2];
      if (walked > 1.5) {
        walked = 0;
        ripple(p[0] - Math.sin(input.yaw) * 0.6, p[2] - Math.cos(input.yaw) * 0.6, 0.09);
      }
      if (input.click) {
        // where you're looking, on the mean surface; capped so the ring is in view
        let d = 30;
        if (f[1] < -0.02) d = Math.min(45, eye[1] / -f[1]);
        ripple(eye[0] + f[0] * d, eye[2] + f[2] * d, 0.75);
      }
      if (input.wheel) { /* no zoom: the sea is about standing still */ }
      draw(t, 0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight, false);
    },

    // 0 full · 1 smaller sprites (less fill) · 2 also thins the far lattice
    setQuality(level) { quality = level | 0; },
  };

  // pre-warm: compile and run both programs once into a 1×1 scissor so the first real frame is clean
  camera(0, 1.7, 0, 0, 0, 0, 0);
  gl.enable(gl.SCISSOR_TEST);
  gl.scissor(0, 0, 1, 1);
  draw(0, 0, 0, 1, 1, false);
  gl.disable(gl.SCISSOR_TEST);
  void entered;
  return world;
}
