// Render the reference stills of both films with a baseline CLI and with this
// checkout, and report whether each pair is bit-identical, else its PSNR.
//
//   node scripts/check-films.mjs --base /path/to/base/packages/anikit [--min 45] [--only film|film2]
//
// The baseline is a saved copy of packages/anikit (bin/, node/, dist/) from
// before a change. Extra environment for the new side (e.g. ANIKIT_NATIVE=0)
// is passed through; NEW_ENV / BASE_ENV add `K=V,K=V` to one side only.
// Exits 1 if any still is below --min dB (default 45).
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const film = join(root, 'film');
const argv = process.argv.slice(2);
const flag = (n, d) => { const i = argv.indexOf(`--${n}`); return i >= 0 ? argv[i + 1] : d; };
const base = flag('base', process.env.BASE);
if (!base || !existsSync(join(base, 'bin/anikit-render.mjs'))) {
  console.error('usage: node scripts/check-films.mjs --base <saved packages/anikit> [--min 45] [--only film|film2]');
  process.exit(2);
}
const min = +flag('min', 45);
const only = flag('only');
const out = flag('out', join(root, 'target/check-films'));
mkdirSync(out, { recursive: true });
const STILLS = { film: [2, 8, 12, 16], film2: [4, 9, 14] };
const envOf = (s) => Object.fromEntries((s ?? '').split(',').filter(Boolean).map((kv) => kv.split('=')));
const { loadImage, createCanvas } = createRequire(join(film, 'noop.js'))('@napi-rs/canvas');

async function rgba(file) {
  const img = await loadImage(readFileSync(file));
  const c = createCanvas(img.width, img.height), ctx = c.getContext('2d');
  ctx.drawImage(img, 0, 0);
  return ctx.getImageData(0, 0, img.width, img.height).data;
}

function render(cli, mod, t, dest, extra) {
  const t0 = Date.now();
  const r = spawnSync(process.execPath, [cli, `${mod}.mjs`, '--still', String(t), '--preroll', '0', '--out', dest], { cwd: film, env: { ...process.env, ...extra }, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`${cli} ${mod} --still ${t} failed:\n${r.stderr}`);
  return Date.now() - t0;
}

let worst = Infinity;
for (const [mod, times] of Object.entries(STILLS)) {
  if (only && only !== mod) continue;
  for (const t of times) {
    const a = join(out, `${mod}-${t}-base.png`), b = join(out, `${mod}-${t}-new.png`);
    const ta = render(join(base, 'bin/anikit-render.mjs'), mod, t, a, envOf(process.env.BASE_ENV));
    const tb = render(join(root, 'packages/anikit/bin/anikit-render.mjs'), mod, t, b, envOf(process.env.NEW_ENV));
    const [x, y] = await Promise.all([rgba(a), rgba(b)]);
    let se = 0, diff = 0, maxd = 0;
    for (let i = 0; i < x.length; i++) {
      if ((i & 3) === 3) continue;
      const d = x[i] - y[i];
      if (d) { diff++; se += d * d; if (Math.abs(d) > maxd) maxd = Math.abs(d); }
    }
    const n = (x.length / 4) * 3;
    const psnr = se === 0 ? Infinity : 10 * Math.log10((255 * 255) / (se / n));
    worst = Math.min(worst, psnr);
    console.log(`${mod} @ ${String(t).padStart(2)} s: ${psnr === Infinity ? 'bit-identical' : `PSNR ${psnr.toFixed(2)} dB (${diff} channels differ, max ${maxd})`}   [base ${ta} ms, new ${tb} ms]`);
  }
}
console.log(worst === Infinity ? 'all stills bit-identical' : `worst PSNR ${worst.toFixed(2)} dB (min ${min})`);
process.exit(worst >= min ? 0 : 1);
