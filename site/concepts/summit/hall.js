// hall.js — the hall of demos. A bright, walkable gallery in hand-written WebGL2.
// createHall(canvas, overlayEl) → { resize, preview, enter, exit, frame }  (see CONTRACT.md)
//
// Every moving thing is driven by anikit. The five set pieces live in hall-demos.js:
// world() ropes, a spring() swarm, a scrubbed timeline(), 200k spring-parted points and
// batch() + stagger pin waves. Movement, HUD fades and station wake-ups are anikit too.

import { ready, animate } from '../../../packages/anikit/dist/anikit.lean.js';
import { createDemos, STATIONS as ST } from './hall-demos.js';
import { program, cubeMesh, cylinderMesh, sphereMesh, group, camera, updateCamera, sees } from './hall-gl.js';
import * as S from './hall-shaders.js';
import { createControls } from './hall-controls.js';

const SUN = [0.72, -0.62, -0.3];
{ const l = Math.hypot(SUN[0], SUN[1], SUN[2]); SUN[0] /= l; SUN[1] /= l; SUN[2] /= l; }
const FOG = [0.55, 0.49, 0.42];
const SPAWN = [0, 1.7, 12];
const PI2 = Math.PI * 2;
// hall extents
const HX = 8, HZ0 = -28, HZ1 = 16, HY = 12;

