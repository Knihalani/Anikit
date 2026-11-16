// hall-demos.js — the hall's five set pieces. Each one shows off a piece of anikit:
//   curtain   world() ropes: ~900 beads on 41 strands that part around your body
//   fireflies spring() swarm: 2,500 glowing points that follow your gaze; E = shockwave
//   glass     timeline() seek: 420 shards; walk in to assemble, back away to shatter
//   sandfall  200k GPU points with spring-driven repulsion: it parts around you
//   pins      batch() + stagger: 1,008 brass pins in flowing waves that form "anikit"
//
// The per-frame paths do not allocate; springs are retargeted only on change, at most
// a few times a second.

import { world, spring, animate, batch, stagger, timeline } from '../../../packages/anikit/dist/anikit.lean.js';
import { program } from './hall-gl.js';

const PI2 = Math.PI * 2;
const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);

// material slots (see hall-shaders.js): mat * 2 + gloss
const PLAIN = 0, METAL = 6, WALNUT = 10, LACQUER = 12, GLASS = 14;
const BRONZE = [0.78, 0.55, 0.32], WOOD = [0.42, 0.29, 0.2], STEEL = [0.2, 0.2, 0.22];

export const STATIONS = [
  { id: 'curtain', name: 'Bead curtain', feature: 'world() · 41 ropes, 902 beads', hint: 'Walk through it · E for a gust', x: 0, z: 5, r: 3.0, ring: 0, pool: 0.5 },
  { id: 'fireflies', name: 'Fireflies', feature: 'spring() swarm · 2,500 lights', hint: 'They follow your gaze · E to scatter', x: -4.9, z: -3.2, r: 3.6, ring: 0, pool: 0 },
  { id: 'glass', name: 'Glass', feature: 'timeline() seek · 420 shards', hint: 'Walk in to assemble, back away to shatter · wheel scrubs · E', x: 4.7, z: -3.2, r: 5.2, ring: 1.25, pool: 0.9 },
  { id: 'sand', name: 'Sandfall', feature: '200,000 points · spring repulsion', hint: 'Walk into it · E to burst', x: 0, z: -14, r: 3.4, ring: 2.65, pool: 0.6 },
  { id: 'pins', name: 'Pin wall', feature: 'batch() + stagger · 1,008 pins', hint: 'Come closer · E sends a ripple', x: 0, z: -23, r: 5.5, ring: 0, pool: 0.7 },
];

// ---------------- shaders for the two point systems ----------------

const FF_VS = /* glsl */ `#version 300 es
layout(location=0) in vec4 aS;
uniform mat4 uVP; uniform float uMirror; uniform float uTime; uniform float uPx;
uniform vec3 uC1; uniform vec3 uC2; uniform vec3 uShockC; uniform float uShock; uniform float uSpread;
out vec3 vC; out float vA;
void main() {
  float a = uTime * (0.35 + 0.7 * aS.y) + aS.x * 6.2832;
  float b = uTime * (0.25 + 0.5 * aS.z) + aS.w * 6.2832;
  float r = (0.35 + 1.5 * pow(aS.z, 0.7)) * uSpread;
  vec3 c = mix(uC1, uC2, aS.w);
  vec3 p = c + vec3(cos(a) * cos(b), sin(b) * 0.6, sin(a) * cos(b)) * r;
  p += 0.22 * vec3(sin(uTime * 1.7 + aS.x * 40.0), sin(uTime * 1.3 + aS.y * 31.0), sin(uTime * 1.1 + aS.z * 23.0));
  vec3 d = p - uShockC;
  float l = length(d) + 1e-3;
  p += d / l * uShock * (1.6 + 3.2 * aS.y);
  p.y = max(p.y, 0.25);
  vec4 cp = uVP * vec4(p.x, p.y * uMirror, p.z, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * 0.05 / cp.w, 2.0, 18.0);
  float bl = 0.5 + 0.5 * sin(uTime * (1.5 + 2.5 * aS.x) + aS.y * 50.0);
  vA = 0.15 + 0.85 * bl * bl * bl;
  vC = mix(vec3(1.0, 0.82, 0.35), vec3(0.55, 1.0, 0.75), step(0.82, aS.y));
}`;
const FF_FS = /* glsl */ `#version 300 es
precision mediump float;
in vec3 vC; in float vA; out vec4 o;
void main() {
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  float r = dot(q, q);
  if (r > 1.0) discard;
  float g = exp(-r * 9.0) * 1.6 + exp(-r * 2.5) * 0.25;
  o = vec4(vC * g * vA, 0.0);
}`;

