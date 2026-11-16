import {
  ready, engine, timeline, stagger, staggerValues, spring, inertia, animate,
  motionValue, scrollProgress, flightPath, htmlCanvas, scene, material, linearColor, lathe,
} from '../../packages/anikit/dist/anikit.lean.js';
import { buildHeadphones, yieldTask, CUP_Y } from './model.js';

const qs = new URLSearchParams(location.search);
const calm = matchMedia('(prefers-reduced-motion: reduce)').matches; // no idle spin, a gentler wave
const canvas = document.getElementById('stage');
await ready;

// ---------------------------------------------------------------- finishes
const FINISHES = {
  chrome: { label: 'polished chrome', color: '#f2f2f4', metalness: 1, roughness: 0.06 },
  brushed: { label: 'brushed aluminium', color: '#dfe1e5', metalness: 1, roughness: 0.3 },
  brushedMist: { label: 'anodised, mist', color: '#9fb4d2', metalness: 1, roughness: 0.3 },
  coral: { label: 'anodised, coral', color: '#ff7a5c', metalness: 1, roughness: 0.3 },
  sage: { label: 'matte sage', color: '#a3b59b', metalness: 0, roughness: 0.55 },
  graphite: { label: 'soft rubber, graphite', color: '#45484e', metalness: 0, roughness: 0.7 },
  butter: { label: 'soft rubber, butter', color: '#f4d47e', metalness: 0, roughness: 0.7 },
  rubberSand: { label: 'soft rubber, sand', color: '#e6ddd1', metalness: 0, roughness: 0.8 },
  glass: { label: 'frosted glass', color: '#f7f9fb', metalness: 0, roughness: 0.38, opacity: 0.86 },
  glassSmoke: { label: 'smoked glass', color: '#7d838c', metalness: 0, roughness: 0.18, opacity: 0.86 },
  glassAmber: { label: 'amber glass', color: '#f5b25f', metalness: 0, roughness: 0.22, opacity: 0.86 },
};
const CYCLE = {
  band: ['chrome', 'brushed', 'coral', 'graphite'],
  bandpad: ['rubberSand', 'graphite', 'butter', 'sage'],
  slider: ['brushed', 'chrome', 'coral', 'graphite'],
  yoke: ['chrome', 'brushed', 'graphite', 'coral'],
  cup: ['brushedMist', 'coral', 'brushed', 'sage', 'graphite'],
  glass: ['glass', 'glassSmoke', 'glassAmber'],
  pad: ['rubberSand', 'graphite', 'butter', 'sage'],
};
const lin = (hex) => linearColor(hex);

// ---------------------------------------------------------------- the scene
const pinned = qs.get('q');
let quality = pinned !== null ? +pinned : 3;
const FLOOR_Y = -8.75;
const s = scene(canvas, {
  camera: { fov: 30, position: [0, 6, 60], target: [0, -0.6, 0], near: 1, far: 2400 },
  environment: 'studio', background: '#eceae6', shadows: true,
  bloom: { strength: 0.16, threshold: 3.2 }, fxaa: true, dpr: 2, adaptive: false, autoRender: false,
});
// A bright, warm studio. Library gap: scene() only ships a dark 'studio'
// environment and has no public way to recolour it, so we tint its private
// gradient here (top, horizon, ground: the softboxes stay).
Object.assign(s.envC, { top: lin('#d9d6d1'), hor: lin('#c9c5bf'), ground: lin('#57544f') });
s.lightDir.set(0.25, 1, 0.35); // overhead key: shadows sit under the parts, like contact shadows
s.lightIntensity = 1.6;
s.envIntensity = 1.0;
s.exposure = 0.95;

// A seamless photo-studio cyclorama, lathed: a round floor curving up into a
// wall all the way around, so orbiting never finds an edge.
function cycloramaGeometry() {
  const prof = [[0, 0]];
  for (let r = 60; r <= 420; r += 60) prof.push([r, 0]);
  for (let k = 1; k <= 18; k++) { const a = (k / 18) * (Math.PI / 2); prof.push([420 + Math.sin(a) * 150, 150 - Math.cos(a) * 150]); }
  prof.push([570, 1300]);
  const g = lathe(prof, 128);
  // Seen from inside: flip normals and winding.
  for (let i = 0; i < g.normals.length; i++) g.normals[i] = -g.normals[i];
  const I = g.indices;
  for (let i = 0; i < I.length; i += 3) { const t = I[i + 1]; I[i + 1] = I[i + 2]; I[i + 2] = t; }
  for (let i = 1; i < g.positions.length; i += 3) g.positions[i] += FLOOR_Y;
  return g;
}
const sweep = s.add(cycloramaGeometry(), { material: { color: '#f3f1ee', roughness: 0.95, metalness: 0 }, castShadow: false });
void sweep;

