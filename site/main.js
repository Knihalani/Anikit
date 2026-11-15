// Anikit — a kinetic poster film. Every frame on this page runs on anikit.
import {
  ready, engine, animate, set, timeline, stagger, spring, scroll, inView, splitText, effects, world,
  record, noise2, circle, star, rect, pathThrough, flush,
} from '../packages/anikit/dist/anikit.js';
import { cosmos, bulletTime } from './cosmos.js';
import { gravity } from './gravity.js';

await ready;
engine.reducedMotion = 'user';
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const dpr = Math.min(2, devicePixelRatio || 1);
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;

/** Run `fn(dt)` every frame only while `el` is on screen. */
function whileVisible(el, fn) {
  let off = null;
  inView(el, () => {
    off = engine.onFrame((_t, dt) => fn(dt));
    return () => { off?.(); off = null; };
  }, { amount: 0 });
}
/** Scroll progress through a pinned section (0 at pin start, 1 at pin end). */
const pinned = (section, fn) => scroll(fn, { target: section, offset: ['start start', 'end end'] });
/** Wrap every character of an element in an inline-block span. */
const chars = (el) => splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;

// ---------------------------------------------------------------------------
// The cosmos, time, gravity, frame UI
// ---------------------------------------------------------------------------
const cos = await cosmos({ reduced });
const bt = bulletTime({
  glow: cos.glow,
  buttons: [$('#hud-slow'), $('#hold-btn')],
  vignette: $('#vignette'),
  onChange: (slow) => animate('#t-key', { y: slow ? '0.08em' : 0, boxShadow: slow ? '0 0em 0 #0c0c0c' : '0 0.09em 0 #0c0c0c' }, { duration: 0.12 }),
});
const grav = gravity({ onChange: (on, n) => { $('#hud-gravity').classList.toggle('on', on); hudNote = on ? `${n} bodies · drag · double-click · Esc` : ''; } });
let hudNote = '';
for (const b of [$('#hud-gravity'), $('#hero-gravity')]) b.addEventListener('click', () => grav.toggle());
addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && grav.active) grav.off();
  if ((e.key === 'g' || e.key === 'G') && !e.metaKey && !e.ctrlKey) grav.toggle();
});
setInterval(() => {
  const s = cos.stats;
  $('#hud-stats').textContent = hudNote || `${cos.count.toLocaleString('en-US')} particles · ${s.ms.toFixed(1)} ms · ${s.fps} fps${bt.speed < 0.99 ? ` · ${bt.speed.toFixed(2)}×` : ''}`;
}, 250);

// Scene counter + progress.
{
  const names = [...new Set(cos.stations.map((s) => s.name))];
  $('#scene-total').textContent = String(names.length).padStart(2, '0');
  let last = '';
  engine.onFrame(() => {
    const name = cos.state.station.name;
    if (name !== last) {
      last = name;
      $('#scene-no').textContent = String(names.indexOf(name) + 1).padStart(2, '0');
      animate('#scene-name', { text: { value: name, type: 'scramble', chars: 'upper' } }, { duration: 0.45, ease: 'linear' });
    }
    const max = document.documentElement.scrollHeight - innerHeight;
    set('#progress', { scaleX: max > 0 ? scrollY / max : 0 });
  });
}

// Cursor: a difference-blended dot that swells over anything interactive.
if (fine) {
  const cur = $('#cursor');
  const label = $('#cursor-label');
  const pos = { x: innerWidth / 2, y: innerHeight / 2 };
  addEventListener('pointermove', (e) => {
    spring(pos, { x: e.clientX, y: e.clientY }, { stiffness: 900, damping: 50 });
    set(cur, { opacity: 1 });
  }, { passive: true });
  engine.onFrame(() => set(cur, { x: pos.x, y: pos.y }));
  let over = null;
  addEventListener('pointerover', (e) => {
    const t = e.target.closest?.('[data-cursor], a, button');
    if (t === over) return;
    over = t;
    const text = t?.dataset?.cursor || '';
    label.textContent = text;
    animate($('i', cur), { scale: t ? (text ? 5 : 2.4) : 1 }, { duration: 0.5, ease: 'spring(bounce: 0.35, duration: 0.45)' });
    animate(label, { opacity: t && text ? 1 : 0 }, { duration: 0.2 });
  });
  addEventListener('pointerdown', () => animate($('i', cur), { scale: [null, 0.6, null] }, { duration: 0.35, ease: 'outCubic' }));
}

