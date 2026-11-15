// The cosmos: one particle cloud that lives behind the whole page, flowing
// into a new shape at every section. Simulation, projection and lighting all
// run in anikit's WebAssembly core; JavaScript does one putImageData a frame.
import { engine, pointCloud, glow, scene3d, textPoints, animate, noise2 } from '../packages/anikit/dist/anikit.js';

const TAU = Math.PI * 2;

export function cosmos({ reduced = false } = {}) {
  const small = innerWidth < 700;
  const N = reduced ? 30_000 : small ? 50_000 : 140_000;
  const canvas = document.getElementById('cosmos');
  // Adaptive: if a machine can't hold the budget, glow() lowers its pixel resolution.
  const g = glow(canvas, { resolution: small ? 0.6 : 0.75, background: '#07080b', trail: 0.5, exposure: 0, budget: 6 });
  const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 6000 });
  let vw = innerWidth, vh = innerHeight;
  // World units per CSS pixel on the z = 0 plane.
  const halfH = () => Math.tan((scene.fov * Math.PI) / 360) * scene.camera.z;
  const wpp = () => (2 * halfH()) / vh;
  const resize = () => {
    vw = innerWidth;
    vh = innerHeight;
    g.resize(vw, vh);
    scene.resize(vw, vh, 1);
  };
  resize();

  // ---- Shapes ------------------------------------------------------------
  let seed = 7;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const gauss = () => (rnd() + rnd() + rnd() - 1.5) * 1.15;
  const make = (fn) => {
    const a = new Float32Array(N * 3);
    for (let i = 0; i < N; i++) fn(i, a, i * 3);
    return a;
  };
  const W = () => (2 * halfH() * vw) / vh; // visible width in world units
  const font = getComputedStyle(document.body).fontFamily;
  const heroY = () => {
    const r = document.querySelector('.wordmark-space').getBoundingClientRect();
    return -((r.top + scrollY + r.height / 2) - vh / 2) * wpp();
  };
  const rotX = (a, o, deg) => {
    const c = Math.cos((deg * Math.PI) / 180), s = Math.sin((deg * Math.PI) / 180);
    const y = a[o + 1], z = a[o + 2];
    a[o + 1] = y * c - z * s;
    a[o + 2] = y * s + z * c;
  };

  const shapes = {
    origin: () => make((i, a, o) => { const r = 3 * Math.cbrt(rnd()), t = rnd() * TAU, p = Math.acos(2 * rnd() - 1); a[o] = r * Math.sin(p) * Math.cos(t); a[o + 1] = r * Math.cos(p); a[o + 2] = r * Math.sin(p) * Math.sin(t); }),
    wordmark: () => {
      const t = textPoints('anikit', N, { font, weight: 800, width: Math.min(W() * 0.5, 820), depth: 26 });
      const y = heroY();
      const idx = Array.from({ length: N }, (_, i) => i).sort((a, b) => t[a * 3] - t[b * 3]);
      const out = new Float32Array(N * 3);
      idx.forEach((k, i) => { out[i * 3] = t[k * 3]; out[i * 3 + 1] = t[k * 3 + 1] + y; out[i * 3 + 2] = t[k * 3 + 2]; });
      return out;
    },
    galaxy: () => make((i, a, o) => {
      const arm = i % 4, d = Math.pow(rnd(), 0.65), r = 30 + d * 560;
      const ang = (arm / 4) * TAU + d * 4.2 + gauss() * 0.28 * (1.1 - d);
      a[o] = Math.cos(ang) * r + gauss() * 18;
      a[o + 1] = gauss() * (38 * (1 - d) + 6);
      a[o + 2] = Math.sin(ang) * r + gauss() * 18;
      if (rnd() < 0.12) { const b = 60 * Math.cbrt(rnd()); a[o] = gauss() * b; a[o + 1] = gauss() * b * 0.7; a[o + 2] = gauss() * b; }
      rotX(a, o, 62);
    }),
    sphere: () => make((i, a, o) => {
      const y = 1 - (i / (N - 1)) * 2, r = Math.sqrt(1 - y * y), t = i * 2.399963;
      const R = 165 + (rnd() < 0.08 ? rnd() * 120 : 0);
      a[o] = Math.cos(t) * r * R + W() * 0.32; a[o + 1] = y * R + 150; a[o + 2] = Math.sin(t) * r * R;
    }),
    wave: () => { const s = Math.ceil(Math.sqrt(N)); return make((i, a, o) => {
      const gx = (i % s) / s - 0.5, gz = Math.floor(i / s) / s - 0.5;
      a[o] = gx * W() * 1.5; a[o + 2] = gz * 1400 - 200;
      a[o + 1] = -halfH() * 0.55 + Math.sin(gx * 11) * Math.cos(gz * 7) * 70 + noise2(gx * 3, gz * 3) * 40;
    }); },
    knot: () => make((i, a, o) => {
      const t = (i / N) * TAU, p = 2, q = 5, R = 230, rr = 95;
      const r = R + rr * Math.cos(q * t), tube = 16 * Math.sqrt(rnd()), ang = rnd() * TAU;
      a[o] = r * Math.cos(p * t) + tube * Math.cos(ang) + W() * 0.2;
      a[o + 1] = r * Math.sin(p * t) + tube * Math.sin(ang);
      a[o + 2] = rr * 1.6 * Math.sin(q * t) + tube * Math.cos(ang * 1.3);
    }),
    helix: () => make((i, a, o) => {
      const strand = i % 3, t = (rnd() - 0.5) * W() * 1.2, ph = t * 0.012 + (strand === 2 ? 0 : strand * Math.PI);
      const R = strand === 2 ? rnd() * 110 : 110;
      if (strand === 2 && rnd() < 0.85) { const k = Math.round(t / 40) * 40, ph2 = k * 0.012, s = rnd() * 2 - 1; a[o] = k; a[o + 1] = Math.cos(ph2) * 110 * s - 40; a[o + 2] = Math.sin(ph2) * 110 * s; return; }
      a[o] = t; a[o + 1] = Math.cos(ph) * R - 40 + gauss() * 4; a[o + 2] = Math.sin(ph) * R + gauss() * 4;
    }),
    chains: () => make((i, a, o) => {
      // Hanging chains (catenaries) — the shape of a physics solver at rest.
      const k = i % 9, span = W() * 1.1, x0 = -span / 2 + (k * span) / 9, w = span / 9 * (1.6 + (k % 3) * 0.5);
      const u = rnd() - 0.5, x = u * w, c = w * (0.55 + (k % 4) * 0.08);
      a[o] = x0 + span / 18 + x + gauss() * 1.5;
      a[o + 1] = halfH() * 1.05 - (c * (Math.cosh(w / 2 / c) - Math.cosh(x / c))) - 40 * (k % 2) + gauss() * 1.5;
      a[o + 2] = (k - 4) * 60 + gauss() * 1.5;
    }),
    ground: () => make((i, a, o) => {
      const x = (rnd() - 0.5) * W() * 1.4, h = Math.max(0, 1 - Math.abs(x) / (W() * 0.7));
      a[o] = x; a[o + 2] = gauss() * 160;
      a[o + 1] = -halfH() * 0.95 + Math.pow(rnd(), 2.2) * 150 * h * (0.6 + 0.4 * Math.cos(x * 0.01));
    }),
    lattice: () => make((i, a, o) => {
      const n = 4, s = 92, e = rnd(), axis = i % 3, c = [Math.floor(rnd() * (n + 1)), Math.floor(rnd() * (n + 1)), Math.floor(rnd() * (n + 1))];
      c[axis] = e * n;
      a[o] = (c[0] - n / 2) * s; a[o + 1] = (c[1] - n / 2) * s; a[o + 2] = (c[2] - n / 2) * s;
      const cy = Math.cos(0.6), sy = Math.sin(0.6), x = a[o], z = a[o + 2];
      a[o] = x * cy + z * sy + W() * 0.24; a[o + 1] += 40; a[o + 2] = -x * sy + z * cy;
      rotX(a, o, -24);
    }),
    vortex: () => make((i, a, o) => {
      const h = rnd(), r = 18 + Math.pow(h, 1.6) * 420 + gauss() * 10, ang = h * 26 + rnd() * TAU * 0.06 + (i % 2) * Math.PI;
      a[o] = Math.cos(ang) * r; a[o + 2] = Math.sin(ang) * r; a[o + 1] = (h - 0.5) * halfH() * 1.9;
    }),
    burst: () => make((i, a, o) => {
      const y = 2 * rnd() - 1, t = rnd() * TAU, r = Math.sqrt(1 - y * y);
      const spike = i % 5 === 0 ? 1 + Math.pow(rnd(), 3) * 2.2 : 1;
      const R = 170 * spike;
      a[o] = r * Math.cos(t) * R + W() * 0.25; a[o + 1] = y * R + 40; a[o + 2] = r * Math.sin(t) * R;
    }),
    rings: () => make((i, a, o) => {
      const k = i % 3, t = rnd() * TAU, R = 150 + k * 70 + gauss() * 3;
      let x = Math.cos(t) * R, y = Math.sin(t) * R, z = gauss() * 3;
      const tilt = [0.3, 1.2, 2.1][k];
      const c = Math.cos(tilt), s = Math.sin(tilt);
      [y, z] = [y * c - z * s, y * s + z * c];
      a[o] = x + W() * 0.22; a[o + 1] = y + 20; a[o + 2] = z;
    }),
    grid: () => { const s = Math.ceil(Math.sqrt(N)); return make((i, a, o) => {
      const gx = (i % s) / s - 0.5, gy = Math.floor(i / s) / s - 0.5;
      a[o] = gx * W() * 2.2; a[o + 1] = gy * 2200; a[o + 2] = -900 + noise2(gx * 6, gy * 6) * 60;
      rotX(a, o, -55);
    }); },
    install: () => make((i, a, o) => {
      // A ring of light around the final call to action, with a dusting of stars.
      if (i % 9 === 0) { a[o] = (rnd() - 0.5) * W() * 1.3; a[o + 1] = (rnd() - 0.5) * halfH() * 2; a[o + 2] = (rnd() - 0.5) * 900; return; }
      const t = rnd() * TAU, ring = i % 3, R = 470 + ring * 26 + gauss() * (ring === 1 ? 10 : 3);
      a[o] = Math.cos(t) * R; a[o + 1] = gauss() * 2; a[o + 2] = Math.sin(t) * R;
      rotX(a, o, 76);
      a[o + 1] += 30;
    }),
  };
  const sway = { galaxy: 1, sphere: 1, knot: 1, lattice: 1, burst: 1, rings: 1, install: 0.5, vortex: 0.6, helix: 0.4, chains: 0.3 };
  const names = Object.keys(shapes);
  const index = Object.fromEntries(names.map((n, i) => [n, i]));
  const cloud = pointCloud(N, { autoStep: false, stiffness: 34, damping: 7.5, spread: 0.75, noise: 2.4, swirl: 70, frequency: 0.012 });
  const built = new Set();
  const build = (name) => {
    cloud.shape(index[name], shapes[name]());
    built.add(name);
  };
  build('origin');
  build('wordmark');
  cloud.snap(index.origin);

  // Colours: periwinkle light with pink, white-hot and amber accents.
  const A = [0.34, 0.42, 1], B = [0.62, 0.42, 1], C = [1, 0.4, 0.72];
  g.colors(N, (i) => {
    const r = rnd();
    if (r < 0.07) return [1, 1, 1];
    if (r < 0.1) return [1, 0.72, 0.4];
    const u = i / N + (rnd() - 0.5) * 0.12;
    const t = Math.min(1, Math.max(0, u));
    const [p, q, f] = t < 0.55 ? [A, B, t / 0.55] : [B, C, (t - 0.55) / 0.45];
    return [p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f, p[2] + (q[2] - p[2]) * f];
  });

  // Build the remaining shapes in idle slices so the first frame is instant.
  const rest = names.filter((n) => !built.has(n));
  const idle = window.requestIdleCallback || ((f) => setTimeout(f, 16));
  const buildNext = () => {
    const n = rest.shift();
    if (!n) return;
    build(n);
    idle(buildNext);
  };
  idle(buildNext);
  addEventListener('resize', () => {
    resize();
    for (const n of names) if (built.has(n) && n !== 'origin') build(n);
  });

  // ---- Stations: scroll position → shape blend -----------------------------
  const stations = Array.from(document.querySelectorAll('.station')).map((el) => ({
    el,
    shape: el.dataset.shape,
    exposure: parseFloat(el.dataset.exposure || '0.5'),
  }));
  let intro = !reduced;
  let exposure = 0;
  let introExposure = { v: reduced ? 1 : 0 };
  const smooth = (t) => t * t * (3 - 2 * t);
  const blend = () => {
    const y = scrollY + vh / 2;
    const maxY = document.documentElement.scrollHeight - vh / 2;
    const c = stations.map((s) => {
      const r = s.el.getBoundingClientRect();
      return Math.min(maxY, r.top + scrollY + Math.min(r.height, vh * 1.2) / 2);
    });
    let k = 0;
    while (k < c.length - 2 && y > c[k + 1]) k++;
    const f = Math.min(1, Math.max(0, (y - c[k]) / Math.max(1, c[k + 1] - c[k])));
    const m = smooth(Math.min(1, Math.max(0, (f - 0.18) / 0.64)));
    return { a: stations[k], b: stations[k + 1], m };
  };

  // ---- Pointer: lens, parallax, shockwave ----------------------------------
  const ptr = { x: -9999, y: -9999, tx: 0, ty: 0, rx: 0, ry: 0 };
  addEventListener('pointermove', (e) => {
    ptr.x = e.clientX;
    ptr.y = e.clientY;
    ptr.tx = (e.clientY / vh - 0.5) * -7;
    ptr.ty = (e.clientX / vw - 0.5) * 10;
  }, { passive: true });
  document.documentElement.addEventListener('pointerleave', () => { ptr.x = ptr.y = -9999; });
  const shock = (x, y, s = 1) => {
    cloud.explode((x - vw / 2) * wpp(), -(y - vh / 2) * wpp(), 0, 1100 * s);
  };
  for (const el of document.querySelectorAll('.hero, .moment')) {
    el.addEventListener('click', (e) => {
      if (e.target.closest('a, button')) return;
      shock(e.clientX, e.clientY);
    });
  }

  // ---- Frame loop -----------------------------------------------------------
  // Frame cost is measured on a real clock even when time is virtualised
  // (frame-exact video capture installs fake timers; pass the real clock in).
  const realNow = globalThis.__ANIKIT_REAL_NOW__ || (() => performance.now());
  const stats = { ms: 0, frames: 0, fps: 60, acc: 0, last: performance.now(), lastFrame: performance.now() };
  let t = 0;
  engine.onFrame((_time, dt) => {
    const now = performance.now();
    const t0 = realNow();
    t += dt / 1000;
    const { a, b, m } = blend();
    if (!intro) {
      cloud.from = index[a.shape];
      cloud.to = index[b.shape];
      cloud.mix = m;
    }
    const targetExp = (a.exposure + (b.exposure - a.exposure) * m) * introExposure.v;
    exposure += (targetExp - exposure) * Math.min(1, dt / 120);
    const sw = (sway[a.shape] ?? 0) * (1 - m) + (sway[b.shape] ?? 0) * m;
    ptr.rx += (ptr.tx - ptr.rx) * 0.05;
    ptr.ry += (ptr.ty - ptr.ry) * 0.05;
    scene.rotation.x = ptr.rx;
    scene.rotation.y = ptr.ry + sw * 22 * Math.sin(t * 0.22);
    cloud.step(dt / 1000);
    g.render(scene, cloud, {
      size: 1.15,
      intensity: small ? 0.5 : 0.3,
      fog: [700, 2600],
      lens: ptr.x > -9000 ? { x: ptr.x, y: ptr.y, radius: 170, strength: 26 } : null,
      // Long trails accumulate more light: normalise so slow-mo isn't blown out.
      exposure: exposure * Math.min(1, (1 - g.options.trail) / 0.5),
      maxRadius: 5,
    });
    stats.acc += realNow() - t0;
    stats.frames++;
    if (now - stats.last > 500) {
      stats.ms = stats.acc / stats.frames;
      stats.fps = Math.round((stats.frames * 1000) / (now - stats.last));
      stats.acc = stats.frames = 0;
      stats.last = now;
    }
  });

  // ---- Intro: a singularity that becomes the wordmark ------------------------
  const playIntro = () => {
    if (reduced) {
      cloud.snap(index.wordmark);
      intro = false;
      return;
    }
    cloud.set({ stiffness: 34 });
    animate(introExposure, { v: [0, 1] }, { duration: 1.4, ease: 'outQuad' });
    setTimeout(() => {
      cloud.explode(0, 0, 0, 2600);
      build('galaxy');
      cloud.from = index.origin;
      cloud.to = index.galaxy;
      animate(cloud, { mix: [0, 1] }, { duration: 1.1, ease: 'outCubic' });
    }, 350);
    setTimeout(() => {
      cloud.from = index.galaxy;
      cloud.to = index.wordmark;
      animate(cloud, { mix: [0, 1] }, { duration: 2.2, ease: 'inOutCubic', onComplete: () => (intro = false) });
    }, 1500);
    // If the visitor scrolls during the intro, hand over immediately.
    addEventListener('scroll', () => (intro = false), { once: true, passive: true });
  };

  return { cloud, glow: g, scene, stats, count: N, playIntro, shock };
}