// 300 copies: a 20×15 grid around the hero (the hero itself is the real objects).
const COLS = 20, ROWS = 15, N = COLS * ROWS, HERO = 7 * COLS + 10;
const delays = staggerValues(N, 1, { grid: [COLS, ROWS], from: HERO, ease: 'inQuad' });
const maxDelay = Math.max(...delays);
const copies = new Float32Array((N - 1) * 4); // x, z, delay, phase
{
  let k = 0;
  for (let j = 0; j < ROWS; j++) for (let i = 0; i < COLS; i++) {
    const idx = j * COLS + i;
    if (idx === HERO) continue;
    const o = k++ * 4;
    copies[o] = (i - 10) * 23 + (Math.abs(j - 7) % 2) * 7;
    copies[o + 1] = (j - 7) * 21;
    copies[o + 2] = 0.04 + (delays[idx] / maxDelay) * 0.95;
    copies[o + 3] = ((idx * 2654435761) % 1000) / 1000;
  }
}
const PALETTE = ['#ff6f52', '#7f9cc6', '#93ab88', '#f2c75c', '#3e4148'].map(lin);

const hero = await buildHeadphones(1);
const lod = await buildHeadphones(0.2);
const parts = [];
for (let i = 0; i < hero.length; i++) { const p = hero[i]; if (i % 4 === 3) await yieldTask(); parts.push((() => {
  const fin = FINISHES[p.mat];
  const mat = material({ color: fin.color, metalness: fin.metalness, roughness: fin.roughness, opacity: fin.opacity ?? 1 });
  const obj = s.add(p.geometry, { material: mat, position: p.centre });
  const c = lin(fin.color);
  const m = { r: c[0], g: c[1], b: c[2], metalness: fin.metalness, roughness: fin.roughness, glow: 0 };
  const vary = p.group === 'cup';
  const cm = material({ color: vary ? '#ffffff' : fin.color, metalness: fin.metalness, roughness: fin.roughness });
  const inst = s.instances(lod[i].geometry, N - 1, (k, o) => {
    o.scale.set(0);
    if (vary) { const col = PALETTE[Math.floor(copies[k * 4 + 3] * 5) % 5]; o.color[0] = col[0]; o.color[1] = col[1]; o.color[2] = col[2]; }
  }, { material: cm });
  inst.visible = false;
  inst.castShadow = false; // the shadow map only covers the hero's neighbourhood
  return { ...p, obj, mat, m, finish: p.mat, e: 0, inst, vary, axis: p.spin[0], deg: p.spin[1] };
})()); }
const groups = {};
for (const p of parts) (groups[p.group] ||= []).push(p);

// ---------------------------------------------------------------- choreography: one anikit timeline, scrubbed by scroll
const rig = { dist: 60, ty: -0.6, lift: 0, yawOff: 0, pitchOff: 0, labels: 0, fin: 0, appear: 0, wave: 0 };
const order = ['band', 'bandpad', 'slider', 'yoke', 'cup', 'glass', 'pad'];
const explodeOrder = [...parts].sort((a, b) => order.indexOf(a.group) - order.indexOf(b.group));
const tl = timeline({ autoplay: false })
  .add(rig, { dist: 86, lift: 3.2, yawOff: -0.3, pitchOff: 0.1, ty: 2.8 }, { duration: 2.6, ease: 'inOutCubic' }, 0.8)
  .add(explodeOrder, { e: 1 }, { duration: 1.8, ease: 'inOutCubic', delay: stagger(0.11) }, 1.2)
  .add(rig, { labels: 1 }, { duration: 0.6, ease: 'outCubic' }, 3.9)
  .add(rig, { labels: 0 }, { duration: 0.35, ease: 'inCubic' }, 5.7)
  .add([...explodeOrder].reverse(), { e: 0 }, { duration: 1.3, ease: 'inOutCubic', delay: stagger(0.06) }, 6.1)
  .add(rig, { lift: 0, yawOff: 0, pitchOff: 0, dist: 60, ty: -0.6 }, { duration: 1.4, ease: 'inOutCubic' }, 6.2)
  .add(rig, { fin: 1 }, { duration: 2.6, ease: 'inOutSine' }, 7.4)
  .add(rig, { appear: 1.05 }, { duration: 1.7, ease: 'linear' }, 8.4)
  .add(rig, { wave: 1 }, { duration: 1.3, ease: 'inOutSine' }, 8.9)
  .add(rig, { dist: 60 }, { duration: 0.4 }, 10);
