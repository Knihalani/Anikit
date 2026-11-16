// anikit/node — render compositions to video files in Node (no browser).
//
//   import { loadAnikit, renderToFile } from 'anikit/node';
//   const { anikit, node } = await loadAnikit({ fonts: { Display: '/path/Inter-Black.otf' } });
//   const film = anikit.composition({ width: 1920, height: 1080, fps: 60, duration: 10 });
//   …
//   await renderToFile(film, { out: 'film.mp4' });
//   await renderStill(film, { t: 2, out: 'frame.png' });   // one frame, for a quick look
//
// Or put the composition in a module and use the CLI (parallel workers):
//   npx anikit-render film.mjs --out film.mp4 --workers 4 [--audio track.wav]
//
// Requires the optional peer dependency `@napi-rs/canvas` (Skia) and ffmpeg.
import { spawn } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
let napi = null;
function canvasLib(from = process.cwd()) {
  if (napi) return napi;
  for (const base of [from, here, process.cwd()]) {
    try {
      napi = createRequire(join(base, 'noop.js'))('@napi-rs/canvas');
      return napi;
    } catch { /* try next */ }
  }
  throw new Error('anikit/node: install the optional peer dependency "@napi-rs/canvas" (npm i -D @napi-rs/canvas); it draws the frames in Node');
}

/**
 * Prepare Node globals (ImageData, a canvas-creating document shim, manual
 * engine time, seeded Math.random) and import anikit. Call before anything
 * else imports anikit.
 */