const SAND_VS = /* glsl */ `#version 300 es
layout(location=0) in vec4 aS;
uniform mat4 uVP; uniform float uMirror; uniform float uTime; uniform float uPx; uniform vec3 uEye; uniform vec3 uFog;
uniform vec3 uC; uniform vec2 uP; uniform float uPart; uniform float uBurst;
out vec3 vC; out float vFog;
void main() {
  float ph = fract(uTime * (0.16 + 0.06 * aS.w) + aS.x);
  float top = 11.7;
  float y = top * (1.0 - (0.62 * ph + 0.38 * ph * ph));
  float sy = aS.y - 0.5, sz = (aS.z - 0.5) * (aS.z - 0.5) * sign(aS.z - 0.5) * 2.0;
  vec3 p = vec3(uC.x + sy * 2.8 * (1.0 + ph * 0.15), y, uC.z + sz * 0.45 * (1.0 + ph * 0.8));
  // part around the player: a soft cylinder, body height
  vec2 d = p.xz - uP;
  float l = length(d) + 1e-4;
  float R = 0.62 * uPart;
  float push = R * exp(-(l * l) / (R * R + 0.02)) * smoothstep(2.3, 1.6, p.y);
  p.xz += d / l * push * 1.6;
  // splash at the bottom
  float lo = 1.0 - smoothstep(0.0, 0.45, p.y);
  vec2 o = p.xz - uC.xz;
  p.xz += normalize(o + vec2(1e-3)) * lo * (0.4 + 1.6 * aS.w) * 0.45;
  p.y = max(p.y, 0.03 + 0.12 * aS.w * lo);
  // burst
  vec3 bd = normalize(vec3(o.x, 0.6 + aS.w, o.y) + 0.001);
  p += bd * uBurst * (1.0 + 3.0 * aS.w) * (0.5 + 0.5 * sin(aS.x * 40.0));
  vec4 cp = uVP * vec4(p.x, p.y * uMirror, p.z, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * (0.011 + 0.01 * aS.w) / cp.w, 1.0, 5.0);
  float sp = pow(fract(aS.x * 91.7 + uTime * 0.6), 24.0);
  vec3 c = mix(vec3(1.0, 0.76, 0.38), vec3(0.92, 0.48, 0.2), aS.w * aS.w) * (0.85 + 0.25 * aS.z) + vec3(1.0, 0.95, 0.8) * sp * 1.4;
  vC = c * (0.62 + 0.5 * uBurst);
  vFog = (1.0 - exp(-length(uEye - p) * 0.014)) * 0.7;
}`;
const SAND_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform vec3 uFog;
in vec3 vC; in float vFog; out vec4 o;
vec3 aces(vec3 c) { c *= 0.9; c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2)); }
void main() {
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  if (dot(q, q) > 1.0) discard;
  o = vec4(aces(mix(vC, uFog, vFog)), 1.0);
}`;

function mulberry(s) {
  return () => {
    s |= 0; s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ctx: { gl, cam, sees, group, cube, sph, sphLo, cyl, box, archCyl, colliders, setCommon }
export function createDemos(ctx) {
  const { gl, cam, sees, group, cube, sphLo, box, archCyl, colliders } = ctx;
  const rnd = mulberry(1234);
  const wake = STATIONS.map(() => ({ v: 0 }));
  let ready = false;

  // =====================================================================
  // 1. Bead curtain — world() ropes
  // =====================================================================
  const CZ = 5, CX0 = -6.4, CTOP = 6.6, STR = 41, SEG = 21;
  const NBEAD = STR * (SEG + 1);
  const beads = group(gl, sphLo, NBEAD);
  const strings = group(gl, cube, STR * SEG);
  let cw = null, beadFirst = 0, playerFirst = 0;
  const beadZ = new Float32Array(NBEAD);
  box(ctx.archBox, 0, CTOP + 0.06, CZ, 13.4, 0.12, 0.14, METAL + 0.6, ...BRONZE);
  let gustT = 0;
  function initCurtain() {
    cw = world({ gravity: [0, 980], bounds: false, damping: 0.6, substeps: 4, iterations: 3, restitution: 0.1, friction: 0.2 });
    playerFirst = cw.add(-900, 0, 26, 0);
    for (let k = 1; k < 4; k++) cw.add(-900, 0, 26, 0);
    for (let s = 0; s < STR; s++) {
      const x = s * 32;
      const idx = cw.rope([[x, 0], [x, SEG * 28]], { segments: SEG, radius: 4, pinStart: true, stiffness: 1 });
      if (s === 0) beadFirst = idx[0];
    }
  }
  const PY = [0.45, 0.9, 1.35, 1.75];
  function updateCurtain(t, dt, px, pz, active) {
    if (!cw) return;
    const near = active && Math.abs(pz - CZ) < 0.42;
    for (let k = 0; k < 4; k++) {
      if (near) cw.setPosition(playerFirst + k, (px - CX0) * 100, (CTOP - PY[k]) * 100);
      else cw.setPosition(playerFirst + k, -900, k * 60);
    }
    gustT -= dt;
    if (gustT <= 0) { gustT = 1.6 + rnd() * 2; cw.impulse(rnd() * 1280, 300 + rnd() * 250, 220, 60); }
    if (!sees(cam, 0, 3.4, CZ, 7.5)) { beads.visible = strings.visible = false; return; }
    beads.visible = strings.visible = true;
    const d = cw.data;
    const side = pz > CZ ? -1 : 1;
    const wk = wake[0].v;
    beads.count = NBEAD;
    strings.count = STR * SEG;
    let si = 0;
    for (let s = 0; s < STR; s++) {
      for (let k = 0; k <= SEG; k++) {
        const i = s * (SEG + 1) + k, b = (beadFirst + i) * 3;
        const x = CX0 + d[b] / 100, y = CTOP - d[b + 1] / 100;
        // a soft depth bulge away from the body, eased per bead
        const dx = x - px, dy = y - 1.1;
        const want = near || Math.abs(pz - CZ) < 1.2 ? side * 0.55 * Math.exp(-(dx * dx) / 0.35 - (dy * dy) / 1.6) * clamp(1.4 - Math.abs(pz - CZ), 0, 1) : 0;
        beadZ[i] += (want - beadZ[i]) * Math.min(1, dt * 10);
        const z = CZ + beadZ[i];
        const h = k / SEG;
        const sc = k === 0 ? 0.03 : 0.062 + 0.012 * ((s + k) % 3);
        beads.put(i, x, y, z, 0, sc, sc, sc, LACQUER + 0.95,
          1.0, 0.62 + 0.25 * h, 0.22 + 0.45 * h, 0.04 + 0.25 * wk * h);
        if (k > 0) {
          const pb = b - 3;
          const x0 = CX0 + d[pb] / 100, y0 = CTOP - d[pb + 1] / 100;
          const ex = x - x0, ey = y - y0, len = Math.hypot(ex, ey);
          strings.put(si++, (x + x0) / 2, (y + y0) / 2, CZ + (beadZ[i] + beadZ[i - 1]) / 2, 0, 0.007, len, 0.007, METAL + 0.5,
            0.75, 0.6, 0.4, 0, 0, Math.atan2(-ex, ey));
        }
      }
    }
    beads.upload(); strings.upload(si);
  }
  function useCurtain(px) { if (cw) cw.impulse((px - CX0) * 100, 250, 380, 520); }

  // =====================================================================
  // 2. Fireflies — spring() swarm in a dark alcove
  // =====================================================================
  const NF = 2500, FX = -4.9, FZ = -3.2;
  // the alcove: a deep-blue velvet bay and rug so the lights read
  box(ctx.archBox, -7.9, 3.4, FZ, 0.12, 6.8, 7.4, PLAIN + 0.15, 0.1, 0.12, 0.2);
  archCyl.push(FX, 0.012, FZ, 0, 5.2, 0.024, 5.2, PLAIN + 0.2, 0.1, 0.12, 0.2, 0);
  const progFF = program(gl, FF_VS, FF_FS);
  const ffVao = gl.createVertexArray();
  gl.bindVertexArray(ffVao);
  const ffSeed = new Float32Array(NF * 4);
  for (let i = 0; i < ffSeed.length; i++) ffSeed[i] = rnd();
  const ffb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, ffb);
  gl.bufferData(gl.ARRAY_BUFFER, ffSeed, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);
  const ff = { shock: 0, spread: 0.8, sx: FX, sy: 2, sz: FZ };
  const fc1 = { x: FX, y: 2.2, z: FZ }, fc2 = { x: FX, y: 2.0, z: FZ };
  let ffT = 0, ffTx = FX, ffTy = 2.2, ffTz = FZ, ffFollow = false;
  function updateFireflies(t, dt, px, pz, active) {
    const inZone = active && Math.hypot(px - FX, pz - FZ) < 4.2;
    ffT -= dt;
    if (ffT > 0) return;
    ffT = 0.07;
    let tx = FX, ty = 2.2, tz = FZ;
    if (inZone) {
      tx = clamp(cam.eye[0] + cam.f[0] * 2.4, -7.3, -1.2);
      ty = clamp(cam.eye[1] + cam.f[1] * 2.4, 0.9, 4.2);
      tz = clamp(cam.eye[2] + cam.f[2] * 2.4, FZ - 3.6, FZ + 3.6);
    }
    if (inZone !== ffFollow) { ffFollow = inZone; spring(ff, { spread: inZone ? 1 : 0.8 }, { stiffness: 30, damping: 9 }); }
    if (Math.abs(tx - ffTx) + Math.abs(ty - ffTy) + Math.abs(tz - ffTz) < 0.04) return;
    ffTx = tx; ffTy = ty; ffTz = tz;
    spring(fc1, { x: tx, y: ty, z: tz }, { stiffness: 34, damping: 10 });
    spring(fc2, { x: tx, y: ty, z: tz }, { stiffness: 7, damping: 4.5 });
  }
  function useFireflies() {
    ff.sx = fc1.x; ff.sy = fc1.y; ff.sz = fc1.z;
    animate(ff, { shock: [0, 1, 0] }, { duration: 2.0, times: [0, 0.1, 1], ease: ['outExpo', 'inOutSine'] });
  }
  function drawFireflies(mirror) {
    if (!sees(cam, fc1.x, fc1.y, fc1.z, 4)) return;
    ctx.setCommon(progFF, mirror);
    const u = progFF.u;
    gl.uniform3f(u.uC1, fc1.x, fc1.y, fc1.z);
    gl.uniform3f(u.uC2, fc2.x, fc2.y, fc2.z);
    gl.uniform3f(u.uShockC, ff.sx, ff.sy, ff.sz);
    gl.uniform1f(u.uShock, ff.shock);
    gl.uniform1f(u.uSpread, ff.spread);
    gl.bindVertexArray(ffVao);
    gl.drawArrays(gl.POINTS, 0, mirror < 0 ? NF >> 1 : NF);
  }

  // =====================================================================
  // 3. Glass — 420 shards on one paused timeline, scrubbed by distance and wheel
  // =====================================================================
  const NG = 420, GX = 4.7, GY = 2.75, GZ = -3.2, GR = 1.05;
  archCyl.push(GX, 0.45, GZ, 0, 1.1, 0.9, 1.1, WALNUT + 0.5, ...WOOD, 0);
  archCyl.push(GX, 0.92, GZ, 0, 1.16, 0.05, 1.16, METAL + 0.7, ...BRONZE, 0);
  colliders.push(GX - 0.6, GZ - 0.6, GX + 0.6, GZ + 0.6);
  const shards = group(gl, cube, NG);
  const sh = []; // per shard: assembled + exploded transforms
  {
    const ga = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < NG; i++) {
      const y = 1 - (i / (NG - 1)) * 2, r = Math.sqrt(1 - y * y), th = ga * i;
      const nx = Math.cos(th) * r, ny = y, nz = Math.sin(th) * r;
      const dist = GR + 0.9 + 2.4 * rnd();
      sh.push({
        k: 0,
        ax: GX + nx * GR, ay: GY + ny * GR, az: GZ + nz * GR,
        arx: -Math.asin(ny), ary: Math.atan2(nx, nz), arz: rnd() * PI2,
        ex: GX + nx * dist + (rnd() - 0.5) * 0.8, ey: Math.max(0.25, GY + ny * dist * 0.8 + (rnd() - 0.3) * 0.8), ez: GZ + nz * dist + (rnd() - 0.5) * 0.8,
        erx: (rnd() - 0.5) * 9, ery: (rnd() - 0.5) * 9, erz: (rnd() - 0.5) * 9,
        w: 0.17 + 0.08 * rnd(), h: 0.2 + 0.1 * rnd(),
        tint: rnd(),
      });
    }
    sh.sort((a, b) => a.ay - b.ay); // assemble from the base up
  }
  let glassTl = null;
  const scrub = { p: 0 };
  let scrubTarget = 0, scrubT = 0, wheelBias = 0, scrubLock = 0, lastP = -1;
  function initGlass() {
    glassTl = timeline({ autoplay: false })
      .add(sh, { k: [0, 1] }, { duration: 1.1, ease: 'outCubic', delay: stagger(1.5 / NG) });
  }
  function updateGlass(t, dt, px, pz, active) {
    const dist = Math.hypot(px - GX, pz - GZ);
    const inZone = active && dist < 6.5;
    if (!inZone) wheelBias *= Math.max(0, 1 - dt * 2);
    const target = clamp((6.0 - dist) / 3.6 + wheelBias, 0, 1);
    scrubT -= dt;
    if (t > scrubLock && scrubT <= 0 && Math.abs(target - scrubTarget) > 0.004) {
      scrubT = 0.05; scrubTarget = target;
      spring(scrub, { p: target }, { stiffness: 60, damping: 15 });
    }
    if (glassTl && Math.abs(scrub.p - lastP) > 1e-4) { lastP = scrub.p; glassTl.progress = clamp(scrub.p, 0, 1); }
    if (!sees(cam, GX, GY, GZ, 4.5)) { shards.visible = false; return; }
    shards.visible = true;
    shards.count = NG;
    const wk = wake[2].v;
    for (let i = 0; i < NG; i++) {
      const s = sh[i], k = s.k, j = 1 - k;
      const lift = Math.sin(k * Math.PI) * 0.15;
      shards.put(i, s.ex * j + s.ax * k, s.ey * j + s.ay * k + lift, s.ez * j + s.az * k,
        s.ery * j + s.ary * k, s.w, s.h, 0.02, GLASS + 0.95,
        0.6 + 0.3 * s.tint, 0.85, 1.0 - 0.15 * s.tint, 0.02 + 0.12 * wk * k,
        s.erx * j + s.arx * k, s.erz * j + s.arz * k);
    }
    shards.upload();
  }
  function onWheel(e) { if (wheelActive) wheelBias = clamp(wheelBias - e.deltaY * 0.0012, -1, 1); }
  let wheelActive = false;
  addEventListener('wheel', onWheel, { passive: true });
  function useGlass(t) {
    // shatter and re-form
    scrubLock = t + 2.4;
    const p = scrub.p;
    animate(scrub, { p: [p, 0, Math.max(p, 0.999)] }, { duration: 2.4, times: [0, 0.14, 1], ease: ['outExpo', 'inOutCubic'] });
    scrubTarget = -1;
  }

  // =====================================================================
  // 4. Sandfall — 200k points falling from a slot, parting around you
  // =====================================================================
  const NS = 200_000, SX = 0, SZ = -14;
  box(ctx.archBox, SX, 11.85, SZ, 4.4, 0.25, 0.9, METAL + 0.5, ...STEEL);
  archCyl.push(SX, 0.05, SZ, 0, 5.4, 0.1, 5.4, METAL + 0.6, ...BRONZE, 0);
  archCyl.push(SX, 0.07, SZ, 0, 5.2, 0.1, 5.2, PLAIN + 0.1, 0.86, 0.68, 0.42, 0);
  const progSand = program(gl, SAND_VS, SAND_FS);
  const sandVao = gl.createVertexArray();
  gl.bindVertexArray(sandVao);
  const sSeed = new Float32Array(NS * 4);
  for (let i = 0; i < sSeed.length; i++) sSeed[i] = rnd();
  const sb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, sb);
  gl.bufferData(gl.ARRAY_BUFFER, sSeed, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);
  const sand = { part: 0, burst: 0, px: 0, pz: 0 };
  let sandIn = false;
  function updateSand(t, dt, px, pz, active) {
    const inZone = active && Math.abs(px - SX) < 3.2 && Math.abs(pz - SZ) < 2.2;
    sand.px = px; sand.pz = pz;
    if (inZone !== sandIn) { sandIn = inZone; spring(sand, { part: inZone ? 1 : 0 }, { stiffness: 120, damping: 14 }); }
  }
  function useSand() { animate(sand, { burst: [0, 1, 0] }, { duration: 1.8, times: [0, 0.15, 1], ease: ['outExpo', 'inOutSine'] }); }
  function drawSand(mirror) {
    if (!sees(cam, SX, 6, SZ, 6.5)) return;
    ctx.setCommon(progSand, mirror);
    const u = progSand.u;
    gl.uniform3f(u.uC, SX, 0, SZ);
    gl.uniform2f(u.uP, sand.px, sand.pz);
    gl.uniform1f(u.uPart, sand.part);
    gl.uniform1f(u.uBurst, sand.burst);
    gl.bindVertexArray(sandVao);
    gl.drawArrays(gl.POINTS, 0, mirror < 0 ? NS >> 2 : NS);
  }

  // =====================================================================
  // 5. Pin wall — batch() + stagger waves; forms "anikit" as you approach
  // =====================================================================
  const PGX = 56, PGY = 18, NP = PGX * PGY, PSP = 0.22, PWZ = -27.7, PY0 = 1.0;
  box(ctx.archBox, 0, PY0 + (PGY - 1) * PSP / 2, -27.93, PGX * PSP + 0.5, PGY * PSP + 0.5, 0.1, METAL + 0.35, 0.12, 0.12, 0.13);
  box(ctx.archBox, 0, PY0 + PGY * PSP + 0.1, -27.85, PGX * PSP + 0.8, 0.14, 0.25, METAL + 0.6, ...BRONZE);
  box(ctx.archBox, 0, PY0 - 0.32, -27.85, PGX * PSP + 0.8, 0.14, 0.25, METAL + 0.6, ...BRONZE);
  box(ctx.archBox, 0, 0.3, -27.4, PGX * PSP + 1, 0.6, 1.0, WALNUT + 0.5, ...WOOD, 0, true);
  const pins = group(gl, sphLo, NP);
  const rods = group(gl, cube, NP);
  const mask = new Float32Array(NP);
  {
    const cv = document.createElement('canvas');
    cv.width = PGX; cv.height = PGY;
    const c = cv.getContext('2d', { willReadFrequently: true });
    c.fillStyle = '#fff';
    c.font = '800 15px system-ui, -apple-system, Segoe UI, sans-serif';
    c.textAlign = 'center'; c.textBaseline = 'middle';
    c.fillText('anikit', PGX / 2, PGY / 2 + 1);
    const d = c.getImageData(0, 0, PGX, PGY).data;
    for (let gy = 0; gy < PGY; gy++) for (let gx = 0; gx < PGX; gx++) {
      mask[(PGY - 1 - gy) * PGX + gx] = Math.min(1, d[(gy * PGX + gx) * 4 + 3] / 160);
    }
  }
  let pinWave = null, pinRipple = null;
  const logo = { m: 0 };
  let logoOn = false;
  function initPins() {
    pinWave = batch(NP, { z: [0, 0.32, 0, -0.18, 0] }, {
      duration: 3.2, repeat: -1, ease: 'inOutSine', delay: stagger(0.045, { grid: [PGX, PGY], from: 'center' }),
    });
  }
  function updatePins(t, dt, px, pz, active) {
    const on = Math.hypot(px, pz - PWZ) < 10.5 && pz < -16;
    if (on !== logoOn) { logoOn = on; spring(logo, { m: on ? 1 : 0 }, { stiffness: 26, damping: 8 }); }
    if (!sees(cam, 0, PY0 + 2, PWZ, 7.5)) { pins.visible = rods.visible = false; return; }
    pins.visible = rods.visible = true;
    pins.count = rods.count = NP;
    const w = pinWave, r = pinRipple;
    const wd = w ? w.data : null, ws = w ? w.stride : 0, wz = w ? w.channels.z : 0;
    const rd = r ? r.data : null, rs = r ? r.stride : 0, rz = r ? r.channels.z : 0;
    const m = logo.m, wk = wake[4].v;
    for (let i = 0; i < NP; i++) {
      const gx = i % PGX, gy = (i / PGX) | 0;
      let z = wd ? wd[i * ws + wz] * (1 - 0.8 * m) : 0;
      if (rd) z += rd[i * rs + rz];
      const mk = mask[i];
      z += 0.06 + mk * m * 0.62;
      const x = (gx - (PGX - 1) / 2) * PSP, y = PY0 + gy * PSP;
      const g = mk * m;
      pins.put(i, x, y, PWZ + z, 0, 0.17, 0.17, 0.17, METAL + 0.9,
        0.78 + 0.22 * g, 0.62 + 0.14 * g, 0.42 - 0.12 * g, g * (0.25 + 0.3 * wk));
      const len = Math.max(0.02, z + 0.2);
      rods.put(i, x, y, PWZ + z - len / 2, 0, 0.018, 0.018, len, METAL + 0.6, 0.55, 0.5, 0.45, 0);
    }
    pins.upload(); rods.upload();
  }
  function usePins(px, py) {
    const gx = Math.round(clamp(px / PSP + (PGX - 1) / 2, 0, PGX - 1));
    const gy = Math.round(clamp((py - PY0) / PSP, 0, PGY - 1));
    const b = batch(NP, { z: [0, 0.55, -0.12, 0] }, { duration: 1.3, ease: 'inOutSine', delay: stagger(0.03, { grid: [PGX, PGY], from: gy * PGX + gx }) });
    const old = pinRipple;
    pinRipple = b;
    if (old) old.kill();
  }

  // =====================================================================
  return {
    stations: STATIONS,
    wake,
    blobs: new Float32Array([
      GX, GZ, 0.9, 0.4,   SX, SZ, 2.7, 0.2,   FX, FZ, 2.6, 0.25,
      0, -27.4, 3.0, 0.25,   0, 0, 0, 0,   0, 0, 0, 0,
    ]),
    start() {
      initCurtain(); initGlass(); initPins();
      ready = true;
    },
    update(t, dt, px, pz, active) {
      wheelActive = active && Math.hypot(px - GX, pz - GZ) < 6.5;
      if (!ready) return;
      updateCurtain(t, dt, px, pz, active);
      updateFireflies(t, dt, px, pz, active);
      updateGlass(t, dt, px, pz, active);
      updateSand(t, dt, px, pz, active);
      updatePins(t, dt, px, pz, active);
    },
    drawMeshes() { beads.draw(); strings.draw(); pins.draw(); rods.draw(); },
    drawGlass() { shards.draw(); },
    drawPoints(mirror) { drawSand(mirror); },
    drawGlow(mirror) { drawFireflies(mirror); },
    use(i, t, px, pz) {
      if (!ready) return;
      const id = STATIONS[i].id;
      if (id === 'curtain') useCurtain(px);
      else if (id === 'fireflies') useFireflies();
      else if (id === 'glass') useGlass(t);
      else if (id === 'sand') useSand();
      else if (id === 'pins') {
        // ripple from where you are looking on the wall
        const tz = cam.f[2] < -0.05 ? (PWZ - cam.eye[2]) / cam.f[2] : 0;
        usePins(tz ? cam.eye[0] + cam.f[0] * tz : px, tz ? cam.eye[1] + cam.f[1] * tz : 2.0);
      }
    },
    onWake(i, on) {
      if (on && ready && STATIONS[i].id === 'pins') usePins(0, PY0 + PGY * PSP / 2);
    },
    setVisible(all) { beads.visible = strings.visible = shards.visible = pins.visible = rods.visible = all; },
  };
}
