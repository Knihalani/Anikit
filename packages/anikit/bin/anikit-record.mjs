#!/usr/bin/env node
// anikit-record — frame-exact screenshots / recordings of pages animated with
// Anikit, headlessly (Playwright + Chromium).
//
//   anikit-record <url|file> [--out clip.gif|clip.mp4|clip.webm|frames/] [--duration 3] [--fps 30]
//                 [--size 1280x720] [--selector '#hero'] [--scale 1] [--wait 0] [--realtime]
//   anikit-record <url|file> --screenshot shot.png [--at 1.5]
//
// The page's Anikit engine is started in manual time and advanced exactly
// 1/fps per frame, so recordings are perfect regardless of machine speed.
// Local files are served from the current directory (ES modules work).
// GIFs are encoded by Anikit's WebAssembly encoder; MP4/WebM need ffmpeg.
import { createRequire } from 'node:module';
import { readFileSync, existsSync, mkdirSync, writeFileSync, statSync } from 'node:fs';
import { join, extname, resolve, relative, dirname } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { inflateSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const opt = (name, d) => {
  const i = args.indexOf('--' + name);
  return i >= 0 ? args[i + 1] : d;
};
const flag = (name) => args.includes('--' + name);
const target = args.find((a, i) => !a.startsWith('--') && (i === 0 || !args[i - 1].startsWith('--') || ['--realtime'].includes(args[i - 1])));
if (!target || flag('help')) {
  console.log(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(1, 12).map((l) => l.replace(/^\/\/ ?/, '')).join('\n'));
  process.exit(target ? 0 : 1);
}
const fps = Number(opt('fps', 30));
const duration = Number(opt('duration', 3));
const [vw, vh] = opt('size', '1280x720').split('x').map(Number);
const scale = Number(opt('scale', 1));
const selector = opt('selector');
const wait = Number(opt('wait', 0));
const shot = opt('screenshot');
const out = opt('out', shot ? undefined : 'anikit.gif');
const realtime = flag('realtime');

const require = createRequire(join(process.cwd(), 'package.json'));
let playwright;
for (const p of ['playwright', 'playwright-core', '@playwright/test']) {
  try { playwright = require(p); break; } catch {}
}
if (!playwright) {
  try { playwright = createRequire(import.meta.url)('playwright'); } catch {}
}
if (!playwright) {
  console.error('anikit-record needs Playwright: npm i -D playwright && npx playwright install chromium');
  process.exit(1);
}

// Serve local files over a fake origin so ES modules and fetch() work.
const isUrl = /^https?:\/\//.test(target);
const ORIGIN = 'http://anikit.local';
const root = process.cwd();
let url = target;
if (!isUrl) {
  const abs = resolve(target);
  const rel = relative(root, abs).split('\\').join('/');
  url = `${ORIGIN}/${rel}${existsSync(abs) && statSync(abs).isDirectory() ? '/' : ''}`;
}
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2' };

const browser = await playwright.chromium.launch();
const page = await browser.newPage({ viewport: { width: vw, height: vh }, deviceScaleFactor: scale });
await page.route(`${ORIGIN}/**`, (route) => {
  let f = join(root, decodeURIComponent(new URL(route.request().url()).pathname));
  if (existsSync(f) && statSync(f).isDirectory()) f = join(f, 'index.html');
  if (!existsSync(f)) return route.fulfill({ status: 404, body: 'not found' });
  route.fulfill({ status: 200, contentType: TYPES[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
});
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
if (!realtime) await page.addInitScript(() => { globalThis.__ANIKIT_MANUAL__ = true; });
await page.goto(url, { waitUntil: 'networkidle' });
const hasEngine = await page.waitForFunction(() => !!globalThis.__ANIKIT__, null, { timeout: 5000 }).then(() => true, () => false);
if (!hasEngine && !realtime) console.warn('anikit-record: no Anikit engine found on the page; recording in real time.');
const deterministic = hasEngine && !realtime;
const step = async (ms) => {
  if (deterministic) await page.evaluate((d) => globalThis.__ANIKIT__.tick(d), ms);
  else await page.waitForTimeout(ms);
};
if (wait) await step(wait * 1000);
const el = selector ? await page.$(selector) : null;
if (selector && !el) {
  console.error(`anikit-record: selector "${selector}" not found`);
  process.exit(1);
}
// Lock the capture region so every frame has identical dimensions.
let clip = null;
if (el) {
  const b = await el.boundingBox();
  clip = { x: Math.max(0, Math.floor(b.x)), y: Math.max(0, Math.floor(b.y)), width: Math.ceil(Math.min(b.width, vw - b.x)), height: Math.ceil(Math.min(b.height, vh - b.y)) };
}
const capture = () => page.screenshot({ type: 'png', ...(clip ? { clip } : {}) });

if (shot) {
  await step(Number(opt('at', 0)) * 1000);
  writeFileSync(shot, await capture());
  console.log(`anikit-record: wrote ${shot}`);
  await browser.close();
  process.exit(0);
}

const n = Math.max(1, Math.round(duration * fps));
const frames = [];
const t0 = Date.now();
for (let i = 0; i < n; i++) {
  await step(i === 0 ? 0 : 1000 / fps);
  frames.push(await capture());
  if (process.stdout.isTTY) process.stdout.write(`\ranikit-record: frame ${i + 1}/${n}`);
}
if (process.stdout.isTTY) process.stdout.write('\n');
await browser.close();
if (errors.length) console.warn('anikit-record: page errors:\n  ' + errors.join('\n  '));

const ext = extname(out).toLowerCase();
if (!ext || out.endsWith('/')) {
  mkdirSync(out, { recursive: true });
  frames.forEach((f, i) => writeFileSync(join(out, `frame-${String(i).padStart(5, '0')}.png`), f));
} else if (ext === '.gif') {
  const { encodeGif, ready } = await import(join(dirname(fileURLToPath(import.meta.url)), '../dist/anikit.js'));
  await ready;
  const decoded = frames.map(decodePng);
  const { width, height } = decoded[0];
  writeFileSync(out, encodeGif(decoded.map((d) => d.rgba), width, height, fps));
} else {
  if (spawnSync('ffmpeg', ['-version']).status !== 0) {
    console.error('anikit-record: ffmpeg is required for video output (or use .gif / a frames directory)');
    process.exit(1);
  }
  const codec = ext === '.webm' ? ['-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '32'] : ['-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18', '-movflags', '+faststart'];
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-i', '-', ...codec, '-vf', 'pad=ceil(iw/2)*2:ceil(ih/2)*2', out], { stdio: ['pipe', 'inherit', 'inherit'] });
  for (const f of frames) ff.stdin.write(f);
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
}
console.log(`anikit-record: ${n} frames @ ${fps}fps → ${out} (${((Date.now() - t0) / 1000).toFixed(1)}s, ${deterministic ? 'frame-exact' : 'real-time'})`);

/** Minimal PNG decoder (8-bit RGB/RGBA, non-interlaced — what Chromium emits). */
function decodePng(buf) {
  let p = 8;
  let width = 0, height = 0, ctype = 6;
  const idat = [];
  while (p < buf.length) {
    const len = buf.readUInt32BE(p);
    const type = buf.toString('ascii', p + 4, p + 8);
    const data = buf.subarray(p + 8, p + 8 + len);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      ctype = data[9];
    } else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
    p += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const bpp = ctype === 6 ? 4 : 3;
  const stride = width * bpp;
  const rgba = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride);
  let at = 0;
  for (let y = 0; y < height; y++) {
    const f = raw[at++];
    const line = new Uint8Array(raw.subarray(at, at + stride));
    at += stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? line[x - bpp] : 0;
      const b = prev[x];
      const c = x >= bpp ? prev[x - bpp] : 0;
      let v = line[x];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const pp = a + b - c;
        const pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      line[x] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4;
      rgba[o] = line[x * bpp];
      rgba[o + 1] = line[x * bpp + 1];
      rgba[o + 2] = line[x * bpp + 2];
      rgba[o + 3] = bpp === 4 ? line[x * bpp + 3] : 255;
    }
    prev = line;
  }
  return { width, height, rgba };
}
