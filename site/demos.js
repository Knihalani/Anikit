// The demos between the watch's beats. Each one starts when its section is first near
// the viewport, runs per-frame work only while it is visible, and is alive on its own.
import {
  engine, animate, set, spring, scroll, stagger, splitText, inView, timeline, world, flush,
  pointCloud, glow, scene3d, flip, effects, magnetic,
} from '../packages/anikit/dist/anikit.lean.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const TAU = Math.PI * 2;
const SVGNS = 'http://www.w3.org/2000/svg';

function whileVisible(el, fn) {
  inView(el, () => {
    const off = engine.onFrame((_t, dt) => fn(dt));
    return () => off();
  }, { amount: 0 });
}
const once = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 60% 0px' });

/** A gear outline as an SVG path (n teeth, pitch radius r). */
const gearPath = (n, r, depth = r * 0.12, hole = r * 0.28) => {
  let d = '';
  for (let j = 0; j < n; j++) {
    const a = (j / n) * TAU, w = TAU / n;
    const pts = [[a, r - depth / 2], [a + w * 0.18, r + depth / 2], [a + w * 0.45, r + depth / 2], [a + w * 0.63, r - depth / 2]];
    for (const [ang, rad] of pts) d += `${d ? 'L' : 'M'}${(Math.cos(ang) * rad).toFixed(2)} ${(Math.sin(ang) * rad).toFixed(2)}`;
  }
  d += 'Z';
  d += `M${hole} 0A${hole} ${hole} 0 1 0 ${-hole} 0A${hole} ${hole} 0 1 0 ${hole} 0Z`;
  return d;
};

