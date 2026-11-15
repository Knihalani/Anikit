#!/usr/bin/env node
// anikit-render — render an anikit composition module to video, in parallel.
//
//   anikit-render film.mjs --out film.mp4 [--workers 4] [--crf 18] [--preset medium]
//                [--audio track.wav] [--from 0 --to 10] [--fade-in 0.3 --fade-out 0.8]
//   anikit-render film.mjs --preview 30 --out frames/
//   anikit-render film.mjs --review --out review/   (contact sheet + pacing report)
//
// The module default-exports `async ({ anikit, node }) => composition`. Every
// worker builds the same composition (randomness is seeded), simulates up to
// its slice without drawing, pre-rolls half a second, renders its frames and
// encodes them; slices are concatenated without re-encoding.
import { spawn } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { cpus } from 'node:os';

const args = process.argv.slice(2);
const opt = (name, d) => { const i = args.indexOf('--' + name); return i >= 0 ? args[i + 1] : d; };
const file = args[0] === '--init' ? 'init' : args.find((a, i) => !a.startsWith('--') && (i === 0 || !args[i - 1].startsWith('--')));
if (!file) {
  console.log('usage: anikit-render <composition.mjs> --out film.mp4 [--workers N] [--audio track.wav] [--preview N]\n       anikit-render --init [film.mjs] [--template film|journey]   (write a starter film)');
  process.exit(1);
}
if (args[0] === '--init') {
  const { copyFileSync, existsSync: ex } = await import('node:fs');
  // --init [file] [--template film | journey]
  const name = args[1] && !args[1].startsWith('--') ? args[1] : 'film.mjs';
  const tpl = opt('template', 'film');
  const src = join(dirname(fileURLToPath(import.meta.url)), `../templates/${tpl}.mjs`);
  if (!ex(src)) { console.error(`anikit-render --init: unknown template "${tpl}" (film, journey)`); process.exit(1); }
  const dest = resolve(name);
  if (ex(dest)) { console.error(`anikit-render --init: ${dest} already exists`); process.exit(1); }
  copyFileSync(src, dest);
  console.log(`wrote ${dest} (template: ${tpl})\n  review:  npx anikit-render ${name} --review review/\n  preview: npx anikit-render ${name} --preview 15 --out preview/\n  render:  npx anikit-render ${name} --out film.mp4`);
  process.exit(0);
}
const out = resolve(opt('out', 'anikit.mp4'));
const modUrl = pathToFileURL(resolve(file)).href;
const SEG = process.env.ANIKIT_SEG;
const here = dirname(fileURLToPath(import.meta.url));

async function build() {
  const { loadAnikit } = await import(join(here, '../node/index.mjs'));
  const fonts = JSON.parse(process.env.ANIKIT_FONTS || '{}');
  const env = await loadAnikit({ fonts, resolveFrom: dirname(resolve(file)) });
  const mod = await import(modUrl);
  const comp = await mod.default(env);
  return { comp, env };
}

const node = await import(join(here, '../node/index.mjs'));
const enc = { crf: opt('crf', 18), preset: opt('preset', 'medium'), codec: opt('codec', 'libx264') };

if (opt('review')) {
  // One pass: contact sheet + pacing report, so an agent can judge a film without watching it.
  const { comp } = await build();
  const rv = opt('review');
  const dir = rv && !rv.startsWith('--') ? resolve(rv) : args.includes('--out') ? out : resolve('review');
  mkdirSync(dir, { recursive: true });
  const r = await node.reviewFilm(comp, { every: opt('every') && +opt('every'), sheetEvery: opt('sheet-every') && +opt('sheet-every'), from: opt('from') && +opt('from'), to: opt('to') && +opt('to') });
  writeFileSync(join(dir, 'sheet.jpg'), r.sheet);
  writeFileSync(join(dir, 'pacing.txt'), r.report + '\n');
  writeFileSync(join(dir, 'pacing.csv'), r.csv + '\n');
  console.log(r.report + `\nreview → ${dir}/sheet.jpg, pacing.txt, pacing.csv`);
  process.exit(0);
}

if (opt('preview')) {
  const { comp } = await build();
  await node.renderPreview(comp, { dir: out, every: +opt('preview'), from: opt('from') && +opt('from'), to: opt('to') && +opt('to') });
  console.log('preview →', out);
  process.exit(0);
}

if (SEG) {
  const [a, b, target, fi, fo, fc] = SEG.split('|');
  const { comp } = await build();
  // Fades are applied by the first/last worker while it encodes, so joining
  // the slices never needs a second encode.
  await node.renderToFile(comp, { ...enc, out: target, from: +a, to: +b, fadeIn: +fi || 0, fadeOut: +fo || 0, fadeColor: fc || undefined });
  process.exit(0);
}

const { comp } = await build();
const from = +(opt('from', 0)), to = +(opt('to', comp.duration));
const workers = Math.max(1, +(opt('workers', Math.max(1, Math.min(8, cpus().length - 1)))));
const fps = comp.fps;
const frames = Math.round((to - from) * fps);
const per = Math.ceil(frames / workers);
const t0 = Date.now();
const fadeIn = +opt('fade-in', 0), fadeOut = +opt('fade-out', 0), audio = opt('audio');
const tmp = out + '.parts';
mkdirSync(tmp, { recursive: true });
const parts = [];
await Promise.all(Array.from({ length: workers }, (_, k) => {
  const fa = Math.round(from * fps) + k * per, fb = Math.min(Math.round(to * fps), fa + per);
  if (fa >= fb) return null;
  const part = join(tmp, `p${k}.mp4`);
  parts.push(part);
  return new Promise((res, rej) => spawn(process.execPath, [fileURLToPath(import.meta.url), ...args], {
    env: { ...process.env, ANIKIT_SEG: [fa / fps, fb / fps, part, k === 0 ? fadeIn : 0, fb === Math.round(to * fps) ? fadeOut : 0, opt('fade-color', '')].join('|') }, stdio: 'inherit',
  }).on('close', (c) => (c ? rej(new Error(`worker ${k} exited ${c}`)) : res())));
}));
writeFileSync(join(tmp, 'list.txt'), parts.map((p) => `file '${p}'`).join('\n'));
const ffArgs = ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(tmp, 'list.txt')];
if (audio) ffArgs.push('-ss', String(from), '-i', resolve(audio));
ffArgs.push('-c:v', 'copy');
if (audio) ffArgs.push('-map', '0:v', '-map', '1:a', '-c:a', 'aac', '-b:a', '192k', '-shortest');
ffArgs.push('-movflags', '+faststart', out);
await new Promise((res, rej) => spawn('ffmpeg', ffArgs, { stdio: 'inherit' }).on('close', (c) => (c ? rej(new Error('ffmpeg mux failed')) : res())));
rmSync(tmp, { recursive: true, force: true });
const secs = (Date.now() - t0) / 1000;
console.log(`anikit-render: ${frames} frames (${(frames / fps).toFixed(1)} s @ ${fps} fps) → ${out} in ${secs.toFixed(0)} s · ${(frames / secs).toFixed(1)} fps · ${workers} workers`);
