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
  anikit-render --init [film.mjs] [--template film|journey]   write a working starter film
  anikit-render <film.mjs> --still <t> [--out f.png]  one frame at t seconds, as PNG (or .jpg): a quick check
  anikit-render <film.mjs> --still <t> --watch        re-render that frame every time the file is saved
  anikit-render <film.mjs> --review [dir/]            contact sheet + pacing report (default dir: review/)
  anikit-render <film.mjs> --preview <N> --out dir/   every Nth frame as JPEG
  anikit-render <film.mjs> --profile [--every N]      where render time goes, per frame and per 3D pass
  anikit-render --check <video.mp4>                   frame-to-frame check of a rendered video: pops, seams, freezes
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
  --template <name>     with --init: film (default) or journey
  --every <n>           with --review: analyse every nth frame; with --profile: time every nth (default 1/s)
  --sheet-every <n>     with --review: one contact-sheet tile every nth frame
  -h, --help            show this help

The module default-exports  async ({ anikit, node }) => composition;  run --init
for a complete example. Needs ffmpeg on PATH and the peer dependency
@napi-rs/canvas (npm i -D @napi-rs/canvas).

Workflow: --still 2 to look at one frame, --review to check pacing and the whole
film at a glance (or --preview 15 for full frames), then render. Output never depends on machine speed.`;

const VALUE = ['check', 'out', 'workers', 'from', 'to', 'audio', 'fade-in', 'fade-out', 'fade-color', 'crf', 'preset', 'codec', 'preview', 'still', 'template', 'every', 'sheet-every'];
const BOOL = ['help', 'init', 'profile', 'watch'];
/** Flags whose value is optional (`--review` or `--review dir/`). */
const OPTIONAL = ['review'];
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
  if (OPTIONAL.includes(name)) {
    if (eq >= 0) opts[name] = a.slice(eq + 1);
    else if (args[i + 1] !== undefined && !args[i + 1].startsWith('--') && /[\\/]$|^[^.]*$/.test(args[i + 1]) && positional.length) opts[name] = args[++i];
    else opts[name] = true;
    continue;
  }
  if (!VALUE.includes(name)) fail(`unknown option --${name}.${closest(name, [...VALUE, ...BOOL, ...OPTIONAL])}`);
  const val = eq >= 0 ? a.slice(eq + 1) : args[++i];
  if (val === undefined || (eq < 0 && val.startsWith('--'))) fail(`--${name} needs a value.`);
  opts[name] = val;
}
const opt = (name, d) => opts[name] ?? d;
const num = (name, d) => {
  if (opts[name] === undefined) return d;
  const n = Number(opts[name]);
  if (!Number.isFinite(n)) fail(`--${name} must be a number${['workers', 'preview', 'crf', 'every', 'sheet-every'].includes(name) ? '' : ' of seconds'}, got "${opts[name]}".`);
  return n;
};
if (opts.help) {
  console.log(HELP);
  process.exit(0);
}
const here = dirname(fileURLToPath(import.meta.url));
if (opts.init) {
  // --init [file] [--template film | journey]
  const name = positional[0] || 'film.mjs';
  const tpl = opt('template', 'film');
  const src = join(here, `../templates/${tpl}.mjs`);
  if (!/^[\w-]+$/.test(tpl) || !existsSync(src)) fail(`--init: unknown template "${tpl}"; use --template film or --template journey.`);
  const dest = resolve(name);
  if (existsSync(dest)) fail(`--init: ${dest} already exists; pass another name: anikit-render --init my-film.mjs`);
  copyFileSync(src, dest);
  console.log(`wrote ${dest} (template: ${tpl})\n  look:    npx anikit-render ${name} --still 1.5 --out still.png\n  review:  npx anikit-render ${name} --review review/\n  preview: npx anikit-render ${name} --preview 15 --out preview/\n  render:  npx anikit-render ${name} --out film.mp4`);
  process.exit(0);
}
if (opts.check !== undefined) {
  // Check a rendered video (no composition module needed).
  const video = resolve(String(opts.check));
  if (!existsSync(video)) fail(`--check: ${video} not found.`);
  const node = await import(join(here, '../node/index.mjs'));
  console.log((await node.checkVideo(video)).report);
  process.exit(0);
}
const file = positional[0];
if (!file) {
  console.log(HELP);
  process.exit(1);
}
if (positional.length > 1) fail(`unexpected argument "${positional[1]}". Options start with -- (e.g. --out ${positional[1]}).`);
if (!existsSync(resolve(file))) fail(`${file} not found.${/\.m?js$/.test(file) ? '' : ' Pass the composition module, e.g. film.mjs (anikit-render --init writes one).'}`);
for (const n of ['workers', 'preview', 'still', 'from', 'to', 'fade-in', 'fade-out', 'crf', 'every', 'sheet-every']) num(n);
if (opts.preview !== undefined && !(num('preview') >= 1)) fail('--preview takes N >= 1: write every Nth frame.');
if (opts.workers !== undefined && !(num('workers') >= 1)) fail('--workers must be at least 1.');
const out = resolve(opt('out', 'anikit.mp4'));
const modUrl = pathToFileURL(resolve(file)).href;
const SEG = process.env.ANIKIT_SEG;

async function build(version = 0) {
  const { loadAnikit } = await import(join(here, '../node/index.mjs'));
  const fonts = JSON.parse(process.env.ANIKIT_FONTS || '{}');
  let env;
  try {
    env = await loadAnikit({ fonts, resolveFrom: dirname(resolve(file)) });
  } catch (e) {
    fail(String(e?.message ?? e).replace(/^anikit\/node: /, ''));
  }
  const mod = await import(version ? `${modUrl}?v=${version}` : modUrl);
  if (typeof mod.default !== 'function') fail(`${file} must default-export a function:\n  export default async ({ anikit, node }) => { const film = anikit.composition({ … }); …; return film; }`);
  const comp = await mod.default(env);
  if (!comp || typeof comp.render !== 'function') fail(`the default export of ${file} must return the composition (anikit.composition({ … })); it returned ${comp === undefined ? 'nothing (missing "return film"?)' : typeof comp}.`);
  return { comp, env };
}

const node = await import(join(here, '../node/index.mjs'));
const enc = { crf: opt('crf', 18), preset: opt('preset', 'medium'), codec: opt('codec', 'libx264') };

if (opts.review !== undefined) {
  // One pass: contact sheet + pacing report, so an agent can judge a film without watching it.
  const { comp } = await build();
  const rv = opts.review;
  const dir = rv !== true ? resolve(rv) : opts.out !== undefined ? out : resolve('review');
  mkdirSync(dir, { recursive: true });
  const r = await node.reviewFilm(comp, { every: num('every'), sheetEvery: num('sheet-every'), from: num('from'), to: num('to') });
  writeFileSync(join(dir, 'sheet.jpg'), r.sheet);
  writeFileSync(join(dir, 'pacing.txt'), r.report + '\n');
  writeFileSync(join(dir, 'pacing.csv'), r.csv + '\n');
  console.log(r.report + `\nreview → ${dir}/sheet.jpg, pacing.txt, pacing.csv`);
  process.exit(0);
}

if (opts.profile) {
  // Where the time goes: steps, draws (and each renderer3d pass), read-back, grade.
  const { comp } = await build();
  const p = await node.profileFilm(comp, { every: num('every') });
  console.log(node.formatProfile(p));
  process.exit(0);
}

if (opts.still !== undefined) {
  const t = num('still');
  const { comp } = await build();
  if (t < 0 || t > comp.duration) fail(`--still ${t} is outside the composition (0 to ${comp.duration} s).`);
  const dest = resolve(opt('out', `${basename(file).replace(/\.[cm]?[jt]s$/, '')}-${t}s.png`));
  if (!/\.(png|jpe?g)$/i.test(dest)) fail(`--still writes an image: use --out frame.png (or .jpg), not ${basename(dest)}.`);
  await node.renderStill(comp, { t, out: dest });
  console.log('still →', dest);
  if (!opts.watch) process.exit(0);
  // Watch: re-import the module (cache-busted) and re-render on every save.
  const { watch } = await import('node:fs');
  let version = 1, timer = null, busy = false, again = false;
  const rerender = async () => {
    if (busy) { again = true; return; }
    busy = true;
    const t0 = Date.now();
    try {
      const { comp: next } = await build(version++);
      await node.renderStill(next, { t: Math.min(t, next.duration), out: dest });
      console.log(`still → ${dest} (${Date.now() - t0} ms)`);
    } catch (e) {
      console.error(`anikit-render: ${e?.stack ?? e}`);
    }
    busy = false;
    if (again) { again = false; rerender(); }
  };
  watch(resolve(file), () => { clearTimeout(timer); timer = setTimeout(rerender, 120); });
  console.log(`watching ${file} — save it to re-render (Ctrl+C to stop)`);
  await new Promise(() => {});
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
  // Report progress to the parent (it prints one aggregated line).
  let last = 0;
  await node.renderToFile(comp, {
    ...enc, out: target, from: +a, to: +b, fadeIn: +fi || 0, fadeOut: +fo || 0, fadeColor: fc || undefined,
    onProgress: (_fr, n) => { const now = Date.now(); if (process.send && now - last > 250) { last = now; process.send({ n }); } },
  });
  process.send?.({ n: Math.round((+b - +a) * comp.fps) });
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
// Progress: workers report frames done over IPC; one line on a terminal, every 10% otherwise.
const done = new Array(workers).fill(0);
const tty = process.stderr.isTTY;
let lastDecile = -1;
const report = () => {
  const n = done.reduce((x, y) => x + y, 0), secs = (Date.now() - t0) / 1000, rate = n / Math.max(0.001, secs);
  const eta = rate > 0 ? (frames - n) / rate : 0;
  const line = `anikit-render: ${n}/${frames} frames · ${rate.toFixed(1)} fps · ETA ${Math.floor(eta / 60)}m${String(Math.round(eta % 60)).padStart(2, '0')}s`;
  if (tty) process.stderr.write(`\r${line}   `);
  else if (Math.floor((n / frames) * 10) > lastDecile) { lastDecile = Math.floor((n / frames) * 10); console.error(line); }
};
const ticker = setInterval(report, 500);
await Promise.all(Array.from({ length: workers }, (_, k) => {
  const fa = Math.round(from * fps) + k * per, fb = Math.min(Math.round(to * fps), fa + per);
  if (fa >= fb) return null;
  const part = join(tmp, `p${k}.mp4`);
  parts.push(part);
  return new Promise((res, rej) => spawn(process.execPath, [fileURLToPath(import.meta.url), ...args], {
    env: { ...process.env, ANIKIT_SEG: [fa / fps, fb / fps, part, k === 0 ? fadeIn : 0, fb === Math.round(to * fps) ? fadeOut : 0, opt('fade-color', '')].join('|') }, stdio: ['inherit', 'inherit', 'inherit', 'ipc'],
  }).on('message', (m) => { if (m && typeof m.n === 'number') done[k] = m.n; })
    .on('close', (c) => (c ? rej(new Error(`worker ${k} exited ${c}`)) : res())));
}));
clearInterval(ticker);
report();
if (tty) process.stderr.write('\n');
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
