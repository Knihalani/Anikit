// Frame-exact demo video of the Anikit landing page.
//
//   node site/demo/record-demo.mjs out.mp4            full 1080p60 render
//   PREVIEW=30 node site/demo/record-demo.mjs dir/    every 30th frame as PNG
//   FROM=40 TO=60 …                                   only that time range
//
// Time inside the page is virtualised (Playwright's fake clock): every frame
// advances exactly 1/60 s, so the video is perfectly smooth however long a
// frame takes to render and capture. The HUD's "ms" is still the real
// compute cost of the cosmos, measured on a real clock.
import { createRequire } from 'node:module';
import { readFileSync, existsSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '../..');
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }

const OUT = process.argv[2] || 'anikit-demo.mp4';
const FPS = 60;
const W = +(process.env.W || 1920);
const H = +(process.env.H || 1080);
const PREVIEW = +(process.env.PREVIEW || 0);
const FROM = +(process.env.FROM || 0);

// ---------------------------------------------------------------------------
// Storyboard (seconds)
// ---------------------------------------------------------------------------
const center = (sel, i = 0) => ({ kind: 'center', sel, i });
const top = (sel, off = 0, i = 0) => ({ kind: 'top', sel, off, i });
const el = (sel, i = 0, dx = 0, dy = 0) => ({ kind: 'el', sel, i, dx, dy });
const word = (text, dx = 0, dy = 0) => ({ kind: 'word', text, dx, dy });
const at = (x, y) => [x * W, y * H];

let mouse = [W * 0.78, H * 0.7];
const scrolls = [];
const moves = [];
const events = [];
const scroll = (t0, t1, to, ease = 'inOutCubic') => scrolls.push({ t0, t1, to, ease });
const move = (t0, t1, pts, ease = 'inOutSine') => moves.push({ t0, t1, pts, ease });
const on = (t, fn) => events.push({ t, fn });
const caption = (t, kicker, text, dur = 3.2) => on(t, (p) => p.evaluate(([k, s, d]) => { window.__caption(k, s, d); }, [kicker, text, dur]));
const click = (t) => { on(t, (p) => p.mouse.down()); on(t + 0.07, (p) => p.mouse.up()); };
// Click an element wherever it is *now* (it may still be animating in).
const clickOn = (t, spec) => {
  on(t, async (p) => {
    mouse = await resolvePoint(p, spec);
    await p.mouse.move(mouse[0], mouse[1]);
    await p.mouse.down();
  });
  on(t + 0.07, (p) => p.mouse.up());
};
const drag = (t0, t1, from, to) => {
  move(t0 - 0.5, t0, [from]);
  on(t0, (p) => p.mouse.down());
  move(t0 + 0.02, t1, [to], 'inQuad');
  on(t1, (p) => p.mouse.up());
};

// 1 · Intro: a singularity becomes the wordmark
on(3.9, (p) => p.evaluate(() => { window.__cursor(true); }));
caption(3.4, 'Rust → WebAssembly', '140,000 particles, simulated and lit in WebAssembly', 3.4);
move(3.9, 7.2, [at(0.78, 0.66), at(0.66, 0.33), at(0.5, 0.4), at(0.33, 0.3), at(0.24, 0.4)]);
move(7.2, 7.6, [at(0.5, 0.36)]);
click(7.7);
move(8.0, 9.6, [at(0.58, 0.6)]);

// 2 · The galaxy
scroll(9.6, 11.6, center('.moment', 0));
caption(10.6, 'Scroll', 'Every section reshapes the same particles', 3);
move(11.7, 14.6, [at(0.36, 0.3), at(0.3, 0.55), at(0.45, 0.78), at(0.68, 0.7), at(0.72, 0.4)]);
click(12.7);
click(14.0);

// 3 · Performance
scroll(14.8, 16.6, top('#perf', -40));
caption(15.6, 'Benchmarks', 'Up to 60× faster physics than matter.js · every number from a script in the repo', 3.4);
move(15.0, 18.0, [at(0.72, 0.3), at(0.8, 0.22)]);

