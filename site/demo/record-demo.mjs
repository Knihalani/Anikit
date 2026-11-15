// The Anikit film: a directed, frame-exact 1080p60 cut of the landing page.
//
//   node site/demo/record-demo.mjs out.mp4              full render (2 parallel workers)
//   PREVIEW=30 node site/demo/record-demo.mjs dir/      every 30th frame as JPEG
//   FROM=20 END=30 PREVIEW=10 …                         only part of the timeline
//
// How it works
// - Time is fully virtual. The page boots with anikit in manual mode and the
//   recorder calls engine.tick(ms) itself; Playwright's fake clock drives
//   timers. Every frame is exact, however long it takes to render.
// - Motion blur: each output frame blends SUB sub-frames spread over a 180°
//   shutter (ffmpeg tmix), like a film camera.
// - Camera: push-ins and pans are captured with a CDP clip + scale, so DOM
//   text stays sharp when the camera moves in.
// - Trailer cuts: full-screen title cards wipe in between scenes.
// - Parallel workers render slices; Math.random is seeded so every worker
//   computes identical frames and the slices join seamlessly.
// - The live frame-cost readout is hidden: recording load would distort it.
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

const OUT = process.argv[2] || 'anikit-film.mp4';
const FPS = 60;
const W = +(process.env.W || 1920);
const H = +(process.env.H || 1080);
const PREVIEW = +(process.env.PREVIEW || 0);
const FROM = +(process.env.FROM || 0);
const SUB = PREVIEW ? 1 : +(process.env.SUB || 3);
const SHUTTER = 0.5;

// ---------------------------------------------------------------------------
// Storyboard (seconds)
// ---------------------------------------------------------------------------
const pin = (id, f = 0) => ({ kind: 'pin', id, f });
const el = (sel, i = 0, dx = 0, dy = 0) => ({ kind: 'el', sel, i, dx, dy });
const word = (text, dx = 0, dy = 0) => ({ kind: 'word', text, dx, dy });
const nearest = (sel) => ({ kind: 'nearest', sel });
const at = (x, y) => [x * W, y * H];

let mouse = [W * 0.86, H * 0.6];
const scrolls = [], moves = [], events = [], cams = [{ t: 0, z: 2.8, x: 0.5, y: 0.37, ease: 'linear' }];
const scroll = (t0, t1, to, ease = 'inOutCubic') => scrolls.push({ t0, t1, to, ease });
const jump = (t, to) => scrolls.push({ t0: t, t1: t + 1 / FPS, to, ease: 'linear' });
const move = (t0, t1, pts, ease = 'inOutSine') => moves.push({ t0, t1, pts, ease });
const on = (t, fn) => events.push({ t, fn });
const cam = (t, z, x = 0.5, y = 0.5, ease = 'inOutCubic') => cams.push({ t, z, x, y, ease });
const click = (t, spec) => {
  on(t, async (p) => { if (spec) { mouse = await resolvePoint(p, spec); await p.mouse.move(mouse[0], mouse[1]); } await p.mouse.down(); });
  on(t + 0.06, (p) => p.mouse.up());
};
const drag = (t0, t1, from, to) => {
  move(t0 - 0.45, t0, [from]);
  on(t0, async (p) => { mouse = await resolvePoint(p, from); await p.mouse.move(mouse[0], mouse[1]); await p.mouse.down(); });
  move(t0 + 0.02, t1, [to], 'inQuad');
  on(t1, (p) => p.mouse.up());
};
const key = (t0, t1, k) => { on(t0, (p) => p.keyboard.down(k)); if (t1) on(t1, (p) => p.keyboard.up(k)); };
const cut = (t, text, bg, fg, dur = 0.72) => on(t, (p) => p.evaluate(([a, b, c, d]) => { window.__cut(a, b, c, d); }, [text, bg, fg, dur]));

