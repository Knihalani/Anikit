import { ok, near } from './assert.mjs';

const tall = (n, h = 600) => Array.from({ length: n }, (_, i) => `<section style="height:${h}px">s${i}</section>`).join('');

export const tests = [
  {
    name: 'scroll(): the batched first apply lands before the first paint, and in order with later links',
    html: `${tall(1)}<div id="a" style="height:100px"></div><div id="b" style="height:100px"></div>${tall(3)}`,
    async run(page, h) {
      const r = await page.evaluate(() => new Promise((res) => {
        const a = document.getElementById('a');
        const b = document.getElementById('b');
        ak.scroll(ak.animate(a, { opacity: [0.2, 1] }, { ease: 'linear', autoplay: false }), { target: a });
        ak.scroll(ak.animate(b, { opacity: [0.3, 1] }, { ease: 'linear', autoplay: false }), { target: b });
        const sync = [a.style.opacity, b.style.opacity];
        requestAnimationFrame(() => res({ sync, raf: [getComputedStyle(a).opacity, getComputedStyle(b).opacity] }));
      }));
      near(+r.raf[0], 0.2 + 0.8 * (100 / 800), 0.02, `a applied by the first frame (${JSON.stringify(r)})`);
      near(+r.raf[1], 0.3, 0.02, 'b applied at progress 0');
      // A link removed before its first apply never writes.
      const w = await page.evaluate(async () => {
        const c = document.createElement('div');
        document.body.appendChild(c);
        const off = ak.scroll(ak.animate(c, { opacity: [0.5, 1] }, { autoplay: false }), { target: c });
        off();
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        return c.style.opacity;
      });
      ok(w === '', `removed link wrote nothing (${w})`);
    },
  },
  {
    name: '1000-element staggered entrance ends at its end values; revert() restores',
    html: `<div id="s">${'<i style="display:inline-block;width:8px;height:8px;opacity:0.9"></i>'.repeat(1000)}</div>`,
    async run(page, h) {
      const r = await page.evaluate(async () => {
        const els = document.querySelectorAll('#s i');
        const a = ak.animate(els, { opacity: [0, 1], y: [20, 0] }, { duration: 0.2, delay: ak.stagger(0.0002) });
        await a.finished;
        const end = new Set([...els].map((e) => e.style.opacity + '|' + e.style.transform));
        a.revert();
        const back = new Set([...els].map((e) => e.style.opacity + '|' + e.style.transform));
        return { end: [...end], back: [...back] };
      });
      ok(r.end.length === 1 && r.end[0].startsWith('1|'), `all at the end: ${r.end.slice(0, 3)}`);
      ok(r.back.length === 1 && r.back[0] === '0.9|', `all restored: ${r.back.slice(0, 3)}`);
    },
  },
  {
    name: 'flip() with a target listed twice still moves it to its new place',
    html: '<div id="g" style="display:flex;gap:10px"><div class="c" style="width:50px;height:50px;background:#9cf">1</div><div class="c" style="width:50px;height:50px;background:#fc9">2</div></div>',
    async run(page, h) {
      const r = await page.evaluate(async () => {
        const [a, b] = document.querySelectorAll('.c');
        const tl = ak.flip([a, b, a], () => document.getElementById('g').appendChild(a), { duration: 0.2 });
        const mid = a.style.transform;
        await tl.finished;
        return { mid, end: getComputedStyle(a).transform, left: a.getBoundingClientRect().left };
      });
      ok(r.mid && r.mid !== 'none', `started from the old place: ${r.mid}`);
      ok(r.end === 'none' || r.end === 'matrix(1, 0, 0, 1, 0, 0)', `settled: ${r.end}`);
      near(r.left, 60, 1, 'in its new place');
    },
  },
];