// 4 · Easing lab
scroll(17.8, 19.2, top('#ease', -50));
caption(19.4, 'Easing', 'Every syntax: CSS, GSAP, Penner, springs, custom curves', 3.2);
move(19.2, 19.8, [el('#ease-chips .chip', 1)]); clickOn(19.9, el('#ease-chips .chip', 1));
move(20.4, 21.0, [el('#ease-chips .chip', 2)]); clickOn(21.1, el('#ease-chips .chip', 2));
move(21.6, 22.2, [el('#ease-chips .chip', 3)]); clickOn(22.3, el('#ease-chips .chip', 3));

// 5 · Pinned, scroll-scrubbed timeline
scroll(23.0, 24.4, top('#story', 0));
scroll(24.5, 30.4, top('#story', 2580), 'inOutSine');
caption(25.0, 'Timelines × scroll', 'Pinned and scrubbed: labels, morphing, OKLab colour, 3D', 3.6);
move(23.0, 30.0, [at(0.7, 0.45), at(0.68, 0.55)]);

// 6 · SVG
scroll(30.6, 32.0, top('#svg', -50));
caption(32.0, 'SVG', 'Morph anything into anything · stroke drawing · motion paths', 2.8);
move(31.0, 33.4, [el('.card', 0), el('.card', 1), el('.card', 2)]);

// 7 · Gravity
scroll(33.6, 35.4, center('.moment', 1));
move(35.4, 36.2, [el('#gravity-cta')]);
clickOn(36.35, el('#gravity-cta'));
caption(36.7, 'Physics', 'Every word is now a rigid body in a WebAssembly physics world', 3.4);
drag(38.4, 38.7, word('page.'), at(0.3, 0.2));
drag(39.5, 39.8, word('drop'), at(0.78, 0.15));
move(40.2, 40.7, [at(0.5, 0.85)]);
on(40.8, (p) => p.mouse.dblclick(W * 0.5, H * 0.85));
move(41.2, 42.0, [el('#hud-gravity')]);
clickOn(42.2, el('#hud-gravity'));
caption(42.5, 'Restore', '…and everything springs back into the layout', 2.6);

// 8 · Physics section
scroll(44.0, 45.4, top('#physics', -30));
drag(46.0, 46.25, el('#letters-stage .letter', 2), at(0.33, 0.25));
drag(46.9, 47.15, el('#letters-stage .letter', 8), at(0.15, 0.35));
move(47.5, 50.0, [el('#field-stage', 0, -180, 40), el('#field-stage', 0, 0, 90), el('#field-stage', 0, 160, 60), el('#field-stage', 0, 60, 120)]);
caption(47.6, 'Physics', '950 colliding bodies, a rope and pointer forces · fractions of a millisecond', 2.8);

// 9 · 3D
scroll(50.2, 51.6, top('#flip-cards', -230));
caption(51.8, '3D', 'Transforms interpolate like browsers do: decompose, slerp, recompose', 3.2);
move(51.6, 53.4, [el('.flip', 0, -80, -40), el('.flip', 0, 90, 50), el('.flip', 1, -60, 30), el('.flip', 1)]);
clickOn(53.6, el('.flip', 1));
move(54.0, 54.6, [el('#shuffle')]); clickOn(54.75, el('#shuffle'));
move(55.3, 55.8, [el('#relayout')]); clickOn(55.95, el('#relayout'));

// 10 · Bullet time
scroll(57.0, 58.8, center('.moment', 2));
move(58.4, 59.0, [at(0.3, 0.3)]);
click(59.1);
on(59.28, (p) => p.keyboard.down('Space'));
caption(59.5, 'One clock', 'Hold Space: tweens, physics and particles slow down together', 3.4);
move(59.4, 62.6, [at(0.42, 0.42), at(0.6, 0.62), at(0.7, 0.4)]);
on(63.0, (p) => p.keyboard.up('Space'));

