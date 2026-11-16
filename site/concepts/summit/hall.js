// hall.js — the hall of portals. A short, calm gallery in hand-written WebGL2 whose side
// bays hold four spark-ringed portals, each a live window into a small world
// (hall-world-*.js). Walk through one to enter that world; a portal behind you leads back.
// createHall(canvasOrGl, overlayEl) → { resize, preview, enter, exit, frame }  (CONTRACT.md)
//
// Budget: one world at full cost at a time. In the hall, a world is drawn only through its
// portal (stencil-masked, scissored to the portal's screen box); worlds you are not nearest
// to update every second frame. Inside a world, only that world (and the hall, through the
// small return portal) is drawn.

import { ready } from '../../../packages/anikit/dist/anikit.lean.js';
import { program, cubeMesh, cylinderMesh, group, camera, updateCamera, sees } from './hall-gl.js';
import * as S from './hall-shaders.js';
import { createControls } from './hall-controls.js';
import { createZeroG } from './hall-world-zerog.js';

const SUN = [0.72, -0.62, -0.3];
{ const l = Math.hypot(SUN[0], SUN[1], SUN[2]); SUN[0] /= l; SUN[1] /= l; SUN[2] /= l; }
const FOG = [0.55, 0.49, 0.42];
const SPAWN = [0, 1.7, 12];
const PI = Math.PI;
// hall extents (a short gallery)
const HX = 8, HZ0 = -13.5, HZ1 = 16, HY = 12;
const BLOBS = new Float32Array(24);

// ?perf=no-mirror,no-bloom,no-fxaa,no-shafts,no-worlds switches passes off for measurement
function perfFlags() {
  const off = new Set(((globalThis.location && new URLSearchParams(location.search).get('perf')) || '').split(',').map((s) => s.trim()));
  const on = (k) => !off.has('no-' + k);
  return { mirror: on('mirror'), bloom: on('bloom'), fxaa: on('fxaa'), shafts: on('shafts'), worlds: on('worlds') };
}

// portals: in the side bays between columns, facing the aisle. yaw = the view direction of
// someone looking into the portal (forward = (-sin yaw, -cos yaw)).
const PORTALS = [
  { x: -7.15, z: 3.5, yaw: PI / 2, world: 0, tint: [1, 0.55, 0.2] },
  { x: 7.15, z: 3.5, yaw: -PI / 2, world: 1, tint: [1, 0.55, 0.2] },
  { x: -7.15, z: -6.5, yaw: PI / 2, world: 2, tint: [1, 0.55, 0.2] },
  { x: 7.15, z: -6.5, yaw: -PI / 2, world: 3, tint: [1, 0.55, 0.2] },
];
const PR = 1.45, PY = 2.15; // portal radius and centre height

