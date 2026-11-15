#!/usr/bin/env node
// anikit-render — render an anikit composition module to video, in parallel.
// `anikit-render --help` prints the usage below.
//
// The module default-exports `async ({ anikit, node }) => composition`. Every
// worker builds the same composition (randomness is seeded), simulates up to
// its slice without drawing, pre-rolls half a second, renders its frames and
// encodes them; slices are concatenated without re-encoding.
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync, existsSync, copyFileSync } from 'node:fs';
import { join, resolve, dirname, basename } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { cpus } from 'node:os';

const HELP = `anikit-render — render an anikit composition to video, frame-exact, in Node (no browser)

Usage
  anikit-render --init [film.mjs]                     write a working starter film
  anikit-render <film.mjs> --still <t> [--out f.png]  one frame at t seconds, as PNG (or .jpg): a quick check
  anikit-render <film.mjs> --preview <N> --out dir/   every Nth frame as JPEG
  anikit-render <film.mjs> --out film.mp4 [options]   render the video

Options
  --out <path>          video file, image (--still) or directory (--preview). Default anikit.mp4
  --workers <n>         parallel render processes (default: CPU cores - 1, at most 8)
  --from <s>, --to <s>  render only this time range, in seconds
  --audio <file>        mux a soundtrack (cut to the video's length)
  --fade-in <s>         fade in from --fade-color (default black) at the start
  --fade-out <s>        fade out at the end
  --crf <n>             quality: lower is better and bigger (default 18)
  --preset <name>       x264 speed/size trade-off (default medium)
  --codec <name>        ffmpeg video codec (default libx264)
  -h, --help            show this help

The module default-exports  async ({ anikit, node }) => composition;  run --init
for a complete example. Needs ffmpeg on PATH and the peer dependency
@napi-rs/canvas (npm i -D @napi-rs/canvas).

Workflow: --still 2 to look at one frame, --preview 15 to check the whole film,
then render. Output never depends on machine speed.`;

const VALUE = ['out', 'workers', 'from', 'to', 'audio', 'fade-in', 'fade-out', 'fade-color', 'crf', 'preset', 'codec', 'preview', 'still'];
const BOOL = ['help', 'init'];
const fail = (msg) => {
  console.error(`anikit-render: ${msg}\nRun "anikit-render --help" for usage.`);
  process.exit(1);
};
const closest = (s, list) => {
  let best = '', bd = Infinity;
  for (const c of list) {
    const d = Array.from({ length: c.length + 1 }, (_, j) => j);
    for (let i = 1; i <= s.length; i++) {
      let prev = d[0]; d[0] = i;
      for (let j = 1; j <= c.length; j++) { const t = d[j]; d[j] = Math.min(d[j] + 1, d[j - 1] + 1, prev + (s[i - 1] === c[j - 1] ? 0 : 1)); prev = t; }
    }
    if (d[c.length] < bd) { bd = d[c.length]; best = c; }
  }
  return bd <= Math.max(2, s.length / 3) ? ` Did you mean --${best}?` : '';
};

