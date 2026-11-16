// The demos along the descent. Each starts when its section is near and runs
// per-frame work only while visible, so it is already alive when the light
// reaches it.
import {
  engine, animate, set, spring, scroll, stagger, splitText, inView, magnetic, timeline, world,
  circle, star, pathThrough, noise2, pointCloud, glow, scene3d, flip, effects,
} from '../../../packages/anikit/dist/anikit.lean.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const TAU = Math.PI * 2;

function whileVisible(el, fn) {
  inView(el, () => {
    const off = engine.onFrame((_t, dt) => fn(Math.min(dt, 50)));
    return () => off();
  }, { amount: 0 });
}
const once = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 60% 0px' });

function glowSprite(r, rgb) {
  const c = document.createElement('canvas');
  c.width = c.height = r * 2;
  const g = c.getContext('2d');
  const gr = g.createRadialGradient(r, r, 0, r, r, r);
  gr.addColorStop(0, 'rgba(255,255,255,1)');
  gr.addColorStop(0.18, `rgba(${rgb},0.95)`);
  gr.addColorStop(0.45, `rgba(${rgb},0.28)`);
  gr.addColorStop(1, `rgba(${rgb},0)`);
  g.fillStyle = gr;
  g.fillRect(0, 0, r * 2, r * 2);
  return c;
}