// Shockwaves on the open "stage" scenes.
for (const el of $$('.hero, .particles .pin, .time .pin, .install .pin')) {
  el.addEventListener('click', (e) => { if (!e.target.closest('a, button')) cos.shock(e.clientX, e.clientY); });
}

// ---------------------------------------------------------------------------
// 01 · Hero
// ---------------------------------------------------------------------------
{
  const cs = chars($('#hero-title'));
  $('#hero-title').dataset.gsplit = '1';
  set(cs, { yPercent: 110, rotate: 8, opacity: 0 });
  set('#hero-copy, .hero-actions, .hero .corner, .scroll-cue', { opacity: 0, y: 14 });
  setTimeout(() => {
    animate(cs, { yPercent: 0, rotate: 0, opacity: 1 }, { duration: 1.1, ease: 'spring(bounce: 0.2, duration: 0.9)', delay: stagger(0.03) });
    animate('#hero-copy, .hero-actions, .hero .corner, .scroll-cue', { opacity: 1, y: 0 }, { duration: 0.9, ease: 'outCubic', delay: stagger(0.08, { start: 0.4 }) });
  }, reduced ? 0 : 2300);
  animate('.scroll-cue i', { scaleX: [0, 1, 1], x: [0, 0, 46], opacity: [1, 1, 0] }, { duration: 1.8, ease: 'inOutCubic', repeat: Infinity });
  cos.playIntro();
}

// ---------------------------------------------------------------------------
// 02 · Manifesto: a kinetic list scrubbed by scroll
// ---------------------------------------------------------------------------
{
  const items = [
    ['Any CSS property, SVG attribute or JS object.', "animate(el, { x: 400, rotate: 90 })", 'outExpo'],
    ['Retarget mid-flight — velocity is kept.', "spring(el, { x }, { bounce: 0.4 })", 'spring(bounce: 0.55)'],
    ['Labels, overlaps, nesting, exact seeking.', "timeline().add(a).add(b, '<0.2')", 'outBack'],
    ['Rigid bodies, ropes and cloth in WebAssembly.', 'world({ gravity: [0, 980] })', 'outBounce'],
    ['150,000 particles, simulated and lit in the core.', 'pointCloud(150_000, { shapes })', 'outElastic'],
    ['Transform lists, slerped like browsers do.', "animate(el, { transform: 'rotateY(180deg)' })", 'inOutCubic'],
    ['Scrub, pin, snap to labels.', 'scrollTrigger({ scrub: true, pin: true })', 'outQuart'],
    ['Split by line, word, char. Scramble. Type.', "splitText(el, { type: 'chars' })", 'steps(6)'],
    ['Morph anything into anything.', 'animate(path, { d: star(…) })', 'outCirc'],
    ['Frame-exact GIF, WebM and PNG.', "record('#hero', { duration: 2 })", 'rough(16, 1.4, 7, outQuad)'],
    ['Compose films in code; render MP4 or WebM, frame-exact.', "composition({ fps: 60, motionBlur: 3 })", 'outBack'],
    ['Beats, onsets and tempo, found in WebAssembly.', 'analyzeAudio(track).beats', 'spring(bounce: 0.7)'],
  ];
  const section = $('#manifesto');
  const stack = $('#m-stack');
  const words = $$('.m-word', stack);
  const wordChars = words.map((w) => chars(w));
  let active = -1;
  let descTween = null;
  let wordH = words[0].getBoundingClientRect().height;
  addEventListener('resize', () => (wordH = words[0].getBoundingClientRect().height));
  const activate = (i) => {
    if (i === active) return;
    const prev = active;
    active = i;
    words.forEach((w, k) => w.classList.toggle('on', k === i));
    const [desc, code, ease] = items[i];
    $('#m-index').textContent = String(i + 1).padStart(2, '0');
    descTween?.kill();
    descTween = animate('#m-desc', { text: { value: desc, type: 'scramble', chars: 'lower' } }, { duration: 0.3, ease: 'linear' });
    $('#m-code').textContent = `${code}  // ease: '${ease}'`;
    const dir = prev < i ? 1 : -1;
    const three = i === 5;
    animate(wordChars[i], three ? { rotateX: [-90 * dir, 0], opacity: [0, 1] } : { yPercent: [60 * dir, 0], opacity: [0, 1] }, { duration: 0.9, ease, delay: stagger(0.035) });
  };
  pinned(section, (p) => {
    const f = Math.min(words.length - 1, Math.max(0, p * (words.length - 0.2) - 0.1));
    set(stack, { y: -(f + 0.5) * wordH });
    words.forEach((w, k) => {
      const d = Math.abs(k - f);
      set(w, { x: Math.max(0, 1 - d) * 40, opacity: Math.max(0.12, 1 - d * 0.35) });
    });
    activate(Math.round(f));
  });
}