const TL_LEN = tl.duration;

// Finale camera: a crane move along an anikit flight path, sampled once.
const crane = flightPath([[0, 6, 60], [30, 50, 140], [70, 160, 280], [90, 250, 380]], { lookAhead: 40, bank: 0 });
const CRANE_N = 240, craneXYZ = new Float32Array(CRANE_N * 3);
crane.points(CRANE_N).forEach((p, i) => craneXYZ.set(p, i * 3));

const scrolled = scrollProgress();
const progress = motionValue(0);
scrolled.on((v) => progress.spring(v, { stiffness: 70, damping: 20 }));

// ---------------------------------------------------------------- orbit: drag with inertia, settle with a spring
const orbit = { yaw: -0.5, pitch: 0.12, drift: 0 };
let drag = null;
const orbitAnims = [];
const stopOrbit = () => { for (const a of orbitAnims) a.kill(); orbitAnims.length = 0; };
canvas.addEventListener('pointerdown', (e) => {
  stopOrbit();
  drag = { x: e.clientX, y: e.clientY, t: performance.now(), vx: 0, vy: 0, moved: 0 };
  canvas.setPointerCapture(e.pointerId);
  document.body.classList.add('grabbing');
});
canvas.addEventListener('pointermove', (e) => {
  if (!drag) { hover(e); return; }
  const now = performance.now(), dt = Math.max(1, now - drag.t) / 1000;
  const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
  orbit.yaw -= dx * 0.0065;
  orbit.pitch = Math.max(-0.12, Math.min(0.85, orbit.pitch + dy * 0.004));
  drag.vx = drag.vx * 0.6 + (-dx * 0.0065 / dt) * 0.4;
  drag.vy = drag.vy * 0.6 + (dy * 0.004 / dt) * 0.4;
  drag.moved += Math.abs(dx) + Math.abs(dy);
  drag.x = e.clientX; drag.y = e.clientY; drag.t = now;
});
const endDrag = (e) => {
  if (!drag) return;
  document.body.classList.remove('grabbing');
  const d = drag; drag = null;
  if (d.moved < 6 && e.type === 'pointerup') { click(e); return; }
  const v = performance.now() - d.t > 80 ? 0 : d.vx;
  orbitAnims.push(inertia(orbit, { yaw: { velocity: v } }, { timeConstant: 0.5 }));
  orbitAnims.push(spring(orbit, { pitch: 0.12 }, { stiffness: 60, damping: 11, velocity: d.vy }));
};
canvas.addEventListener('pointerup', endDrag);
canvas.addEventListener('pointercancel', endDrag);
canvas.addEventListener('pointerleave', () => { hoverQueued = { clientX: -1e4, clientY: -1e4 }; });

// ---------------------------------------------------------------- picking (scene.raycast): hover and click to change a finish
const partOf = (hit) => hit && parts.find((p) => p.obj === hit.object);
let hovered = null, hoverQueued = null;
function hover(e) { hoverQueued = e; }
function pickHover() {
  const e = hoverQueued; hoverQueued = null;
  if (!e) return;
  const r = canvas.getBoundingClientRect();
  const p = rig.appear > 0.02 || e.clientX < -1e3 ? null : partOf(s.raycast(e.clientX - r.left, e.clientY - r.top));
  const g = p ? p.group : null;
  if (g === hovered) return;
  if (hovered) for (const q of groups[hovered]) spring(q.m, { glow: 0 }, { stiffness: 140, damping: 20 });
  if (g) for (const q of groups[g]) spring(q.m, { glow: 1 }, { stiffness: 140, damping: 20 });
  hovered = g;
  canvas.style.cursor = g ? 'pointer' : '';
}
function click(e) {
  if (rig.appear > 0.02) return;
  const r = canvas.getBoundingClientRect();
  const p = partOf(s.raycast(e.clientX - r.left, e.clientY - r.top));
  if (!p) return;
  const g = p.group, list = CYCLE[g];
  const next = list[(list.indexOf(groups[g][0].finish) + 1) % list.length];
  const fin = FINISHES[next], c = lin(fin.color);
  for (const q of groups[g]) {
    q.finish = next;
    spring(q.m, { r: c[0], g: c[1], b: c[2], metalness: fin.metalness, roughness: fin.roughness }, { stiffness: 120, damping: 16 });
    // A little nudge so the change reads as physical.
    animate(q.obj.scale, { x: [1.06, 1], y: [1.06, 1], z: [1.06, 1] }, { ease: 'spring(bounce: 0.45, duration: 0.6)' });
  }
  const label = labels.find((l) => l.group === g);
  if (label) animate(label.sub, { text: { value: fin.label, type: 'scramble' } }, { duration: 0.5 });
}