export function start({ reduced, fine, small }) {
  // Springs: a siphonophore, a chain of lights each springing after the one ahead.
  once('#springs', () => {
    const box = $('#spring-box');
    const presets = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
    let cfg = presets[0];
    const n = 14;
    const beads = Array.from({ length: n }, (_, i) => {
      const d = document.createElement('i');
      d.className = 'bead';
      d.style.opacity = String(1 - i * 0.055);
      d.style.scale = String(1.25 - i * 0.055);
      box.appendChild(d);
      return d;
    });
    const pos = beads.map(() => ({ x: 0, y: 0 }));
    let bw = box.clientWidth, bh = box.clientHeight;
    const aim = { x: bw / 2, y: bh / 2 };
    pos.forEach((p) => { p.x = aim.x; p.y = aim.y; });
    let inside = false;
    addEventListener('resize', () => { bw = box.clientWidth; bh = box.clientHeight; });
    box.addEventListener('pointermove', (e) => { const r = box.getBoundingClientRect(); inside = true; aim.x = e.clientX - r.left; aim.y = e.clientY - r.top; });
    box.addEventListener('pointerleave', () => { inside = false; });
    let idle = 0;
    whileVisible(box, (dt) => {
      if (!inside && !reduced) {
        idle += dt / 1000;
        aim.x = bw / 2 + Math.sin(idle * 0.9) * bw * 0.34 + noise2(idle * 0.4, 3) * 30;
        aim.y = bh / 2 + Math.sin(idle * 1.8) * bh * 0.26 + noise2(5, idle * 0.4) * 20;
      }
      spring(pos[0], aim, cfg);
      for (let i = 1; i < n; i++) spring(pos[i], pos[i - 1], cfg);
      for (let i = 0; i < n; i++) set(beads[i], { x: pos[i].x, y: pos[i].y });
    });
    $$('#springs .mag').forEach((b, i) => b.addEventListener('click', () => { cfg = presets[i]; effects.pop(b, { duration: 0.3 }); }));
    if (fine) magnetic('#springs .mag, #copy', { strength: 0.3, radius: 70 });
  });

  // Split text: a timeline that lights the sentence letter by letter, holds, lets it sink, and again.
  once('#text', () => {
    const el = $('#reveal');
    const { chars } = splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' });
    set(chars, { opacity: 0, yPercent: 60 });
    const tl = timeline({ repeat: reduced ? 0 : Infinity, repeatDelay: 0.6, autoplay: false })
      .add(chars, { opacity: [0, 1], yPercent: [60, 0], color: ['#5effd0', '#eafff8'] }, { duration: 1.2, ease: 'outCubic', delay: stagger(0.035) })
      .add(chars, { opacity: [1, 0.15], yPercent: [0, -30], color: ['#eafff8', '#5ab8ff'] }, { duration: 1.4, ease: 'inOutSine', delay: stagger(0.025, { from: 'random' }) }, '+=2.6');
    inView(el, () => { tl.play(); return () => tl.pause(); }, { amount: 0.3 });
  });

  // SVG morph: scrubbed by the scroll through this stop.
  once('#svg', () => {
    const blobShape = (seed, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
      const a = (k / n) * TAU, r = 82 + noise2(k * 0.6, seed) * 34;
      return [150 + Math.cos(a) * r, 150 + Math.sin(a) * r];
    }), { closed: true });
    const shapes = [circle(150, 150, 80), star(150, 150, 14, 118, 62), blobShape(2), star(150, 150, 6, 112, 52), blobShape(7, 11), star(150, 150, 24, 110, 84), circle(150, 150, 80)];
    for (const p of ['#blob', '#blob-glow']) $(p).setAttribute('d', shapes[0]);
    const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } });
    for (let k = 1; k < shapes.length; k++) tl.add(['#blob', '#blob-glow'], { d: shapes[k] }, k - 1);
    tl.add('.svg-demo', { rotate: [0, 120] }, { duration: shapes.length - 1, ease: 'linear' }, 0);
    scroll(tl, { target: $('#svg'), offset: ['start start', 'end end'], smooth: 0.2 });
    if (!reduced) animate('#blob-glow', { opacity: [0.45, 0.95] }, { duration: 1.8, ease: 'inOutSine', repeat: Infinity, yoyo: true });
  });

  // Physics: glowing bodies that float up, collide and can be thrown.
  once('#physics', () => {
    const canvas = $('#world-canvas');
    const ctx = canvas.getContext('2d');
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const cols = ['140,230,255', '185,150,255', '120,255,215', '255,160,230'];
    const sprites = cols.map((c) => glowSprite(48, c));
    let W = 0, H = 0, w = null, kinds = [];
    const build = () => {
      W = canvas.clientWidth; H = canvas.clientHeight;
      canvas.width = Math.round(W * dpr); canvas.height = Math.round(H * dpr);
      w?.destroy();
      w = world({ bounds: [0, 0, W, H], gravity: [0, -520], substeps: 4, iterations: 3, restitution: 0.35, friction: 0.04, damping: 0.03 });
      const n = small ? 34 : 56;
      const bodies = Array.from({ length: n }, (_, i) => ({ x: 20 + Math.random() * (W - 40), y: H * 0.5 + Math.random() * H * 0.5, r: (small ? 7 : 9) + Math.pow(Math.random(), 2.2) * (small ? 15 : 22) }));
      w.addMany(bodies);
      kinds = bodies.map((_, i) => i % sprites.length);
      w.dragging({ hitSlop: 12 });
      if (fine) w.pointer({ mode: 'repel', radius: 90 });
    };
    let t = 0;
    inView(canvas, () => { if (!w) build(); }, { amount: 0 });
    addEventListener('resize', () => { if (w && canvas.clientWidth !== W) build(); });
    canvas.addEventListener('click', (e) => { const r = canvas.getBoundingClientRect(); w?.impulse(e.clientX - r.left, e.clientY - r.top, 160, 1800); });
    whileVisible(canvas, (dt) => {
      if (!w) return;
      t += dt / 1000;
      // The current turns slowly: the bodies sink, then rise again.
      if (!reduced) { const a = Math.sin(t * 0.45); w.set({ gravity: [Math.sin(t * 0.21) * 160, -520 * Math.sign(a) * Math.min(1, Math.abs(a) * 3)], substeps: 4, iterations: 3, restitution: 0.35, friction: 0.04, damping: 0.03 }); }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);
      ctx.globalCompositeOperation = 'lighter';
      for (let i = 0; i < w.count; i++) {
        const r = w.radius(i), x = w.x(i), y = w.y(i);
        const pulse = 0.75 + 0.25 * Math.sin(t * 2 + i);
        ctx.globalAlpha = pulse;
        ctx.drawImage(sprites[kinds[i]], x - r * 2.4, y - r * 2.4, r * 4.8, r * 4.8);
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = 'source-over';
    });
  });

  // FLIP: a grid of lit cells that reorders itself.
  once('#layout', () => {
    const grid = $('#tiles');
    const cols = ['#5effd0', '#7ff6ff', '#8fb4ff', '#b98cff', '#ff8fd8', '#9cffd9'];
    for (let i = 0; i < 12; i++) {
      const t = document.createElement('div');
      t.className = 'tile';
      t.style.setProperty('--c', cols[i % cols.length]);
      t.textContent = String(i + 1).padStart(2, '0');
      grid.appendChild(t);
    }
    const opts = { duration: 0.9, ease: 'spring(bounce: 0.18, duration: 0.8)', delay: stagger(0.03) };
    const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
    const regrid = () => flip('.tile', () => { grid.classList.toggle('alt'); $$('.tile', grid)[0].classList.toggle('big'); }, opts);
    grid.addEventListener('click', shuffle);
    let n = 0, id = 0;
    if (!reduced) inView(grid, () => { id = setInterval(() => (++n % 3 ? shuffle() : regrid()), 2800); return () => clearInterval(id); }, { amount: 0.3 });
  });

  // 100,000 particles: a plankton bloom. Built in idle time, long before it's reached.
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  let seed = 7;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const shapes = [
    // A jellyfish of light: a bell and trailing tentacles.
    (i, a, o) => {
      if (i % 3) {
        const u = rnd(), th = rnd() * TAU, phi = u * 1.35, R = 230 + rnd() * 10;
        a[o] = Math.sin(phi) * Math.cos(th) * R; a[o + 1] = Math.cos(phi) * R * 0.75 - 60; a[o + 2] = Math.sin(phi) * Math.sin(th) * R;
      } else {
        const k = Math.floor(rnd() * 14), th = (k / 14) * TAU, s = rnd(), L = 420;
        const r0 = 205 * (1 - s * 0.6);
        const wv = Math.sin(s * 7 + k) * 22 * s; a[o] = Math.cos(th) * (r0 + wv); a[o + 1] = -120 - s * L; a[o + 2] = Math.sin(th) * (r0 + wv);
      }
    },
    // A diffuse bloom: a cloud, denser at the core.
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 300 * Math.pow(rnd(), 0.55), q = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * q * r; a[o + 1] = u * r * 0.7; a[o + 2] = Math.sin(th) * q * r; },
    // A spiral, like a current gathering it up.
    (i, a, o) => { const arm = i % 3, s = rnd(), th = s * 4.2 + (arm * TAU) / 3, R = 40 + s * 330, j = 26 * (1 - s * 0.5); a[o] = Math.cos(th) * R + (rnd() - 0.5) * j * 2; a[o + 1] = (rnd() - 0.5) * j * 1.6; a[o + 2] = Math.sin(th) * R + (rnd() - 0.5) * j * 2; },
  ];
  let cloud = null, g = null;
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(N, { autoStep: false, stiffness: 26, damping: 7, spread: 0.85, noise: 1.8, swirl: 55, frequency: 0.01 }); return true; });
  shapes.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((deadline) => {
      buf ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); }
      if (i < N) return false;
      cloud.shape(k, buf);
      if (k === 0) cloud.snap(0);
      return true;
    });
  });
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.8, background: '#000000', trail: 0.55, budget: 4 }); return true; });
  {
    let i = 0, cols = null;
    jobs.push((deadline) => {
      cols ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) {
        const stop = Math.min(N, i + 4000);
        for (; i < stop; i++) {
          const k = rnd(), sp = k < 0.03 ? 2.2 : 0.7 + k * 0.4, v = rnd();
          // Mostly blue-green, some cyan, a few violet.
          const c = v < 0.6 ? [0.25, 1.0, 0.8] : v < 0.9 ? [0.35, 0.8, 1.0] : [0.7, 0.5, 1.0];
          cols[i * 3] = c[0] * sp; cols[i * 3 + 1] = c[1] * sp; cols[i * 3 + 2] = c[2] * sp;
        }
      }
      if (i < N) return false;
      g.colors(N, (j) => [cols[j * 3], cols[j * 3 + 1], cols[j * 3 + 2]]);
      return true;
    });
  }
  const cloudReady = new Promise((resolve) => {
    const idle = window.requestIdleCallback || ((f) => setTimeout(() => f({ timeRemaining: () => 8 }), 30));
    const run = (deadline) => {
      if (!jobs.length) return resolve();
      if (jobs[0](deadline)) jobs.shift();
      idle(run, { timeout: 400 });
    };
    idle(run, { timeout: 400 });
  });
  once('#cloud', () => cloudReady.then(() => {
    $('#cloud-n').textContent = N.toLocaleString('en-US');
    $('#cloud-code').textContent = `pointCloud(${N.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2 })`;
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1100 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    let next = 1, timer = 2, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    const go = () => { cloud.morph(next, { duration: 3, ease: 'inOutCubic' }); next = (next + 1) % 3; timer = 0; };
    canvas.addEventListener('click', () => { cloud.explode(0, 0, 0, 700); go(); });
    whileVisible(canvas, (dt) => {
      timer += dt / 1000;
      cloud.step(dt / 1000);
      if (!reduced) rot += dt / 1000;
      if (timer > 6.5 && !reduced) go();
      scene.camera.x = Math.sin(rot * 0.18) * 1100; scene.camera.z = Math.cos(rot * 0.18) * 1100; scene.camera.y = -160;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.3, fog: [700, 2300], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));

  // Benchmarks: counters and bars come up once.
  once('#numbers', () => {
    const section = $('#numbers');
    const counts = $$('.n-count', section);
    const bars = $$('.n-row u', section);
    set(bars, { scaleX: 0 });
    for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
    inView($('.n-stats', section), () => {
      animate(bars, { scaleX: 1 }, { duration: 1.8, ease: 'outCubic', delay: stagger(0.2, { start: 0.3 }) });
      counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 2.2, ease: 'outExpo', delay: 0.2 + i * 0.1, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
    }, { amount: 0.5, once: true });
  });

  // Install: the command lights up character by character; copy button.
  once('#install', () => {
    const { chars } = splitText('#cmd', { type: 'chars', charClass: 'ch' });
    set(chars, { opacity: 0.08 });
    inView('#cmd', () => {
      animate(chars, { opacity: [0.08, 1] }, { duration: 1.6, ease: 'inOutSine', delay: stagger(0.07, { from: 'random' }) });
    }, { amount: 1, once: true });
    const copy = $('#copy');
    copy.addEventListener('click', async () => {
      try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
      copy.textContent = 'Copied';
      effects.pop(copy, { duration: 0.35 });
      setTimeout(() => (copy.textContent = 'Copy'), 2000);
    });
  });
}
