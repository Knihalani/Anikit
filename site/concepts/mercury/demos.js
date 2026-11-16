// What happens inside each stage. Each demo starts when its section comes near
// and does per-frame work only while it is on screen.
import {
  engine, animate, set, timeline, stagger, splitText, scroll, magnetic, flip, pointCloud, glow, scene3d,
  circle, star, rect, pathThrough, noise2, effects,
} from '../../../packages/anikit/dist/anikit.lean.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

export function startDemos({ reduced, fine, small, setChainPreset, inView, liquid }) {
  const near = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 60% 0px' });
  const whileVisible = (el, fn) => {
    let off = null;
    inView(el, () => { off = engine.onFrame((_t, dt) => fn(dt)); return () => { off?.(); off = null; }; }, { amount: 0.2 });
  };

  // Hero line: words rise once.
  {
    const words = splitText('.lede', { type: 'words' }).words;
    animate(words, { y: [12, 0], opacity: [0, 1] }, { duration: 0.8, ease: 'outCubic', delay: stagger(0.03, { start: 0.6 }) });
  }

  // 100,000 particles (40,000 on phones), built in idle time well before the section arrives.
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  const TAU = Math.PI * 2;
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const shapes = [
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 220 * Math.cbrt(0.86 + rnd() * 0.14), s = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * s * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * s * r; },
    (i, a, o) => { const th = rnd() * TAU, ph = rnd() * TAU, r = 62 * Math.sqrt(rnd()), R = 240 + Math.cos(ph) * r; a[o] = Math.cos(th) * R; a[o + 1] = Math.sin(ph) * r; a[o + 2] = Math.sin(th) * R; },
    (i, a, o) => { const t = (i / N) * TAU, r = 150 + 66 * Math.cos(3 * t), ph = rnd() * TAU, rr = 24 * Math.sqrt(rnd());
      a[o] = r * Math.cos(2 * t) + Math.cos(ph) * rr; a[o + 1] = r * Math.sin(2 * t) + Math.sin(ph) * rr; a[o + 2] = -66 * Math.sin(3 * t) + Math.cos(ph * 1.7) * rr; },
  ];
  let cloud = null, g = null;
  const jobs = [() => { cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.5, swirl: 60, frequency: 0.012 }); return true; }];
  shapes.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((dl) => {
      buf ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, dl.timeRemaining() - 1));
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); }
      if (i < N) return false;
      cloud.shape(k, buf);
      if (k === 0) cloud.snap(0);
      return true;
    });
  });
  jobs.push(() => {
    g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.8, background: '#000000', trail: 0.35, budget: 4 });
    // Mercury light: cool silver with a few warm glints.
    g.colors(N, (j) => { const k = rnd(); const s = k < 0.03 ? 2.0 : 0.7 + k * 0.4; const warm = (j * 7919) % 13 === 0; return warm ? [1.0 * s, 0.78 * s, 0.55 * s] : [0.78 * s, 0.82 * s, 0.92 * s]; });
    return true;
  });
  const cloudReady = new Promise((resolve) => {
    const idle = window.requestIdleCallback || ((f) => setTimeout(() => f({ timeRemaining: () => 8 }), 30));
    const run = (dl) => { if (!jobs.length) return resolve(); if (jobs[0](dl)) jobs.shift(); idle(run, { timeout: 400 }); };
    idle(run, { timeout: 400 });
  });
  near('#cloud', () => cloudReady.then(() => {
    $('#cloud-n').textContent = N.toLocaleString('en-US');
    $('#cloud-code').textContent = `pointCloud(${N.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2.4 })`;
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    let next = 1, timer = 1.2, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    const stage = canvas.parentElement;
    stage.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    stage.addEventListener('pointerleave', () => (ptr.on = false));
    stage.addEventListener('click', () => { cloud.explode(0, 0, 0, 700); cloud.morph(next, { duration: 1.8, ease: 'inOutCubic' }); next = (next + 1) % 3; timer = 0; });
    whileVisible(canvas, (dt) => {
      timer += dt / 1000;
      cloud.step(Math.min(dt, 50) / 1000);
      if (!reduced) rot += dt / 1000;
      if (timer > 4.6 && !reduced) { timer = 0; cloud.morph(next, { duration: 2.4, ease: 'inOutCubic' }); next = (next + 1) % 3; }
      scene.camera.x = Math.sin(rot * 0.22) * 1000; scene.camera.z = Math.cos(rot * 0.22) * 1000; scene.camera.y = Math.sin(rot * 0.13) * 220;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.3, fog: [600, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));

  // Split text: a timeline that rises in, holds, and falls away, looping while visible.
  near('#text', () => {
    // The word is split into letters; each letter's rise out of the pool is one value the canvas reads.
    const { chars } = splitText('#reveal', { type: 'chars', charClass: 'ch' });
    const rise = liquid.rise.slice(0, chars.length);
    const eases = ['spring(bounce: 0.35, duration: 1)', 'outBack', 'outExpo'];
    const tl = timeline({ repeat: Infinity, autoplay: false });
    eases.forEach((e) => {
      tl.add(rise, { v: [0, 1] }, { duration: 1.2, ease: e, delay: stagger(0.11) })
        .add(rise, { v: [1, 0] }, { duration: 1.1, ease: 'inCubic', delay: stagger(0.08, { from: 'last' }) }, '+=2.2');
    });
    inView('#reveal', () => { tl.play(); return () => tl.pause(); }, { amount: 0.5 });
  });

  // Springs: the drops in the canvas form the chain (see mercury.js); these switch its character.
  near('#springs', () => {
    const code = $('#spring-code');
    const presets = [[170, 15], [120, 6], [30, 9]];
    const btns = $$('#springs .mag');
    const pick = (k) => {
      setChainPreset(k);
      btns.forEach((b, j) => b.classList.toggle('on', j === k));
      code.textContent = `spring(drop, { x, y }, { stiffness: ${presets[k][0]}, damping: ${presets[k][1]} })`;
    };
    btns.forEach((b, k) => b.addEventListener('click', () => { pick(k); effects.pop(b, { duration: 0.3 }); }));
    pick(0);
    if (fine) magnetic('#springs .mag, #copy', { strength: 0.32, radius: 80 });
  });

  // SVG: the shape morphs as you scroll through the section; the reflection drifts on its own.
  near('#svg', () => {
    const blob = (seed, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
      const a = (k / n) * Math.PI * 2, r = 92 + noise2(k * 0.6, seed) * 34;
      return [150 + Math.cos(a) * r, 150 + Math.sin(a) * r];
    }), { closed: true });
    const shapes = [circle(150, 150, 104), star(150, 150, 8, 126, 74), blob(2), rect(52, 52, 196, 196, 40), star(150, 150, 5, 130, 56), blob(7, 11), circle(150, 150, 104)];
    $('#blob').setAttribute('d', shapes[0]);
    const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } });
    for (let k = 1; k < shapes.length; k++) tl.add('#blob', { d: shapes[k] });
    tl.add('.morph', { rotate: [0, 120] }, { duration: shapes.length - 1, ease: 'linear' }, 0);
    scroll(tl, { target: $('#svg'), offset: ['start start', 'end end'], smooth: 0.12 });
    if (!reduced) animate('#chrome', { attr: { x2: [0.2, 0.9], y1: [0, 0.25] } }, { duration: 5, ease: 'inOutSine', repeat: Infinity, yoyo: true });
  });

  // FLIP: twelve tiles reshuffle and regrid on their own.
  near('#layout', () => {
    const grid = $('#tiles');
    for (let i = 0; i < 12; i++) {
      const t = document.createElement('div');
      t.className = 'tile' + (i === 0 ? ' big' : '');
      t.textContent = String(i + 1).padStart(2, '0');
      grid.appendChild(t);
    }
    const opts = { duration: 0.9, ease: 'spring(bounce: 0.2, duration: 0.8)', delay: stagger(0.025) };
    const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
    const regrid = () => flip('.tile', () => grid.classList.toggle('alt'), opts);
    grid.addEventListener('click', shuffle);
    let n = 0, id = 0;
    if (!reduced) inView(grid, () => { id = setInterval(() => (++n % 3 ? shuffle() : regrid()), 2600); return () => clearInterval(id); }, { amount: 0.5 });
  });

  // Benchmarks: bars and counters run in once.
  near('#numbers', () => {
    const section = $('#numbers');
    const counts = $$('.n-count', section);
    // The bars are poured in metal by the canvas; anikit animates how full each one is.
    const bars = liquid.bars;
    for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
    inView($('.stage', section), () => {
      animate(bars, { v: [0, 1] }, { duration: 1.8, ease: 'spring(bounce: 0.18, duration: 1.4)', delay: stagger(0.18, { start: 0.3 }) });
      counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 1.8, ease: 'outExpo', delay: 0.3 + i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
    }, { amount: 0.6, once: true });
  });

  // Install: copy.
  const copy = $('#copy');
  copy.addEventListener('click', async () => {
    try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
    copy.textContent = 'Copied';
    effects.pop(copy, { duration: 0.35 });
    setTimeout(() => (copy.textContent = 'Copy'), 1800);
  });
}
