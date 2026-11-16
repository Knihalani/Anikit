// The demos. Each starts when its section approaches and runs per-frame work only
// while visible. All of them stay alive without any interaction.
import {
  engine, animate, set, spring, scroll, stagger, splitText, inView, magnetic, timeline, world, flush,
  circle, star, rect, pathThrough, noise2, pointCloud, glow, scene3d, flip, effects,
} from '../../../packages/anikit/dist/anikit.lean.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const TAU = Math.PI * 2;

function whileVisible(el, fn) {
  inView(el, () => {
    const off = engine.onFrame((_t, dt) => fn(dt));
    return () => off();
  }, { amount: 0 });
}
const once = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 50% 0px' });

export function start({ reduced, fine, small }) {
  // ---- Physics: the letters of AURORA as rigid bodies; a gust lifts them every few seconds.
  once('#physics', () => {
    const box = $('#world-box');
    const hues = ['#7dffb4', '#5ff0d6', '#9b8cff', '#ff7aa8', '#7dffb4', '#b8ffcf'];
    const letters = 'AURORA'.split('').map((ch, i) => {
      const s = document.createElement('div');
      s.className = 'w-letter'; s.textContent = ch; s.style.setProperty('--c', hues[i]);
      box.appendChild(s); return s;
    });
    let w = null;
    const bodies = [];
    const build = () => {
      const Wd = box.clientWidth, Hd = box.clientHeight;
      w = world({ container: box, bounds: [0, -2000, Wd, Hd - 10], gravity: [0, 2200], substeps: 4, iterations: 3, restitution: 0.3, friction: 0.15, damping: 0.02 });
      letters.forEach((el, i) => set(el, { x: (Wd / (letters.length + 1)) * (i + 1) - el.offsetWidth / 2, y: -el.offsetHeight - i * 60 }));
      flush();
      const host = box.getBoundingClientRect();
      for (const el of letters) {
        const b = el.getBoundingClientRect();
        const capH = b.height * 0.72, cw = b.width * 0.84, r = Math.min(cw, capH) * 0.2;
        const cx = b.left - host.left + b.width / 2, cy = b.top - host.top + b.height * 0.535;
        const dx = cw / 2 - r, dy = capH / 2 - r;
        const ids = [[-dx, -dy], [dx, -dy], [dx, dy], [-dx, dy]].map(([ox, oy]) => w.add(cx + ox, cy + oy, r, 30));
        for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) w.link(ids[i], ids[j], { stiffness: 1 });
        const vx = (Math.random() - 0.5) * 220;
        for (const id of ids) w.setVelocity(id, vx, 0);
        bodies.push({ el, ids, w: b.width, h: b.height });
      }
      w.onStep(() => {
        for (const o of bodies) {
          const [a, b, c, d] = o.ids;
          const x = (w.x(a) + w.x(b) + w.x(c) + w.x(d)) / 4, y = (w.y(a) + w.y(b) + w.y(c) + w.y(d)) / 4;
          const ang = Math.atan2(w.y(c) - w.y(d) + w.y(b) - w.y(a), w.x(c) - w.x(d) + w.x(b) - w.x(a));
          set(o.el, { x: x - o.w / 2, y: y - o.h * 0.535, rotate: (ang * 180) / Math.PI });
        }
      });
      w.dragging({ hitSlop: 16 });
      box.addEventListener('click', (e) => { const r = box.getBoundingClientRect(); w.impulse(e.clientX - r.left, e.clientY - r.top, 220, 2400); });
      // A gust from below, somewhere along the floor, every few seconds.
      let t = 0, k = 0;
      if (!reduced) whileVisible(box, (dt) => {
        t += dt / 1000;
        if (t > 4.2) { t = 0; k++; const x = Wd * (0.2 + 0.6 * ((k * 0.618) % 1)); w.impulse(x, Hd + 20, Wd * 0.32, 1900); }
      });
    };
    inView(box, () => { if (!w) build(); }, { amount: 0.4 });
  });

  // ---- 100,000 particles: an aurora curtain, a sphere and a ring, coloured by altitude.
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  let seed = 3;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const heights = new Float32Array(N);
  const shapes = [
    // curtain: a folded sheet, dense at its lower edge, fading upward
    (i, a, o) => {
      const u = i / N, x = (u - 0.5) * 760;
      const h = Math.pow(rnd(), 1.8);
      heights[i] = h;
      const fold = Math.sin(x * 0.012) * 60 + Math.sin(x * 0.031 + 1) * 22;
      const xr = x + Math.sin(x * 0.11) * 5; a[o] = xr + (rnd() - 0.5) * 4; a[o + 1] = -170 + h * 360 + Math.sin(x * 0.02) * 16; a[o + 2] = fold * 1.6 + (rnd() - 0.5) * 8;
    },
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 210 * Math.cbrt(0.85 + rnd() * 0.15), q = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * q * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * q * r; },
    (i, a, o) => { const th = rnd() * TAU, ph = rnd() * TAU, r = 60 * Math.sqrt(rnd()); const R = 240 + Math.cos(ph) * r; a[o] = Math.cos(th) * R; a[o + 1] = Math.sin(ph) * r; a[o + 2] = Math.sin(th) * R; },
  ];
  let cloud = null, g = null;
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.4, swirl: 50, frequency: 0.012 }); return true; });
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
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.75, background: '#000000', trail: 0.45, budget: 4 }); return true; });
  jobs.push(() => {
    // 557.7 nm green at the base, a violet fringe below it, 630 nm red on top
    g.colors(N, (j) => {
      const h = heights[j], sp = (j * 2654435761 >>> 0) % 100 < 3 ? 1.7 : 1;
      if (h < 0.04) return [0.6 * sp, 0.35 * sp, 1.1 * sp];
      const r = Math.max(0, (h - 0.45) / 0.55);
      return [(0.25 + 0.95 * r) * sp, (1.05 - 0.9 * r) * sp, (0.5 - 0.2 * r) * sp];
    });
    return true;
  });
  const cloudReady = new Promise((resolve) => {
    const idle = window.requestIdleCallback || ((f) => setTimeout(() => f({ timeRemaining: () => 8 }), 30));
    const run = (deadline) => { if (!jobs.length) return resolve(); if (jobs[0](deadline)) jobs.shift(); idle(run, { timeout: 400 }); };
    idle(run, { timeout: 400 });
  });
  once('#cloud', () => cloudReady.then(() => {
    $('#cloud-n').textContent = N.toLocaleString('en-US');
    $('#cloud-code').textContent = `pointCloud(${N.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2 })`;
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    let next = 1, timer = 0, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    const go = () => { cloud.morph(next, { duration: 2.4, ease: 'inOutCubic' }); next = (next + 1) % 3; timer = 0; };
    canvas.addEventListener('click', () => { cloud.explode(0, 0, 0, 600); go(); });
    whileVisible(canvas, (dt) => {
      const s = Math.min(dt, 50) / 1000;
      timer += s;
      cloud.step(s);
      if (!reduced) rot += s;
      if (timer > 5.5 && !reduced) go();
      scene.camera.x = Math.sin(rot * 0.22) * 1000; scene.camera.z = Math.cos(rot * 0.22) * 1000; scene.camera.y = -120;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.3, fog: [600, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));

  // ---- Split text: a looping timeline — letters rise, a light passes over them, they sink.
  once('#text', () => {
    const el = $('#reveal');
    const { chars } = splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' });
    set(chars, { yPercent: 110, opacity: 0 });
    const tl = timeline({ repeat: Infinity, repeatDelay: 0.6, autoplay: false })
      .add(chars, { yPercent: [110, 0], rotate: [8, 0], opacity: [0, 1] }, { duration: 1.1, ease: 'spring(bounce: 0.3, duration: 0.9)', delay: stagger(0.022) }, 0)
      .add(chars, { color: ['#e9f3f2', '#7dffb4', '#e9f3f2'] }, { duration: 1.2, ease: 'inOutSine', delay: stagger(0.03) }, 1.6)
      .add(chars, { yPercent: [0, -110], opacity: [1, 0] }, { duration: 0.7, ease: 'inCubic', delay: stagger(0.012, { from: 'last' }) }, 4.4);
    inView(el, () => { tl.play(); return () => tl.pause(); }, { amount: 0.5 });
  });

  // ---- Springs: a chain of dots, each springing towards the one ahead.
  once('#springs', () => {
    const box = $('#spring-box');
    const presets = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
    let cfg = presets[0];
    const n = 12;
    const dots = Array.from({ length: n }, (_, i) => {
      const d = document.createElement('i');
      d.className = 'dot';
      d.style.opacity = String(1 - i * 0.07);
      d.style.scale = String(1 - i * 0.05);
      d.style.setProperty('--c', `hsl(${255 + i * 9} 100% ${72 - i * 1.5}%)`);
      box.appendChild(d);
      return d;
    });
    const pos = dots.map(() => ({ x: 0, y: 0 }));
    let bw = box.clientWidth, bh = box.clientHeight;
    const aim = { x: bw / 2, y: bh / 2 };
    pos.forEach((p) => { p.x = aim.x; p.y = aim.y; });
    addEventListener('resize', () => { bw = box.clientWidth; bh = box.clientHeight; });
    let inside = false, idle = 0, auto = 0;
    box.addEventListener('pointermove', (e) => { const r = box.getBoundingClientRect(); inside = true; aim.x = e.clientX - r.left; aim.y = e.clientY - r.top; });
    box.addEventListener('pointerleave', () => { inside = false; });
    whileVisible(box, (dt) => {
      if (!inside && !reduced) {
        idle += dt / 1000; auto += dt / 1000;
        aim.x = bw / 2 + Math.sin(idle * 1.2) * bw * 0.34; aim.y = bh / 2 + Math.sin(idle * 2.4) * bh * 0.26;
        if (auto > 4) { auto = 0; cfg = presets[(presets.indexOf(cfg) + 1) % 3]; $$('#springs .mag').forEach((b, i) => b.classList.toggle('on', presets[i] === cfg)); }
      }
      spring(pos[0], aim, cfg);
      for (let i = 1; i < n; i++) spring(pos[i], pos[i - 1], cfg);
      for (let i = 0; i < n; i++) set(dots[i], { x: pos[i].x, y: pos[i].y });
    });
    $$('#springs .mag').forEach((b, i) => b.addEventListener('click', () => { cfg = presets[i]; auto = -6; effects.pop(b, { duration: 0.3 }); }));
    if (fine) magnetic('#springs .mag, #copy', { strength: 0.3, radius: 70 });
  });

  // ---- SVG: the shape morphs as you scroll through the section; a rider keeps circling.
  once('#svg', () => {
    const blobShape = (seedv, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
      const a = (k / n) * TAU, r = 80 + noise2(k * 0.6, seedv) * 34;
      return [260 + Math.cos(a) * r, 150 + Math.sin(a) * r];
    }), { closed: true });
    const shapes = [circle(260, 150, 90), star(260, 150, 10, 110, 66), blobShape(2), rect(170, 60, 180, 180, 28), star(260, 150, 5, 112, 46), blobShape(7, 11), circle(260, 150, 90)];
    $('#blob').setAttribute('d', shapes[0]);
    const morph = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } });
    for (let k = 1; k < shapes.length; k++) morph.add('#blob', { d: shapes[k] }, k - 1);
    morph.add('#trail', { draw: [0, 1] }, { duration: shapes.length - 1, ease: 'linear' }, 0);
    scroll(morph, { target: $('#svg'), offset: ['start 20%', 'end 80%'], smooth: 0.2 });
    const ride = animate('#rider', { path: { path: '#trail' } }, { duration: 6, ease: 'inOutSine', repeat: Infinity, yoyo: true, autoplay: false });
    inView('#svg', () => { ride.play(); return () => ride.pause(); }, { amount: 0 });
  });

  // ---- FLIP: the grid reshuffles and changes layout by itself.
  once('#layout', () => {
    const grid = $('#tiles');
    const cols = ['#7dffb4', '#5ff0d6', '#4fc3ff', '#9b8cff', '#ff7aa8', '#ff8f6b'];
    for (let i = 0; i < 12; i++) {
      const t = document.createElement('div');
      t.className = 'tile';
      t.style.setProperty('--t', cols[i % cols.length]);
      t.textContent = String(i + 1).padStart(2, '0');
      grid.appendChild(t);
    }
    const opts = { duration: 0.9, ease: 'spring(bounce: 0.2, duration: 0.8)', delay: stagger(0.025) };
    const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
    const regrid = () => flip('.tile', () => { grid.classList.toggle('alt'); $$('.tile', grid)[0].classList.toggle('big'); }, opts);
    $('#shuffle').addEventListener('click', shuffle);
    $('#regrid').addEventListener('click', regrid);
    grid.addEventListener('click', shuffle);
    let n = 0, id = 0;
    if (!reduced) inView(grid, () => { id = setInterval(() => (++n % 3 ? shuffle() : regrid()), 2600); return () => clearInterval(id); }, { amount: 0.5 });
  });

  // ---- Benchmarks: counters and bars run in once.
  once('#numbers', () => {
    const section = $('#numbers');
    const counts = $$('.n-count', section);
    const bars = $$('.n-row u', section);
    set(bars, { scaleX: 0 });
    for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
    inView($('.panel', section), () => {
      animate(bars, { scaleX: 1 }, { duration: 1.6, ease: 'spring(bounce: 0.15, duration: 1.3)', delay: stagger(0.18, { start: 0.3 }) });
      counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 1.8, ease: 'outExpo', delay: 0.3 + i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
    }, { amount: 0.6, once: true });
  });

  // ---- Install: copy button.
  const copy = $('#copy');
  copy.addEventListener('click', async () => {
    try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
    copy.textContent = 'Copied';
    effects.pop(copy, { duration: 0.35 });
    setTimeout(() => { copy.textContent = 'Copy'; }, 1800);
  });
}