// 11 · Effects
scroll(63.8, 65.4, top('#effects', -40));
caption(65.6, 'Effects', '39 presets — or register your own in one line', 3);
move(65.4, 68.6, [el('.fx', 0), el('.fx', 3), el('.fx', 5), el('.fx', 10), el('.fx', 8), el('.fx', 7), el('.fx', 12), el('.fx', 15), el('.fx', 17)], 'linear');

// 12 · Feel
scroll(68.8, 70.2, top('#feel', -60));
caption(70.4, 'Feel', 'Springs that keep their velocity · inertia · snapping', 3);
drag(70.6, 70.85, el('#puck'), el('#puck', 0, 240, 110));
move(71.4, 73.0, [el('#follow-zone', 0, -90, -50), el('#follow-zone', 0, 90, 40), el('#follow-zone', 0, -40, 60)]);
drag(73.5, 74.2, el('#knob', 0, 0, -50), el('#knob', 0, 55, 10));

// 13 · Agents
scroll(74.6, 76.0, top('#agents', -50));
caption(76.2, 'Agents', 'Live engine.inspect() · fake time · frame-exact recording', 3);
move(75.0, 78.4, [el('.code-card', 0), el('.code-card', 1)]);

// 14 · Install, then rewind through everything
scroll(78.6, 80.4, { kind: 'bottom' });
caption(80.6, 'npm install anikit', 'TypeScript and Python · MIT', 2.6);
move(80.2, 80.8, [at(0.3, 0.62)]); click(80.9);
move(81.1, 81.5, [at(0.72, 0.6)]); click(81.6);
move(82.0, 83.0, [at(0.5, 0.95)]);
scroll(82.4, 86.0, { kind: 'y', y: 0 }, 'inOutQuart');
on(85.6, (p) => p.evaluate(() => { window.__cursor(false); }));
caption(86.4, 'npm install anikit', 'Rust core · WebAssembly · TypeScript · Python', 3.4);
const END = +(process.env.END || 90.5);

// ---------------------------------------------------------------------------
// Machinery
// ---------------------------------------------------------------------------
const EASE = {
  linear: (t) => t,
  inQuad: (t) => t * t,
  inOutSine: (t) => -(Math.cos(Math.PI * t) - 1) / 2,
  inOutCubic: (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2),
  inOutQuart: (t) => (t < 0.5 ? 8 * t ** 4 : 1 - Math.pow(-2 * t + 2, 4) / 2),
};
const catmull = (p0, p1, p2, p3, t) => {
  const t2 = t * t, t3 = t2 * t;
  return [0, 1].map((k) => 0.5 * (2 * p1[k] + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3));
};
const along = (pts, u) => {
  if (pts.length === 1) return pts[0];
  const n = pts.length - 1;
  const f = Math.min(n - 1e-9, u * n);
  const i = Math.floor(f);
  return catmull(pts[Math.max(0, i - 1)], pts[i], pts[i + 1], pts[Math.min(n, i + 2)], f - i);
};

async function resolvePoint(page, spec) {
  if (Array.isArray(spec)) return spec;
  return page.evaluate((s) => {
    let e;
    if (s.kind === 'word') e = [...document.querySelectorAll('.gw')].find((w) => w.textContent.trim() === s.text && w.getBoundingClientRect().width);
    else e = document.querySelectorAll(s.sel)[s.i];
    if (!e) return [innerWidth / 2, innerHeight / 2];
    const r = e.getBoundingClientRect();
    return [r.left + r.width / 2 + s.dx, r.top + r.height / 2 + s.dy];
  }, spec);
}
async function resolveScroll(page, spec) {
  return page.evaluate((s) => {
    const max = document.documentElement.scrollHeight - innerHeight;
    if (s.kind === 'bottom') return max;
    if (s.kind === 'y') return s.y;
    const e = document.querySelectorAll(s.sel)[s.i];
    const r = e.getBoundingClientRect();
    const y = s.kind === 'center' ? r.top + scrollY + r.height / 2 - innerHeight / 2 : r.top + scrollY + s.off;
    return Math.max(0, Math.min(max, y));
  }, spec);
}

