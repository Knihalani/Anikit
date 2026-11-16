import { ok } from './assert.mjs';

export const tests = [
  {
    name: 'tilt(): follows the pointer; revert() mid-tilt puts the element\'s transform back',
    html: '<div id="card" style="position:absolute;left:100px;top:100px;width:200px;height:200px;transform:translateX(10px);background:#9cf"></div>',
    async run(page, h) {
      await page.evaluate(() => (window.t = ak.tilt('#card', { max: 12 })));
      await page.mouse.move(150, 150);
      await page.mouse.move(280, 280);
      await h.wait(200);
      const mid = await page.$eval('#card', (el) => el.style.transform);
      ok(/rotateY\(/.test(mid) && /perspective\(/.test(mid), `tilting: ${mid}`);
      await page.evaluate(() => t.revert());
      await h.wait(200);
      const after = await page.$eval('#card', (el) => el.style.transform);
      ok(after === 'translateX(10px)', `restored: ${after}`);
      // A later animate() starts from the restored transform, not the tilt.
      await page.evaluate(() => ak.animate('#card', { y: 5 }, { duration: 0 }).finished);
      const later = await page.$eval('#card', (el) => el.style.transform);
      ok(!/rotate|perspective|scale/.test(later), `no tilt left in the cached transform: ${later}`);
    },
  },
];
