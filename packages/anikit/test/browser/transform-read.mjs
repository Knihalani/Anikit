import { ok } from './assert.mjs';

export const tests = [
  {
    name: 'an element\'s existing inline transform (translateX, rotate) is kept when animating another component',
    html: '<div id="a" style="width:10px;height:10px;transform:translateX(10px)"></div><div id="b" style="width:10px;height:10px;transform:rotate(30deg) translateX(5px)"></div>',
    async run(page) {
      const r = await page.evaluate(async () => {
        await ak.animate('#a', { y: 5 }, { duration: 0 }).finished;
        await ak.animate('#b', { scale: 2 }, { duration: 0 }).finished;
        const m = (id) => new DOMMatrix(getComputedStyle(document.getElementById(id)).transform);
        const a = m('a'), b = m('b');
        return { a: [a.e, a.f], b: [Math.atan2(b.b, b.a) * 180 / Math.PI, Math.hypot(b.a, b.b)] };
      });
      ok(Math.abs(r.a[0] - 10) < 0.01 && Math.abs(r.a[1] - 5) < 0.01, `translateX kept with y added: ${r.a}`);
      ok(Math.abs(r.b[0] - 30) < 0.1 && Math.abs(r.b[1] - 2) < 0.01, `rotation kept, scale applied: ${r.b}`);
    },
  },
];