export function createHall(canvasOrGl, overlayEl) {
  const PERF = perfFlags();
  const shared = typeof WebGL2RenderingContext !== 'undefined' && canvasOrGl instanceof WebGL2RenderingContext;
  const gl = shared ? canvasOrGl : canvasOrGl.getContext('webgl2', { antialias: false, alpha: false, depth: true, stencil: true, powerPreference: 'high-performance', preserveDrawingBuffer: false });
  if (!gl) throw new Error('hall: WebGL2 unavailable');
  const canvas = gl.canvas;

  const progMesh = program(gl, S.MESH_VS, S.MESH_FS);
  const progFloor = program(gl, S.FLOOR_VS, S.FLOOR_FS);
  const progBeam = program(gl, S.BEAM_VS, S.BEAM_FS);
  const progDust = program(gl, S.DUST_VS, S.DUST_FS);
  const progPortal = program(gl, S.PORTAL_VS, S.PORTAL_FS);

  const cube = cubeMesh(), cyl = cylinderMesh(22);
  const camH = camera();      // the player's camera
  const camX = camera();      // a camera seen through a portal
  let cam = camH;             // whichever camera is drawing right now
  let W = 1, H = 1, pxScale = 1;

  // ---------- static architecture ----------
  const archBox = group(gl, cube, 256);
  const archCyl = group(gl, cyl, 128);
  const windows = group(gl, cube, 32);
  const colliders = [];
  const box = (g, x, y, z, sx, sy, sz, gloss, r, gg, b, e = 0, collide = false) => {
    g.push(x, y, z, 0, sx, sy, sz, gloss, r, gg, b, e);
    if (collide) colliders.push(x - sx / 2, z - sz / 2, x + sx / 2, z + sz / 2);
  };
  const PLAIN = 0, PLASTER = 2, FLUTE = 4, METAL = 6, COFFER = 8, WALNUT = 10;
  const WALL = [0.8, 0.74, 0.66], MARBLE = [0.92, 0.9, 0.86], BRONZE = [0.78, 0.55, 0.32], STEEL = [0.2, 0.2, 0.22];
  const WOOD = [0.42, 0.29, 0.2], BEAM = [0.66, 0.61, 0.55], DEEP = [0.36, 0.38, 0.44];
  box(archBox, -HX - 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, PLASTER + 0.05, ...WALL);
  box(archBox, HX + 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ0 - 0.5, 2 * HX + 2, HY, 1, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ1 + 0.5, 2 * HX + 2, HY, 1, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY + 0.5, (HZ0 + HZ1) / 2, 2 * HX + 2, 1, HZ1 - HZ0 + 2, COFFER, ...DEEP, 0.12);
  for (const s of [-1, 1]) {
    box(archBox, s * (HX - 0.15), 11.6, (HZ0 + HZ1) / 2, 0.3, 0.3, HZ1 - HZ0, METAL + 0.5, ...BRONZE);
    box(archBox, s * (HX - 0.08), 5.9, (HZ0 + HZ1) / 2, 0.16, 0.2, HZ1 - HZ0, PLAIN + 0.2, ...MARBLE);
  }
  for (let z = HZ0 + 2.5; z < HZ1; z += 5) box(archBox, 0, HY - 0.45, z, 2 * HX, 0.9, 0.5, COFFER + 0.1, ...BEAM);
  for (const x of [-4, 0, 4]) box(archBox, x, HY - 0.35, (HZ0 + HZ1) / 2, 0.4, 0.7, HZ1 - HZ0, COFFER + 0.1, ...BEAM);
  for (let z = 11; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      const x = s * 7.0;
      archCyl.push(x, 5.6, z, 0, 0.82, 10.4, 0.82, FLUTE + 0.35, ...MARBLE, 0);
      box(archBox, x, 0.3, z, 1.2, 0.6, 1.2, WALNUT + 0.4, ...WOOD, 0, true);
      box(archBox, x, 10.95, z, 1.15, 0.5, 1.15, METAL + 0.6, ...BRONZE, 0);
      archCyl.push(x, 0.7, z, 0, 1.0, 0.22, 1.0, METAL + 0.6, ...BRONZE, 0);
    }
  }
  for (let z = 8.5; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      windows.push(s * (HX - 0.02), 8.6, z, 0, 0.06, 4.2, 2.2, 0, 1, 1, 1, 3);
      box(archBox, s * (HX - 0.05), 6.35, z, 0.25, 0.25, 2.6, METAL + 0.4, ...STEEL);
    }
  }
  windows.push(0, 3.4, HZ1 - 0.02, 0, 3.6, 6.4, 0.06, 0, 1, 1, 1, 3);
  box(archBox, 0, 6.8, HZ1 - 0.1, 4.4, 0.5, 0.3, METAL + 0.5, ...BRONZE);
  box(archBox, -2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, METAL + 0.5, ...BRONZE);
  box(archBox, 2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, METAL + 0.5, ...BRONZE);
  // end wall: a tall bronze niche
  box(archBox, 0, 5.5, HZ0 + 0.1, 5.0, 0.3, 0.3, METAL + 0.6, ...BRONZE);
  box(archBox, 0, 2.75, HZ0 + 0.05, 4.6, 5.5, 0.1, PLAIN + 0.1, 0.3, 0.26, 0.24);
  // portal plinths: a low walnut step and a bronze base under each ring
  for (const p of PORTALS) {
    const s = Math.sign(p.x);
    box(archBox, p.x - s * 0.15, 0.08, p.z, 1.1, 0.16, 3.4, WALNUT + 0.5, ...WOOD);
    box(archBox, p.x - s * 0.3, 0.17, p.z, 0.5, 0.03, 3.2, METAL + 0.6, ...BRONZE);
    BLOBS.set([p.x, p.z, 1.6, 0.25], PORTALS.indexOf(p) * 4);
  }
  archBox.upload(); archCyl.upload(); windows.upload();

  // floor, shafts, dust
  const floorVao = gl.createVertexArray();
  gl.bindVertexArray(floorVao);
  const fb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, fb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-HX, HZ0, HX, HZ0, HX, HZ1, -HX, HZ0, HX, HZ1, -HX, HZ1]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const beamV = [];
  for (let z = 8.5; z > HZ0; z -= 5) {
    const L = Math.min((HX * 2 - 1.5) / SUN[0], 8.6 / -SUN[1] + 1.5);
    for (const [ya, yb, za, zb] of [[6.6, 10.6, z, z], [8.6, 8.6, z - 1.1, z + 1.1]]) {
      const p0 = [-HX + 0.05, ya, za], p1 = [-HX + 0.05, yb, zb];
      const q0 = [p0[0] + SUN[0] * L, p0[1] + SUN[1] * L, p0[2] + SUN[2] * L];
      const q1 = [p1[0] + SUN[0] * L, p1[1] + SUN[1] * L, p1[2] + SUN[2] * L];
      beamV.push(...p0, 0, 0, ...p1, 1, 0, ...q1, 1, 1, ...p0, 0, 0, ...q1, 1, 1, ...q0, 0, 1);
    }
  }
  const beamVao = gl.createVertexArray();
  gl.bindVertexArray(beamVao);
  const bb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, bb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(beamV), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 20, 12);
  const beamCount = beamV.length / 5;
  const DUST = 600;
  const dustData = new Float32Array(DUST * 4);
  for (let i = 0; i < DUST; i++) {
    const bay = 8.5 - 5 * Math.floor(Math.random() * 5);
    const t = Math.random() * 13;
    dustData[i * 4] = -HX + SUN[0] * t + (Math.random() - 0.5) * 0.6;
    dustData[i * 4 + 1] = 8.6 + SUN[1] * t + (Math.random() - 0.5) * 3.5;
    dustData[i * 4 + 2] = bay + SUN[2] * t + (Math.random() - 0.5) * 2.0;
    dustData[i * 4 + 3] = Math.random();
  }
  const dustVao = gl.createVertexArray();
  gl.bindVertexArray(dustVao);
  const db = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, db);
  gl.bufferData(gl.ARRAY_BUFFER, dustData, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);
  const emptyVao = gl.createVertexArray();

  // ---------- worlds ----------
  const lib = { program, group, cube, MESH_VS: S.MESH_VS, sees };
  const WORLDS = [createZeroG(gl, lib), null, null, null];
  const st = { ready: false, active: false, mode: -1, near: -1, frame: 0 };
  ready.then(() => { for (const w of WORLDS) if (w) w.start(); st.ready = true; });

  // ---------- controls & HUD ----------
  const HALL_BOUNDS = [-HX + 0.45, HZ0 + 0.45, HX - 0.45, HZ1 - 0.45];
  const ctl = createControls(canvas, overlayEl, {
    spawn: SPAWN, colliders, bounds: HALL_BOUNDS,
    onUse: (kind) => use(kind),
  });
  function use(kind) {
    if (!st.ready || st.mode < 0) return;
    WORLDS[st.mode].use(kind, ctl.pos, camH.f);
  }

  // ---------- portal maths (no allocation) ----------
  // hall → world: q' = spawn + R(phi) (q - P), yaw' = yaw + phi, with phi = worldYaw - portalYaw
  function rot(phi, x, z, out, o) { const c = Math.cos(phi), s = Math.sin(phi); out[o] = x * c + z * s; out[o + 1] = -x * s + z * c; }
  const tmp2 = new Float32Array(2);
  function hallToWorld(p, w, src, yaw, dst) {
    const phi = w.spawnYaw - p.yaw;
    rot(phi, src[0] - p.x, src[2] - p.z, tmp2, 0);
    dst[0] = w.spawn[0] + tmp2[0]; dst[1] = w.spawn[1] + (src[1] - 1.7); dst[2] = w.spawn[2] + tmp2[1];
    return yaw + phi;
  }
  function worldToHall(p, w, src, yaw, dst) {
    const phi = p.yaw - w.spawnYaw;
    rot(phi, src[0] - w.spawn[0], src[2] - w.spawn[2], tmp2, 0);
    dst[0] = p.x + tmp2[0]; dst[1] = 1.7 + (src[1] - w.spawn[1]); dst[2] = p.z + tmp2[1];
    return yaw + phi;
  }
  // signed distance past a portal plane, along its view direction, and the lateral offset
  function crossing(px, pz, cx, cz, yaw, out) {
    const fx = -Math.sin(yaw), fz = -Math.cos(yaw);
    out[0] = (px - cx) * fx + (pz - cz) * fz;
    out[1] = (px - cx) * -fz + (pz - cz) * fx;
  }
  const cr = new Float32Array(2);
  let prevS = 0, prevWS = 0;

  // ---------- rendering ----------
  let time = 0, baseFov = 1.12;
  const POOLS = new Float32Array(24);
  const GLASSL = new Float32Array(4);
  function setCommon(p, mirror) {
    gl.useProgram(p.p);
    const u = p.u;
    if (u.uVP) gl.uniformMatrix4fv(u.uVP, false, cam.vp);
    if (u.uMirror) gl.uniform1f(u.uMirror, mirror);
    if (u.uEye) gl.uniform3f(u.uEye, cam.eye[0], cam.eye[1], cam.eye[2]);
    if (u.uSun) gl.uniform3f(u.uSun, SUN[0], SUN[1], SUN[2]);
    if (u.uFog) gl.uniform3f(u.uFog, FOG[0], FOG[1], FOG[2]);
    if (u.uTime) gl.uniform1f(u.uTime, time);
    if (u.uPx) gl.uniform1f(u.uPx, pxScale);
    if (u.uPool) gl.uniform4fv(u.uPool, POOLS);
    if (u.uGlassL) gl.uniform4fv(u.uGlassL, GLASSL);
  }
  function updatePools() {
    for (let i = 0; i < 4; i++) {
      const p = PORTALS[i];
      POOLS[i * 4] = p.x - Math.sign(p.x) * 1.2; POOLS[i * 4 + 1] = p.z; POOLS[i * 4 + 2] = 2.4;
      POOLS[i * 4 + 3] = i === st.near ? 0.9 : 0.5;
    }
  }
  const quality = { refl: true, slow: 0, lock: false };
  function watch(dt) {
    if (!quality.refl || quality.lock) return;
    const d = Math.min(dt, 0.1);
    quality.slow = d > 0.024 ? quality.slow + d : Math.max(0, quality.slow - d * 0.5);
    if (quality.slow > 2) quality.refl = false;
  }
  function drawHallGeometry(mirror) {
    setCommon(progMesh, mirror);
    archCyl.draw(); archBox.draw(); windows.draw();
  }
  function drawFloor(reflOn) {
    setCommon(progFloor, 1);
    gl.uniform4fv(progFloor.u.uBlob, BLOBS);
    gl.uniform1f(progFloor.u.uRefl, reflOn ? 1 : 0);
    gl.uniform2f(progFloor.u.uRes, W, H);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, rtRefl.tex);
    gl.uniform1i(progFloor.u.uReflTex, 0);
    gl.uniform4f(progFloor.u.uRing, 0, 0, 0, 0);
    gl.bindVertexArray(floorVao);
    gl.disable(gl.CULL_FACE);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.enable(gl.CULL_FACE);
  }
  function drawLight() {
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    if (PERF.shafts) {
      setCommon(progBeam, 1);
      gl.uniform1f(progBeam.u.uBeam, 0.028);
      gl.bindVertexArray(beamVao);
      gl.drawArrays(gl.TRIANGLES, 0, beamCount);
    }
    setCommon(progDust, 1);
    gl.bindVertexArray(dustVao);
    gl.drawArrays(gl.POINTS, 0, DUST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);
  }
  // a portal disc (mode 0: stencil mask) or its spark ring (mode 1)
  function drawPortalQuad(cx, cy, cz, yaw, mode, inner, glow) {
    gl.useProgram(progPortal.p);
    const u = progPortal.u;
    gl.uniformMatrix4fv(u.uVP, false, cam.vp);
    gl.uniform3f(u.uC, cx, cy, cz);
    gl.uniform3f(u.uR, Math.cos(yaw), 0, -Math.sin(yaw));
    gl.uniform3f(u.uU, 0, 1, 0);
    gl.uniform1f(u.uRad, mode ? PR * 1.25 : PR);
    gl.uniform1f(u.uMode, mode);
    gl.uniform1f(u.uInner, mode ? 0.8 : 1.0);
    gl.uniform1f(u.uTime, time);
    gl.uniform1f(u.uGlow, glow);
    gl.bindVertexArray(emptyVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
  // screen box of a portal circle, into sc[] (GL coords); false if off screen
  const sc = new Int32Array(4);
  const P4 = new Float32Array(4);
  function portalBox(c, cx, cy, cz, yaw) {
    const rx = Math.cos(yaw), rz = -Math.sin(yaw);
    let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
    for (let k = 0; k < 8; k++) {
      const a = (k / 8) * PI * 2, ca = Math.cos(a) * PR * 1.09, sa = Math.sin(a) * PR * 1.09;
      const px = cx + rx * ca, py = cy + sa, pz = cz + rz * ca;
      const m = c.vp;
      P4[0] = m[0] * px + m[4] * py + m[8] * pz + m[12];
      P4[1] = m[1] * px + m[5] * py + m[9] * pz + m[13];
      P4[3] = m[3] * px + m[7] * py + m[11] * pz + m[15];
      if (P4[3] < 0.05) { x0 = 0; y0 = 0; x1 = W; y1 = H; break; }
      const sx = (P4[0] / P4[3] * 0.5 + 0.5) * W, sy = (P4[1] / P4[3] * 0.5 + 0.5) * H;
      if (sx < x0) x0 = sx; if (sx > x1) x1 = sx; if (sy < y0) y0 = sy; if (sy > y1) y1 = sy;
    }
    x0 = Math.max(0, Math.floor(x0)); y0 = Math.max(0, Math.floor(y0));
    x1 = Math.min(W, Math.ceil(x1)); y1 = Math.min(H, Math.ceil(y1));
    if (clip.on) { x0 = Math.max(x0, clip.x); y0 = Math.max(y0, clip.y); x1 = Math.min(x1, clip.x + clip.w); y1 = Math.min(y1, clip.y + clip.h); }
    if (x1 <= x0 || y1 <= y0) return false;
    sc[0] = x0; sc[1] = y0; sc[2] = x1 - x0; sc[3] = y1 - y0;
    return true;
  }
  // set camX to the world-side view through portal p
  function worldCam(p, w) {
    camX.yaw = hallToWorld(p, w, camH.eye, camH.yaw, camX.eye);
    camX.pitch = camH.pitch; camX.fov = camH.fov; camX.aspect = camH.aspect; camX.near = 0.05; camX.far = 400;
    camX.time = time; camX.px = pxScale;
    updateCamera(camX);
  }
  function hallCam(p, w) {
    camX.yaw = worldToHall(p, w, camH.eye, camH.yaw, camX.eye);
    camX.pitch = camH.pitch; camX.fov = camH.fov; camX.aspect = camH.aspect; camX.near = 0.05; camX.far = 120;
    camX.time = time; camX.px = pxScale;
    updateCamera(camX);
  }

  // ---------- render targets ----------
  const progBright = program(gl, S.POST_VS, S.BRIGHT_FS);
  const progBlur = program(gl, S.POST_VS, S.BLUR_FS);
  const progFinal = program(gl, S.POST_VS, S.FINAL_FS);
  function target(w, h, depth) {
    const t = { w, h, tex: gl.createTexture(), fb: gl.createFramebuffer(), rb: null };
    gl.bindTexture(gl.TEXTURE_2D, t.tex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, w, h);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, t.fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t.tex, 0);
    if (depth) {
      t.rb = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, t.rb);
      gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH24_STENCIL8, w, h);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_STENCIL_ATTACHMENT, gl.RENDERBUFFER, t.rb);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return t;
  }
  function freeTarget(t) { if (!t) return; gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fb); if (t.rb) gl.deleteRenderbuffer(t.rb); }
  let rtScene = null, rtRefl = null, rtB0 = null, rtB1 = null;
  function buildTargets() {
    freeTarget(rtScene); freeTarget(rtRefl); freeTarget(rtB0); freeTarget(rtB1);
    rtScene = target(W, H, true);
    rtRefl = target(Math.max(1, W >> 1), Math.max(1, H >> 1), true);
    rtB0 = target(Math.max(1, W >> 2), Math.max(1, H >> 2), false);
    rtB1 = target(Math.max(1, W >> 2), Math.max(1, H >> 2), false);
  }
  function bindTarget(t) { gl.bindFramebuffer(gl.FRAMEBUFFER, t ? t.fb : null); gl.viewport(0, 0, t ? t.w : W, t ? t.h : H); }
  function post(p, src, dst) {
    bindTarget(dst);
    gl.useProgram(p.p);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, src.tex);
    gl.uniform1i(p.u.uTex, 0);
    gl.bindVertexArray(emptyVao);
  }
  const clip = { on: false, x: 0, y: 0, w: 0, h: 0 };
  function scissorOn() {
    if (!clip.on) { gl.disable(gl.SCISSOR_TEST); return; }
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(clip.x, clip.y, clip.w, clip.h);
  }

  // the hall as seen by `cam` (no post); mirror only when asked
  function drawHall(reflOn) {
    drawHallGeometry(1);
    drawFloor(reflOn);
    drawLight();
  }

  // worlds through the hall's portals: stencil mask, then the world scissored to the box
  function drawPortals(cheap) {
    if (!PERF.worlds) return;
    for (let i = 0; i < 4; i++) {
      const p = PORTALS[i];
      if (!sees(camH, p.x, PY, p.z, PR * 1.3)) { p.vis = false; continue; }
      // only the side facing the viewer
      crossing(camH.eye[0], camH.eye[2], p.x, p.z, p.yaw, cr);
      p.vis = cr[0] < 0;
      if (!p.vis) continue;
      gl.enable(gl.STENCIL_TEST);
      gl.stencilFunc(gl.ALWAYS, i + 1, 0xff);
      gl.stencilOp(gl.KEEP, gl.KEEP, gl.REPLACE);
      gl.colorMask(false, false, false, false);
      gl.depthMask(false);
      gl.disable(gl.CULL_FACE);
      drawPortalQuad(p.x, PY, p.z, p.yaw, 0, 1, 1);
      gl.colorMask(true, true, true, true);
      gl.depthMask(true);
      const w = WORLDS[p.world];
      if (w && portalBox(camH, p.x, PY, p.z, p.yaw)) {
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(sc[0], sc[1], sc[2], sc[3]);
        gl.clear(gl.DEPTH_BUFFER_BIT);
        gl.stencilFunc(gl.EQUAL, i + 1, 0xff);
        gl.stencilOp(gl.KEEP, gl.KEEP, gl.KEEP);
        worldCam(p, w);
        cam = camX;
        w.draw(camX, cheap);
        cam = camH;
        scissorOn();
      }
      gl.disable(gl.STENCIL_TEST);
      gl.enable(gl.CULL_FACE);
    }
    // the rings on top
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    for (let i = 0; i < 4; i++) {
      const p = PORTALS[i];
      if (!p.vis) continue;
      drawPortalQuad(p.x, PY, p.z, p.yaw, 1, 0, i === st.near ? 1.25 : 0.9);
    }
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);
  }

  // inside a world: the world, plus the hall through the return portal behind you
  function drawWorldView(wi) {
    const w = WORLDS[wi], p = PORTALS.find((q) => q.world === wi);
    cam = camH;
    w.draw(camH, false);
    // return portal: in the world, at the spawn, facing back toward the hall
    const ry = w.spawnYaw + PI, rx = w.spawn[0], rz = w.spawn[2], ryy = w.spawn[1] + (PY - 1.7);
    crossing(camH.eye[0], camH.eye[2], rx, rz, ry, cr);
    if (cr[0] < 0 && sees(camH, rx, ryy, rz, PR * 1.3)) {
      gl.enable(gl.STENCIL_TEST);
      gl.stencilFunc(gl.ALWAYS, 9, 0xff);
      gl.stencilOp(gl.KEEP, gl.KEEP, gl.REPLACE);
      gl.colorMask(false, false, false, false);
      gl.depthMask(false);
      gl.disable(gl.CULL_FACE);
      drawPortalQuad(rx, ryy, rz, ry, 0, 1, 1);
      gl.colorMask(true, true, true, true);
      gl.depthMask(true);
      if (portalBox(camH, rx, ryy, rz, ry)) {
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(sc[0], sc[1], sc[2], sc[3]);
        gl.clear(gl.DEPTH_BUFFER_BIT);
        gl.stencilFunc(gl.EQUAL, 9, 0xff);
        gl.stencilOp(gl.KEEP, gl.KEEP, gl.KEEP);
        hallCam(p, w);
        cam = camX;
        drawHall(false);
        cam = camH;
        gl.disable(gl.SCISSOR_TEST);
      }
      gl.disable(gl.STENCIL_TEST);
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE); gl.depthMask(false);
      drawPortalQuad(rx, ryy, rz, ry, 1, 0, 1);
      gl.depthMask(true); gl.disable(gl.BLEND);
      gl.enable(gl.CULL_FACE);
    }
  }

  // cheap = preview through the intro's portal: no mirror pass, no bloom
  function render(cheap = false) {
    cam = camH;
    updateCamera(camH);
    updatePools();
    gl.disable(gl.SCISSOR_TEST);
    gl.disable(gl.STENCIL_TEST);
    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);
    gl.clearColor(0.55, 0.5, 0.45, 1);
    gl.clearStencil(0);

    let reflOn = false;
    if (st.mode < 0) {
      const hy = -Math.tan(camH.pitch) / camH.tanH;
      const frac = Math.min(1, Math.max(0, hy * 0.5 + 0.5 + 0.04));
      reflOn = !cheap && PERF.mirror && quality.refl && frac > 0;
      if (reflOn) {
        bindTarget(rtRefl);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(0, 0, rtRefl.w, Math.ceil(frac * rtRefl.h));
        gl.frontFace(gl.CW);
        drawHallGeometry(-1);
        gl.frontFace(gl.CCW);
        gl.disable(gl.SCISSOR_TEST);
      }
    }

    bindTarget(rtScene);
    scissorOn();
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT | gl.STENCIL_BUFFER_BIT);
    if (st.mode < 0) {
      drawHall(reflOn);
      drawPortals(cheap);
    } else {
      drawWorldView(st.mode);
    }
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.SCISSOR_TEST);

    const bloomOn = !cheap && PERF.bloom;
    if (bloomOn) {
      post(progBright, rtScene, rtB0);
      gl.uniform2f(progBright.u.uTexel, 1 / W, 1 / H);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      post(progBlur, rtB0, rtB1);
      gl.uniform2f(progBlur.u.uDir, 1 / rtB0.w, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      post(progBlur, rtB1, rtB0);
      gl.uniform2f(progBlur.u.uDir, 0, 1 / rtB0.h);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
    post(progFinal, rtScene, null);
    scissorOn();
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, rtB0.tex);
    gl.uniform1i(progFinal.u.uBloom, 1);
    gl.uniform2f(progFinal.u.uTexel, 1 / W, 1 / H);
    gl.uniform1f(progFinal.u.uBloomK, bloomOn ? (st.mode < 0 ? 0.6 : 0.9) : 0);
    gl.uniform1f(progFinal.u.uFxaa, PERF.fxaa ? 1 : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.disable(gl.SCISSOR_TEST);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindVertexArray(null);
    gl.enable(gl.DEPTH_TEST);
  }

  // worlds tick: the one you're in (or nearest to) every frame, other visible ones every
  // second frame, hidden ones not at all
  function tickWorlds(t, dt) {
    if (!st.ready) return;
    st.frame++;
    if (st.mode >= 0) { WORLDS[st.mode].update(t, dt, ctl.pos, true); return; }
    for (let i = 0; i < 4; i++) {
      const p = PORTALS[i], w = WORLDS[p.world];
      if (!w || !sees(camH, p.x, PY, p.z, PR * 1.3)) continue;
      if (i === st.near || (st.frame & 1) === (i & 1)) {
        worldCam(p, w);
        w.update(t, i === st.near ? dt : dt * 2, camX.eye, false);
      }
    }
  }

  // walking through a portal (either way)
  function portalCrossing() {
    if (st.mode < 0) {
      let near = -1, nd = 6;
      for (let i = 0; i < 4; i++) {
        const p = PORTALS[i];
        crossing(ctl.pos[0], ctl.pos[2], p.x, p.z, p.yaw, cr);
        const d = Math.hypot(ctl.pos[0] - p.x, ctl.pos[2] - p.z);
        if (d < nd) { nd = d; near = i; }
        if (Math.abs(cr[1]) < PR * 0.9 && cr[0] >= 0 && cr[0] < 0.6 && p.prevS !== undefined && p.prevS < 0 && WORLDS[p.world]) {
          enterWorld(i);
          return;
        }
        p.prevS = cr[0];
      }
      if (near !== st.near) {
        st.near = near;
        const w = near >= 0 ? WORLDS[PORTALS[near].world] : null;
        if (w) ctl.setStation(w.name, 'Walk through the portal', w.feature);
        else if (near >= 0) ctl.setStation('Coming soon', 'This world is still being built', '');
        else ctl.setStation('', '', '');
      }
    } else {
      const w = WORLDS[st.mode];
      crossing(ctl.pos[0], ctl.pos[2], w.spawn[0], w.spawn[2], w.spawnYaw + PI, cr);
      const dy = ctl.pos[1] - w.spawn[1];
      if (Math.abs(cr[1]) < PR * 0.9 && Math.abs(dy) < PR && cr[0] >= 0 && cr[0] < 0.6 && prevWS < 0) { leaveWorld(); return; }
      prevWS = cr[0];
    }
  }
  function enterWorld(i) {
    const p = PORTALS[i], w = WORLDS[p.world];
    ctl.yaw = hallToWorld(p, w, ctl.pos, ctl.yaw, ctl.pos);
    st.mode = p.world;
    ctl.fly = !!w.fly; ctl.flyRadius = w.radius || 30;
    crossing(ctl.pos[0], ctl.pos[2], w.spawn[0], w.spawn[2], w.spawnYaw + PI, cr);
    prevWS = cr[0];
    ctl.setStation(w.name, w.hint, w.feature);
  }
  function leaveWorld() {
    const w = WORLDS[st.mode], p = PORTALS.find((q) => q.world === st.mode);
    ctl.yaw = worldToHall(p, w, ctl.pos, ctl.yaw, ctl.pos);
    ctl.pos[1] = 1.7;
    st.mode = -1;
    ctl.fly = false;
    crossing(ctl.pos[0], ctl.pos[2], p.x, p.z, p.yaw, cr);
    p.prevS = cr[0];
    st.near = -2;
  }

  function prewarm() {
    // draw once with every portal in view so all programs and buffers are touched
    const ex = camH.eye[0], ey = camH.eye[1], ez = camH.eye[2], yw = camH.yaw, pt = camH.pitch;
    camH.eye[0] = 0; camH.eye[1] = 1.7; camH.eye[2] = 9; camH.yaw = 0; camH.pitch = 0;
    render();
    camH.eye[0] = ex; camH.eye[1] = ey; camH.eye[2] = ez; camH.yaw = yw; camH.pitch = pt;
  }

  // ---------- the contract ----------
  const api = {
    _ctl: ctl,
    _quality: quality,
    _st: st,
    _enterWorld: (i) => enterWorld(i),
    resize(cssW, cssH, dpr) {
      const d = Math.min(dpr || 1, 2);
      const nw = Math.max(1, Math.round(cssW * d)), nh = Math.max(1, Math.round(cssH * d));
      const changed = nw !== W || nh !== H || !rtScene;
      W = nw; H = nh;
      if (!shared && (canvas.width !== W || canvas.height !== H)) { canvas.width = W; canvas.height = H; }
      camH.aspect = W / H;
      baseFov = cssW < cssH ? 1.35 : 1.12;
      camH.fov = baseFov;
      pxScale = (H * 0.5) / Math.tan(camH.fov / 2);
      if (changed) { buildTargets(); prewarm(); }
    },
    // scissor (optional): the intro portal's bounding box in device px, top-left origin, as
    // { x, y, w, h }, [x, y, w, h] or a circle { x, y, r }. Only that region is drawn.
    preview(t, dt, k, scissor) {
      time = t;
      if (scissor) {
        let x, y, w, h;
        if (Array.isArray(scissor)) { x = scissor[0]; y = scissor[1]; w = scissor[2]; h = scissor[3]; }
        else if (scissor.r !== undefined) { x = scissor.x - scissor.r; y = scissor.y - scissor.r; w = h = scissor.r * 2; }
        else { x = scissor.x; y = scissor.y; w = scissor.w; h = scissor.h; }
        const x0 = Math.max(0, Math.floor(x)), y0 = Math.max(0, Math.floor(y));
        const x1 = Math.min(W, Math.ceil(x + w)), y1 = Math.min(H, Math.ceil(y + h));
        if (x1 <= x0 || y1 <= y0) return;
        clip.on = true; clip.x = x0; clip.y = H - y1; clip.w = x1 - x0; clip.h = y1 - y0;
      } else clip.on = false;
      const e = k * k * (3 - 2 * k);
      camH.eye[0] = Math.sin(t * 0.21) * 0.25 * (1 - e);
      camH.eye[1] = 2.3 - 0.6 * e + Math.sin(t * 0.5) * 0.04 * (1 - e);
      camH.eye[2] = 15.4 - 3.4 * e;
      camH.yaw = Math.sin(t * 0.17) * 0.05 * (1 - e);
      camH.pitch = 0.02 * (1 - e);
      camH.fov = baseFov;
      updateCamera(camH);
      tickWorlds(t, Math.min(dt, 0.05));
      render(true);
      clip.on = false;
    },
    enter() {
      if (st.active) return;
      st.active = true;
      st.mode = -1; ctl.fly = false; st.near = -2;
      for (const p of PORTALS) p.prevS = undefined;
      ctl.enable();
    },
    exit() {
      if (!st.active) return;
      st.active = false;
      st.mode = -1; ctl.fly = false;
      ctl.disable();
      ctl.setStation('', '', '');
    },
    frame(t, dt) {
      time = t;
      watch(dt);
      ctl.update(t, Math.min(dt, 0.1));
      dt = Math.min(dt, 0.05);
      portalCrossing();
      camH.eye[0] = ctl.pos[0]; camH.eye[1] = ctl.pos[1]; camH.eye[2] = ctl.pos[2];
      camH.yaw = ctl.yaw; camH.pitch = ctl.pitch;
      camH.fov = baseFov; camH.near = st.mode < 0 ? 0.08 : 0.05; camH.far = st.mode < 0 ? 120 : 400;
      camH.time = t; camH.px = pxScale;
      updateCamera(camH);
      tickWorlds(t, dt);
      render();
    },
  };
  return api;
}