// ---------------------------------------------------------------- labels: real HTML via htmlCanvas(), drawn by a scene layer
const LABELS = [['band', 'Headband'], ['bandpad', 'Band cushion'], ['slider', 'Slider'], ['yoke', 'Yoke'], ['cup', 'Housing'], ['glass', 'Glass cap'], ['pad', 'Ear cushion']];
const labelLayer = document.getElementById('labels');
const labels = LABELS.map(([group, name]) => {
  const part = groups[group].find((p) => p.centre[0] < 0.5) || groups[group][0];
  const el = document.createElement('div');
  el.className = 'label';
  el.innerHTML = `<div class="card"><b></b><span></span></div>`;
  el.querySelector('b').textContent = name;
  const sub = el.querySelector('span');
  sub.textContent = FINISHES[part.finish].label;
  labelLayer.appendChild(el);
  const hc = htmlCanvas(el, { target: s.gl, live: true, scale: Math.min(2, devicePixelRatio || 1) });
  return { group, part, el, sub, hc, minor: group === 'slider' || group === 'yoke' || group === 'bandpad', tex: null, x: 0, y: 0, w: 0, h: 0, ax: 0, ay: 0, o: 0, side: 1 };
});
await Promise.all(labels.map((l) => l.hc.ready.catch(() => null)));

