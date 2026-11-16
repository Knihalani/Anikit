// Scoreboard: every benchmark script against every competitor installed in
// node_modules, plus bundle size and start-up, as one Markdown table
// (docs/scoreboard.md). Ratio = best competitor / Anikit for "lower is
// better" metrics, so > 1 means Anikit leads.
//
// Usage: node scripts/scoreboard.mjs            (Node benches, sizes, start-up)
//        SCOREBOARD_DOM=1 node scripts/scoreboard.mjs   (+ Chromium DOM bench, slower)
// Needs a built dist (node scripts/build.mjs).
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const out = join(pkg, '..', '..', 'docs', 'scoreboard.md');
const require = createRequire(join(pkg, 'package.json'));
const has = (m) => { try { require.resolve(m); return true; } catch { return false; } };
const COMPETITORS = ['gsap', 'motion', 'animejs', 'matter-js', 'popmotion', '@tweenjs/tween.js', 'velocity-animate'];
const installed = COMPETITORS.filter(has);
const missing = COMPETITORS.filter((m) => !installed.includes(m));

const rows = []; // { group, metric, ak, comp, best, unit, target }
const add = (group, metric, ak, comp, best, unit, target = '≥ 1.00x') => rows.push({ group, metric, ak, comp, best, unit, target });

function run(script, args = [], node = []) {
  process.stderr.write(`scoreboard: ${script}\n`);
  return execFileSync(process.execPath, [...node, join(here, script), ...args], { cwd: pkg, encoding: 'utf8', maxBuffer: 1 << 26, timeout: 15 * 60e3 });
}

// --- Per-frame, plain objects (bench.mjs: Anikit vs GSAP) -------------------
{
  const txt = run('bench.mjs');
  for (const line of txt.split('\n')) {
    const m = /^(.+?)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+[\d.]+x\s*$/.exec(line);
    if (m) add('frame (objects)', `${m[1].trim()}, N=${m[2]}`, +m[3], 'GSAP', +m[4], 'ms/frame');
  }
}
// --- Creation (bench-create.mjs: Anikit vs GSAP) ---------------------------
{
  const txt = run('bench-create.mjs', [], ['--expose-gc']);
  for (const line of txt.split('\n')) {
    const s = /^startup: import ([\d.]+) ms, ready \(wasm instantiated\) ([\d.]+) ms/.exec(line);
    if (s) continue;
    const m = /^((?:create|kill|frame):.+?)\s{2,}([\d.]+)\s+([\d.]+)\s+[\d.]+x/.exec(line);
    if (m) add(m[1].startsWith('frame') ? 'frame (objects)' : 'creation', m[1].trim(), +m[2], 'GSAP', +m[3], 'ms');
  }
}
// --- Physics (bench-physics.mjs: Anikit vs matter.js) -----------------------
if (installed.includes('matter-js')) {
  const txt = run('bench-physics.mjs');
  for (const line of txt.split('\n')) {
    const m = /^\s*(\d+)\s+([\d.]+) ms\s+([\d.]+) ms/.exec(line);
    if (m) add('physics', `step, ${m[1]} bodies`, +m[2], 'matter.js', +m[3], 'ms/step');
  }
}
// --- DOM in Chromium (bench-dom.mjs: Anikit vs GSAP / Motion), optional -----
if (process.env.SCOREBOARD_DOM) {
  const txt = run('bench-dom.mjs');
  const by = new Map();
  for (const line of txt.split('\n')) {
    const m = /^(\S+)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+\d+\s*$/.exec(line);
    if (!m) continue;
    const k = m[2];
    if (!by.has(k)) by.set(k, []);
    by.get(k).push({ lib: m[1], ms: +m[3] + +m[4] });
  }
  for (const [n, libs] of by) {
    const ak = libs.filter((l) => l.lib.startsWith('anikit')).sort((a, b) => a.ms - b.ms)[0];
    const comp = libs.filter((l) => !l.lib.startsWith('anikit')).sort((a, b) => a.ms - b.ms)[0];
    if (ak && comp) add('frame (DOM, Chromium)', `script+style+layout, ${n} elements`, ak.ms, comp.lib, comp.ms, 'ms/frame');
  }
}

