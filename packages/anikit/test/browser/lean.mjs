import { ok } from './assert.mjs';

export const tests = [
  {
    name: 'lean build re-bundled as a plain script: loads without error, init(url) boots it',
    lean: true,
    html: '<div id="a"></div>',
    async run(page, h) {
      // Before the fix the import threw "Invalid URL" (no import.meta.url): a page error.
      await page.evaluate(() => window.ak.init('anikit.wasm'));
      await page.evaluate(() => window.ak.animate('#a', { x: 50 }, { duration: 0.05 }).finished);
      const t = await page.$eval('#a', (el) => el.style.transform);
      ok(t.includes('50px'), `animated after init(url): ${t}`);
    },
  },
];