const OVERLAY = () => {
  const css = document.createElement('style');
  css.textContent = `
    html, body, * { cursor: none !important; user-select: none !important; -webkit-user-select: none !important; }
    ::selection { background: transparent; }
    .demo-cursor { position: fixed; left: 0; top: 0; width: 26px; height: 26px; margin: -13px 0 0 -13px; border-radius: 50%;
      border: 2px solid rgba(255,255,255,.92); background: rgba(255,255,255,.08); box-shadow: 0 0 22px rgba(160,170,255,.55), inset 0 0 8px rgba(255,255,255,.25);
      z-index: 2147483647; pointer-events: none; opacity: 0; }
    .demo-cursor i { position: absolute; left: 50%; top: 50%; width: 4px; height: 4px; margin: -2px; border-radius: 50%; background: #fff; }
    .demo-ripple { position: fixed; width: 60px; height: 60px; margin: -30px 0 0 -30px; border-radius: 50%; border: 2px solid rgba(190,200,255,.9);
      z-index: 2147483646; pointer-events: none; }
    .demo-cap { position: fixed; left: 50%; bottom: 74px; transform: translateX(-50%); z-index: 2147483640; pointer-events: none; text-align: center;
      padding: 14px 26px 16px; border-radius: 18px; background: rgba(8,9,13,.8); border: 1px solid rgba(255,255,255,.1);
      backdrop-filter: blur(14px); -webkit-backdrop-filter: blur(14px); box-shadow: 0 20px 60px rgba(0,0,0,.45); max-width: 1100px; }
    .demo-cap b { display: block; font: 700 12px Inter, system-ui, sans-serif; letter-spacing: .18em; text-transform: uppercase;
      background: linear-gradient(90deg, #8f9cff, #ff8ac2); -webkit-background-clip: text; background-clip: text; color: transparent; margin-bottom: 6px; }
    .demo-cap span { font: 600 25px Inter, system-ui, sans-serif; letter-spacing: -.01em; color: #eef0f6; white-space: nowrap; }
    .demo-cap span .ch { display: inline-block; white-space: pre; }`;
  document.head.appendChild(css);
};

async function setupPage(page) {
  await page.evaluate(OVERLAY);
  await page.evaluate(async () => {
    const A = await import('/packages/anikit/dist/anikit.js');
    const cur = document.createElement('div');
    cur.className = 'demo-cursor';
    cur.innerHTML = '<i></i>';
    document.body.appendChild(cur);
    let x = innerWidth / 2, y = innerHeight / 2;
    addEventListener('pointermove', (e) => { x = e.clientX; y = e.clientY; cur.style.left = x + 'px'; cur.style.top = y + 'px'; }, true);
    addEventListener('pointerdown', () => {
      A.animate(cur, { scale: [1, 0.72, 1] }, { duration: 0.35, ease: 'outCubic' });
      const r = document.createElement('div');
      r.className = 'demo-ripple';
      r.style.left = x + 'px';
      r.style.top = y + 'px';
      document.body.appendChild(r);
      A.animate(r, { scale: [0.3, 1.8], opacity: [1, 0] }, { duration: 0.6, ease: 'outCubic', onComplete: () => r.remove() });
    }, true);
    // Never return an animation from page.evaluate: animations are thenables and
    // virtual time is paused between frames, so awaiting one would deadlock.
    window.__cursor = (show) => void A.animate(cur, { opacity: show ? 1 : 0, scale: show ? [0.4, 1] : 0.4 }, { duration: 0.5, ease: 'outBack' });
    let current = null;
    window.__caption = (kicker, text, dur) => {
      if (current) { const old = current; A.animate(old, { opacity: 0, y: 12 }, { duration: 0.3, onComplete: () => old.remove() }); }
      const c = document.createElement('div');
      c.className = 'demo-cap';
      const b = document.createElement('b');
      b.textContent = kicker;
      const s = document.createElement('span');
      for (const ch of text) { const k = document.createElement('span'); k.className = 'ch'; k.textContent = ch; s.appendChild(k); }
      c.append(b, s);
      document.body.appendChild(c);
      current = c;
      A.set(c, { xPercent: -50 });
      c.style.transform = '';
      A.animate(c, { opacity: [0, 1], y: [24, 0], scale: [0.96, 1] }, { duration: 0.6, ease: 'spring(bounce: 0.2, duration: 0.5)' });
      A.animate(b, { opacity: [0, 1], y: [8, 0] }, { duration: 0.5, delay: 0.1, ease: 'outCubic' });
      A.animate(Array.from(s.children), { opacity: [0, 1], y: [10, 0], filter: ['blur(6px)', 'blur(0px)'] }, { duration: 0.5, ease: 'outCubic', delay: A.stagger(0.012, { start: 0.15 }) });
      A.delayedCall(dur, () => {
        if (current !== c) return;
        A.animate(c, { opacity: 0, y: 14, scale: 0.98 }, { duration: 0.45, ease: 'inCubic', onComplete: () => { c.remove(); if (current === c) current = null; } });
      });
    };
  });
}

