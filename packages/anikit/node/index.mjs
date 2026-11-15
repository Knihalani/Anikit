// anikit/node — render compositions to video files in Node (no browser).
//
//   import { loadAnikit, renderToFile } from 'anikit/node';
//   const { anikit, node } = await loadAnikit({ fonts: { Display: '/path/Inter-Black.otf' } });
//   const film = anikit.composition({ width: 1920, height: 1080, fps: 60, duration: 10 });
//   …
//   await renderToFile(film, { out: 'film.mp4' });
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
  throw new Error('anikit/node: install the optional peer dependency "@napi-rs/canvas"');
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
  const anikit = await import(join(here, '../dist/anikit.js'));
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
  const write = (b) => new Promise((r) => (ff.stdin.write(b) ? r() : ff.stdin.once('drain', r)));
  const canvas = o.canvas ?? canvasLib().createCanvas(w, h);
  const t0 = Date.now();
  let n = 0;
  await comp.render({
    canvas, from: o.from, to: o.to, preroll: o.preroll, readPixels, createCanvas: (w, h) => canvasLib().createCanvas(w, h),
    onFrame: async (fr) => {
      // Copy out of wasm memory: the pipe may not drain before the next frame.
      await write(Buffer.from(fr.i420()));
      n++;
      o.onProgress?.(fr, n, (Date.now() - t0) / 1000);
    },
  });
  ff.stdin.end();
  await done;
  return { frames: n, seconds: (Date.now() - t0) / 1000 };
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