const LABEL_VS = `#version 300 es
uniform vec4 uRect; uniform vec4 uLine; uniform vec2 uPx; uniform float uMode;
out vec2 vUv;
void main() {
  vec2 c = vec2(gl_VertexID & 1, gl_VertexID >> 1);
  vUv = vec2(c.x, 1.0 - c.y);
  vec2 p;
  if (uMode == 1.0) { vec2 d = uLine.zw - uLine.xy; vec2 n = normalize(vec2(-d.y, d.x) / uPx) * uPx * 0.75; p = mix(uLine.xy, uLine.zw, c.x) + n * (c.y * 2.0 - 1.0); }
  else p = mix(uRect.xy, uRect.zw, c);
  gl_Position = vec4(p, 0.0, 1.0);
}`;
// The layer draws into the scene's linear HDR buffer before ACES, so the card
// colours are passed through the inverse curve to land on their sRGB values.
const LABEL_FS = `#version 300 es
precision highp float;
in vec2 vUv;
uniform sampler2D uTex; uniform float uMode, uAlpha, uExposure, uHdr;
out vec4 o;
vec3 invAces(vec3 y) {
  y = clamp(y, 0.0, 0.9);
  vec3 a = 2.43 * y - 2.51, b = 0.59 * y - 0.03, c = 0.14 * y;
  return (-b - sqrt(max(b * b - 4.0 * a * c, 0.0))) / (2.0 * a);
}
vec3 toScene(vec3 srgb) { vec3 l = pow(srgb, vec3(2.2)); return uHdr > 0.5 ? invAces(l) / uExposure : srgb; }
void main() {
  vec4 c;
  if (uMode == 0.0) { c = texture(uTex, vUv); if (c.a > 0.0) c.rgb = toScene(c.rgb / c.a) * c.a; }
  else if (uMode == 1.0) c = vec4(toScene(vec3(0.25)), 1.0) * 0.6;
  else { float r = length(vUv - 0.5) * 2.0; float halo = 1.0 - smoothstep(0.8, 1.0, r), core = 1.0 - smoothstep(0.38, 0.5, r);
    c = vec4(toScene(mix(vec3(1.0), vec3(0.11, 0.11, 0.13), core)), 1.0) * halo; }
  o = c * uAlpha;
}`;
let lp = null;
// A scene layer hands us this frame's camera matrices; the cards themselves are
// drawn after the scene's post chain (tone mapping, bloom, FXAA) so the HTML
// keeps its exact colours and crisp text.
s.add({ render(ctx) { VP.set(ctx.viewProj); }, dispose() {} });
const labelPass = {
  render(ctx) {
    const gl = ctx.gl;
    if (rig.labels < 0.004) return;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    if (!lp) {
      const p = gl.createProgram();
      for (const [t, src] of [[gl.VERTEX_SHADER, LABEL_VS], [gl.FRAGMENT_SHADER, LABEL_FS]]) {
        const sh = gl.createShader(t); gl.shaderSource(sh, src); gl.compileShader(sh);
        if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh));
        gl.attachShader(p, sh);
      }
      gl.linkProgram(p);
      const u = {};
      for (const n of ['uRect', 'uLine', 'uPx', 'uMode', 'uTex', 'uAlpha', 'uExposure', 'uHdr']) u[n] = gl.getUniformLocation(p, n);
      lp = { p, u, vao: gl.createVertexArray() };
    }
    layoutLabels();
    const prevProg = gl.getParameter(gl.CURRENT_PROGRAM);
    const u = lp.u, sx = 2 / cssW, sy = 2 / cssH;
    gl.useProgram(lp.p);
    gl.bindVertexArray(lp.vao);
    gl.disable(gl.DEPTH_TEST);
    gl.depthMask(false);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.uniform1i(u.uTex, 0);
    gl.uniform2f(u.uPx, sx, sy);
    gl.uniform1f(u.uExposure, s.exposure);
    gl.uniform1f(u.uHdr, ctx.hdr ? 1 : 0);
    gl.activeTexture(gl.TEXTURE0);
    for (const l of labels) {
      if (l.o < 0.004 || !l.tex) continue;
      gl.bindTexture(gl.TEXTURE_2D, l.tex); // unit 0 must hold a colour texture for every mode
      gl.uniform1f(u.uAlpha, l.o);
      const ex = l.x + (l.side < 0 ? l.w - 12 : 12), ey = l.y + l.h * 0.5;
      gl.uniform1f(u.uMode, 1);
      gl.uniform4f(u.uLine, l.ax * sx - 1, 1 - l.ay * sy, ex * sx - 1, 1 - ey * sy);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.uniform1f(u.uMode, 2);
      gl.uniform4f(u.uRect, (l.ax - 5) * sx - 1, 1 - (l.ay + 5) * sy, (l.ax + 5) * sx - 1, 1 - (l.ay - 5) * sy);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.uniform1f(u.uMode, 0);
      gl.bindTexture(gl.TEXTURE_2D, l.tex);
      gl.uniform4f(u.uRect, l.x * sx - 1, 1 - (l.y + l.h) * sy, (l.x + l.w) * sx - 1, 1 - l.y * sy);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
    gl.depthMask(true);
    gl.enable(gl.DEPTH_TEST);
    gl.disable(gl.BLEND);
    gl.bindVertexArray(null);
    gl.useProgram(prevProg);
  },
  dispose() {},
};

// ---------------------------------------------------------------- captions
const captions = [...document.querySelectorAll('[data-at]')].map((el) => ({ el, a: +el.dataset.at, b: +el.dataset.to, o: -1 }));