// ---------------------------------------------------------------------------
// 03 · Physics: giant letters and 3,000 grains share one WebAssembly world
// ---------------------------------------------------------------------------
{
  const section = $('#physics');
  const pin = $('.pin', section);
  const host = $('#p-letters');
  const canvas = $('#sand');
  const ctx = canvas.getContext('2d');
  const letters = 'GRAVITY'.split('').map((ch) => {
    const s = document.createElement('div');
    s.className = 'p-letter';
    s.textContent = ch;
    host.appendChild(s);
    return s;
  });
  let w = null, firstGrain = 0, grains = 0, started = false;
  const letterIds = new Set();
  const bodies = [];
  const fit = () => {
    canvas.width = pin.clientWidth * dpr;
    canvas.height = pin.clientHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    w?.set({ bounds: [0, -3000, pin.clientWidth, pin.clientHeight - 26] });
  };
  const start = () => {
    if (started) return;
    started = true;
    fit();
    addEventListener('resize', fit);
    // Open top (letters fall in from above), floor slightly above the fold so glyphs rest fully in view.
    w = world({ container: pin, bounds: [0, -3000, pin.clientWidth, pin.clientHeight - 26], gravity: [0, 2200], substeps: 4, iterations: 3, restitution: 0.25, friction: 0.12, damping: 0.02 });
    // Letters enter one by one from above, spread across the width.
    const W = pin.clientWidth;
    letters.forEach((el, i) => {
      const r = el.getBoundingClientRect();
      set(el, { x: (W / (letters.length + 1)) * (i + 1) - r.width / 2, y: 20 - r.height });
    });
    flush();
    letterIds.clear();
    // Each letter is a rigid pair of circles stacked over the glyph's cap
    // height, so tall letters stand, tip and tumble like real objects.
    const host = pin.getBoundingClientRect();
    letters.forEach((el, i) => setTimeout(() => {
      const b = el.getBoundingClientRect();
      const capH = b.height * 0.72;
      const r = Math.min(b.width * 0.46, capH * 0.32);
      const cx = b.left - host.left + b.width / 2, cy = b.top - host.top + b.height * 0.53;
      const off = capH / 2 - r;
      const top = w.add(cx, cy - off, r, 40), bot = w.add(cx, cy + off, r, 40);
      w.link(top, bot, { stiffness: 1 });
      letterIds.add(top).add(bot);
      bodies.push({ el, top, bot, w: b.width, h: b.height });
      w.setVelocity(top, (Math.random() - 0.5) * 300, 200);
      w.setVelocity(bot, (Math.random() - 0.5) * 300, 200);
    }, reduced ? 0 : 140 * i));
    w.onStep(() => {
      for (const o of bodies) {
        const x = (w.x(o.top) + w.x(o.bot)) / 2, y = (w.y(o.top) + w.y(o.bot)) / 2;
        const ang = Math.atan2(w.x(o.top) - w.x(o.bot), w.y(o.bot) - w.y(o.top));
        set(o.el, { x: x - o.w / 2, y: y - o.h * 0.53, rotate: (ang * 180) / Math.PI });
      }
    });
    w.pointer({ mode: 'repel', radius: 130, strength: 9000 });
    w.dragging({ hitSlop: 20 });
    pin.addEventListener('click', (e) => {
      const r = pin.getBoundingClientRect();
      w.impulse(e.clientX - r.left, e.clientY - r.top, 320, 2600);
    });
    firstGrain = -1;
  };
  inView(section, start, { amount: 0.25 });
  const MAX = innerWidth < 760 ? 700 : 1600;
  whileVisible(pin, () => {
    if (!w) return;
    // Pour grains from a moving spout until the floor is covered.
    if (grains < MAX) {
      const W = pin.clientWidth, n = Math.min(40, MAX - grains);
      const sx = W * (0.5 + 0.38 * Math.sin(performance.now() / 900));
      const first = w.addMany(Array.from({ length: n }, () => ({ x: sx + (Math.random() - 0.5) * 60, y: 6 + Math.random() * 30, r: 2.4 + Math.random() * 2 })));
      if (firstGrain < 0) firstGrain = first;
      grains += n;
    }
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (firstGrain < 0) return;
    ctx.fillStyle = '#ece6da';
    for (let i = firstGrain; i < w.count; i++) {
      if (letterIds.has(i)) continue;
      const r = w.radius(i);
      ctx.beginPath();
      ctx.arc(w.x(i), w.y(i), r * 0.92, 0, Math.PI * 2);
      ctx.fill();
    }
  });
}

