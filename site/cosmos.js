// The cosmos: one particle cloud living behind the whole page. Each scene
// ("station") declares a shape, a background colour field, a render mode
// (ink on paper, or light on dark) and a tint. While a scene is pinned its
// shape holds; between scenes everything blends with the scroll.
// Simulation, projection and rendering all run in anikit's WebAssembly core.
import { engine, pointCloud, glow, scene3d, textPoints, animate, noise2, mixColor } from '../packages/anikit/dist/anikit.js';

const TAU = Math.PI * 2;
const hex = (c) => {
  const m = c.match(/\d+(\.\d+)?/g).map(Number);
  return '#' + m.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
};

export async function cosmos({ reduced = false } = {}) {
  const small = innerWidth < 760;
  const N = reduced ? 30_000 : small ? 60_000 : 150_000;
  const canvas = document.getElementById('cosmos');
  const g = glow(canvas, { resolution: small ? 0.6 : 0.8, background: '#ece6da', mode: 'ink', trail: 0.55, exposure: 0, budget: 7 });
  const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 6000 });
  let vw = innerWidth, vh = innerHeight;
  const halfH = () => Math.tan((scene.fov * Math.PI) / 360) * scene.camera.z;
  const wpp = () => (2 * halfH()) / vh;
  const W = () => (2 * halfH() * vw) / vh;
  const resize = () => {
    vw = innerWidth;
    vh = innerHeight;
    g.resize(vw, vh);
    scene.resize(vw, vh, 1);
  };
  resize();
  try { await document.fonts.load('900 200px Display'); } catch { /* fall back to system font */ }

  // ---- Shapes ----------------------------------------------------------------
  let seed = 7;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const gauss = () => (rnd() + rnd() + rnd() - 1.5) * 1.15;
  const make = (fn) => {
    const a = new Float32Array(N * 3);
    for (let i = 0; i < N; i++) fn(i, a, i * 3);
    return a;
  };
  const rotX = (a, o, deg) => {
    const c = Math.cos((deg * Math.PI) / 180), s = Math.sin((deg * Math.PI) / 180);
    const y = a[o + 1], z = a[o + 2];
    a[o + 1] = y * c - z * s;
    a[o + 2] = y * s + z * c;
  };
  const rotY = (a, o, deg) => {
    const c = Math.cos((deg * Math.PI) / 180), s = Math.sin((deg * Math.PI) / 180);
    const x = a[o], z = a[o + 2];
    a[o] = x * c + z * s;
    a[o + 2] = -x * s + z * c;
  };
  const yOf = (sel) => {
    const el = document.querySelector(sel);
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    return -((r.top + scrollY + r.height / 2) - vh / 2) * wpp();
  };
  const sortX = (t) => {
    const idx = Array.from({ length: N }, (_, i) => i).sort((p, q) => t[p * 3] - t[q * 3]);
    const out = new Float32Array(N * 3);
    idx.forEach((k, i) => { out[i * 3] = t[k * 3]; out[i * 3 + 1] = t[k * 3 + 1]; out[i * 3 + 2] = t[k * 3 + 2]; });
    return out;
  };

  const shapes = {
    origin: () => make((i, a, o) => { const r = 4 * Math.cbrt(rnd()), t = rnd() * TAU, p = Math.acos(2 * rnd() - 1); a[o] = r * Math.sin(p) * Math.cos(t); a[o + 1] = r * Math.cos(p) + yOf('.wordmark-space'); a[o + 2] = r * Math.sin(p) * Math.sin(t); }),
    wordmark: () => {
      const t = textPoints('anikit', N, { font: 'Display, "Inter Display", system-ui', weight: 900, width: Math.min(W() * 0.86, 1800), depth: 14 });
      const y = yOf('.wordmark-space');
      for (let i = 1; i < t.length; i += 3) t[i] += y;
      return sortX(t);
    },
    galaxy: () => make((i, a, o) => {
      const arm = i % 3, d = Math.pow(rnd(), 0.6), r = 20 + d * 620;
      const ang = (arm / 3) * TAU + d * 4.6 + gauss() * 0.25 * (1.1 - d);
      a[o] = Math.cos(ang) * r + gauss() * 14;
      a[o + 1] = gauss() * (30 * (1 - d) + 5);
      a[o + 2] = Math.sin(ang) * r + gauss() * 14;
      rotX(a, o, 68);
      a[o + 1] += yOf('.wordmark-space') * 0.5;
    }),
    ribbon: () => make((i, a, o) => {
      // A twisting silk band across the screen.
      const u = (rnd() - 0.5) * W() * 1.5, v = (rnd() * 2 - 1);
      const tw = u * 0.0042 + 0.6, w = 150 + 60 * Math.sin(u * 0.003);
      a[o] = u;
      a[o + 1] = Math.sin(u * 0.0035) * 140 + Math.cos(tw) * v * w;
      a[o + 2] = Math.sin(tw) * v * w - 120;
      if (rnd() < 0.06) { a[o + 1] += gauss() * 60; a[o + 2] += gauss() * 60; }
    }),
    rain: () => make((i, a, o) => { a[o] = (rnd() - 0.5) * W(); a[o + 1] = -halfH() * 1.4; a[o + 2] = gauss() * 100; }),
    knot: () => make((i, a, o) => {
      const t = (i / N) * TAU, p = 3, q = 7, R = 270, rr = 110;
      const r = R + rr * Math.cos(q * t), tube = 20 * Math.sqrt(rnd()), ang = rnd() * TAU;
      a[o] = r * Math.cos(p * t) + tube * Math.cos(ang);
      a[o + 1] = r * Math.sin(p * t) + tube * Math.sin(ang);
      a[o + 2] = rr * 1.7 * Math.sin(q * t) + tube * Math.cos(ang * 1.3);
      rotX(a, o, 20);
      a[o] += W() * 0.2;
      a[o + 1] += 60;
    }),
    burst: () => make((i, a, o) => {
      const y = 2 * rnd() - 1, t = rnd() * TAU, r = Math.sqrt(1 - y * y);
      const spike = i % 4 === 0 ? 1 + Math.pow(rnd(), 2.5) * 3.2 : 1 + gauss() * 0.02;
      const R = 190 * spike;
      a[o] = r * Math.cos(t) * R + W() * 0.2; a[o + 1] = y * R + 60; a[o + 2] = r * Math.sin(t) * R;
    }),
    vortex: () => make((i, a, o) => {
      const h = rnd(), r = 14 + Math.pow(h, 1.5) * 520 + gauss() * 8, ang = h * 30 + rnd() * TAU * 0.05 + (i % 2) * Math.PI;
      a[o] = Math.cos(ang) * r; a[o + 2] = Math.sin(ang) * r - 200; a[o + 1] = (h - 0.5) * halfH() * 2.1;
    }),
    grid: () => { const s = Math.ceil(Math.sqrt(N)); return make((i, a, o) => {
      const gx = (i % s) / s - 0.5, gy = Math.floor(i / s) / s - 0.5;
      a[o] = gx * W() * 2.4; a[o + 1] = gy * 2400; a[o + 2] = -700 + noise2(gx * 5, gy * 5) * 90;
      rotX(a, o, -62);
      a[o + 1] -= 120;
    }); },
    lattice: () => make((i, a, o) => {
      const n = 5, s = 120, axis = i % 3, c = [Math.floor(rnd() * (n + 1)), Math.floor(rnd() * (n + 1)), Math.floor(rnd() * (n + 1))];
      c[axis] = rnd() * n;
      a[o] = (c[0] - n / 2) * s; a[o + 1] = (c[1] - n / 2) * s; a[o + 2] = (c[2] - n / 2) * s - 200;
      rotY(a, o, 35);
      rotX(a, o, -22);
    }),
    outro: () => make((i, a, o) => {
      // An ink splash: a ragged blot with droplets thrown along rays.
      const cx = W() * 0.24, cy = -40;
      if (i % 5 === 0) {
        const ray = Math.floor(rnd() * 23), ang = ray * 2.39 + gauss() * 0.04, d = 260 + Math.pow(rnd(), 0.7) * 380;
        const drop = (6 + 30 * Math.pow(1 - (d - 260) / 380, 2)) * Math.sqrt(rnd());
        const ta = rnd() * TAU;
        a[o] = cx + Math.cos(ang) * d + Math.cos(ta) * drop; a[o + 1] = cy + Math.sin(ang) * d + Math.sin(ta) * drop; a[o + 2] = gauss() * 4;
        return;
      }
      const t = rnd() * TAU, edge = 230 + noise2(Math.cos(t) * 2.2, Math.sin(t) * 2.2) * 70;
      const r = edge * Math.sqrt(rnd());
      a[o] = cx + Math.cos(t) * r; a[o + 1] = cy + Math.sin(t) * r; a[o + 2] = gauss() * 6;
    }),
  };
  const sway = { galaxy: 1, knot: 1, burst: 1, lattice: 1, vortex: 0.5, ribbon: 0.35, grid: 0.2 };
  const names = Object.keys(shapes);
  const index = Object.fromEntries(names.map((n, i) => [n, i]));
  const cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 2.2, swirl: 80, frequency: 0.011 });
  const built = new Set();
  const build = (name) => { cloud.shape(index[name], shapes[name]()); built.add(name); };
  build('origin');
  build('wordmark');
  build('galaxy');
  cloud.snap(index.origin);
  // Neutral per-particle brightness; colour comes from the scene tint.
  g.colors(N, () => { const r = rnd(); const k = r < 0.06 ? 1.6 : 0.45 + rnd() * 0.55; return [k, k, k]; });
  const rest = names.filter((n) => !built.has(n));
  const idle = window.requestIdleCallback || ((f) => setTimeout(f, 16));
  const buildNext = () => { const n = rest.shift(); if (!n) return; build(n); idle(buildNext); };
  idle(buildNext);
  addEventListener('resize', () => { resize(); for (const n of names) if (built.has(n)) build(n); });

  // ---- Stations ------------------------------------------------------------------
  const stations = Array.from(document.querySelectorAll('.station')).map((el) => ({
    el,
    shape: el.dataset.shape,
    bg: el.dataset.bg || '#ece6da',
    mode: el.dataset.mode || 'light',
    tint: (el.dataset.tint || '1,1,1').split(',').map(Number),
    exposure: parseFloat(el.dataset.exposure || '0.5'),
    name: el.dataset.name || el.closest('.scene')?.dataset.name || '',
  }));
  const smooth = (t) => t * t * (3 - 2 * t);
  const blend = () => {
    const y = scrollY;
    // Each station holds over its pinned range [top, top + height − vh].
    const r = stations.map((s) => {
      const b = s.el.getBoundingClientRect();
      const top = b.top + scrollY;
      return [top, top + Math.max(0, b.height - vh)];
    });
    for (let k = 0; k < stations.length; k++) {
      if (y <= r[k][1]) {
        if (y >= r[k][0] || k === 0) return { a: stations[k], b: stations[k], m: 0, k };
        const span = Math.max(1, r[k][0] - r[k - 1][1]);
        return { a: stations[k - 1], b: stations[k], m: smooth(Math.min(1, Math.max(0, (y - r[k - 1][1]) / span))), k: k - 1 };
      }
    }
    const l = stations[stations.length - 1];
    return { a: l, b: l, m: 0, k: stations.length - 1 };
  };

  // ---- Pointer -------------------------------------------------------------------
  const ptr = { x: -9999, y: -9999, tx: 0, ty: 0, rx: 0, ry: 0 };
  addEventListener('pointermove', (e) => {
    ptr.x = e.clientX; ptr.y = e.clientY;
    ptr.tx = (e.clientY / vh - 0.5) * -6; ptr.ty = (e.clientX / vw - 0.5) * 9;
  }, { passive: true });
  document.documentElement.addEventListener('pointerleave', () => { ptr.x = ptr.y = -9999; });
  const shock = (x, y, s = 1) => cloud.explode((x - vw / 2) * wpp(), -(y - vh / 2) * wpp(), 0, 1200 * s);

  // ---- Frame -----------------------------------------------------------------------
  const realNow = globalThis.__ANIKIT_REAL_NOW__ || (() => performance.now());
  const stats = { ms: 0, fps: 60, frames: 0, acc: 0, last: performance.now() };
  const state = { bg: '#ece6da', station: stations[0], m: 0, intro: !reduced };
  const intro = { v: reduced ? 1 : 0 };
  let t = 0;
  let exposure = 0;
  engine.onFrame((_time, dt) => {
    const now = performance.now();
    const t0 = realNow();
    t += dt / 1000;
    const { a, b, m } = blend();
    state.station = m < 0.5 ? a : b;
    state.m = m;
    if (!state.intro) {
      cloud.from = index[a.shape];
      cloud.to = index[b.shape];
      cloud.mix = m;
    }
    state.bg = a === b ? a.bg : hex(mixColor(a.bg, b.bg, m));
    g.options.background = state.bg;
    let dip = 1;
    if (a.mode !== b.mode) { g.options.mode = m < 0.5 ? a.mode : b.mode; dip = Math.abs(2 * m - 1); dip *= dip; } else g.options.mode = a.mode;
    const target = (a.exposure + (b.exposure - a.exposure) * m) * dip * intro.v;
    exposure += (target - exposure) * Math.min(1, dt / 90);
    const tint = [0, 1, 2].map((i) => a.tint[i] + (b.tint[i] - a.tint[i]) * m);
    const sw = (sway[a.shape] ?? 0) * (1 - m) + (sway[b.shape] ?? 0) * m;
    ptr.rx += (ptr.tx - ptr.rx) * 0.05;
    ptr.ry += (ptr.ty - ptr.ry) * 0.05;
    scene.rotation.x = ptr.rx * (0.3 + sw);
    scene.rotation.y = ptr.ry * (0.3 + sw) + sw * 24 * Math.sin(t * 0.21);
    cloud.step(dt / 1000);
    const ink = g.options.mode === 'ink';
    g.render(scene, cloud, {
      size: 1.1,
      intensity: ink ? (small ? 1.8 : 1.15) : small ? 0.45 : 0.28,
      fog: [650, 2700],
      lens: ptr.x > -9000 ? { x: ptr.x, y: ptr.y, radius: 180, strength: 30 } : null,
      exposure: exposure * Math.min(1, (1 - g.options.trail) / 0.45),
      tint,
      maxRadius: 4,
    });
    stats.acc += realNow() - t0;
    stats.frames++;
    if (now - stats.last > 500) {
      stats.ms = stats.acc / stats.frames;
      stats.fps = Math.min(Math.round((stats.frames * 1000) / (now - stats.last)), stats.ms > 0 ? Math.floor(1000 / stats.ms) : 999);
      stats.acc = stats.frames = 0;
      stats.last = now;
    }
  });

  // ---- Intro: an ink drop on paper becomes the wordmark --------------------------
  const playIntro = () => {
    if (reduced) { cloud.snap(index.wordmark); state.intro = false; intro.v = 1; return; }
    animate(intro, { v: [0, 1] }, { duration: 0.8, ease: 'outQuad' });
    setTimeout(() => {
      cloud.explode(0, yOf('.wordmark-space'), 0, 2400);
      cloud.from = index.origin;
      cloud.to = index.galaxy;
      animate(cloud, { mix: [0, 1] }, { duration: 1.0, ease: 'outCubic' });
    }, 450);
    setTimeout(() => {
      cloud.from = index.galaxy;
      cloud.to = index.wordmark;
      animate(cloud, { mix: [0, 1] }, { duration: 2.1, ease: 'inOutCubic', onComplete: () => (state.intro = false) });
    }, 1500);
    addEventListener('scroll', () => (state.intro = false), { once: true, passive: true });
  };

  return { cloud, glow: g, scene, stats, count: N, playIntro, shock, state, stations };
}