export function start({ reduced, fine, small }) {
  // 01 · Springs: drag the crown to set the time; the hands spring to it.
  once('#crown', () => {
    const ticks = $('#ticks');
    for (let m = 0; m < 60; m++) {
      const a = (m / 60) * TAU, five = m % 5 === 0, r0 = five ? 112 : 124;
      const l = document.createElementNS(SVGNS, 'line');
      l.setAttribute('x1', (Math.sin(a) * r0).toFixed(2)); l.setAttribute('y1', (-Math.cos(a) * r0).toFixed(2));
      l.setAttribute('x2', (Math.sin(a) * 131).toFixed(2)); l.setAttribute('y2', (-Math.cos(a) * 131).toFixed(2));
      l.setAttribute('stroke-width', five ? '3.2' : '1.1');
      ticks.appendChild(l);
    }
    const hh = $('#hand-h'), mh = $('#hand-m'), sh = $('#hand-s'), knob = $('#crown-knob'), label = $('#crown-time');
    const hands = { m: 10 * 60 + 10 };
    let aim = hands.m, lastTouch = -1e9, crownY = 0;
    const fmt = (m) => { m = ((Math.round(m) % 720) + 720) % 720; const h = Math.floor(m / 60) || 12; return `${h}:${String(m % 60).padStart(2, '0')}`; };
    const draw = () => {
      set(hh, { rotate: (hands.m / 2) % 360 });
      set(mh, { rotate: (hands.m * 6) % 360 });
    };
    const go = (to, cfg = { stiffness: 170, damping: 14 }) => {
      aim = to;
      label.textContent = fmt(aim); knob.setAttribute('aria-valuetext', fmt(aim));
      spring(hands, { m: aim }, { ...cfg, onUpdate: draw });
    };
    draw();
    let dragging = false, lastY = 0;
    knob.addEventListener('pointerdown', (e) => { dragging = true; lastY = e.clientY; knob.setPointerCapture(e.pointerId); lastTouch = performance.now(); });
    knob.addEventListener('pointermove', (e) => {
      if (!dragging) return;
      const dy = e.clientY - lastY; lastY = e.clientY; lastTouch = performance.now();
      crownY += dy; knob.style.backgroundPositionY = `${crownY}px`;
      go(aim - dy * 1.5);
    });
    const up = () => { dragging = false; };
    knob.addEventListener('pointerup', up); knob.addEventListener('pointercancel', up);
    knob.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowUp' || e.key === 'ArrowRight') { go(aim + 5); e.preventDefault(); }
      if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') { go(aim - 5); e.preventDefault(); }
      lastTouch = performance.now();
    });
    // Left alone, the crown sets a new time every few seconds and the hands spring to it.
    let idle = 0;
    whileVisible('#crown-box', (dt) => {
      const now = new Date();
      const s = now.getSeconds() + now.getMilliseconds() / 1000;
      set(sh, { rotate: reduced ? now.getSeconds() * 6 : (Math.floor(s * 8) / 8) * 6 });
      if (reduced || performance.now() - lastTouch < 4000) return;
      idle += dt / 1000;
      if (idle > 3) {
        idle = 0;
        const to = aim + (60 + Math.round(Math.random() * 30) * 5) * (Math.random() < 0.25 ? -1 : 1);
        animate(knob, { backgroundPositionY: [`${crownY}px`, `${(crownY -= (to - aim) / 1.5)}px`] }, { duration: 0.5, ease: 'outCubic' });
        go(to, { stiffness: 120, damping: 11 });
      }
    });
  });

  // 02 · Text: split, then stagger.
  once('#text', () => {
    const el = $('#reveal');
    const chars = splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;
    const eases = ['spring(bounce: 0.35, duration: 0.8)', 'outExpo', 'outBack', 'outBounce'];
    let k = 0;
    const play = () => {
      $('#reveal-ease').textContent = `ease: ${eases[k].split('(')[0]}`;
      animate(chars, { yPercent: [110, 0], rotate: [8, 0], opacity: [0, 1] }, { duration: 0.9, ease: eases[k], delay: stagger(0.018) });
    };
    set(chars, { yPercent: 110, opacity: 0 });
    let timer = 0;
    inView(el, () => {
      play();
      if (!reduced) timer = setInterval(() => { k = (k + 1) % eases.length; play(); }, 4200);
      return () => clearInterval(timer);
    }, { amount: 0.6 });
    $('#reveal-btn').addEventListener('click', () => { k = (k + 1) % eases.length; play(); });
  });

  // 03 · SVG: a gear train whose outlines draw on and turn as you scroll.
  once('#svg', () => {
    const g = $('#train-gears');
    const specs = [[110, 170, 40, 80], [196, 140, 20, 36], [262, 186, 30, 58], [350, 128, 24, 46], [420, 196, 14, 28], [460, 110, 16, 30]];
    // Turn rates in proportion to the teeth, alternating direction (true meshing ratios).
    let dir = 1, rate = 1;
    const gears = specs.map(([x, y, n, r], i) => {
      const grp = document.createElementNS(SVGNS, 'g');
      grp.setAttribute('transform', `translate(${x} ${y})`);
      const p = document.createElementNS(SVGNS, 'path');
      p.setAttribute('d', gearPath(n, r)); p.setAttribute('class', 'tg');
      grp.appendChild(p); g.appendChild(grp);
      if (i) { rate *= specs[i - 1][2] / n; dir = -dir; }
      return { p, turn: 360 * rate * dir * 0.6 };
    });
    const tl = timeline({ autoplay: false, defaults: { ease: 'linear' } });
    gears.forEach((G, i) => {
      tl.add(G.p, { draw: [0, 1] }, { duration: 0.35 }, i * 0.06);
      tl.add(G.p, { rotate: [0, G.turn] }, { duration: 1 }, 0);
    });
    tl.add('#trail', { draw: [0, 1] }, { duration: 1 }, 0).add('#rider', { path: { path: '#trail' } }, { duration: 1 }, 0);
    scroll(tl, { target: $('#svg'), offset: ['start 60%', 'end 40%'], smooth: 0.2 });
  });

  // 04 · Physics: wheels and screws fall, stack and can be thrown.
  once('#physics', () => {
    const box = $('#world-box');
    let w = null;
    const bodies = [];
    const gearSvg = (n, r, fill) => `<svg viewBox="${-r * 1.12} ${-r * 1.12} ${r * 2.24} ${r * 2.24}"><path d="${gearPath(n, r, r * 0.12, r * 0.22)}" fill="${fill}" fill-rule="evenodd"/><circle r="${r * 0.62}" fill="none" stroke="rgba(0,0,0,.25)" stroke-width="${r * 0.05}"/></svg>`;
    const screwSvg = `<svg viewBox="-12 -12 24 24"><circle r="11" fill="#2f4fb8"/><rect x="-9" y="-1.6" width="18" height="3.2" rx="1" fill="#0b1230"/><circle r="11" fill="none" stroke="#7d97f0" stroke-width="1.2" opacity=".6"/></svg>`;
    const spawn = (i) => {
      const Wd = box.clientWidth;
      const isGear = i % 3 !== 2;
      const r = isGear ? 18 + ((i * 7) % 5) * 7 : 11;
      const el = document.createElement('div');
      el.className = 'body';
      el.style.width = el.style.height = `${r * 2.24}px`;
      el.innerHTML = isGear ? gearSvg(10 + Math.round(r / 2.2), r, i % 2 ? '#d9b277' : '#c9ccd4') : screwSvg;
      box.appendChild(el);
      const id = w.add(40 + Math.random() * (Wd - 80), -60 - Math.random() * 200, r * (isGear ? 1.04 : 1), r * r * 0.02);
      w.setVelocity(id, (Math.random() - 0.5) * 300, 0);
      bodies.push({ el, id, r: r * 1.12 });
    };
    const build = () => {
      const Wd = box.clientWidth, Hd = box.clientHeight;
      w = world({ container: box, bounds: [0, -2000, Wd, Hd - 2], gravity: [0, 2200], substeps: 4, iterations: 3, restitution: 0.25, friction: 0.3, damping: 0.02 });
      w.onStep(() => {
        for (const o of bodies) set(o.el, { x: w.x(o.id) - o.r, y: w.y(o.id) - o.r, rotate: (w.angle(o.id) * 180) / Math.PI });
      });
      w.dragging({ hitSlop: 10 });
      box.addEventListener('click', (e) => { const r = box.getBoundingClientRect(); w.impulse(e.clientX - r.left, e.clientY - r.top + 40, 160, 2200); });
      const MAX = small ? 18 : 30;
      let n = 0, t = 0, shake = 0;
      for (; n < 8; n++) spawn(n);
      whileVisible(box, (dt) => {
        t += dt / 1000; shake += dt / 1000;
        if (n < MAX && t > 0.35) { t = 0; spawn(n++); }
        // Every few seconds a nudge from below keeps the pile alive.
        if (n >= MAX && shake > 5 && !reduced) { shake = 0; w.impulse(box.clientWidth * (0.2 + Math.random() * 0.6), box.clientHeight + 10, 220, 1700); }
      });
    };
    inView(box, () => { if (!w) build(); }, { amount: 0.4 });
  });

  // 05 · Particles: 100,000 points (40,000 on phones) morphing into a watch, a wheel and a sphere.
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  let seed = 3;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const shapes = [
    // The watch: case ring, bezel, twelve indices, hands at 10:10, crown, lugs and strap.
    (i, a, o) => {
      const k = rnd(), j = rnd() * 2 - 1;
      let x, y, z = (rnd() - 0.5) * 24;
      if (k < 0.34) { const th = rnd() * TAU, r = 210 + rnd() * 30; x = Math.cos(th) * r; y = Math.sin(th) * r; }
      else if (k < 0.46) { const h = Math.floor(rnd() * 12), th = (h / 12) * TAU, r = 150 + rnd() * 32, w = (rnd() - 0.5) * (h % 3 ? 7 : 12); x = Math.sin(th) * r + Math.cos(th) * w; y = -Math.cos(th) * r + Math.sin(th) * w; }
      else if (k < 0.56) { const hand = rnd() < 0.45, L = hand ? 110 : 165, th = hand ? -60 * Math.PI / 180 : 60 * Math.PI / 180, u = rnd(), w = (1 - u) * (hand ? 9 : 7) * j; x = Math.sin(th) * u * L + Math.cos(th) * w; y = -Math.cos(th) * u * L + Math.sin(th) * w; z = 20 + (rnd() - 0.5) * 6; }
      else if (k < 0.6) { x = 248 + rnd() * 26; y = (rnd() - 0.5) * 46; }
      else if (k < 0.8) { const s = rnd() < 0.5 ? -1 : 1, u = rnd(); x = j * 120; y = s * (232 + u * 230); z = -u * u * 160 + (rnd() - 0.5) * 10; }
      else { const th = rnd() * TAU, r = 140 * Math.sqrt(rnd()); x = Math.cos(th) * r; y = Math.sin(th) * r; z = -14 + (rnd() - 0.5) * 6; }
      a[o] = x; a[o + 1] = -y; a[o + 2] = z;
    },
    // A wheel with teeth and spokes.
    (i, a, o) => {
      const k = rnd(); let x, y;
      if (k < 0.6) { const th = rnd() * TAU, tooth = (Math.cos(th * 24) > 0.2 ? 1 : 0), r = 200 + tooth * 26 - rnd() * 34; x = Math.cos(th) * r; y = Math.sin(th) * r; }
      else if (k < 0.85) { const s = Math.floor(rnd() * 5), th = (s / 5) * TAU + 0.3 + (rnd() - 0.5) * 0.12, r = 40 + rnd() * 140; x = Math.cos(th) * r; y = Math.sin(th) * r; }
      else { const th = rnd() * TAU, r = 22 + rnd() * 26; x = Math.cos(th) * r; y = Math.sin(th) * r; }
      a[o] = x; a[o + 1] = y; a[o + 2] = (rnd() - 0.5) * 40;
    },
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 230 * Math.cbrt(0.85 + rnd() * 0.15), q = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * q * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * q * r; },
  ];
  let cloud = null, g = null;
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.2, swirl: 50, frequency: 0.012 }); return true; });
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
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.75, background: '#000000', trail: 0.35, budget: 5 }); return true; });
  {
    let i = 0, cols = null;
    jobs.push((deadline) => {
      cols ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      // Warm steel with a few gold sparks.
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 4000); for (; i < stop; i++) { const k = rnd(); const gold = k < 0.18; const b = 0.7 + rnd() * 0.5; cols[i * 3] = (gold ? 1.0 : 0.62) * b; cols[i * 3 + 1] = (gold ? 0.72 : 0.68) * b; cols[i * 3 + 2] = (gold ? 0.4 : 0.8) * b; } }
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
    const scene = scene3d({ fov: 40, camera: { z: 1150 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    // The sequence: sphere → watch (held) → wheel → watch …
    const order = [0, 1, 0, 2];
    let step = 0, timer = 2.5, rot = 0, first = true;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    canvas.addEventListener('click', () => { cloud.explode(0, 0, 0, 800); timer = 3.5; });
    whileVisible(canvas, (dt) => {
      timer += dt / 1000;
      cloud.step(Math.min(dt, 50) / 1000);
      if (!reduced) rot += dt / 1000;
      const hold = order[step % order.length] === 0 ? 6.5 : 4.5;
      if ((first && timer > 0.6) || timer > hold) {
        first = false; timer = 0; step++;
        cloud.morph(order[(step - 1) % order.length], { duration: 2.2, ease: 'inOutCubic' });
      }
      const a = Math.sin(rot * 0.3) * 0.5;
      scene.camera.x = Math.sin(a) * 1150; scene.camera.z = Math.cos(a) * 1150; scene.camera.y = Math.sin(rot * 0.21) * 120;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.3, fog: [700, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));

  // 06 · Layout: FLIP between dials and straps, and shuffles.
  once('#layout', () => {
    const grid = $('#tiles');
    const dials = [['#3b5fb0', '#0f1f4a'], ['#f4f1ea', '#cfc9bd'], ['#1d1f24', '#060708'], ['#2f6b52', '#0e2a1f'], ['#e7d3b1', '#b39467'], ['#6e7480', '#2a2d33'], ['#7a2f3a', '#2c0c12'], ['#d9dde6', '#8f96a5']];
    const straps = ['#3a2418', '#121212', '#5b3a24', '#1d2b45', '#7b7f88', '#2f3a2c', '#8a5a33', '#41161c'];
    for (let i = 0; i < 8; i++) {
      const t = document.createElement('div');
      t.className = 'tile';
      t.style.setProperty('--a', dials[i][0]); t.style.setProperty('--b', dials[i][1]); t.style.setProperty('--s', straps[i]);
      grid.appendChild(t);
    }
    const opts = { duration: 0.8, ease: 'spring(bounce: 0.2, duration: 0.7)', delay: stagger(0.03) };
    const bDial = $('#show-dials'), bStrap = $('#show-straps');
    const mode = (straps) => { if (grid.classList.contains('straps') === straps) return; flip('.tile', () => { grid.classList.toggle('straps', straps); }, opts); bDial.classList.toggle('on', !straps); bStrap.classList.toggle('on', straps); };
    const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
    let touched = false;
    bDial.addEventListener('click', () => { touched = true; mode(false); });
    bStrap.addEventListener('click', () => { touched = true; mode(true); });
    $('#shuffle').addEventListener('click', () => { touched = true; shuffle(); });
    grid.addEventListener('click', () => { touched = true; shuffle(); });
    let n = 0, id = 0;
    if (!reduced) inView(grid, () => { id = setInterval(() => { if (touched) return; n++; if (n % 2) shuffle(); else mode(!grid.classList.contains('straps')); }, 2600); return () => clearInterval(id); }, { amount: 0.5 });
  });

  // 07 · Benchmarks: counters and bars run in once.
  once('#numbers', () => {
    const section = $('#numbers');
    const counts = $$('.n-count', section);
    const bars = $$('.n-row u', section);
    set(bars, { scaleX: 0 });
    for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
    inView(section.querySelector('.panel'), () => {
      animate(bars, { scaleX: 1 }, { duration: 1.4, ease: 'spring(bounce: 0.2, duration: 1.1)', delay: stagger(0.15, { start: 0.2 }) });
      counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 1.6, ease: 'outExpo', delay: 0.2 + i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
    }, { amount: 0.35, once: true });
  });

  // Install: copy button.
  const copy = $('#copy');
  copy.addEventListener('click', async () => {
    try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
    copy.textContent = 'Copied';
    effects.pop(copy, { duration: 0.35 });
  });
  if (fine) magnetic('#copy', { strength: 0.3, radius: 70 });
}
