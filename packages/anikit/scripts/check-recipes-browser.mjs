// Run every recipe that has an HTML fixture (docs/recipes.md, AGENTS.md) in
// headless Chromium with real layout: bundle it against src/, load it with its
// fixture, scroll the page, advance 3 s of engine time, and fail on page errors
// or "anikit:" warnings. Needs Playwright (npm i -D playwright && npx playwright
// install chromium). Usage: node scripts/check-recipes-browser.mjs
import { build } from 'esbuild';
import { join, basename } from 'node:path';
import { createRequire } from 'node:module';
import { CHECKED_DOCS, extractSnippets, pkg } from './snippets.mjs';

const require = createRequire(join(pkg, 'package.json'));
const { chromium } = require('playwright');

const recipes = CHECKED_DOCS.flatMap((doc) =>
  extractSnippets(doc)
    .filter((s) => s.lang === 'ts' && s.html && !s.flags.has('no-run') && !s.flags.has('no-check'))
    .map((s) => ({ ...s, name: `${basename(doc)}:${s.line}` })),
);

const browser = await chromium.launch();
let failed = 0;
for (const r of recipes) {
  const out = await build({
    stdin: { contents: r.code, loader: 'ts', resolveDir: pkg },
    bundle: true, format: 'esm', write: false, target: 'es2022', logLevel: 'silent',
    alias: { anikit: join(pkg, 'src/index.ts') },
  });
  const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push('error: ' + e.message));
  page.on('console', (m) => m.text().startsWith('anikit:') && problems.push(m.text()));
  await page.setContent(`<!doctype html><html><body style="min-height: 3000px">${r.html}</body></html>`);
  await page.addScriptTag({ type: 'module', content: out.outputFiles[0].text });
  await page.waitForTimeout(300);
  await page.mouse.move(200, 200);
  await page.mouse.wheel(0, 600);
  await page.waitForTimeout(300);
  await page.evaluate(() => globalThis.scrollTo(0, 0));
  await page.waitForTimeout(200);
  await page.close();
  if (problems.length) {
    failed++;
    console.error(`✗ ${r.name}\n  ${problems.join('\n  ')}`);
  } else console.log(`✓ ${r.name}`);
}
await browser.close();
console.log(`${recipes.length - failed}/${recipes.length} recipes ran cleanly in Chromium`);
process.exit(failed ? 1 : 0);