// ---------------------------------------------------------------------------
// 04 · Particles: the number fills as you scroll through the shapes
// ---------------------------------------------------------------------------
{
  const section = $('#particles');
  $$('#big-num span').forEach((s) => (s.textContent = cos.count.toLocaleString('en-US')));
  pinned(section, (p) => set('#big-num-fill', { clipPath: `inset(0 ${(100 - p * 100).toFixed(2)}% 0 0)` }));
}

// ---------------------------------------------------------------------------
// 05 · Time
// ---------------------------------------------------------------------------
{
  const lines = $$('.t-line');
  set(lines, { yPercent: 40, opacity: 0 });
  inView('#time', () => animate(lines, { yPercent: 0, opacity: 1 }, { duration: 1, ease: 'spring(bounce: 0.3, duration: 0.8)', delay: stagger(0.08) }), { amount: 0.3 });
  animate('#hold-btn', { scale: [1, 1.06, 1] }, { duration: 1.6, ease: 'inOutSine', repeat: Infinity });
}

// ---------------------------------------------------------------------------
// 06 · Paths: a giant blob morphs with scroll; a stroke draws across the page
// ---------------------------------------------------------------------------
{
  const section = $('#paths');
  const blobShape = (seed, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
    const a = (k / n) * Math.PI * 2, r = 150 + noise2(k * 0.6, seed) * 60;
    return [200 + Math.cos(a) * r, 200 + Math.sin(a) * r];
  }), { closed: true });
  const blob = $('#blob');
  blob.setAttribute('d', circle(200, 200, 170));
  const shapes = [circle(200, 200, 170), star(200, 200, 12, 190, 120), blobShape(2), rect(40, 40, 320, 320, 40), star(200, 200, 5, 195, 80), blobShape(7, 11)];
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } });
  for (let k = 1; k < shapes.length; k++) tl.add(blob, { d: shapes[k] });
  const draw = animate('#trail', { draw: [0, 1] }, { autoplay: false, ease: 'linear', duration: 1 });
  const ride = animate('#rider', { path: { path: '#trail' } }, { autoplay: false, ease: 'linear', duration: 1 });
  pinned(section, (p) => {
    tl.progress = p;
    draw.progress = p;
    ride.progress = p;
  });
  animate('.blob', { rotate: 360 }, { duration: 40, ease: 'linear', repeat: Infinity });
}

// ---------------------------------------------------------------------------
// 07 · Depth: a ring of cards turned by scroll
// ---------------------------------------------------------------------------
{
  const section = $('#depth');
  const ring = $('#ring');
  const feats = [
    ['Spring', "ease: 'spring(bounce: .4)'", 'M0 90 C 20 -40 40 120 60 40 S 90 70 120 60'],
    ['Inertia', 'inertia(el, { x: { velocity } })', 'M0 90 C 30 10 60 4 120 2'],
    ['Flip', "flip('.tile', change)", 'M10 80 L40 20 L70 80 L100 20'],
    ['Morph', 'animate(p, { d: star(…) })', 'M0 60 Q 30 0 60 60 T 120 60'],
    ['Scroll', 'scrollTrigger({ pin: true })', 'M0 90 L120 0'],
    ['Text', "splitText(el, { type: 'chars' })", 'M0 70 h20 v-30 h20 v30 h20 v-50 h20 v50 h20'],
    ['OKLab', "animate(el, { color: 'oklch(…)' })", 'M0 50 C 40 50 40 10 60 10 S 80 90 120 90'],
    ['WAAPI', 'compositor(el, { x: 200 })', 'M0 80 C 40 80 60 20 120 20'],
    ['Capture', "record('#hero')", 'M0 50 h120 M60 0 v100'],
    ['Python', 'anikit.animate(box, {…})', 'M0 90 Q 60 -60 120 90'],
  ];
  const R = () => innerHeight * 0.4;
  const cards = feats.map(([name, code, d], i) => {
    const c = document.createElement('button');
    c.className = 'card';
    c.dataset.cursor = 'Flip';
    c.innerHTML = `<span class="n">${String(i + 1).padStart(2, '0')}</span><svg viewBox="0 0 120 100"><path d="${d}"/></svg><b>${name}</b><code>${code}</code>`;
    ring.appendChild(c);
    return c;
  });
  const place = () => cards.forEach((c, i) => set(c, { transform: `rotateY(${i * 36}deg) translateZ(${R()}px)` }));
  place();
  addEventListener('resize', place);
  cards.forEach((c, i) => {
    let flipped = false;
    c.addEventListener('click', () => {
      flipped = !flipped;
      animate(c, { transform: `rotateY(${i * 36}deg) translateZ(${R() + (flipped ? 120 : 0)}px) rotateY(${flipped ? 180 : 0}deg) rotateX(${flipped ? -10 : 0}deg)` }, { duration: 1.1, ease: 'spring(bounce: 0.35, duration: 0.9)' });
    });
  });
  const tilt = { x: 0, y: 0 };
  addEventListener('pointermove', (e) => spring(tilt, { x: (e.clientY / innerHeight - 0.5) * -14, y: (e.clientX / innerWidth - 0.5) * 18 }, { stiffness: 120, damping: 18 }), { passive: true });
  let p = 0;
  pinned(section, (v) => (p = v));
  whileVisible(section, () => set(ring, { rotateX: -12 + tilt.x, rotateY: -p * 324 + tilt.y }));
  // Paths inside the cards draw on as the ring enters.
  inView(section, () => animate($$('.card path'), { draw: [0, 1] }, { duration: 1.4, ease: 'inOutCubic', delay: stagger(0.06) }), { amount: 0.2 });
}

