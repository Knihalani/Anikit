import { ok, near } from './assert.mjs';

const LIST = '<style>#l>div{height:30px;margin:4px;background:#ccd}</style><div id="l"><div id="a">a</div><div id="b">b</div><div id="c">c</div></div>';

// State of the list after everything settled: ids in the DOM, and the inline
// styles left on each.
const settle = async (page, h, ms = 900) => {
  await h.wait(ms);
  return page.$$eval('#l > *', (els) => els.map((e) => ({ id: e.id, hidden: e.hidden, style: e.getAttribute('style') || '', op: +getComputedStyle(e).opacity })));
};
// flip() leaves `transform: none` on the elements it moved: harmless.
const clean = (s) => !/position|top|left|width|height|transform|opacity/.test(s.style.replace(/opacity:\s*1;?/, '').replace(/transform:\s*none;?/, ''));

export const tests = [];
for (const layout of [true, false]) {
  const L = layout ? 'layout' : 'no layout';
  tests.push(
    {
      name: `${L}: re-adding an element while its exit runs keeps it, fully shown`,
      html: LIST,
      async run(page, h) {
        await page.evaluate((layout) => {
          const p = (window.p = window.ak.presence('#l', { layout, duration: 0.4 }));
          const b = document.getElementById('b');
          p.remove(b);
          setTimeout(() => p.add(b), 120);
        }, layout);
        const s = await settle(page, h);
        ok(s.map((x) => x.id).join() === 'a,b,c', `ids ${s.map((x) => x.id)}`);
        for (const x of s) ok(x.op > 0.99 && clean(x), `${x.id}: opacity ${x.op}, style "${x.style}"`);
      },
    },
    {
      name: `${L}: removing twice removes once`,
      html: LIST,
      async run(page, h) {
        const r = await page.evaluate(async (layout) => {
          const p = window.ak.presence('#l', { layout, duration: 0.2 });
          const b = document.getElementById('b');
          const x = p.remove(b);
          const y = p.remove(b);
          await Promise.race([Promise.all([x, y]), new Promise((r) => setTimeout(() => r('timeout'), 2000))]);
          return document.getElementById('b') ? 'still there' : 'gone';
        }, layout);
        ok(r === 'gone', r);
        const s = await settle(page, h, 50);
        ok(s.map((x) => x.id).join() === 'a,c', `ids ${s.map((x) => x.id)}`);
        for (const x of s) ok(x.op > 0.99 && clean(x), `${x.id}: opacity ${x.op}, style "${x.style}"`);
      },
    },
    {
      name: `${L}: removing during the entrance removes it`,
      html: LIST,
      async run(page, h) {
        await page.evaluate((layout) => {
          const p = window.ak.presence('#l', { layout, duration: 0.4 });
          const d = document.createElement('div');
          d.id = 'd';
          d.textContent = 'd';
          p.add(d);
          setTimeout(() => p.remove(d), 100);
        }, layout);
        const s = await settle(page, h);
        ok(s.map((x) => x.id).join() === 'a,b,c', `ids ${s.map((x) => x.id)}`);
        for (const x of s) ok(x.op > 0.99 && clean(x), `${x.id}: opacity ${x.op}, style "${x.style}"`);
      },
    },
    {
      name: `${L}: rapid random add/remove ends matching the last call per element`,
      html: LIST,
      async run(page, h) {
        const want = await page.evaluate(async (layout) => {
          const p = window.ak.presence('#l', { layout, duration: 0.25 });
          const els = ['a', 'b', 'c'].map((id) => document.getElementById(id));
          for (let i = 0; i < 4; i++) {
            const d = document.createElement('div');
            d.id = 'n' + i;
            d.textContent = d.id;
            els.push(d);
          }
          const want = new Map(els.map((e, i) => [e.id, i < 3]));
          let seed = 7;
          const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
          for (let step = 0; step < 40; step++) {
            const e = els[Math.floor(rnd() * els.length)];
            const add = rnd() < 0.5;
            if (add) p.add(e); else if (e.isConnected) p.remove(e); else continue;
            want.set(e.id, add);
            await new Promise((r) => setTimeout(r, rnd() * 60));
          }
          return [...want].filter(([, v]) => v).map(([k]) => k).sort();
        }, layout);
        const s = await settle(page, h, 1200);
        const got = s.filter((x) => !x.hidden).map((x) => x.id).sort();
        ok(JSON.stringify(got) === JSON.stringify(want), `present ${got} want ${want}`);
        const bad = s.filter((x) => !(x.op > 0.99 && clean(x))).map((x) => `${x.id}: opacity ${x.op}, style "${x.style}"`);
        ok(!bad.length, bad.join('; '));
      },
    },
  );
}
tests.push(
  {
    name: 'exit() then enter() turns around; enter() after exit finished on a hidden element shows it',
    html: LIST,
    async run(page, h) {
      await page.evaluate(() => {
        const b = document.getElementById('b');
        window.ak.exit(b, { opacity: 0 }, { duration: 0.4, after: 'hide' });
        setTimeout(() => window.ak.enter(b), 100);
        const c = document.getElementById('c');
        window.ak.exit(c, { opacity: 0 }, { duration: 0.1, after: 'hide' });
        setTimeout(() => window.ak.enter(c), 300);
      });
      const s = await settle(page, h);
      for (const x of s) ok(!x.hidden && x.op > 0.99, `${x.id}: hidden ${x.hidden} opacity ${x.op}`);
    },
  },
  {
    name: 'presence.kill() mid-exit completes the removal; revert() in a context too',
    html: LIST,
    async run(page, h) {
      const r = await page.evaluate(async () => {
        const ctx = window.ak.context(() => {
          const p = window.ak.presence('#l', { duration: 1 });
          p.remove(document.getElementById('a'));
        });
        ctx.revert();
        await new Promise((r) => setTimeout(r, 30));
        return [...document.querySelectorAll('#l > *')].map((e) => e.id + ':' + (e.getAttribute('style') || ''));
      });
      ok(r.join() === 'b:,c:' || r.every((x) => /^[bc]:(opacity: 1;)?$/.test(x)), `after revert ${r}`);
      await h.wait(50);
    },
  },
);
