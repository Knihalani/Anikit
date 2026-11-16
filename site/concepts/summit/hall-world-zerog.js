// hall-world-zerog.js — mini-world 1: zero gravity.
// You float in a nebula among 360 slowly tumbling rocks, glowing crystals and bronze
// bars. Their drift and collisions run in an anikit world() with no gravity (in the
// horizontal plane; each body keeps its own height band); you are a kinematic body in
// it, so flying through pushes things aside. Click / E pushes, right-click / Q pulls.
//
// A world only draws scene content into the framebuffer the host has bound (the host
// owns camera, controls, post and portals). Per-frame paths do not allocate.

import { world, animate } from '../../../packages/anikit/dist/anikit.lean.js';

const PI2 = Math.PI * 2;

// ---------- meshes ----------
function rockMesh(seed) {
  // icosahedron, one subdivision, radial jitter, flat-shaded (unique verts per face)
  const t = (1 + Math.sqrt(5)) / 2;
  let V = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]];
  let F = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
    [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
  V = V.map((v) => { const l = Math.hypot(...v); return v.map((c) => c / l); });
  const mid = new Map();
  const m = (a, b) => {
    const k = a < b ? a + '_' + b : b + '_' + a;
    if (!mid.has(k)) { const p = V[a].map((c, i) => (c + V[b][i]) / 2); const l = Math.hypot(...p); V.push(p.map((c) => c / l)); mid.set(k, V.length - 1); }
    return mid.get(k);
  };
  const F2 = [];
  for (const [a, b, c] of F) { const ab = m(a, b), bc = m(b, c), ca = m(c, a); F2.push([a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]); }
  let s = seed;
  const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const R = V.map(() => 0.38 + 0.14 * rnd());
  const v = [], idx = [];
  for (const f of F2) {
    const P = f.map((i) => V[i].map((c) => c * R[i]));
    const e1 = P[1].map((c, i) => c - P[0][i]), e2 = P[2].map((c, i) => c - P[0][i]);
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    const l = Math.hypot(...n); n = n.map((c) => c / l);
    // keep outward winding
    const ctr = P[0].map((c, i) => c + P[1][i] + P[2][i]);
    if (n[0] * ctr[0] + n[1] * ctr[1] + n[2] * ctr[2] < 0) { P.reverse(); n = n.map((c) => -c); }
    for (const p of P) { idx.push(v.length / 6); v.push(p[0], p[1], p[2], n[0], n[1], n[2]); }
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

function crystalMesh() {
  // elongated octahedron, flat-shaded
  const P = [[0, 0.6, 0], [0.22, 0, 0], [0, 0, 0.22], [-0.22, 0, 0], [0, 0, -0.22], [0, -0.6, 0]];
  const F = [[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4], [5, 1, 2], [5, 2, 3], [5, 3, 4], [5, 4, 1]];
  const v = [], idx = [];
  for (const f of F) {
    const [a, b, c] = f.map((i) => P[i]);
    const e1 = b.map((x, i) => x - a[i]), e2 = c.map((x, i) => x - a[i]);
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    const l = Math.hypot(...n); n = n.map((x) => x / l);
    const ctr = [a, b, c].reduce((s, p) => s.map((x, i) => x + p[i]), [0, 0, 0]);
    let tri = [a, b, c];
    if (n[0] * ctr[0] + n[1] * ctr[1] + n[2] * ctr[2] < 0) { tri = [a, c, b]; n = n.map((x) => -x); }
    for (const p of tri) { idx.push(v.length / 6); v.push(p[0], p[1], p[2], n[0], n[1], n[2]); }
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

// ---------- shaders ----------
const SKY_VS = /* glsl */ `#version 300 es
out vec2 vNdc;
void main() { vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2) * 2.0 - 1.0; vNdc = p; gl_Position = vec4(p, 0.9999, 1.0); }`;
const SKY_FS = /* glsl */ `#version 300 es
precision highp float;
uniform mat4 uInvVP; uniform vec3 uEye; uniform vec3 uSunDir;
in vec2 vNdc; out vec4 o;
float h(vec3 p) { return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453); }
float n3(vec3 p) { vec3 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(h(i), h(i + vec3(1,0,0)), f.x), mix(h(i + vec3(0,1,0)), h(i + vec3(1,1,0)), f.x), f.y),
             mix(mix(h(i + vec3(0,0,1)), h(i + vec3(1,0,1)), f.x), mix(h(i + vec3(0,1,1)), h(i + vec3(1,1,1)), f.x), f.y), f.z); }
vec3 aces(vec3 c) { c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2)); }
void main() {
  vec4 p = uInvVP * vec4(vNdc, 1.0, 1.0);
  vec3 d = normalize(p.xyz / p.w - uEye);
  float a = n3(d * 2.2) * 0.6 + n3(d * 5.1 + 3.0) * 0.3 + n3(d * 11.0 + 7.0) * 0.1;
  float b = n3(d * 3.3 + 11.0);
  vec3 deep = mix(vec3(0.01, 0.012, 0.03), vec3(0.03, 0.02, 0.06), d.y * 0.5 + 0.5);
  vec3 neb = mix(vec3(0.05, 0.32, 0.42), vec3(0.55, 0.12, 0.45), smoothstep(0.35, 0.75, b));
  float m = smoothstep(0.42, 0.85, a) * (0.55 + 0.45 * smoothstep(-0.6, 0.4, d.y));
  vec3 c = deep + neb * m * 0.9;
  // the sun: a warm disc and halo
  float s = max(dot(d, -uSunDir), 0.0);
  c += vec3(1.0, 0.75, 0.45) * (pow(s, 600.0) * 30.0 + pow(s, 12.0) * 0.35 + pow(s, 3.0) * 0.06);
  o = vec4(aces(c), 1.0);
}`;

const STAR_VS = /* glsl */ `#version 300 es
layout(location=0) in vec4 aP;
uniform mat4 uVP; uniform vec3 uEye; uniform float uTime; uniform float uPx;
out float vA;
void main() {
  vec4 cp = uVP * vec4(uEye + aP.xyz * 300.0, 1.0);
  cp.z = cp.w * 0.9998;
  gl_Position = cp;
  gl_PointSize = clamp(1.0 + aP.w * 2.2 * uPx / 900.0, 1.0, 4.0);
  vA = (0.35 + 0.65 * aP.w) * (0.75 + 0.25 * sin(uTime * (1.0 + aP.w * 3.0) + aP.x * 90.0));
}`;
const STAR_FS = /* glsl */ `#version 300 es
precision mediump float;
in float vA; out vec4 o;
void main() { vec2 q = gl_PointCoord * 2.0 - 1.0; float r = 1.0 - dot(q, q); if (r <= 0.0) discard; o = vec4(vec3(0.85, 0.9, 1.0) * r * vA, 0.0); }`;

// objects: the host's MESH_VS layout (16 floats per instance), lit by the sun and nebula
const OBJ_FS = /* glsl */ `#version 300 es
precision highp float;
uniform vec3 uEye; uniform vec3 uSunDir; uniform float uTime;
in vec3 vW; in vec3 vN; in vec4 vCol; in float vGloss; in vec3 vL; flat in int vMat;
out vec4 o;
vec3 aces(vec3 c) { c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2)); }
float h(vec3 p) { return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453); }
void main() {
  vec3 n = normalize(vN), V = normalize(uEye - vW), L = -uSunDir;
  vec3 alb = vCol.rgb * vCol.rgb;
  float nd = max(dot(n, L), 0.0);
  vec3 fillA = vec3(0.06, 0.32, 0.4), fillB = vec3(0.4, 0.08, 0.32);
  vec3 amb = mix(fillB, fillA, n.y * 0.5 + 0.5) * 0.45;
  vec3 col;
  float fr = pow(1.0 - max(dot(n, V), 0.0), 4.0);
  if (vMat == 3) {
    // metal: reflects the nebula, sharp sun highlight
    vec3 R = reflect(-V, n);
    vec3 env = mix(fillB, fillA, R.y * 0.5 + 0.5) * 0.9 + vec3(1.0, 0.75, 0.45) * pow(max(dot(R, L), 0.0), 40.0) * 6.0;
    col = alb * (env + nd * vec3(1.0, 0.8, 0.55) * 0.6);
  } else if (vMat == 7) {
    // crystal: glows from inside, bright facet edges
    col = alb * (0.35 + 0.65 * fr) * 2.2 + alb * vCol.a * 1.5 + vec3(1.0, 0.85, 0.7) * pow(max(dot(reflect(-L, n), V), 0.0), 60.0) * 3.0;
  } else {
    // rock: warm sun, cool and magenta fills, a little grit
    float g = 0.85 + 0.3 * h(floor(vL * 9.0));
    col = alb * g * (vec3(1.0, 0.78, 0.5) * nd * 2.2 + amb) + vec3(0.5, 0.35, 0.6) * fr * 0.35;
  }
  float d = length(uEye - vW);
  col = mix(col, vec3(0.03, 0.05, 0.09), clamp((d - 18.0) / 40.0, 0.0, 0.85));
  o = vec4(aces(col), 1.0);
}`;

const RING_VS = /* glsl */ `#version 300 es
uniform mat4 uVP; uniform vec3 uC; uniform vec3 uR; uniform vec3 uU; uniform float uRad;
out vec2 vQ;
void main() {
  vec2 q = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2) * 2.0 - 1.0;
  q = vec2(gl_VertexID == 0 ? -1.0 : gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0);
  vQ = q;
  gl_Position = uVP * vec4(uC + (uR * q.x + uU * q.y) * uRad, 1.0);
}`;
const RING_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform float uA; uniform vec3 uTint;
in vec2 vQ; out vec4 o;
void main() {
  float r = length(vQ);
  if (r > 1.0) discard;
  float ring = exp(-pow((r - 0.92) * 22.0, 2.0)) + 0.15 * exp(-pow((r - 0.8) * 8.0, 2.0));
  o = vec4(uTint * ring * uA, 0.0);
}`;

export function createZeroG(gl, lib) {
  const { program, group, MESH_VS } = lib;
  let s = 99;
  const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const SUN = [-0.45, -0.35, 0.82];
  { const l = Math.hypot(...SUN); SUN[0] /= l; SUN[1] /= l; SUN[2] /= l; }

  const progSky = program(gl, SKY_VS, SKY_FS);
  const progStar = program(gl, STAR_VS, STAR_FS);
  const progObj = program(gl, MESH_VS, OBJ_FS);
  const progRing = program(gl, RING_VS, RING_FS);
  const emptyVao = gl.createVertexArray();

  // stars
  const NSTAR = 3500;
  const starData = new Float32Array(NSTAR * 4);
  for (let i = 0; i < NSTAR; i++) {
    const u = rnd() * 2 - 1, a = rnd() * PI2, r = Math.sqrt(1 - u * u);
    starData[i * 4] = Math.cos(a) * r; starData[i * 4 + 1] = u; starData[i * 4 + 2] = Math.sin(a) * r;
    starData[i * 4 + 3] = Math.pow(rnd(), 6);
  }
  const starVao = gl.createVertexArray();
  gl.bindVertexArray(starVao);
  const sb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, sb);
  gl.bufferData(gl.ARRAY_BUFFER, starData, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  // objects: rocks (3 shapes), crystals, bronze bars
  const KINDS = 5;
  const meshes = [rockMesh(3), rockMesh(17), rockMesh(41), crystalMesh(), lib.cube];
  const N = 360;
  const groups = meshes.map((m) => group(gl, m, N));
  const obj = [];
  const ROCK = [[0.5, 0.44, 0.4], [0.42, 0.38, 0.36], [0.56, 0.48, 0.4]];
  for (let i = 0; i < N; i++) {
    const kind = i % 9 === 0 ? 3 : i % 7 === 0 ? 4 : i % 3;
    const big = rnd() < 0.08;
    const size = kind === 3 ? 0.5 + rnd() * 0.9 : kind === 4 ? 0.12 + rnd() * 0.1 : (big ? 2.2 + rnd() * 2.2 : 0.35 + rnd() * 1.1);
    const a = rnd() * PI2, rad = 3 + Math.pow(rnd(), 0.7) * 26;
    obj.push({
      kind, size, gi: 0,
      x: Math.cos(a) * rad, z: Math.sin(a) * rad, y: (rnd() - 0.5) * 14 + (big ? 0 : 1.5),
      bob: rnd() * PI2, bobA: 0.2 + rnd() * 0.5,
      rx: rnd() * PI2, ry: rnd() * PI2, rz: rnd() * PI2,
      wx: (rnd() - 0.5) * 0.5, wy: (rnd() - 0.5) * 0.5, wz: (rnd() - 0.5) * 0.5,
      sx: kind === 4 ? size : size, sy: kind === 4 ? size * (6 + rnd() * 8) : size * (kind === 3 ? 1.6 : 0.85 + rnd() * 0.3), sz: kind === 4 ? size : size * (0.85 + rnd() * 0.3),
      c: kind === 3 ? (rnd() < 0.5 ? [0.35, 0.95, 1.0] : [1.0, 0.45, 0.85]) : kind === 4 ? [0.86, 0.6, 0.32] : ROCK[i % 3],
      emis: kind === 3 ? 0.4 + rnd() * 0.6 : 0,
      body: -1,
    });
  }
  // keep the spawn clear
  for (const o of obj) if (Math.hypot(o.x, o.z) < 4 && Math.abs(o.y - 1.7) < 3) o.y += o.y > 1.7 ? 4 : -4;

  let w = null, playerBody = -1;
  const ring = { a: 0, r: 0, x: 0, y: 0, z: 0, pull: 0 };
  let ready = false;

  function start() {
    w = world({ gravity: [0, 0], damping: 0.04, restitution: 0.65, friction: 0.02, substeps: 2, bounds: [-3200, -3200, 3200, 3200] });
    for (const o of obj) {
      const r = Math.max(o.sx, o.sz, o.kind === 4 ? o.sy * 0.35 : 0) * 50;
      o.body = w.add(o.x * 100, o.z * 100, r, r * r);
      w.setVelocity(o.body, (rnd() - 0.5) * 40, (rnd() - 0.5) * 40);
    }
    playerBody = w.add(0, 0, 45, 0);
    ready = true;
  }

  function update(t, dt, eye, active) {
    if (!ready) return;
    w.setPosition(playerBody, active ? eye[0] * 100 : 99999, active ? eye[2] * 100 : 99999);
    const d = w.data;
    for (let g = 0; g < KINDS; g++) groups[g].count = 0;
    for (let i = 0; i < N; i++) {
      const o = obj[i];
      const b = o.body * 3;
      o.x = d[b] / 100; o.z = d[b + 1] / 100;
      // tumble faster after a shove
      const v = w.velocity ? 0 : 0;
      void v;
      o.rx += o.wx * dt; o.ry += o.wy * dt; o.rz += o.wz * dt;
      const g = groups[o.kind];
      const y = o.y + Math.sin(t * 0.3 + o.bob) * o.bobA;
      g.put(g.count++, o.x, y, o.z, o.ry, o.sx, o.sy, o.sz, o.kind === 3 ? 14.6 : o.kind === 4 ? 6.8 : 0.2,
        o.c[0], o.c[1], o.c[2], o.emis, o.rx, o.rz);
    }
    for (let g = 0; g < KINDS; g++) groups[g].upload();
  }

  // push (+1) or pull (-1) where you are looking; spin up what gets hit
  function use(kind, eye, f) {
    if (!ready) return;
    const dist = 3.5;
    const cx = eye[0] + f[0] * dist, cy = eye[1] + f[1] * dist, cz = eye[2] + f[2] * dist;
    const pull = kind === 'pull';
    if (pull) w.impulse(cx * 100, cz * 100, 900, -420);
    else w.impulse(eye[0] * 100 + f[0] * 120, eye[2] * 100 + f[2] * 120, 1100, 900);
    for (const o of obj) {
      const dx = o.x - cx, dz = o.z - cz;
      if (dx * dx + dz * dz < 40) { o.wx += (Math.random() - 0.5) * 3; o.wy += (Math.random() - 0.5) * 3; o.wz += (Math.random() - 0.5) * 3; }
    }
    ring.x = cx; ring.y = cy; ring.z = cz; ring.pull = pull ? 1 : 0;
    animate(ring, { a: [1, 0], r: pull ? [7, 0.3] : [0.3, 8] }, { duration: 0.9, ease: 'outCubic' });
  }

  function draw(cam, setVP) {
    // sky (depth: far), stars, objects, shockwave
    gl.disable(gl.DEPTH_TEST);
    gl.useProgram(progSky.p);
    gl.uniformMatrix4fv(progSky.u.uInvVP, false, cam.invVP);
    gl.uniform3f(progSky.u.uEye, cam.eye[0], cam.eye[1], cam.eye[2]);
    gl.uniform3f(progSky.u.uSunDir, SUN[0], SUN[1], SUN[2]);
    gl.bindVertexArray(emptyVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE);
    gl.useProgram(progStar.p);
    gl.uniformMatrix4fv(progStar.u.uVP, false, cam.vp);
    gl.uniform3f(progStar.u.uEye, cam.eye[0], cam.eye[1], cam.eye[2]);
    gl.uniform1f(progStar.u.uTime, cam.time);
    gl.uniform1f(progStar.u.uPx, cam.px);
    gl.bindVertexArray(starVao);
    gl.drawArrays(gl.POINTS, 0, NSTAR);
    gl.disable(gl.BLEND);
    gl.enable(gl.DEPTH_TEST);
    gl.useProgram(progObj.p);
    gl.uniformMatrix4fv(progObj.u.uVP, false, cam.vp);
    if (progObj.u.uMirror) gl.uniform1f(progObj.u.uMirror, 1);
    gl.uniform3f(progObj.u.uEye, cam.eye[0], cam.eye[1], cam.eye[2]);
    gl.uniform3f(progObj.u.uSunDir, SUN[0], SUN[1], SUN[2]);
    for (let g = 0; g < KINDS; g++) groups[g].draw();
    if (ring.a > 0.01) {
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE); gl.depthMask(false);
      gl.useProgram(progRing.p);
      gl.uniformMatrix4fv(progRing.u.uVP, false, cam.vp);
      gl.uniform3f(progRing.u.uC, ring.x, ring.y, ring.z);
      gl.uniform3f(progRing.u.uR, cam.r[0], cam.r[1], cam.r[2]);
      gl.uniform3f(progRing.u.uU, cam.u[0], cam.u[1], cam.u[2]);
      gl.uniform1f(progRing.u.uRad, ring.r);
      gl.uniform1f(progRing.u.uA, ring.a);
      if (ring.pull) gl.uniform3f(progRing.u.uTint, 1.0, 0.4, 0.9); else gl.uniform3f(progRing.u.uTint, 0.4, 0.95, 1.0);
      gl.bindVertexArray(emptyVao);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.depthMask(true); gl.disable(gl.BLEND);
    }
    gl.bindVertexArray(null);
    void setVP;
  }

  return {
    id: 'zerog', name: 'Zero gravity', feature: 'world() · 360 bodies, no gravity',
    hint: 'Fly: WASD + look · click pushes · right-click pulls',
    fly: true, radius: 30, spawn: [0, 1.7, 0], spawnYaw: 0,
    fog: [0.03, 0.05, 0.09],
    start, update, use, draw,
  };
}