// 01 · Ink drop → wordmark. The camera starts on the drop and pulls out.
cam(0.6, 2.8, 0.5, 0.37, 'linear');
cam(3.4, 1, 0.5, 0.5);
move(3.6, 6.4, [at(0.86, 0.6), at(0.16, 0.3), at(0.34, 0.44), at(0.55, 0.28), at(0.74, 0.44), at(0.9, 0.3)]);
cam(4.0, 1, 0.5, 0.5);
cam(6.3, 1.16, 0.55, 0.4);
click(6.6, at(0.6, 0.36));
cam(7.3, 1, 0.5, 0.5);
cut(7.55, 'No WebGL.', '#ff3b14', '#0c0c0c');

// 02 · Manifesto, scrubbed
jump(7.85, pin('manifesto', 0));
move(7.9, 8.3, [at(0.62, 0.5)]);
scroll(8.3, 17.0, pin('manifesto', 1), 'inOutSine');
cam(8.3, 1.12, 0.36, 0.5);
cam(10.6, 1.12, 0.36, 0.5);
cam(11.6, 1.7, 0.83, 0.52);
cam(12.8, 1.7, 0.83, 0.52);
cam(13.8, 1.1, 0.4, 0.5);
cam(15.0, 1.6, 0.83, 0.52);
cam(16.0, 1.6, 0.83, 0.52);
cam(16.8, 1.0, 0.5, 0.5);
cut(17.15, 'One engine.', '#0c0c0c', '#ece6da');

// 03 · Physics
jump(17.45, pin('physics', 0.2));
cam(17.45, 1, 0.5, 0.5);
move(18, 20, [at(0.62, 0.45), at(0.5, 0.6)]);
drag(20.6, 20.85, el('.p-letter', 3), at(0.68, 0.22));
drag(21.7, 21.95, el('.p-letter', 0), at(0.3, 0.18));
click(22.7, at(0.5, 0.92));
cam(22.6, 1, 0.5, 0.5);
cam(23.6, 1.4, 0.5, 0.78);
cam(24.3, 1.4, 0.5, 0.78);
cut(24.55, 'Rust → Wasm.', '#d2ff3c', '#0c0c0c');

// 04 · Particles
jump(24.85, pin('particles', 0));
cam(24.85, 1, 0.5, 0.5);
move(25.2, 27.2, [at(0.5, 0.3), at(0.78, 0.55), at(0.6, 0.7)]);
scroll(27, 30.2, pin('particles', 1), 'inOutSine');
click(28.7, at(0.68, 0.46));
cam(29, 1, 0.5, 0.5);
cam(30.2, 2.1, 0.68, 0.46);
cam(31, 2.1, 0.68, 0.46);
cam(31.7, 1, 0.5, 0.5);

// 05 · Time (no cut: the black field floods to acid green, ink takes over from light)
scroll(31.8, 33, pin('time', 0));
click(33.3, at(0.5, 0.5));
key(33.45, 36.7, 'Space');
cam(33.4, 1, 0.5, 0.5);
cam(36.5, 1.45, 0.5, 0.5, 'inOutSine');
move(33.6, 36.5, [at(0.42, 0.4), at(0.62, 0.62)]);
cam(37.3, 1, 0.5, 0.5);

// 06 · Paths
scroll(37.4, 38.4, pin('paths', 0));
scroll(38.45, 42.4, pin('paths', 1), 'inOutSine');
cam(39.6, 1, 0.5, 0.5);
cam(40.6, 1.3, 0.72, 0.42);
cam(41.6, 1.3, 0.72, 0.42);
cam(42.4, 1, 0.5, 0.5);

// 07 · Depth
scroll(42.6, 43.6, pin('depth', 0));
scroll(43.65, 47.6, pin('depth', 1), 'inOutSine');
move(43.8, 45.4, [at(0.4, 0.4), at(0.55, 0.55)]);
click(45.7, nearest('.card'));
cam(44.2, 1, 0.5, 0.5);
cam(45.4, 1.28, 0.5, 0.52);
cam(46.8, 1.28, 0.5, 0.52);
cam(47.5, 1, 0.5, 0.5);
cut(47.75, 'Agent-ready.', '#1f2bff', '#ece6da');