export function createHall(canvas, overlayEl) {
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: true, powerPreference: 'high-performance', preserveDrawingBuffer: false });
  if (!gl) throw new Error('hall: WebGL2 unavailable');

  const progMesh = program(gl, S.MESH_VS, S.MESH_FS);
  const progFloor = program(gl, S.FLOOR_VS, S.FLOOR_FS);
  const progBeam = program(gl, S.BEAM_VS, S.BEAM_FS);
  const progDust = program(gl, S.DUST_VS, S.DUST_FS);

  const cube = cubeMesh(), cyl = cylinderMesh(22), sph = sphereMesh(14, 9), sphLo = sphereMesh(10, 6);
  const cam = camera();
  let W = 1, H = 1, pxScale = 1;

  // ---------- static architecture ----------
  const archBox = group(gl, cube, 256);
  const archCyl = group(gl, cyl, 128);
  const windows = group(gl, cube, 32);
  const colliders = []; // [x0, z0, x1, z1]
  const box = (g, x, y, z, sx, sy, sz, gloss, r, gg, b, e = 0, collide = false) => {
    g.push(x, y, z, 0, sx, sy, sz, gloss, r, gg, b, e);
    if (collide) colliders.push(x - sx / 2, z - sz / 2, x + sx / 2, z + sz / 2);
  };
  // material id rides in the gloss slot: mat * 2 + gloss (see hall-shaders.js)
  const PLAIN = 0, PLASTER = 2, FLUTE = 4, METAL = 6, COFFER = 8, WALNUT = 10, LACQUER = 12;
  const WALL = [0.8, 0.74, 0.66], MARBLE = [0.92, 0.9, 0.86], BRONZE = [0.78, 0.55, 0.32], STEEL = [0.2, 0.2, 0.22];
  const WOOD = [0.42, 0.29, 0.2], BEAM = [0.66, 0.61, 0.55], DEEP = [0.36, 0.38, 0.44];
  // walls (thick boxes), ceiling
  box(archBox, -HX - 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, PLASTER + 0.05, ...WALL);
  box(archBox, HX + 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ0 - 0.5, 2 * HX + 2, HY, 1, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ1 + 0.5, 2 * HX + 2, HY, 1, PLASTER + 0.05, ...WALL);
  box(archBox, 0, HY + 0.5, (HZ0 + HZ1) / 2, 2 * HX + 2, 1, HZ1 - HZ0 + 2, COFFER, ...DEEP, 0.12);
  // cornice + string course
  for (const s of [-1, 1]) {
    box(archBox, s * (HX - 0.15), 11.6, (HZ0 + HZ1) / 2, 0.3, 0.3, HZ1 - HZ0, METAL + 0.5, ...BRONZE);
    box(archBox, s * (HX - 0.08), 5.9, (HZ0 + HZ1) / 2, 0.16, 0.2, HZ1 - HZ0, PLAIN + 0.2, ...MARBLE);
  }
  // deep coffered ceiling
  for (let z = HZ0 + 2.5; z < HZ1; z += 5) box(archBox, 0, HY - 0.45, z, 2 * HX, 0.9, 0.5, COFFER + 0.1, ...BEAM);
  for (const x of [-4, 0, 4]) box(archBox, x, HY - 0.35, (HZ0 + HZ1) / 2, 0.4, 0.7, HZ1 - HZ0, COFFER + 0.1, ...BEAM);
  // fluted marble columns on bronze bases between the window bays
  for (let z = 11; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      const x = s * 7.0;
      archCyl.push(x, 5.6, z, 0, 0.82, 10.4, 0.82, FLUTE + 0.35, ...MARBLE, 0);
      box(archBox, x, 0.3, z, 1.2, 0.6, 1.2, WALNUT + 0.4, ...WOOD, 0, true);
      box(archBox, x, 10.95, z, 1.15, 0.5, 1.15, METAL + 0.6, ...BRONZE, 0);
      archCyl.push(x, 0.7, z, 0, 1.0, 0.22, 1.0, METAL + 0.6, ...BRONZE, 0);
    }
  }
  // windows: both walls (emis > 2 flags the window shader), steel sills
  for (let z = 8.5; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      windows.push(s * (HX - 0.02), 8.6, z, 0, 0.06, 4.2, 2.2, 0, 1, 1, 1, 3);
      box(archBox, s * (HX - 0.05), 6.35, z, 0.25, 0.25, 2.6, METAL + 0.4, ...STEEL);
    }
  }
  // door behind spawn (bright: where you came from)
  windows.push(0, 3.4, HZ1 - 0.02, 0, 3.6, 6.4, 0.06, 0, 1, 1, 1, 3);
  box(archBox, 0, 6.8, HZ1 - 0.1, 4.4, 0.5, 0.3, METAL + 0.5, ...BRONZE);
  box(archBox, -2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, METAL + 0.5, ...BRONZE);
  box(archBox, 2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, METAL + 0.5, ...BRONZE);

  // the set pieces (they add their own furniture before the static upload)
  const demos = createDemos({ gl, cam, sees, group, cube, sph, sphLo, cyl, box, archBox, archCyl, colliders, setCommon });
  const BLOBS = demos.blobs;
  const wake = demos.wake;

  archBox.upload(); archCyl.upload(); windows.upload();

  // floor quad
  const floorVao = gl.createVertexArray();
  gl.bindVertexArray(floorVao);
  const fb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, fb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-HX, HZ0, HX, HZ0, HX, HZ1, -HX, HZ0, HX, HZ1, -HX, HZ1]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  // light shafts: one slanted slab per window (two crossing quads)
  const beamV = [];
  for (let z = 8.5; z > HZ0; z -= 5) {
    const t = (HX * 2 - 1.5) / SUN[0]; // run until near the far wall
    const tFloor = (8.6) / -SUN[1];
    const L = Math.min(t, tFloor + 1.5);
    for (const [ya, yb, za, zb] of [[6.6, 10.6, z, z], [8.6, 8.6, z - 1.1, z + 1.1]]) {
      const p0 = [-HX + 0.05, ya, za], p1 = [-HX + 0.05, yb, zb];
      if (ya === yb) { p0[2] = za; p1[2] = zb; }
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

  // dust motes along the shafts
  const DUST = 900;
  const dustData = new Float32Array(DUST * 4);
  for (let i = 0; i < DUST; i++) {
    const bay = 8.5 - 5 * Math.floor(Math.random() * 9);
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

  const st = { ready: false, active: false, near: -1 };
  ready.then(() => {
    demos.start(); st.ready = true;
    // warm each exhibit once (JIT, first uploads, first draws), one per idle slot so the
    // warm-up itself never makes a long task
    const views = [[4.2, 9.5, 0], [-2.6, -3.2, 1.45], [2.4, -3.2, -1.57], [0, -9.5, 0], [0, -18, 0]];
    let k = 0;
    const idle = globalThis.requestIdleCallback || ((f) => setTimeout(f, 16));
    const step = () => {
      if (k >= views.length || st.active) return;
      const [x, z, yaw] = views[k++];
      const ex = cam.eye[0], ey = cam.eye[1], ez = cam.eye[2], yw = cam.yaw, pt = cam.pitch;
      cam.eye[0] = x; cam.eye[1] = 1.7; cam.eye[2] = z; cam.yaw = yaw; cam.pitch = 0.05;
      updateCamera(cam);
      for (let i = 0; i < 3; i++) demos.update(time, 1 / 60, x, z, true);
      if (rtScene) render();
      demos.update(time, 1 / 60, 0, 99, false);
      cam.eye[0] = ex; cam.eye[1] = ey; cam.eye[2] = ez; cam.yaw = yw; cam.pitch = pt;
      idle(step);
    };
    idle(step);
  });

  // ---------- controls & HUD ----------
  const ctl = createControls(canvas, overlayEl, {
    spawn: SPAWN, colliders, bounds: [-HX + 0.45, HZ0 + 0.45, HX - 0.45, HZ1 - 0.45],
    onUse: () => use(),
  });

  function use() {
    if (!st.ready || st.near < 0) return;
    demos.use(st.near, time, ctl.pos[0], ctl.pos[2]);
  }

  function wakeStation(i, on) {
    animate(wake[i], { v: on ? 1 : 0 }, { duration: on ? 0.6 : 0.9, ease: 'outCubic' });
    demos.onWake(i, on);
  }

  function updateNear(px, pz) {
    let best = -1, bd = 1e9;
    for (let i = 0; i < ST.length; i++) {
      const d = Math.hypot(px - ST[i].x, pz - ST[i].z);
      if (d < ST[i].r && d < bd) { bd = d; best = i; }
    }
    if (best !== st.near) {
      if (st.near >= 0) wakeStation(st.near, false);
      if (best >= 0) wakeStation(best, true);
      st.near = best;
      if (best >= 0) ctl.setStation(ST[best].name, ST[best].hint, ST[best].feature);
      else ctl.setStation('', '', '');
    }
  }

  // ---------- rendering ----------
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
    if (u.uGlassL) gl.uniform4fv(u.uGlassL, demos.glassLight);
  }

  let time = 0, baseFov = 1.12;
  // a pool of light over each station: x, z, radius, intensity (intensity follows wake)
  const POOLS = new Float32Array(24);
  function updatePools() {
    for (let i = 0; i < ST.length; i++) {
      POOLS[i * 4] = ST[i].x; POOLS[i * 4 + 1] = ST[i].z;
      POOLS[i * 4 + 2] = ST[i].spot || 2.4;
      POOLS[i * 4 + 3] = ST[i].pool * (0.7 + 0.5 * wake[i].v);
    }
  }
  // safety net for weak GPUs: if frames stay slow, drop the mirror pass (not the resolution)
  const quality = { refl: true, slow: 0, lock: false };
  function watch(dt) {
    if (!quality.refl || quality.lock) return;
    const d = Math.min(dt, 0.1); // one long hitch (tab switch) is not a slow GPU
    quality.slow = d > 0.024 ? quality.slow + d : Math.max(0, quality.slow - d * 0.5);
    if (quality.slow > 2) quality.refl = false;
  }
  function drawScene(mirror) {
    setCommon(progMesh, mirror);
    demos.drawMeshes();
    archCyl.draw(); archBox.draw(); windows.draw();
    demos.drawPoints(mirror);
    // glass: blended over everything opaque, no depth writes
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    setCommon(progMesh, mirror);
    demos.drawGlass();
    gl.enable(gl.CULL_FACE);
    // glowing things: additive, no depth writes
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.depthMask(false);
    demos.drawGlow(mirror);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
  }

  // ---------- render targets (rebuilt on resize only) ----------
  const progBright = program(gl, S.POST_VS, S.BRIGHT_FS);
  const progBlur = program(gl, S.POST_VS, S.BLUR_FS);
  const progFinal = program(gl, S.POST_VS, S.FINAL_FS);
  const postVao = gl.createVertexArray();
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
      gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH_COMPONENT24, w, h);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, t.rb);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return t;
  }
  function freeTarget(t) { if (!t) return; gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fb); if (t.rb) gl.deleteRenderbuffer(t.rb); }
  let rtScene = null, rtRefl = null, rtB0 = null, rtB1 = null;
  function buildTargets() {
    freeTarget(rtScene); freeTarget(rtRefl); freeTarget(rtB0); freeTarget(rtB1);
    rtScene = target(W, H, true);
    rtRefl = target(Math.max(1, W >> 1), Math.max(1, H >> 1), true);   // mirror pass at half resolution
    rtB0 = target(Math.max(1, W >> 2), Math.max(1, H >> 2), false);      // bloom at quarter resolution
    rtB1 = target(Math.max(1, W >> 2), Math.max(1, H >> 2), false);
  }
  function bindTarget(t) { gl.bindFramebuffer(gl.FRAMEBUFFER, t ? t.fb : null); gl.viewport(0, 0, t ? t.w : W, t ? t.h : H); }
  function post(p, src, dst) {
    bindTarget(dst);
    gl.useProgram(p.p);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, src.tex);
    gl.uniform1i(p.u.uTex, 0);
    gl.bindVertexArray(postVao);
  }

  function render() {
    updateCamera(cam);
    updatePools();
    gl.disable(gl.SCISSOR_TEST);
    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);
    gl.clearColor(0.55, 0.5, 0.45, 1);

    // 1) mirrored scene into the half-res target, only below the horizon line
    const hy = -Math.tan(cam.pitch) / cam.tanH; // horizon in NDC
    const frac = Math.min(1, Math.max(0, hy * 0.5 + 0.5 + 0.04));
    const reflOn = quality.refl && frac > 0;
    if (reflOn) {
      bindTarget(rtRefl);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(0, 0, rtRefl.w, Math.ceil(frac * rtRefl.h));
      gl.frontFace(gl.CW);
      drawScene(-1);
      gl.frontFace(gl.CCW);
      gl.disable(gl.SCISSOR_TEST);
    }

    // 2) the scene at full resolution
    bindTarget(rtScene);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    // near, small things first so the big architecture fails the depth test early
    drawScene(1);
    if (frac > 0) {
      setCommon(progFloor, 1);
      gl.uniform4fv(progFloor.u.uBlob, BLOBS);
      gl.uniform1f(progFloor.u.uRefl, reflOn ? 1 : 0);
      gl.uniform2f(progFloor.u.uRes, W, H);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, rtRefl.tex);
      gl.uniform1i(progFloor.u.uReflTex, 0);
      let ri = 0;
      for (let i = 1; i < ST.length; i++) if (wake[i].v > wake[ri].v) ri = i;
      gl.uniform4f(progFloor.u.uRing, ST[ri].x, ST[ri].z, ST[ri].ring, ST[ri].ring > 0 ? wake[ri].v : 0);
      gl.bindVertexArray(floorVao);
      gl.disable(gl.CULL_FACE);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
      gl.enable(gl.CULL_FACE);
    }
    // light: subtle shafts and dust, additive
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    setCommon(progBeam, 1);
    gl.uniform1f(progBeam.u.uBeam, 0.028);
    gl.bindVertexArray(beamVao);
    gl.drawArrays(gl.TRIANGLES, 0, beamCount);
    setCommon(progDust, 1);
    gl.bindVertexArray(dustVao);
    gl.drawArrays(gl.POINTS, 0, DUST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.disable(gl.DEPTH_TEST);

    // 3) bloom: bright pass to quarter res, separable blur
    post(progBright, rtScene, rtB0);
    gl.uniform2f(progBright.u.uTexel, 1 / W, 1 / H);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    post(progBlur, rtB0, rtB1);
    gl.uniform2f(progBlur.u.uDir, 1 / rtB0.w, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    post(progBlur, rtB1, rtB0);
    gl.uniform2f(progBlur.u.uDir, 0, 1 / rtB0.h);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    // 4) composite at native resolution: FXAA, bloom, grade, vignette
    post(progFinal, rtScene, null);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, rtB0.tex);
    gl.uniform1i(progFinal.u.uBloom, 1);
    gl.uniform2f(progFinal.u.uTexel, 1 / W, 1 / H);
    gl.uniform1f(progFinal.u.uBloomK, 0.6);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindVertexArray(null);
    gl.enable(gl.DEPTH_TEST);
  }

  // Compile/link every program and touch every buffer once, so nothing stalls later.
  function prewarm() {
    const ex = cam.eye[0], ey = cam.eye[1], ez = cam.eye[2], yw = cam.yaw, pt = cam.pitch;
    cam.eye[0] = 0; cam.eye[1] = 1.7; cam.eye[2] = 12; cam.yaw = 0; cam.pitch = 0;
    demos.setVisible(true);
    render();
    cam.eye[0] = ex; cam.eye[1] = ey; cam.eye[2] = ez; cam.yaw = yw; cam.pitch = pt;
  }

  // ---------- the contract ----------
  const api = {
    _ctl: ctl,
    _quality: quality,
    _demos: demos,
    resize(cssW, cssH, dpr) {
      // native resolution (DPR capped at 2): the budget is earned by technique, not pixels
      const d = Math.min(dpr || 1, 2);
      const nw = Math.max(1, Math.round(cssW * d)), nh = Math.max(1, Math.round(cssH * d));
      const changed = nw !== W || nh !== H || !rtScene;
      W = nw; H = nh;
      if (canvas.width !== W || canvas.height !== H) { canvas.width = W; canvas.height = H; }
      cam.aspect = W / H;
      baseFov = cssW < cssH ? 1.35 : 1.12;
      cam.fov = baseFov;
      pxScale = (H * 0.5) / Math.tan(cam.fov / 2);
      if (changed) { buildTargets(); prewarm(); }
    },
    preview(t, dt, k) {
      time = t;
      const e = k * k * (3 - 2 * k);
      cam.eye[0] = Math.sin(t * 0.21) * 0.25 * (1 - e);
      cam.eye[1] = 2.3 - 0.6 * e + Math.sin(t * 0.5) * 0.04 * (1 - e);
      cam.eye[2] = 15.4 - 3.4 * e;
      cam.yaw = Math.sin(t * 0.17) * 0.05 * (1 - e);
      cam.pitch = 0.02 * (1 - e);
      cam.fov = baseFov;
        updateCamera(cam);
      demos.update(t, Math.min(dt, 0.05), cam.eye[0], cam.eye[2], false);
      render();
    },
    enter() {
      if (st.active) return;
      st.active = true;
      ctl.enable();
    },
    exit() {
      if (!st.active) return;
      st.active = false;
      ctl.disable();
      if (st.near >= 0) { wakeStation(st.near, false); st.near = -1; }
      ctl.setStation('', '', '');
    },
    frame(t, dt) {
      time = t;
      watch(dt);
      // movement is frame-rate independent down to 10 fps; longer gaps (tab switch) are clamped
      ctl.update(t, Math.min(dt, 0.1));
      dt = Math.min(dt, 0.05);
      cam.eye[0] = ctl.pos[0]; cam.eye[1] = ctl.pos[1]; cam.eye[2] = ctl.pos[2];
      cam.yaw = ctl.yaw; cam.pitch = ctl.pitch;
      cam.fov = baseFov;
      updateCamera(cam);
      if (st.ready) {
        updateNear(ctl.pos[0], ctl.pos[2]);
        demos.update(t, dt, ctl.pos[0], ctl.pos[2], true);
      }
      render();
    },
  };
  return api;
}

function clampN(v, a, b) { return v < a ? a : v > b ? b : v; }