// ---------------------------------------------------------------------------
// 08 · Agents: live engine.inspect() and in-page recording
// ---------------------------------------------------------------------------
{
  const term = $('#term');
  let tick = 0;
  whileVisible(term, () => {
    if (++tick % 15) return;
    const all = engine.inspect().filter((s) => s.state === 'running');
    const live = all.filter((s) => s.progress > 0 && s.progress < 1);
    const rows = (live.length ? live : all).slice(0, 14);
    const q = (v) => (typeof v === 'string' ? `'${v}'` : Array.isArray(v) ? `[${v.map(q).join(', ')}]` : typeof v === 'number' ? +v.toFixed(3) : v);
    term.innerHTML = `<span class="k">&gt; engine.inspect()</span>  // ${all.length} running\n` + rows.map((s) =>
      `{ <span class="k">id</span>: ${s.id}, <span class="k">type</span>: ${q(s.type)}, <span class="k">target</span>: ${q(s.target ?? '–')}, <span class="k">props</span>: ${q(s.props ?? [])}, <span class="k">progress</span>: ${q(s.progress)} }`).join('\n');
  });
  const btn = $('#rec-btn');
  btn.addEventListener('click', async () => {
    btn.disabled = true;
    btn.textContent = '● Recording…';
    const t0 = performance.now();
    const rec = await record('#term', { duration: 2, fps: 20, scale: 1, background: '#0c0c0c' });
    const img = new Image();
    img.src = rec.url;
    $('#rec-out').replaceChildren(img);
    $('#rec-out').append(` ${rec.frames} frames · ${(rec.blob.size / 1024).toFixed(0)} KB · ${((performance.now() - t0) / 1000).toFixed(1)} s`);
    btn.textContent = '● Record 2 s';
    btn.disabled = false;
  });
}

// ---------------------------------------------------------------------------
// 09 · Film: play while on screen; sound toggle
// ---------------------------------------------------------------------------
{
  const v = $('#film-video');
  const title = chars($('.film-title'));
  $('.film-title').dataset.gsplit = '1';
  set(title, { yPercent: 100, opacity: 0 });
  inView('#film', () => {
    animate(title, { yPercent: 0, opacity: 1 }, { duration: 1, ease: 'spring(bounce: 0.25, duration: 0.8)', delay: stagger(0.03) });
    v.play().catch(() => {});
    return () => v.pause();
  }, { amount: 0.3 });
}

// ---------------------------------------------------------------------------
// 10 · Install
// ---------------------------------------------------------------------------
{
  const cs = chars($('#outro'));
  $('#outro').dataset.gsplit = '1';
  set(cs, { yPercent: 100, opacity: 0 });
  inView('#install', () => animate(cs, { yPercent: 0, opacity: 1 }, { duration: 1, ease: 'spring(bounce: 0.3, duration: 0.8)', delay: stagger(0.035) }), { amount: 0.4 });
  $('#outro').addEventListener('click', () => effects.rubberBand(cs, { stagger: 0.03 }));
  $('#copy').addEventListener('click', async (e) => {
    try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
    e.currentTarget.textContent = 'Copied';
    effects.pop(e.currentTarget, { duration: 0.35 });
  });
}

window.anikit = await import('../packages/anikit/dist/anikit.js');