// 08 · Agents, then the whole page falls
jump(48.05, pin('agents', 0.35));
cam(48.05, 1, 0.5, 0.5);
cam(48.8, 1.55, 0.8, 0.38);
cam(49.9, 1.55, 0.8, 0.38);
cam(50.5, 1, 0.5, 0.5);
key(50.7, 50.8, 'g');
drag(52, 52.25, word('Machines'), at(0.72, 0.18));
on(53.1, (p) => p.mouse.dblclick(W * 0.42, H * 0.86));
key(54.3, 54.4, 'g');

// 09 · Install, then rewind through every colour field
scroll(55.6, 57, { kind: 'bottom' });
click(57.7, at(0.72, 0.46));
click(58.5, at(0.2, 0.36));
scroll(59.4, 63, { kind: 'y', y: 0 }, 'inOutQuart');
move(59.4, 62, [at(0.5, 0.9)]);
cam(63, 1, 0.5, 0.5);
cam(66.5, 1.12, 0.5, 0.42, 'inOutSine');
const END = +(process.env.END || 66.5);

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
  const n = pts.length - 1, f = Math.min(n - 1e-9, u * n), i = Math.floor(f);
  return catmull(pts[Math.max(0, i - 1)], pts[i], pts[i + 1], pts[Math.min(n, i + 2)], f - i);
};
cams.sort((a, b) => a.t - b.t);
const camAt = (t) => {
  let k = 0;
  while (k < cams.length - 1 && cams[k + 1].t <= t) k++;
  const a = cams[k], b = cams[k + 1];
  if (!b) return a;
  const u = EASE[b.ease]((t - a.t) / (b.t - a.t));
  return { z: a.z + (b.z - a.z) * u, x: a.x + (b.x - a.x) * u, y: a.y + (b.y - a.y) * u };
};

async function resolvePoint(page, spec) {
  if (Array.isArray(spec)) return spec;
  return page.evaluate((s) => {
    let e;
    if (s.kind === 'word') e = [...document.querySelectorAll('.gw')].find((w) => w.textContent.trim().toLowerCase() === s.text.toLowerCase() && w.getBoundingClientRect().width);
    else if (s.kind === 'nearest') {
      let best = Infinity;
      for (const c of document.querySelectorAll(s.sel)) {
        const r = c.getBoundingClientRect();
        const d = Math.hypot(r.left + r.width / 2 - innerWidth / 2, r.top + r.height / 2 - innerHeight / 2);
        if (d < best) { best = d; e = c; }
      }
    } else e = document.querySelectorAll(s.sel)[s.i];
    if (!e) return [innerWidth / 2, innerHeight / 2];
    const r = e.getBoundingClientRect();
    return [r.left + r.width / 2 + (s.dx || 0), r.top + r.height / 2 + (s.dy || 0)];
  }, spec);
}
async function resolveScroll(page, spec) {
  return page.evaluate((s) => {
    const max = document.documentElement.scrollHeight - innerHeight;
    if (s.kind === 'bottom') return max;
    if (s.kind === 'y') return s.y;
    const e = document.getElementById(s.id);
    const top = e.getBoundingClientRect().top + scrollY;
    return Math.max(0, Math.min(max, top + Math.max(0, e.offsetHeight - innerHeight) * s.f));
  }, spec);
}