// ---------------------------------------------------------------------------
// Rendering: WORKERS browsers each render a slice of the timeline in
// parallel. A worker fast-forwards (simulates without capturing) to its first
// frame; Math.random is seeded so every worker computes identical frames and
// the slices join seamlessly (ffmpeg concat, no re-encode).
// ---------------------------------------------------------------------------
const frames = Math.round(END * FPS);
const WORKERS = +(process.env.WORKERS || 2);
const SEG = process.env.SEG; // "from:to:file" when running as a worker

if (!SEG && !PREVIEW && WORKERS > 1) {
  const tmp = OUT + '.parts';
  mkdirSync(tmp, { recursive: true });
  const start = Math.round(FROM * FPS);
  const per = Math.ceil((frames - start) / WORKERS);
  const parts = [];
  const t0 = Date.now();
  await Promise.all(Array.from({ length: WORKERS }, (_, k) => {
    const a = start + k * per, b = Math.min(frames, a + per), file = join(tmp, `part${k}.mp4`);
    parts.push(file);
    return new Promise((res, rej) => {
      const c = spawn(process.execPath, [fileURLToPath(import.meta.url), OUT], { env: { ...process.env, SEG: `${a}:${b}:${file}` }, stdio: ['ignore', 'inherit', 'inherit'] });
      c.on('close', (code) => (code ? rej(new Error(`worker ${k} exited ${code}`)) : res()));
    });
  }));
  writeFileSync(join(tmp, 'list.txt'), parts.map((f) => `file '${f}'`).join('\n'));
  await new Promise((res, rej) => {
    const c = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(tmp, 'list.txt'), '-c', 'copy', '-movflags', '+faststart', OUT], { stdio: 'inherit' });
    c.on('close', (code) => (code ? rej(new Error('concat failed')) : res()));
  });
  rmSync(tmp, { recursive: true, force: true });
  console.log(`done → ${OUT} · ${frames - start} frames in ${((Date.now() - t0) / 1000).toFixed(0)}s with ${WORKERS} workers`);
  process.exit(0);
}

const [segFrom, segTo, segFile] = SEG ? SEG.split(':') : [Math.round(FROM * FPS), frames, OUT];
const first = +segFrom, last = +segTo, target = segFile;
const tag = SEG ? `[${(first / FPS).toFixed(1)}–${(last / FPS).toFixed(1)}s]` : '';

