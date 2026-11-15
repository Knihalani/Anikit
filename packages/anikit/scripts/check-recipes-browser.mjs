// Run every recipe that has an HTML fixture (docs/recipes.md, AGENTS.md) in
// headless Chromium with real layout and real time: bundle it against src/,
// load it with its fixture, exercise it (clicks, drags, hovers, scrolling; see
// CHECKS), and fail on page errors, "anikit:" warnings or failed assertions.
// Needs Playwright (npm i -D playwright && npx playwright install chromium).
// Usage: node scripts/check-recipes-browser.mjs
import { build } from 'esbuild';
import { join, basename } from 'node:path';
import { createRequire } from 'node:module';
import { CHECKED_DOCS, extractSnippets, writeSnippets, OUT_DIR, pkg } from './snippets.mjs';

const require = createRequire(join(pkg, 'package.json'));
const { chromium } = require('playwright');

const tx = (page, sel) => page.$eval(sel, (el) => getComputedStyle(el).transform);
const translateX = (m) => (m === 'none' ? 0 : +m.match(/matrix\(([^)]+)\)/)[1].split(',')[4]);
const wait = (page, ms) => page.waitForTimeout(ms);
function expect(ok, msg) {
  if (!ok) throw new Error(msg);
}

/** Interactions + assertions per recipe heading (recipes without an entry are only loaded and scrolled). */
const CHECKS = {
  'Hero entrance': async (page) => {
    await wait(page, 1600);
    expect(+(await page.$eval('.hero-cta', (el) => getComputedStyle(el).opacity)) > 0.99, 'CTA visible at the end');
  },
  'Staggered list': async (page) => {
    await wait(page, 900);
    await page.evaluate(() => globalThis.__recipe.addItem(document.querySelector('.todo'), 'New'));
    await page.evaluate(() => globalThis.__recipe.removeItem(document.querySelector('.todo li')));
    await wait(page, 700);
    const items = await page.$$eval('.todo li', (els) => els.map((e) => e.textContent));
    expect(items.length === 3 && items[0] === 'Test the recipes' && items[2] === 'New', `list is ${JSON.stringify(items)}`);
  },
  'Reveal on scroll': async (page) => {
    await wait(page, 900);
    expect(+(await page.$eval('.reveal', (el) => getComputedStyle(el).opacity)) > 0.99, 'first card revealed');
  },
  'Scroll-scrubbed pinned section': async (page) => {
    await page.evaluate(() => scrollTo(0, document.querySelector('.story').getBoundingClientRect().top + scrollY + 600));
    await wait(page, 900);
    const b = +(await page.$eval('.story .b', (el) => getComputedStyle(el).opacity));
    expect(b > 0.2 && b < 0.95, `panel B mid-scrub (opacity ${b})`);
    const top = await page.$eval('.story', (el) => el.getBoundingClientRect().top);
    expect(Math.abs(top) < 2, `section pinned at the top (top ${top})`);
  },
  'Page transition': async (page) => {
    await page.click('[data-to="about"]');
    await wait(page, 900);
    expect(await page.$eval('#about', (el) => el.classList.contains('active') && getComputedStyle(el).display !== 'none'), 'about page shown');
    expect(await page.$eval('#home', (el) => getComputedStyle(el).display === 'none'), 'home page hidden');
    await page.click('#about [data-to="home"]');
    await wait(page, 900);
    expect(await page.$eval('#home', (el) => el.classList.contains('active')), 'back home');
  },
  'Draggable carousel': async (page) => {
    await page.click('.next');
    await wait(page, 700);
    await page.click('.next');
    await wait(page, 700);
    expect(Math.round(translateX(await tx(page, '.track'))) === -640, `two "next" clicks reach slide 3 (${await tx(page, '.track')})`);
    await page.click('.prev');
    await wait(page, 700);
    expect(Math.round(translateX(await tx(page, '.track'))) === -320, 'prev goes back one');
    const box = await page.$eval('.track', (el) => { const r = el.getBoundingClientRect(); return { x: r.left + 480, y: r.top + 10 }; });
    await page.mouse.move(box.x, box.y);
    await page.mouse.down();
    for (let i = 1; i <= 10; i++) await page.mouse.move(box.x - i * 25, box.y);
    await page.mouse.up();
    await wait(page, 2500);
    const x = translateX(await tx(page, '.track'));
    const slide = Math.round(x / 320) * 320;
    expect(Math.abs(x - slide) < 1 && slide <= 0 && slide >= -640, `drag settles on a slide (x ${x})`);
    await page.click('.prev');
    await wait(page, 700);
    const after = Math.round(translateX(await tx(page, '.track')));
    expect(after === Math.min(0, slide + 320), `buttons continue from the dragged slide (${slide} → ${after})`);
  },
  'Physics toy': async (page) => {
    await wait(page, 1200);
    expect((await tx(page, '#stage .ball')) !== 'none', 'balls are simulated');
  },
  'SVG morph and line drawing': async (page) => {
    const d0 = await page.$eval('#shape', (el) => el.getAttribute('d'));
    await wait(page, 600);
    expect((await page.$eval('#shape', (el) => el.getAttribute('d'))) !== d0, 'shape morphs');
  },
  'Number counter': async (page) => {
    await wait(page, 2200);
    const t = await page.$eval('#revenue', (el) => el.textContent);
    expect(t === '$128,400', `counter ends at $128,400 (got ${t})`);
  },
  'Hover and press': async (page) => {
    await page.hover('.btn');
    await wait(page, 600);
    expect((await tx(page, '.btn')) !== 'none', 'hover scales the button');
  },
  'Recording a GIF or MP4': async (page, logs) => {
    for (let i = 0; i < 100 && !logs.some((l) => /^\d+ \d+$/.test(l)); i++) await wait(page, 100);
    const line = logs.find((l) => /^\d+ \d+$/.test(l));
    expect(line, 'the recording finished and logged its frame count and PNG size');
    const [frames, bytes] = line.split(' ').map(Number);
    expect(frames > 0 && bytes > 0, `recorded ${frames} frames, PNG ${bytes} bytes`);
  },
  'Layout change (FLIP)': async (page) => {
    await page.click('.toggle');
    await wait(page, 100);
    expect(await page.$eval('.grid', (el) => el.classList.contains('dense')), 'layout toggled');
  },
};