async function setupPage(page) {
  await page.evaluate(async () => {
    const A = window.anikit;
    const css = document.createElement('style');
    css.textContent = `
      #hud-stats { visibility: hidden; }
      html, body, * { user-select: none !important; -webkit-user-select: none !important; }
      ::selection { background: transparent; }
      .cut { position: fixed; inset: 0; z-index: 2147483000; display: grid; place-items: center; pointer-events: none; }
      .cut p { margin: 0; font: 900 13vw/0.82 Display, 'Inter Display', system-ui; letter-spacing: -0.055em; text-transform: uppercase; white-space: nowrap; overflow: hidden; padding: 0.04em 0.06em 0.08em; }
      .cut .ch { display: inline-block; }`;
    document.head.appendChild(css);
    window.__cut = (text, bg, fg, dur) => {
      const c = document.createElement('div');
      c.className = 'cut';
      c.style.background = bg;
      c.style.color = fg;
      const p = document.createElement('p');
      for (const ch of text) { const s = document.createElement('span'); s.className = 'ch'; s.textContent = ch === ' ' ? ' ' : ch; p.appendChild(s); }
      c.appendChild(p);
      // Fit the line to the frame: never wider than 88% of the viewport.
      p.style.fontSize = Math.min(13, 88 / (text.length * 0.6)) + 'vw';
      document.body.appendChild(c);
      A.set(c, { clipPath: 'inset(100% 0% 0% 0%)' });
      A.animate(c, { clipPath: 'inset(0% 0% 0% 0%)' }, { duration: 0.22, ease: 'outExpo' });
      A.animate(Array.from(p.children), { yPercent: [110, 0], rotate: [6, 0] }, { duration: 0.42, ease: 'outExpo', delay: A.stagger(0.018, { start: 0.04 }) });
      A.delayedCall(dur, () => A.animate(c, { clipPath: 'inset(0% 0% 100% 0%)' }, { duration: 0.22, ease: 'inExpo', onComplete: () => c.remove() }));
    };
  });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------
const frames = Math.round(END * FPS);
const WORKERS = +(process.env.WORKERS || 2);
const SEG = process.env.SEG;

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
  console.log(`done → ${OUT} · ${frames - start} frames × ${SUB} sub-frames in ${((Date.now() - t0) / 1000).toFixed(0)}s with ${WORKERS} workers`);
  process.exit(0);
}

const [segFrom, segTo, segFile] = SEG ? SEG.split(':') : [Math.round(FROM * FPS), frames, OUT];
const first = +segFrom, last = +segTo, target = segFile;
const tag = SEG ? `[${(first / FPS).toFixed(1)}–${(last / FPS).toFixed(1)}s]` : '';