const browser = await playwright.chromium.launch();
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
await page.addInitScript(() => {
  globalThis.__ANIKIT_REAL_NOW__ = performance.now.bind(performance);
  // Deterministic randomness so parallel workers render identical frames.
  let a = 0x9e3779b9;
  Math.random = () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
});
await page.clock.install({ time: 0 });
await page.clock.pauseAt(1000);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
await page.route('http://anikit.local/**', (route) => {
  let f = join(root, decodeURIComponent(new URL(route.request().url()).pathname));
  if (f.endsWith('/')) f += 'index.html';
  if (!existsSync(f)) return route.fulfill({ status: 404, body: '' });
  route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
});
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto('http://anikit.local/site/', { waitUntil: 'load' });
// Wait (on the real clock; fake timers are paused) for the cosmos to boot.
for (let i = 0; i < 600; i++) {
  if (await page.evaluate(() => document.getElementById('m1-count')?.textContent !== '150,000')) break;
  await new Promise((r) => setTimeout(r, 100));
}
await setupPage(page);
await page.mouse.move(W * 0.78, H * 0.7);
// Raw CDP capture: JPEG straight from the compositor, tuned for speed.
const cdp = await page.context().newCDPSession(page);
const capture = async (quality) => Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'jpeg', quality, optimizeForSpeed: true })).data, 'base64');

let ff = null;
if (!PREVIEW) {
  const fades = [];
  if (first === 0) fades.push('fade=t=in:st=0:d=0.4');
  if (last === frames) fades.push(`fade=t=out:st=${((last - first) / FPS - 0.8).toFixed(3)}:d=0.8`);
  ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'mjpeg', '-i', '-',
    ...(fades.length ? ['-vf', fades.join(',')] : []),
    '-c:v', 'libx264', '-preset', 'medium', '-crf', '17', '-pix_fmt', 'yuv420p', '-r', String(FPS), '-movflags', '+faststart', target], { stdio: ['pipe', 'inherit', 'inherit'] });
} else mkdirSync(OUT, { recursive: true });
const write = (buf) => new Promise((res) => (ff.stdin.write(buf) ? res() : ff.stdin.once('drain', res)));

const toFrame = (t) => Math.round(t * FPS);
const evAt = new Map();
for (const e of events) { const f = toFrame(e.t); if (!evAt.has(f)) evAt.set(f, []); evAt.get(f).push(e.fn); }
let scrollSeg = null, moveSeg = null;
const t0 = Date.now();
let pending = Promise.resolve();
for (let f = 0; f < last; f++) {
  const t = f / FPS;
  for (const fn of evAt.get(f) ?? []) await fn(page);
  for (const s of scrolls) if (toFrame(s.t0) === f) scrollSeg = { ...s, from: await page.evaluate(() => scrollY), target: await resolveScroll(page, s.to) };
  for (const m of moves) if (toFrame(m.t0) === f) moveSeg = { ...m, path: [mouse, ...(await Promise.all(m.pts.map((p) => resolvePoint(page, p))))] };
  if (scrollSeg) {
    const u = Math.min(1, (t - scrollSeg.t0) / (scrollSeg.t1 - scrollSeg.t0));
    const y = scrollSeg.from + (scrollSeg.target - scrollSeg.from) * EASE[scrollSeg.ease](u);
    await page.evaluate((y) => scrollTo(0, y), y);
    if (u >= 1) scrollSeg = null;
  }
  if (moveSeg) {
    const u = Math.min(1, (t - moveSeg.t0) / (moveSeg.t1 - moveSeg.t0));
    mouse = along(moveSeg.path, EASE[moveSeg.ease](u));
    await page.mouse.move(mouse[0], mouse[1]);
    if (u >= 1) moveSeg = null;
  }
  const ms = Math.round(((f + 1) * 1000) / FPS) - Math.round((f * 1000) / FPS);
  await page.clock.runFor(ms);
  if (f < first) continue;
  if (PREVIEW) {
    if (f % PREVIEW === 0) writeFileSync(join(OUT, `f${String(f).padStart(5, '0')}.jpg`), await capture(80));
  } else {
    const buf = await capture(92);
    await pending; // encoder backpressure, overlapped with the next frame's simulation
    pending = write(buf);
  }
  if ((f - first) % 300 === 0) console.log(`${tag} ${t.toFixed(1)}s · ${((Date.now() - t0) / 1000).toFixed(0)}s elapsed`);
}
await pending;
if (ff) {
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
}
if (errors.length) console.log(tag, 'page errors:', errors);
await browser.close();
if (!SEG) console.log('done →', OUT);
