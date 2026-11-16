import { ok } from './assert.mjs';
export const tests = [
  {
    name: 'bundle loads and animates',
    html: '<div id="a" style="width:10px;height:10px"></div>',
    async run(page, h) {
      await page.evaluate(() => window.ak.animate('#a', { x: 100 }, { duration: 0.1 }).finished);
      const t = await page.$eval('#a', (el) => el.style.transform);
      ok(t.includes('100px'), `transform ${t}`);
    },
  },
];