const browser = await playwright.chromium.launch();
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
await page.addInitScript(() => {
  globalThis.__ANIKIT_MANUAL__ = true; // the recorder ticks the engine itself
  globalThis.__ANIKIT_REAL_NOW__ = performance.now.bind(performance);
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
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.woff2': 'font/woff2' };
await page.route('http://anikit.local/**', (route) => {
  let f = join(root, decodeURIComponent(new URL(route.request().url()).pathname));
  if (f.endsWith('/')) f += 'index.html';
  if (!existsSync(f)) return route.fulfill({ status: 404, body: '' });
  route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
});
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto('http://anikit.local/site/', { waitUntil: 'load' });
for (let i = 0; i < 600; i++) {
  if (await page.evaluate(() => !!window.anikit)) break;
  await new Promise((r) => setTimeout(r, 100));
}
await setupPage(page);
await page.mouse.move(mouse[0], mouse[1]);
const cdp = await page.context().newCDPSession(page);
let scrollPos = 0;
const capture = async (quality, t) => {
  const c = camAt(t);
  const cw = W / c.z, ch = H / c.z;
  const x = Math.max(0, Math.min(W - cw, c.x * W - cw / 2));
  const y = Math.max(0, Math.min(H - ch, c.y * H - ch / 2));
  const r = await cdp.send('Page.captureScreenshot', { format: 'jpeg', quality, optimizeForSpeed: true, clip: { x, y: y + scrollPos, width: cw, height: ch, scale: c.z } });
  return Buffer.from(r.data, 'base64');
};
// Virtual time: anikit is ticked exactly; the fake clock follows (whole ms).
let vt = 0, clockMs = 0;
const advance = async (ms) => {
  vt += ms;
  await page.evaluate((d) => globalThis.__ANIKIT__.tick(d), ms);
  const whole = Math.floor(vt) - clockMs;
  if (whole > 0) { clockMs += whole; await page.clock.runFor(whole); }
};

let ff = null;
if (!PREVIEW) {
  const vf = [];
  if (SUB > 1) vf.push(`tmix=frames=${SUB}`, `select='eq(mod(n\\,${SUB})\\,${SUB - 1})'`, `setpts=N/(${FPS}*TB)`);
  if (first === 0) vf.push('fade=t=in:st=0:d=0.5');
  if (last === frames) vf.push(`fade=t=out:st=${((last - first) / FPS - 1.0).toFixed(3)}:d=1.0`);
  ff = spawn('nice', ['-n', '19', 'ffmpeg', '-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS * SUB), '-c:v', 'mjpeg', '-i', '-',
    '-vf', vf.join(','), '-c:v', 'libx264', '-preset', 'medium', '-crf', '18', '-pix_fmt', 'yuv420p', '-r', String(FPS), '-movflags', '+faststart', target], { stdio: ['pipe', 'inherit', 'inherit'] });
} else mkdirSync(OUT, { recursive: true });
const write = (buf) => new Promise((res) => (ff.stdin.write(buf) ? res() : ff.stdin.once('drain', res)));

const toFrame = (t) => Math.round(t * FPS);
const evAt = new Map();
for (const e of events) { const f = toFrame(e.t); if (!evAt.has(f)) evAt.set(f, []); evAt.get(f).push(e.fn); }
let scrollSeg = null, moveSeg = null;
const dt = 1000 / FPS;
const t0 = Date.now();
let pending = Promise.resolve();
for (let f = 0; f < last; f++) {
  const t = f / FPS;
  for (const fn of evAt.get(f) ?? []) await fn(page);
  for (const s of scrolls) if (toFrame(s.t0) === f) scrollSeg = { ...s, from: await page.evaluate(() => scrollY), target: await resolveScroll(page, s.to) };
  for (const m of moves) if (toFrame(m.t0) === f) moveSeg = { ...m, path: [mouse, ...(await Promise.all(m.pts.map((p) => resolvePoint(page, p))))] };
  if (scrollSeg) {
    const u = Math.min(1, (t + 1 / FPS - scrollSeg.t0) / (scrollSeg.t1 - scrollSeg.t0));
    const y = scrollSeg.from + (scrollSeg.target - scrollSeg.from) * EASE[scrollSeg.ease](u);
    scrollPos = await page.evaluate((y) => { scrollTo(0, y); return scrollY; }, y);
    if (u >= 1) scrollSeg = null;
  }
  if (moveSeg) {
    const u = Math.min(1, (t - moveSeg.t0) / (moveSeg.t1 - moveSeg.t0));
    mouse = along(moveSeg.path, EASE[moveSeg.ease](u));
    await page.mouse.move(mouse[0], mouse[1]);
    if (u >= 1) moveSeg = null;
  }
  if (f < first) { await advance(dt); continue; }
  if (PREVIEW) {
    await advance(dt);
    if (f % PREVIEW === 0) writeFileSync(join(OUT, `f${String(f).padStart(5, '0')}.jpg`), await capture(82, t));
  } else {
    // Sub-frames across a 180° shutter, then the rest of the frame interval.
    for (let s = 0; s < SUB; s++) {
      const step = (dt * SHUTTER) / SUB;
      await advance(step);
      const buf = await capture(92, t + (s * step) / 1000);
      await pending;
      pending = write(buf);
    }
    await advance(dt * (1 - SHUTTER));
  }
  if ((f - first) % 300 === 0) console.log(`${tag} ${t.toFixed(1)}s · ${((Date.now() - t0) / 1000).toFixed(0)}s elapsed`);
}
await pending;
if (ff) { ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
if (errors.length) console.log(tag, 'page errors:', errors);
await browser.close();
if (!SEG) console.log('done →', OUT);