// ---------------------------------------------------------------- per frame
let cssW = canvas.clientWidth, cssH = canvas.clientHeight, lastP = -1, F_TIME = 0;
// The captions own a text area the object and labels keep out of: a left
// column on wide screens, a bottom band on small or portrait ones.
const rigDrift = { yaw: 0, pitch: 0 };
const FREE = { x0: 0, y0: 0, x1: 1, y1: 1, column: true };
function measureFree() {
  cssW = canvas.clientWidth; cssH = canvas.clientHeight;
  let right = 0, top = cssH;
  for (const sec of document.querySelectorAll('.captions section')) {
    if (sec.matches(':last-child')) continue; // the finale card floats over the wave
    const r = sec.getBoundingClientRect();
    right = Math.max(right, r.right); top = Math.min(top, r.top);
  }
  FREE.column = cssW >= 900 && cssW / cssH > 1.15;
  if (FREE.column) { FREE.x0 = right + 24; FREE.y0 = cssH < 560 ? 48 : 64; FREE.x1 = cssW; FREE.y1 = cssH; }
  else { FREE.x0 = 0; FREE.y0 = cssH < 560 ? 48 : 64; FREE.x1 = cssW; FREE.y1 = top - 12; }
}
addEventListener('resize', measureFree);
measureFree();
const P = new Float32Array(3), VP = new Float32Array(16), LCTX = { gl: s.gl, hdr: false };
const sideBuf = new Array(8).fill(null);
function project(x, y, z) {
  const m = VP;
  const cx = m[0] * x + m[4] * y + m[8] * z + m[12], cy = m[1] * x + m[5] * y + m[9] * z + m[13], cw = m[3] * x + m[7] * y + m[11] * z + m[15];
  P[0] = (cx / cw * 0.5 + 0.5) * cssW; P[1] = (0.5 - cy / cw * 0.5) * cssH; P[2] = cw;
}

  // Labels: anchored to each exploded part, cards pushed away from the object.
function layoutLabels() {
  const lo = rig.labels, lift = rig.lift;
  {
    project(0, rig.ty + lift, 0);
    const midX = P[0];
    for (const l of labels) {
      const q = l.part.obj.position;
      project(q.x, q.y, q.z);
      l.ax = P[0]; l.ay = P[1];
      l.side = l.ax < midX ? -1 : 1;
      l.tex = l.hc.texture; l.w = l.hc.width; l.h = l.hc.height;
      const reach = 34 + 14 * lo;
      l.x = l.side < 0 ? l.ax - reach - l.w : l.ax + reach;
      l.y = l.ay - l.h * 0.5 - 18;
      const minX = Math.max(16, FREE.column ? FREE.x0 : 16);
      if (l.x < minX) l.x = minX; else if (l.x + l.w > cssW - 16) l.x = cssW - 16 - l.w;
      if (l.y < 56) l.y = 56;
      l.o = (cssW < 560 || FREE.y1 - FREE.y0 < 420) && l.minor ? 0 : lo;
    }
    for (let side = -1; side <= 1; side += 2) {
      let n = 0;
      for (const l of labels) if (l.side === side && l.o > 0) sideBuf[n++] = l;
      for (let i = 1; i < n; i++) { const x = sideBuf[i]; let j = i - 1; while (j >= 0 && sideBuf[j].y > x.y) { sideBuf[j + 1] = sideBuf[j]; j--; } sideBuf[j + 1] = x; }
      for (let i = 1; i < n; i++) { const a = sideBuf[i - 1], b = sideBuf[i]; const min = a.y + a.h - 22; if (b.y < min) b.y = min; }
      // Keep the stack above the text band: push it back up from the bottom if needed.
      const bottom = (FREE.column ? cssH : FREE.y1) - 8;
      for (let i = n - 1; i >= 0; i--) { const b = sideBuf[i]; const max = (i === n - 1 ? bottom : sideBuf[i + 1].y + 22) - b.h; if (b.y > max) b.y = max; }
    }
  }
}

const AX = [s.camera.position, s.camera.target];
let copiesOn = false;
let waveT = 0;
const copyFn = (k, o) => {
  const ox = copies[k * 4], oz = copies[k * 4 + 1], delay = copies[k * 4 + 2], ph0 = copies[k * 4 + 3];
  let a = Math.min(1, Math.max(0, (rig.appear - delay) / 0.35));
  a = a * a * (3 - 2 * a) * (1 + 0.25 * Math.sin(Math.PI * a));
  const d = Math.hypot(ox, oz), ph = d * 0.055 - waveT * 2.1 + ph0 * 0.8;
  const w = calm ? rig.wave * 0.25 : rig.wave;
  const h = w * (Math.sin(ph) * 3.4 + 2);
  const yaw = w * (0.6 * Math.sin(ph * 0.5 + ph0) + d * 0.004), pitch = w * 0.32 * Math.cos(ph);
  // Rotate this part's centre with the copy (R = Ry(yaw)·Rx(pitch), as compose() does).
  const c = curPart.centre;
  const cp = Math.cos(pitch), sp = Math.sin(pitch), cy = Math.cos(yaw), sy = Math.sin(yaw);
  const y1 = c[1] * cp - c[2] * sp, z1 = c[1] * sp + c[2] * cp;
  const x2 = c[0] * cy + z1 * sy, z2 = -c[0] * sy + z1 * cy;
  o.position.set(ox + x2 * a, h + y1 * a, oz + z2 * a);
  o.rotation.set(pitch, yaw, 0);
  o.scale.set(a);
  if (curPart.vary) { const col = PALETTE[Math.floor(ph0 * 5) % 5]; o.color[0] = col[0]; o.color[1] = col[1]; o.color[2] = col[2]; }
};
let curPart = parts[0];