// --- Bundle size: min+gzip of what an app ships ------------------------------
async function bundle(code) {
  const { build } = await import('esbuild');
  const r = await build({ stdin: { contents: code, resolveDir: pkg }, bundle: true, minify: true, format: 'esm', write: false, logLevel: 'silent' });
  return gzipSync(r.outputFiles[0].contents, { level: 9 }).length;
}
{
  const lean = gzipSync(readFileSync(join(pkg, 'dist', 'anikit.lean.js')), { level: 9 }).length;
  const wasm = gzipSync(readFileSync(join(pkg, 'dist', 'anikit.wasm')), { level: 9 }).length;
  const ak = (lean + wasm) / 1024;
  const tw = (gzipSync(readFileSync(join(pkg, 'dist', 'anikit.tween.js')), { level: 9 }).length + gzipSync(readFileSync(join(pkg, 'dist', 'anikit.tween.wasm')), { level: 9 }).length) / 1024;
  const comp = [];
  if (installed.includes('gsap')) comp.push(['gsap (core)', await bundle("import gsap from 'gsap'; console.log(gsap);") / 1024]);
  if (installed.includes('motion')) comp.push(['motion (animate)', await bundle("import { animate } from 'motion'; console.log(animate);") / 1024]);
  if (installed.includes('animejs')) comp.push(['animejs', await bundle("import * as a from 'animejs'; console.log(a);") / 1024]);
  comp.sort((a, b) => a[1] - b[1]);
  if (comp.length) {
    add('size', 'tween bundle, min+gzip (anikit/tween: JS + stripped wasm)', tw, comp[0][0], comp[0][1], 'KB');
    add('size', 'full bundle, min+gzip (anikit/lean: JS + full wasm)', ak, comp[0][0], comp[0][1], 'KB');
  }
  for (const [name, kb] of comp) process.stderr.write(`scoreboard: ${name} ${kb.toFixed(1)} KB\n`);
  process.stderr.write(`scoreboard: anikit lean ${(lean / 1024).toFixed(1)} KB + wasm ${(wasm / 1024).toFixed(1)} KB\n`);
}

// --- Start-up: import to first usable call, fresh process each, median of 5 --
function startup(code) {
  const ts = [];
  for (let i = 0; i < 5; i++) {
    const o = execFileSync(process.execPath, ['--input-type=module', '-e', code], { cwd: pkg, encoding: 'utf8' });
    ts.push(+o.trim());
  }
  return ts.sort((a, b) => a - b)[2];
}
{
  const ak = startup(`const t=performance.now();const a=await import('./dist/anikit.js');await a.ready;console.log(performance.now()-t)`);
  const comp = [];
  if (installed.includes('gsap')) comp.push(['gsap', startup(`const t=performance.now();await import('gsap');console.log(performance.now()-t)`)]);
  if (installed.includes('motion')) comp.push(['motion', startup(`const t=performance.now();await import('motion');console.log(performance.now()-t)`)]);
  comp.sort((a, b) => a[1] - b[1]);
  const tween = startup(`const t=performance.now();const a=await import('./dist/anikit.tween.js');await a.ready;console.log(performance.now()-t)`);
  const lean = startup(`const t=performance.now();const a=await import('./dist/anikit.lean.js');await a.ready;console.log(performance.now()-t)`);
  if (comp.length) {
    add('start-up', 'import → ready, tween build (stripped wasm read + compile + instantiate)', tween, comp[0][0], comp[0][1], 'ms');
    add('start-up', 'import → ready, lean build (full wasm read + compile + instantiate)', lean, comp[0][0], comp[0][1], 'ms');
    add('start-up', 'import → ready, full build (wasm embedded as base64)', ak, comp[0][0], comp[0][1], 'ms');
  }
}

// --- Output -----------------------------------------------------------------
const fmt = (v) => (v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v >= 1 ? v.toFixed(2) : v.toFixed(3));
let md = `# Scoreboard\n\nGenerated by \`node packages/anikit/scripts/scoreboard.mjs\` on ${new Date().toISOString().slice(0, 16).replace('T', ' ')} UTC`;
md += ` (Node ${process.version}${process.env.SCOREBOARD_DOM ? ', DOM rows in Chromium' : '; DOM rows: run with SCOREBOARD_DOM=1'}).\n`;
md += `Every row: lower is better; **ratio = best competitor ÷ Anikit** (> 1: Anikit leads). Shared machine: single runs move by up to ~1.5x, so treat ratios within 0.8–1.25 as ties.\n\n`;
md += `Competitors installed: ${installed.join(', ') || 'none'}. Not installed (not measured): ${missing.join(', ')}.\n\n`;
const lead = rows.filter((r) => r.best / r.ak >= 1).length;
md += `**Anikit leads ${lead} of ${rows.length} rows.**\n\n`;
md += `| metric | anikit | best competitor | ratio | target |\n|---|---|---|---|---|\n`;
let group = '';
for (const r of rows) {
  if (r.group !== group) {
    group = r.group;
    md += `| **${group}** | | | | |\n`;
  }
  const ratio = r.best / r.ak;
  const mark = ratio >= 1 ? '' : ' 🔴';
  md += `| ${r.metric} | ${fmt(r.ak)} ${r.unit} | ${r.comp} ${fmt(r.best)} ${r.unit} | ${ratio.toFixed(2)}x${mark} | ${r.target} |\n`;
}
writeFileSync(out, md);
process.stdout.write(md);