// ---------------------------------------------------------------------------
// Bullet time: hold Space (or a button) and the whole engine slows down.
// ---------------------------------------------------------------------------
export function bulletTime({ glow: g, buttons = [], vignette }) {
  let target = 1;
  let speed = 1;
  let raf = 0;
  const loop = () => {
    speed += (target - speed) * 0.12;
    if (Math.abs(target - speed) < 0.002) speed = target;
    engine.speed = speed;
    const k = (1 - speed) / 0.96;
    g.options.trail = 0.5 + 0.42 * k;
    if (vignette) vignette.style.opacity = String(k);
    raf = speed !== target ? requestAnimationFrame(loop) : 0;
  };
  const go = (v) => {
    target = v;
    for (const b of buttons) b.classList.toggle('on', v < 1);
    if (!raf) raf = requestAnimationFrame(loop);
  };
  addEventListener('keydown', (e) => {
    if (e.code !== 'Space' || e.repeat || e.target.closest?.('input, textarea, [contenteditable]')) return;
    e.preventDefault();
    go(0.04);
  });
  addEventListener('keyup', (e) => e.code === 'Space' && go(1));
  for (const b of buttons) {
    b.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      b.setPointerCapture?.(e.pointerId);
      go(0.04);
    });
    for (const ev of ['pointerup', 'pointercancel', 'lostpointercapture']) b.addEventListener(ev, () => go(1));
  }
  addEventListener('blur', () => go(1));
  return { get speed() { return speed; } };
}