function update(dtMs) {
  const dt = Math.min(0.05, dtMs / 1000);
  F_TIME += dt;
  const p = Math.min(1, Math.max(0, progress.get()));
  if (p !== lastP) { tl.seek(p * TL_LEN); lastP = p; }
  orbit.drift += ((!drag && p < 0.02 ? 1 : 0) - orbit.drift) * Math.min(1, dt * 1.5);
  if (!drag && !calm) orbit.yaw += orbit.drift * dt * 0.12;
  if (hoverQueued) pickHover();

  const lift = rig.lift;
  for (const q of parts) {
    const e = q.e;
    q.obj.position.set(q.centre[0] + q.explode[0] * e, q.centre[1] + q.explode[1] * e + lift, q.centre[2] + q.explode[2] * e);
    const r = (q.deg * Math.PI / 180) * e;
    q.obj.rotation.set(q.axis === 0 ? r : 0, q.axis === 1 ? r : 0, q.axis === 2 ? r : 0);
    const m = q.m, c = q.mat.color;
    c[0] = Math.max(0, m.r); c[1] = Math.max(0, m.g); c[2] = Math.max(0, m.b);
    q.mat.metalness = Math.min(1, Math.max(0, m.metalness));
    q.mat.roughness = Math.min(1, Math.max(0.03, m.roughness));
    const gl = Math.max(0, m.glow) * 0.07; q.mat.emissive[0] = gl; q.mat.emissive[1] = gl * 0.97; q.mat.emissive[2] = gl * 0.92;
  }

  // Camera: orbit, blended into the crane path for the finale.
  // Frame the object inside the free area: pull back until it fits, then shift it to its centre.
  const freeW = Math.max(120, FREE.x1 - FREE.x0), freeH = Math.max(120, FREE.y1 - FREE.y0);
  // Hero scale: as close as 1/1.6 of the rig distance, but never so close that
  // the object (about 0.85 × the view height wide, 0.76 × tall at rig distance) leaves the free area.
  // On narrow screens the exploded view is relatively wider than the hero: allow for it.
  const kw = 0.85 * (1 + (freeW < 700 ? 0.35 : 0) * Math.min(1, rig.lift / 3.2));
  const fitK = Math.max(1 / 1.6, (kw * cssH) / freeW, (0.76 * cssH) / freeH);
  // A slow cinematic drift, so the shot breathes even without dragging.
  const drift = calm ? 0 : 1;
  rigDrift.yaw = Math.sin(F_TIME * 0.11) * 0.09 * drift;
  rigDrift.pitch = Math.sin(F_TIME * 0.083 + 1.3) * 0.035 * drift;
  const yaw = orbit.yaw + rig.yawOff + rigDrift.yaw, pitch = orbit.pitch + rig.pitchOff + rigDrift.pitch, d = rig.dist * fitK * (1 + 0.015 * Math.sin(F_TIME * 0.07) * drift);
  let ex = Math.sin(yaw) * Math.cos(pitch) * d, ey = rig.ty + Math.sin(pitch) * d, ez = Math.cos(yaw) * Math.cos(pitch) * d;
  let ty = rig.ty;
  const fin = rig.fin;
  if (fin > 0) {
    const sc = fin * (CRANE_N - 1), k = Math.min(CRANE_N - 2, Math.floor(sc)), f = sc - k, o = k * 3;
    const cx = craneXYZ[o] + (craneXYZ[o + 3] - craneXYZ[o]) * f, cy = craneXYZ[o + 1] + (craneXYZ[o + 4] - craneXYZ[o + 1]) * f, cz = craneXYZ[o + 2] + (craneXYZ[o + 5] - craneXYZ[o + 2]) * f;
    const cs = Math.cos(orbit.yaw + 0.5), sn = Math.sin(orbit.yaw + 0.5), w = fin * fin * (3 - 2 * fin);
    ex += (cx * cs + cz * sn - ex) * w; ey += (cy - 2 - ey) * w; ez += (-cx * sn + cz * cs - ez) * w;
    ty += (-24 - ty) * w;
  }
  // Wide screens: slide the camera sideways so the object sits right of the captions.
  const wpp = (2 * d * Math.tan((30 * Math.PI) / 360)) / cssH; // world units per CSS px at the object
  const keep = 1 - fin;
  const shift = -((FREE.x0 + FREE.x1) / 2 - cssW / 2) * wpp * keep;
  const lower = ((FREE.y0 + FREE.y1) / 2 - cssH / 2) * wpp * keep;
  const rl = Math.hypot(ex, ez) || 1, rx = ez / rl, rz = -ex / rl;
  AX[0].set(ex + rx * shift, ey + lower, ez + rz * shift);
  AX[1].set(rx * shift, ty + lower, rz * shift);

  // The key light sweeps across the chrome as you scroll: highlights travel, shadows turn.
  const sweepA = p * Math.PI * 1.6 - 0.4;
  s.lightDir.set(Math.sin(sweepA) * 0.55, 1, 0.35 + Math.cos(sweepA) * 0.3);

  // Copies.
  const on = rig.appear > 0.001;
  if (on || copiesOn) {
    waveT += dt;
    for (const q of parts) { q.inst.visible = on; if (on) { curPart = q; q.inst.update(copyFn); } }
    copiesOn = on;
  }

  const t = p * TL_LEN;
  for (const c of captions) {
    const o = Math.max(0, Math.min(1, (t - c.a) / 0.35, (c.b - t) / 0.35));
    if (Math.abs(o - c.o) > 0.002) { c.el.style.opacity = o.toFixed(3); c.el.style.transform = `translateY(${((1 - o) * 10).toFixed(1)}px)`; c.el.style.visibility = o > 0 ? 'visible' : 'hidden'; c.o = o; }
  }
}