writeSnippets();
const recipes = CHECKED_DOCS.flatMap((doc) =>
  extractSnippets(doc)
    .filter((s) => s.lang === 'ts' && s.html && !s.flags.has('no-run') && !s.flags.has('no-check'))
    .map((s) => ({ ...s, name: `${basename(doc)}:${s.line} (${s.heading})`, file: join(OUT_DIR, `${basename(doc, '.md').toLowerCase()}-${s.line}.ts`) })),
);

const browser = await chromium.launch();
let failed = 0;
for (const r of recipes) {
  const out = await build({
    stdin: { contents: `import * as m from ${JSON.stringify(r.file)}; globalThis.__recipe = m;`, loader: 'ts', resolveDir: pkg },
    bundle: true, format: 'esm', write: false, target: 'es2022', logLevel: 'silent',
    alias: { anikit: join(pkg, 'src/index.ts') },
  });
  const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push('error: ' + e.message));
  const logs = [];
  page.on('console', (m) => (m.text().startsWith('anikit:') ? problems.push(m.text()) : logs.push(m.text())));
  await page.setContent(`<!doctype html><html><body style="margin: 0; min-height: 3000px">${r.html}</body></html>`);
  await page.addScriptTag({ type: 'module', content: out.outputFiles[0].text });
  await wait(page, 100);
  try {
    if (CHECKS[r.heading]) await CHECKS[r.heading](page, logs);
    else {
      await page.mouse.move(200, 200);
      await page.mouse.wheel(0, 600);
      await wait(page, 300);
    }
  } catch (e) {
    problems.push('check: ' + e.message);
  }
  await page.close();
  if (problems.length) {
    failed++;
    console.error(`✗ ${r.name}\n  ${problems.join('\n  ')}`);
  } else console.log(`✓ ${r.name}${CHECKS[r.heading] ? ' (exercised)' : ''}`);
}
await browser.close();
console.log(`${recipes.length - failed}/${recipes.length} recipes ran cleanly in Chromium`);
process.exit(failed ? 1 : 0);
