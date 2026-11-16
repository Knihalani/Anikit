// The seven demos beside the knot's pieces. Each one starts when its section is
// first near the viewport and runs per-frame work only while it is visible.
import {
  engine, animate, set, spring, stagger, splitText, inView, magnetic, timeline, world, flush,
  circle, star, rect, pathThrough, noise2, pointCloud, glow, scene3d, flip, marquee, effects,
} from '../packages/anikit/dist/anikit.lean.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

function whileVisible(el, fn) {
  let off = null;
  inView(el, () => {
    off = engine.onFrame((_t, dt) => fn(dt));
    return () => { off?.(); off = null; };
  }, { amount: 0 });
}
const once = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 40% 0px' });

export function start({ reduced, fine, small }) {
  // 01 · Springs: a chain of dots, each springing towards the one ahead.
  once('#springs', () => {
    const box = $('#spring-box');
    const presets = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
    let cfg = presets[0];
    const n = 10;
    const dots = Array.from({ length: n }, (_, i) => {
      const d = document.createElement('i');
      d.className = 'dot';
      d.style.opacity = String(1 - i * 0.075);
      d.style.scale = String(1 - i * 0.05);
      box.appendChild(d);
      return d;
    });
    const pos = dots.map(() => ({ x: 0, y: 0 }));
    const aim = { x: 0, y: 0 };
    const home = () => { aim.x = box.clientWidth / 2; aim.y = box.clientHeight / 2; };
    home();
    pos.forEach((p) => { p.x = aim.x; p.y = aim.y; });
    let inside = false, bw = box.clientWidth, bh = box.clientHeight;
    addEventListener('resize', () => { bw = box.clientWidth; bh = box.clientHeight; });
    box.addEventListener('pointermove', (e) => { const r = box.getBoundingClientRect(); inside = true; aim.x = e.clientX - r.left; aim.y = e.clientY - r.top; });
    box.addEventListener('pointerleave', () => { inside = false; });
    let idle = 0;
    whileVisible(box, (dt) => {
      // Without a pointer, the head draws a slow figure eight.
      // Without the pointer inside, the head traces a slow figure eight.
      if (!inside && !reduced) { idle += dt / 1000; aim.x = bw / 2 + Math.sin(idle * 1.3) * bw * 0.32; aim.y = bh / 2 + Math.sin(idle * 2.6) * bh * 0.25; }
      spring(pos[0], aim, cfg);
      for (let i = 1; i < n; i++) spring(pos[i], pos[i - 1], cfg);
      for (let i = 0; i < n; i++) set(dots[i], { x: pos[i].x, y: pos[i].y });
    });
    $$('#springs .mag').forEach((b, i) => b.addEventListener('click', () => { cfg = presets[i]; effects.pop(b, { duration: 0.3 }); }));
    if (fine) magnetic('#springs .mag, .actions .pill, #copy', { strength: 0.3, radius: 70 });
  });

  // 02 · Text: split, then stagger.
  once('#text', () => {
    const el = $('#reveal');
    const chars = splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;
    const eases = ['spring(bounce: 0.35, duration: 0.8)', 'outExpo', 'outBack', 'outBounce', 'steps(5)'];
    let k = 0;
    const play = () => {
      $('#reveal-ease').textContent = `ease: ${eases[k].split('(')[0]}`;
      animate(chars, { yPercent: [110, 0], rotate: [10, 0], opacity: [0, 1] }, { duration: 0.9, ease: eases[k], delay: stagger(0.016) });
    };
    set(chars, { yPercent: 110, opacity: 0 });
    inView(el, play, { amount: 0.6 });
    $('#reveal-btn').addEventListener('click', () => { k = (k + 1) % eases.length; play(); });
  });

  // 03 · SVG: a looping morph, a stroke drawing on and a rider on the path.
  once('#svg', () => {
    const blobShape = (seed, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
      const a = (k / n) * Math.PI * 2, r = 80 + noise2(k * 0.6, seed) * 34;
      return [150 + Math.cos(a) * r, 150 + Math.sin(a) * r];
    }), { closed: true });
    const shapes = [circle(150, 150, 95), star(150, 150, 10, 110, 66), blobShape(2), rect(60, 60, 180, 180, 28), star(150, 150, 5, 112, 46), blobShape(7, 11), circle(150, 150, 95)];
    $('#blob').setAttribute('d', shapes[0]);
    const tl = timeline({ repeat: Infinity, defaults: { ease: 'spring(bounce: 0.25, duration: 0.9)', duration: 1 } });
    for (let k = 1; k < shapes.length; k++) tl.add('#blob', { d: shapes[k] }, k === 1 ? 0.4 : '+=0.45');
    const draw = timeline({ repeat: Infinity, defaults: { ease: 'inOutCubic' } });
    draw.add('#rider', { opacity: [0, 1] }, { duration: 0.3 }, 0)
      .add('#trail', { draw: [0, 1] }, { duration: 2.2 }, 0)
      .add('#rider', { path: { path: '#trail' } }, { duration: 2.2 }, 0)
      .add('#rider', { opacity: 0, scale: 0.4 }, { duration: 0.35, ease: 'inCubic' }, 2.6)
      .add('#trail', { draw: [1, 0] }, { duration: 1.2, ease: 'inCubic' }, 2.8)
      .set('#rider', { scale: 1 }, 4.1);
    inView('#svg', () => { tl.play(); draw.play(); return () => { tl.pause(); draw.pause(); }; }, { amount: 0 });
  });

  // 04 · Physics: letters with real bodies in a box.
  once('#physics', () => {
    const box = $('#world-box');
    const letters = 'WORLD'.split('').map((ch) => { const s = document.createElement('div'); s.className = 'w-letter'; s.textContent = ch; box.appendChild(s); return s; });
    let w = null;
    const bodies = [];
    const build = () => {
      const Wd = box.clientWidth, Hd = box.clientHeight;
      w = world({ container: box, bounds: [0, -2000, Wd, Hd - 14], gravity: [0, 2200], substeps: 4, iterations: 3, restitution: 0.3, friction: 0.15, damping: 0.02 });
      letters.forEach((el, i) => set(el, { x: (Wd / (letters.length + 1)) * (i + 1) - el.offsetWidth / 2, y: -el.offsetHeight - i * 70 }));
      flush();
      const host = box.getBoundingClientRect();
      // Each letter is a rigid box: four circles at the corners of its cap-height
      // rectangle, linked to each other, so letters stand, stack and tumble like blocks.
      for (const el of letters) {
        const b = el.getBoundingClientRect();
        const capH = b.height * 0.72, cw = b.width * 0.84, r = Math.min(cw, capH) * 0.2;
        const cx = b.left - host.left + b.width / 2, cy = b.top - host.top + b.height * 0.535;
        const dx = cw / 2 - r, dy = capH / 2 - r;
        const ids = [[-dx, -dy], [dx, -dy], [dx, dy], [-dx, dy]].map(([ox, oy]) => w.add(cx + ox, cy + oy, r, 30));
        for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) w.link(ids[i], ids[j], { stiffness: 1 });
        const vx = (Math.random() - 0.5) * 260;
        for (const id of ids) w.setVelocity(id, vx, 0);
        bodies.push({ el, ids, w: b.width, h: b.height });
      }
      w.onStep(() => {
        for (const o of bodies) {
          const [tl, tr, br, bl] = o.ids;
          const x = (w.x(tl) + w.x(tr) + w.x(br) + w.x(bl)) / 4, y = (w.y(tl) + w.y(tr) + w.y(br) + w.y(bl)) / 4;
          const ang = Math.atan2(w.y(br) - w.y(bl) + w.y(tr) - w.y(tl), w.x(br) - w.x(bl) + w.x(tr) - w.x(tl));
          set(o.el, { x: x - o.w / 2, y: y - o.h * 0.535, rotate: (ang * 180) / Math.PI });
        }
      });
      w.dragging({ hitSlop: 16 });
      box.addEventListener('click', (e) => { const r = box.getBoundingClientRect(); w.impulse(e.clientX - r.left, e.clientY - r.bottom + r.height, 220, 2400); });
    };
    inView(box, () => { if (!w) build(); }, { amount: 0.5 });
  });

  // 05 · Particles: 100,000 points (40,000 on phones) morphing between shapes.
  // The cloud and its shapes are built in idle time, one step per idle slot,
  // long before the section arrives; the section itself only wires up rendering.
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  let seed = 3;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const TAU = Math.PI * 2;
  // Shape generators, filled in slices of a few milliseconds so no frame is ever blocked.
  const shapes = [
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 230 * Math.cbrt(0.85 + rnd() * 0.15), q = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * q * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * q * r; },
    (i, a, o) => { const th = rnd() * TAU, ph = rnd() * TAU, r = 70 * Math.sqrt(rnd()); const R = 250 + Math.cos(ph) * r; a[o] = Math.cos(th) * R; a[o + 1] = Math.sin(ph) * r; a[o + 2] = Math.sin(th) * R; },
    // The same (2, 3) torus knot as the chrome object, as a glowing tube of points.
    (i, a, o) => {
      const t = (i / N) * TAU, r = 150 + 68 * Math.cos(3 * t);
      const ph = rnd() * TAU, rr = 26 * Math.sqrt(rnd());
      a[o] = r * Math.cos(2 * t) + Math.cos(ph) * rr; a[o + 1] = r * Math.sin(2 * t) + Math.sin(ph) * rr; a[o + 2] = -68 * Math.sin(3 * t) + Math.cos(ph * 1.7) * rr;
    },
  ];
  let cloud = null, g = null;
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.6, swirl: 60, frequency: 0.012 }); return true; });
  shapes.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((deadline) => {
      buf ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); }
      if (i < N) return false;
      cloud.shape(k, buf);
      if (k === 2) cloud.snap(2);
      return true;
    });
  });
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.75, background: '#05060a', trail: 0.4, budget: 4 }); return true; });
  {
    let i = 0, cols = null;
    jobs.push((deadline) => {
      cols ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 4000); for (; i < stop; i++) { const u = i / N, k = rnd(); const sp = k < 0.04 ? 1.8 : 0.75 + k * 0.35; cols[i * 3] = (0.55 + 0.45 * Math.cos(TAU * u)) * sp; cols[i * 3 + 1] = (0.45 + 0.4 * Math.cos(TAU * (u - 0.33))) * sp; cols[i * 3 + 2] = (0.75 + 0.25 * Math.cos(TAU * (u - 0.66))) * sp; } }
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
      idle(run, { timeout: 500 });
    };
    idle(run, { timeout: 500 });
  });
  once('#cloud', () => cloudReady.then(() => {
    $('#cloud-n').textContent = N.toLocaleString('en-US');
    $('.snippet', $('#cloud')).textContent = `pointCloud(${N.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2 })`;
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    let next = 1, timer = 1.4, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    canvas.addEventListener('click', () => cloud.explode(0, 0, 0, 900));
    whileVisible(canvas, (dt) => {
      timer += dt / 1000;
      cloud.step(Math.min(dt, 50) / 1000);
      if (!reduced) rot += dt / 1000;
      if (timer > 3.2 && !reduced) { timer = 0; cloud.morph(next, { duration: 2, ease: 'inOutCubic' }); next = (next + 1) % 3; }
      scene.camera.x = Math.sin(rot * 0.25) * 1000; scene.camera.z = Math.cos(rot * 0.25) * 1000;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.32, fog: [600, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));

  // 06 · Layout: FLIP.
  once('#layout', () => {
    const grid = $('#tiles');
    const cols = ['#ff3d8b', '#ff6a3d', '#ffd23d', '#3dffb0', '#3d8bff', '#c86bff'];
    for (let i = 0; i < 12; i++) {
      const t = document.createElement('div');
      t.className = 'tile';
      t.style.setProperty('--c', cols[i % cols.length]);
      t.textContent = String(i + 1).padStart(2, '0');
      grid.appendChild(t);
    }
    const opts = { duration: 0.7, ease: 'spring(bounce: 0.22, duration: 0.6)', delay: stagger(0.02) };
    const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
    const regrid = () => flip('.tile', () => { grid.classList.toggle('alt'); $$('.tile', grid)[0].classList.toggle('big'); }, opts);
    $('#shuffle').addEventListener('click', shuffle);
    $('#regrid').addEventListener('click', regrid);
    let n = 0, id = 0;
    if (!reduced) inView(grid, () => { id = setInterval(() => (++n % 3 ? shuffle() : regrid()), 2600); return () => clearInterval(id); }, { amount: 0.5 });
  });

  // 07 · Benchmarks: counters and bars run in once.
  once('#numbers', () => {
    const section = $('#numbers');
    const counts = $$('.n-count', section);
    const bars = $$('.n-row u', section);
    set(bars, { scaleX: 0 });
    for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
    inView(section, () => {
      animate(bars, { scaleX: 1 }, { duration: 1.4, ease: 'spring(bounce: 0.2, duration: 1.1)', delay: stagger(0.15, { start: 0.2 }) });
      counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 1.6, ease: 'outExpo', delay: 0.2 + i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
    }, { amount: 0.35 });
  });

  // Install: the feature ticker follows the scroll; copy button.
  once('#install', () => {
    marquee('#ticker', { speed: 60, pauseOnHover: true, scrollBoost: 0.6 });
    const copy = $('#copy');
    copy.addEventListener('click', async () => {
      try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
      copy.textContent = 'Copied';
      effects.pop(copy, { duration: 0.35 });
    });
  });
}
