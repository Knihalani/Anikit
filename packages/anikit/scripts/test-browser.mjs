// Real-browser tests for the DOM layer (layout, pointer events, scrolling):
// every `test/browser/*.mjs` exports `tests`, a list of
// `{ name, html?, viewport?, reducedMotion?, hasTouch?, run(page, h) }`.
// src/index.ts is bundled once (as `window.ak`), each test gets a fresh page
// with that bundle and its HTML, and fails on a thrown error, a page error or
// an "anikit:" warning it did not expect (`h.warnings()` returns and clears them).
// Usage: node scripts/test-browser.mjs [filter]
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { readdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const require = createRequire(join(pkg, 'package.json'));
const { chromium } = require('playwright');
const filter = process.argv[2] || '';

const dir = mkdtempSync(join(tmpdir(), 'anikit-browser-'));
await build({
  stdin: { contents: "import * as ak from './src/index'; window.ak = ak;", resolveDir: pkg, loader: 'ts' },
  bundle: true, format: 'iife', outfile: join(dir, 'ak.js'), logLevel: 'error',
});
const bundle = readFileSync(join(dir, 'ak.js'), 'utf8');

const files = readdirSync(join(pkg, 'test/browser')).filter((f) => f.endsWith('.mjs')).sort();
const GL = ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'];
const browser = await chromium.launch({ args: GL });
// Tests with `args` (Chromium flags, e.g. experimental web platform features) get their own browser.
const flagged = new Map();
const browserFor = async (args) => {
  if (!args?.length) return browser;
  const key = args.join(' ');
  if (!flagged.has(key)) flagged.set(key, await chromium.launch({ args: [...GL, ...args] }));
  return flagged.get(key);
};
let failed = 0;
let ran = 0;
for (const f of files) {
  const { tests } = await import(pathToFileURL(join(pkg, 'test/browser', f)).href);
  for (const t of tests ?? []) {
    const name = `${f.replace(/\.mjs$/, '')} › ${t.name}`;
    if (filter && !name.includes(filter)) continue;
    ran++;
    const ctx = await (await browserFor(t.args)).newContext({
      viewport: t.viewport ?? { width: 1000, height: 700 },
      reducedMotion: t.reducedMotion ?? 'no-preference',
      hasTouch: !!t.hasTouch,
    });
    const page = await ctx.newPage();
    const errors = [];
    let warnings = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => {
      const s = m.text();
      if (s.includes('anikit:')) warnings.push(s);
    });
    await page.route('http://test.local/**', (r) => r.fulfill({
      status: 200, contentType: 'text/html',
      body: `<!doctype html><meta charset="utf-8"><style>body{margin:0;font:16px/1.4 sans-serif}</style>${t.html ?? ''}<script>${bundle}</script>`,
    }));
    await page.goto('http://test.local/index.html');
    await page.evaluate(() => window.ak.ready);
    const h = {
      warnings: () => { const w = warnings; warnings = []; return w; },
      wait: (ms) => page.waitForTimeout(ms),
      frames: (n = 2) => page.evaluate((n) => new Promise((r) => { let k = 0; const f = () => (++k >= n ? r() : requestAnimationFrame(f)); requestAnimationFrame(f); }), n),
    };
    const t0 = Date.now();
    try {
      await t.run(page, h);
      if (errors.length) throw new Error('page error: ' + errors.join('\n'));
      if (warnings.length) throw new Error('unexpected warnings: ' + warnings.join('\n'));
      console.log(`  ✓ ${name} (${Date.now() - t0} ms)`);
    } catch (e) {
      failed++;
      console.log(`  ✗ ${name}\n      ${String(e?.stack || e).split('\n').slice(0, 6).join('\n      ')}`);
    }
    await ctx.close();
  }
}
await browser.close();
for (const b of flagged.values()) await b.close();
rmSync(dir, { recursive: true, force: true });
console.log(`\n${ran - failed}/${ran} browser tests passed`);
if (failed) process.exit(1);