const args = process.argv.slice(2);
const opts = {};
const positional = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '-h') { opts.help = true; continue; }
  if (!a.startsWith('--')) { positional.push(a); continue; }
  const eq = a.indexOf('=');
  const name = a.slice(2, eq < 0 ? undefined : eq);
  if (BOOL.includes(name)) { opts[name] = true; continue; }
  if (!VALUE.includes(name)) fail(`unknown option --${name}.${closest(name, [...VALUE, ...BOOL])}`);
  const val = eq >= 0 ? a.slice(eq + 1) : args[++i];
  if (val === undefined || (eq < 0 && val.startsWith('--'))) fail(`--${name} needs a value.`);
  opts[name] = val;
}
const opt = (name, d) => opts[name] ?? d;
const num = (name, d) => {
  if (opts[name] === undefined) return d;
  const n = Number(opts[name]);
  if (!Number.isFinite(n)) fail(`--${name} must be a number${['workers', 'preview', 'crf'].includes(name) ? '' : ' of seconds'}, got "${opts[name]}".`);
  return n;
};
if (opts.help) {
  console.log(HELP);
  process.exit(0);
}
const here = dirname(fileURLToPath(import.meta.url));
if (opts.init) {
  const name = positional[0] || 'film.mjs';
  const dest = resolve(name);
  if (existsSync(dest)) fail(`--init: ${dest} already exists; pass another name: anikit-render --init my-film.mjs`);
  copyFileSync(join(here, '../templates/film.mjs'), dest);
  console.log(`wrote ${dest}\n  look:    npx anikit-render ${name} --still 1.5 --out still.png\n  preview: npx anikit-render ${name} --preview 15 --out preview/\n  render:  npx anikit-render ${name} --out film.mp4`);
  process.exit(0);
}
const file = positional[0];
if (!file) {
  console.log(HELP);
  process.exit(1);
}
if (positional.length > 1) fail(`unexpected argument "${positional[1]}". Options start with -- (e.g. --out ${positional[1]}).`);
if (!existsSync(resolve(file))) fail(`${file} not found.${/\.m?js$/.test(file) ? '' : ' Pass the composition module, e.g. film.mjs (anikit-render --init writes one).'}`);
for (const n of ['workers', 'preview', 'still', 'from', 'to', 'fade-in', 'fade-out', 'crf']) num(n);
if (opts.preview !== undefined && !(num('preview') >= 1)) fail('--preview takes N >= 1: write every Nth frame.');
if (opts.workers !== undefined && !(num('workers') >= 1)) fail('--workers must be at least 1.');
const out = resolve(opt('out', 'anikit.mp4'));
const modUrl = pathToFileURL(resolve(file)).href;
const SEG = process.env.ANIKIT_SEG;

async function build() {
  const { loadAnikit } = await import(join(here, '../node/index.mjs'));
  const fonts = JSON.parse(process.env.ANIKIT_FONTS || '{}');
  let env;
  try {
    env = await loadAnikit({ fonts, resolveFrom: dirname(resolve(file)) });
  } catch (e) {
    fail(String(e?.message ?? e).replace(/^anikit\/node: /, ''));
  }
  const mod = await import(modUrl);
  if (typeof mod.default !== 'function') fail(`${file} must default-export a function:\n  export default async ({ anikit, node }) => { const film = anikit.composition({ … }); …; return film; }`);
  const comp = await mod.default(env);
  if (!comp || typeof comp.render !== 'function') fail(`the default export of ${file} must return the composition (anikit.composition({ … })); it returned ${comp === undefined ? 'nothing (missing "return film"?)' : typeof comp}.`);
  return { comp, env };
}

const node = await import(join(here, '../node/index.mjs'));
const enc = { crf: opt('crf', 18), preset: opt('preset', 'medium'), codec: opt('codec', 'libx264') };

if (opts.still !== undefined) {
  const t = num('still');
  const { comp } = await build();
  if (t < 0 || t > comp.duration) fail(`--still ${t} is outside the composition (0 to ${comp.duration} s).`);
  const dest = resolve(opt('out', `${basename(file).replace(/\.[cm]?[jt]s$/, '')}-${t}s.png`));
  if (!/\.(png|jpe?g)$/i.test(dest)) fail(`--still writes an image: use --out frame.png (or .jpg), not ${basename(dest)}.`);
  await node.renderStill(comp, { t, out: dest });
  console.log('still →', dest);
  process.exit(0);
}

if (opts.preview !== undefined) {
  const { comp } = await build();
  await node.renderPreview(comp, { dir: out, every: num('preview'), from: num('from'), to: num('to') });
  console.log('preview →', out);
  process.exit(0);
}

if (!SEG && spawnSync('ffmpeg', ['-version'], { stdio: 'ignore' }).error) {
  fail('ffmpeg was not found on PATH; it encodes the video (apt install ffmpeg, brew install ffmpeg, or ffmpeg.org). --still and --preview work without it.');
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