export async function loadAnikit(o = {}) {
  const lib = canvasLib(o.resolveFrom);
  for (const [family, path] of Object.entries(o.fonts ?? {})) lib.GlobalFonts.registerFromPath(path, family);
  globalThis.ImageData ??= lib.ImageData;
  globalThis.document ??= { createElement: () => lib.createCanvas(1, 1) };
  globalThis.__ANIKIT_MANUAL__ = true;
  if (o.seed !== false) {
    // Deterministic randomness: identical frames across runs and workers.
    let a = (o.seed ?? 0x2545f491) | 0;
    Math.random = () => {
      a |= 0; a = (a + 0x6d2b79f5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  const dist = join(here, '../dist/anikit.js');
  const { existsSync } = await import('node:fs');
  if (!existsSync(dist)) throw new Error(`anikit/node: ${dist} is missing; build the package first (npm run build in packages/anikit)`);
  const anikit = await import(dist);
  await anikit.ready;
  return { anikit, node: nodeHelpers(lib) };
}

function nodeHelpers(lib) {
  return {
    createCanvas: lib.createCanvas,
    /** Register a font file under a family name (for ctx.font and textPoints). */
    registerFont: (path, family) => lib.GlobalFonts.registerFromPath(path, family),
    /**
     * A canvas object `glow()` can render into (it resizes the backing
     * canvas). Draw it with `ctx.drawImage(handle.canvas, 0, 0, w, h)`.
     */
    glowTarget(width, height) {
      const c = lib.createCanvas(1, 1);
      return {
        canvas: c, style: {}, clientWidth: width, clientHeight: height,
        get width() { return c.width; }, set width(v) { c.width = v; },
        get height() { return c.height; }, set height(v) { c.height = v; },
        getContext: () => c.getContext('2d'),
      };
    },
  };
}

/** Zero-copy RGBA readback for Skia canvases (falls back to getImageData). */
export const readPixels = (ctx, w, h) => (typeof ctx.canvas?.data === 'function' ? ctx.canvas.data() : ctx.getImageData(0, 0, w, h).data);

/**
 * Render a composition to a video file with ffmpeg. Frames are converted to
 * I420 in WebAssembly and piped raw, so ffmpeg only encodes.
 */
export async function renderToFile(comp, o) {
  const { width: w, height: h, fps } = comp;
  const crf = String(o.crf ?? 18);
  const codec = o.codec ?? 'libx264';
  const vf = [];
  if (o.fadeIn) vf.push(`fade=t=in:st=0:d=${o.fadeIn}`);
  if (o.fadeOut) vf.push(`fade=t=out:st=${((o.to ?? comp.duration) - (o.from ?? 0) - o.fadeOut).toFixed(3)}:d=${o.fadeOut}${o.fadeColor ? `:color=${o.fadeColor}` : ''}`);
  const args = ['-y', '-loglevel', 'error', '-f', 'rawvideo', '-pix_fmt', 'yuv420p', '-s', `${w}x${h}`, '-r', String(fps), '-i', '-',
    ...(vf.length ? ['-vf', vf.join(',')] : []),
    '-c:v', codec, ...(codec === 'libx264' ? ['-preset', o.preset ?? 'medium', '-crf', crf, '-pix_fmt', 'yuv420p'] : ['-crf', crf, '-b:v', '0']),
    '-r', String(fps), '-movflags', '+faststart', o.out];
  const ff = spawn('ffmpeg', args, { stdio: ['pipe', 'inherit', 'inherit'] });
  const done = new Promise((res, rej) => ff.on('close', (c) => (c ? rej(new Error(`ffmpeg exited ${c}`)) : res())));
  // Keep up to `o.queue` frames (default 3) buffered for ffmpeg so encoding
  // overlaps rendering instead of waiting for each frame to drain.
  const frameBytes = w * h * 1.5, maxQueued = frameBytes * (o.queue ?? 3);
  const write = (b) => {
    ff.stdin.write(b);
    return ff.stdin.writableLength > maxQueued ? new Promise((r) => ff.stdin.once('drain', r)) : null;
  };
  const canvas = o.canvas ?? canvasLib().createCanvas(w, h);
  const t0 = Date.now();
  let n = 0;
  await comp.render({
    canvas, from: o.from, to: o.to, preroll: o.preroll, readPixels, createCanvas: (w, h) => canvasLib().createCanvas(w, h),
    onFrame: async (fr) => {
      // Copy out of wasm memory: the pipe may not drain before the next frame.
      const wait = write(Buffer.from(fr.i420()));
      if (wait) await wait;
      n++;
      o.onProgress?.(fr, n, (Date.now() - t0) / 1000);
    },
  });
  ff.stdin.end();
  await done;
  return { frames: n, seconds: (Date.now() - t0) / 1000 };
}

/**
 * Render the single frame at `t` seconds to an image (`.png`, or `.jpg`/`.jpeg`).
 * Everything before `t` is simulated (not drawn), so the frame matches the video.
 */
export async function renderStill(comp, o) {
  const { writeFileSync } = await import('node:fs');
  const canvas = o.canvas ?? canvasLib().createCanvas(comp.width, comp.height);
  const t = Math.max(0, Math.min(o.t, comp.duration - 1 / comp.fps));
  const jpeg = /\.jpe?g$/i.test(o.out);
  let wrote = false;
  await comp.render({
    canvas, from: t, to: t + 1 / comp.fps, readPixels, createCanvas: (w, h) => canvasLib().createCanvas(w, h),
    onFrame: () => {
      if (wrote) return;
      writeFileSync(o.out, jpeg ? canvas.toBuffer('image/jpeg', 90) : canvas.toBuffer('image/png'));
      wrote = true;
    },
  });
  return o.out;
}

/** Write every Nth frame as a JPEG (quick previews). */
export async function renderPreview(comp, o) {
  const { mkdirSync, writeFileSync } = await import('node:fs');
  mkdirSync(o.dir, { recursive: true });
  const canvas = o.canvas ?? canvasLib().createCanvas(comp.width, comp.height);
  await comp.render({
    canvas, from: o.from, to: o.to, every: o.every ?? 30, readPixels, createCanvas: (w, h) => canvasLib().createCanvas(w, h),
    onFrame: (fr) => {
      // Save the graded frame (bloom, streaks, grade…), not the raw canvas.
      const out = canvasLib().createCanvas(comp.width, comp.height);
      out.getContext('2d').putImageData(new globalThis.ImageData(new Uint8ClampedArray(fr.rgba()), comp.width, comp.height), 0, 0);
      writeFileSync(join(o.dir, `f${String(fr.index).padStart(5, '0')}.jpg`), out.toBuffer('image/jpeg', 88));
    },
  });
}

/**
 * Profile a film: renders every `every`-th frame (default one per second) and
 * reports where the time goes — scene steps, draws, pixel read-back, the
 * grade, and each `renderer3d` pass inside the draws — per sampled frame and
 * on average, with an estimate of the full render time.
 *
 *   anikit-render film.mjs --profile        (prints the table)
 *
 * Returns `{ frames: [{ t, step, draw, read, post, passes }], mean, estimate }`
 * (milliseconds; `estimate` in seconds for one worker, without encoding).
 */
export async function profileFilm(comp, o = {}) {
  const lib = canvasLib();
  const every = o.every ?? comp.fps;
  const frames = [];
  const canvas = lib.createCanvas(comp.width, comp.height);
  await comp.render({
    canvas, every, readPixels, createCanvas: (w, h) => lib.createCanvas(w, h),
    onFrame: (fr) => { frames.push({ t: fr.t, ...fr.timing, passes: { ...fr.timing.passes } }); },
  });
  const keys = ['step', 'draw', 'read', 'post'];
  const passKeys = [...new Set(frames.flatMap((f) => Object.keys(f.passes)))];
  const avg = (get) => frames.reduce((a, f) => a + get(f), 0) / Math.max(1, frames.length);
  const mean = Object.fromEntries(keys.map((k) => [k, avg((f) => f[k])]));
  mean.passes = Object.fromEntries(passKeys.map((k) => [k, avg((f) => f.passes[k] ?? 0)]));
  const per = keys.reduce((a, k) => a + mean[k], 0);
  return { frames, mean, perFrame: per, estimate: (per * comp.frames) / 1000, passKeys };
}

/** Format `profileFilm()`'s result as a table. */
export function formatProfile(p) {
  const cols = ['step', 'draw', 'read', 'post', ...p.passKeys.map((k) => `3d:${k}`)];
  const cell = (v) => (v >= 100 ? v.toFixed(0) : v.toFixed(1)).padStart(8);
  const val = (f, c) => (c.startsWith('3d:') ? f.passes[c.slice(3)] ?? 0 : f[c]);
  const lines = [`${'t (s)'.padStart(7)} ${cols.map((c) => c.padStart(8)).join(' ')}   total ms`];
  for (const f of p.frames) lines.push(`${f.t.toFixed(2).padStart(7)} ${cols.map((c) => cell(val(f, c))).join(' ')}   ${cell(f.step + f.draw + f.read + f.post)}`);
  lines.push(`${'mean'.padStart(7)} ${cols.map((c) => cell(val(p.mean, c))).join(' ')}   ${cell(p.perFrame)}`);
  const hot = [...['step', 'read', 'post'].map((k) => [k, p.mean[k]]), ...Object.entries(p.mean.passes).map(([k, v]) => [`3d:${k}`, v])].sort((a, b) => b[1] - a[1]).slice(0, 3);
  lines.push('', `≈ ${p.perFrame.toFixed(0)} ms per frame → ${p.estimate.toFixed(0)} s for the whole film on one worker (divide by --workers; encoding not included).`);
  lines.push(`biggest costs: ${hot.map(([k, v]) => `${k} ${v.toFixed(0)} ms`).join(', ')}. ('draw' includes the 3d passes; 'step' is your step() hooks and simulations.)`);
  return lines.join('\n');
}

/**
 * Review a film without watching it: renders every `every`-th frame once
 * (default 6, i.e. 10 per second at 60 fps), and returns
 * - a contact sheet (JPEG buffer): graded frames every `sheetEvery` seconds
 *   (default 0.5) tiled `cols` across with timestamps;
 * - a pacing report: motion energy over time (mean absolute luma change
 *   between samples, 0–255), holds (stretches with little motion longer than
 *   `hold` seconds, default 1), flat frames (almost no contrast) and cuts.
 *
 *   anikit-render film.mjs --review review/   (writes sheet.jpg, pacing.txt, pacing.csv)
 */
export async function reviewFilm(comp, o = {}) {
  const lib = canvasLib();
  const { width: W, height: H, fps } = comp;
  const every = o.every ?? Math.max(1, Math.round(fps / 10));
  const sheetEvery = o.sheetEvery ?? 0.5;
  const cols = o.cols ?? 8, tw = o.thumb ?? 320, th = Math.round((tw * H) / W);
  const holdS = o.hold ?? 1, quiet = o.quiet ?? 1.2;
  // Small luma probe per sample (64 px wide).
  const pw = 64, ph = Math.max(1, Math.round((pw * H) / W));
  const probe = lib.createCanvas(pw, ph), pctx = probe.getContext('2d');
  const full = lib.createCanvas(W, H), fctx = full.getContext('2d');
  const thumbs = [];
  const samples = []; // { t, energy, contrast }
  let prev = null, nextSheet = 0;
  const canvas = o.canvas ?? lib.createCanvas(W, H);
  await comp.render({
    canvas, from: o.from, to: o.to, every, readPixels, createCanvas: (w, h) => lib.createCanvas(w, h),
    onFrame: (fr) => {
      fctx.putImageData(new globalThis.ImageData(new Uint8ClampedArray(fr.rgba()), W, H), 0, 0);
      pctx.drawImage(full, 0, 0, pw, ph);
      const d = pctx.getImageData(0, 0, pw, ph).data;
      const y = new Float32Array(pw * ph);
      let mean = 0;
      for (let i = 0; i < y.length; i++) { y[i] = 0.2126 * d[i * 4] + 0.7152 * d[i * 4 + 1] + 0.0722 * d[i * 4 + 2]; mean += y[i]; }
      mean /= y.length;
      let varc = 0, energy = 0;
      for (let i = 0; i < y.length; i++) { varc += (y[i] - mean) ** 2; if (prev) energy += Math.abs(y[i] - prev[i]); }
      samples.push({ t: fr.t, energy: prev ? energy / y.length : 0, contrast: Math.sqrt(varc / y.length), mean });
      prev = y;
      if (fr.t + 1e-6 >= nextSheet) {
        const c = lib.createCanvas(tw, th);
        c.getContext('2d').drawImage(full, 0, 0, tw, th);
        thumbs.push({ t: fr.t, c });
        nextSheet += sheetEvery;
      }
    },
  });
  // Contact sheet.
  const nc = Math.max(1, Math.min(cols, thumbs.length));
  const rows = Math.ceil(thumbs.length / nc);
  const sheet = lib.createCanvas(nc * tw, rows * (th + 18));
  const sctx = sheet.getContext('2d');
  sctx.fillStyle = '#111'; sctx.fillRect(0, 0, sheet.width, sheet.height);
  sctx.font = '13px monospace'; sctx.fillStyle = '#ddd';
  thumbs.forEach(({ t, c }, i) => {
    const x = (i % nc) * tw, y = Math.floor(i / nc) * (th + 18);
    sctx.drawImage(c, x, y);
    sctx.fillText(`${t.toFixed(2)}s`, x + 4, y + th + 13);
  });
  // Pacing: holds = runs of low energy longer than holdS; flats; cuts.
  const holds = [];
  let run = null;
  for (const s of samples) {
    if (s.energy < quiet) run ??= { from: s.t, to: s.t };
    if (s.energy < quiet) run.to = s.t;
    else if (run) { if (run.to - run.from >= holdS) holds.push(run); run = null; }
  }
  if (run && run.to - run.from >= holdS) holds.push(run);
  const flats = samples.filter((s) => s.contrast < 4).map((s) => s.t);
  const energies = samples.map((s) => s.energy).sort((a, b) => a - b);
  const median = energies[Math.floor(energies.length / 2)] ?? 0;
  const cuts = samples.filter((s) => s.energy > Math.max(25, median * 6)).map((s) => s.t);
  const lines = [
    `film ${W}x${H} @ ${fps} fps, ${samples.length} samples (every ${every} frames)`,
    `motion energy: median ${median.toFixed(1)}, mean ${(energies.reduce((a, b) => a + b, 0) / Math.max(1, energies.length)).toFixed(1)} (mean |Δluma| per sample, 0-255)`,
    holds.length ? `HOLDS (little motion for ≥ ${holdS}s): ${holds.map((h) => `${h.from.toFixed(2)}–${h.to.toFixed(2)}s`).join(', ')}` : `no holds longer than ${holdS}s`,
    flats.length ? `FLAT frames (almost no contrast): ${flats.map((t) => t.toFixed(2)).join(', ')}` : 'no flat frames',
    `cuts / hits: ${cuts.map((t) => t.toFixed(2)).join(', ') || 'none'}`,
  ];
  const csv = 't,energy,contrast,mean\n' + samples.map((s) => `${s.t.toFixed(3)},${s.energy.toFixed(2)},${s.contrast.toFixed(2)},${s.mean.toFixed(1)}`).join('\n');
  return { sheet: sheet.toBuffer('image/jpeg', 85), report: lines.join('\n'), csv, samples, holds, flats, cuts };
}