// ---------------------------------------------------------------- perf overlay + quality governor (effects before resolution)
const times = new Float32Array(240), sorted = new Float32Array(240);
let tn = 0, lastT = 0, lastReport = 0, govT = 0;
const perfEl = qs.has('perf') ? Object.assign(document.body.appendChild(document.createElement('pre')), { id: 'perf' }) : null;
function applyQuality() {
  // Library gap: scene() has no public runtime toggles for bloom or the pixel ratio yet.
  s.bloomOn = quality >= 3;
  s.shadows = quality >= 2;
  const dpr = Math.min(2, devicePixelRatio || 1) * (quality >= 1 ? 1 : 0.75);
  if (s.dpr !== dpr) { s.dpr = dpr; s.resize?.(); }
}
applyQuality();
function stats() {
  const n = Math.min(tn, 240);
  sorted.set(times); const a = sorted.subarray(0, n).sort();
  return [a[Math.floor(n * 0.5)] || 0, a[Math.floor(n * 0.95)] || 0, a[n - 1] || 0];
}
function govern(now) {
  if (pinned !== null || tn < 90 || now - govT < 1500) return;
  const [, p95] = stats();
  if (p95 > 22 && quality > 0) { quality--; applyQuality(); govT = now; tn = 0; }
}
function frame(now, dtMs) {
  if (lastT) times[tn++ % 240] = now - lastT;
  lastT = now;
  update(dtMs);
  s.render();
  labelPass.render(LCTX);
  govern(now);
  if (perfEl && now - lastReport > 500) {
    const [a, b, c] = stats();
    perfEl.textContent = `frame p50 ${a.toFixed(1)}  p95 ${b.toFixed(1)}  max ${c.toFixed(1)} ms\nquality ${quality}  dpr ${s.dpr}  ${s.stats.drawCalls} draws  ${(s.stats.triangles / 1000).toFixed(0)}k tris`;
    lastReport = now;
  }
}

// Pre-warm: render the finale (all copies) and the labels once before the first visible frame.
await yieldTask();
rig.appear = 1.2; update(16); s.render();
await yieldTask();
rig.appear = 0; rig.labels = 1; update(16); s.render(); labelPass.render(LCTX);
await yieldTask();
rig.labels = 0; update(16); s.render();
engine.onFrame((t, dt) => frame(performance.now(), dt));
canvas.style.opacity = '0';
animate(canvas, { opacity: 1 }, { duration: 1.1, ease: 'outCubic', delay: 0.05 });
document.body.classList.add('ready');
void CUP_Y;