// ---------------------------------------------------------------------------
// Bullet time: hold Space (or a button) and the whole engine slows down.
// ---------------------------------------------------------------------------
export function bulletTime({ glow: g, buttons = [], vignette, onChange }) {
  let target = 1, speed = 1, raf = 0;
  const loop = () => {
    speed += (target - speed) * 0.12;
    if (Math.abs(target - speed) < 0.002) speed = target;
    engine.speed = speed;
    const k = (1 - speed) / 0.96;
    g.options.trail = 0.55 + 0.38 * k;
    if (vignette) vignette.style.opacity = String(k);
    raf = speed !== target ? requestAnimationFrame(loop) : 0;
  };
  const go = (v) => {
    target = v;
    for (const b of buttons) b.classList.toggle('on', v < 1);
    onChange?.(v < 1);
    if (!raf) raf = requestAnimationFrame(loop);
  };
  addEventListener('keydown', (e) => {
    if (e.code !== 'Space' || e.repeat || e.target.closest?.('input, textarea, [contenteditable]')) return;
    e.preventDefault();
    go(0.04);
  });
  addEventListener('keyup', (e) => e.code === 'Space' && go(1));
  for (const b of buttons) {
    b.addEventListener('pointerdown', (e) => { e.preventDefault(); b.setPointerCapture?.(e.pointerId); go(0.04); });
    for (const ev of ['pointerup', 'pointercancel', 'lostpointercapture']) b.addEventListener(ev, () => go(1));
  }
  addEventListener('blur', () => go(1));
  return { get speed() { return speed; } };
}
