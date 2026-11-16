import { ok, near } from './assert.mjs';

// Body markup with every style attribute in its serialised form (a style
// put back through the CSSOM reads "a: b;" where the source had "a:b").
const snap = (page) => page.evaluate(() => {
  const c = document.body.cloneNode(true);
  for (const el of c.querySelectorAll('[style]')) el.setAttribute('style', el.style.cssText);
  return c.innerHTML;
});

export const tests = [
  {
    name: 'context().revert() restores the DOM after marquee, splitText, magnetic, parallax, bindValues, presence',
    html: `<div id="root" style="height:2000px">
      <div id="m" style="width:300px;color:red"><span style="display:inline-block;width:120px">one</span><span style="display:inline-block;width:120px">two</span></div>
      <p id="t" style="width:200px">Some <b>words</b> to split over lines</p>
      <button id="b" style="margin:100px;width:80px;height:40px;transform:rotate(5deg)">b</button>
      <div id="p" style="height:100px;opacity:0.9">p</div>
      <div id="v" style="width:10px;height:10px"></div>
      <div id="l"><div id="la">a</div><div id="lb">b</div></div>
    </div>`,
    async run(page, h) {
      const before = await snap(page);
      await page.evaluate(() => {
        const ak = window.ak;
        window.ctx = ak.context(() => {
          ak.marquee('#m', { speed: 200 });
          ak.splitText('#t', { type: 'lines,words,chars', mask: 'lines' });
          ak.magnetic('#b', { radius: 200 });
          ak.parallax('#p', { speed: -0.5 });
          const v = ak.motionValue(0.5);
          ak.bindValues('#v', { scale: v, opacity: v });
          const pr = ak.presence('#l', { layout: false, duration: 0.2 });
          const c = document.createElement('div');
          c.id = 'lc';
          pr.add(c);
        });
      });
      await page.mouse.move(150, 160);
      await page.mouse.move(160, 170);
      await page.evaluate(() => scrollTo(0, 300));
      await h.wait(250);
      await page.evaluate(() => { scrollTo(0, 0); window.ctx.revert(); });
      await h.wait(400);
      const after = await snap(page);
      // presence keeps the DOM changes it was asked for (documented): drop #lc.
      const norm = (s) => s.replace(/<div id="lc"[^>]*><\/div>/, '');
      const a = before.split("\n"), b = norm(after).split("\n");
      const diff = a.map((l, i) => (l === b[i] ? null : `- ${l}\n+ ${b[i]}`)).filter(Boolean);
      ok(!diff.length, `DOM differs after revert:\n${diff.join("\n")}`);
    },
  },
  {
    name: 'after bindValues().revert() a later animation does not bring the bound transform back',
    html: '<div id="v" style="width:10px;height:10px"></div>',
    async run(page) {
      const t = await page.evaluate(async () => {
        const ak = window.ak;
        const b = ak.bindValues('#v', { scale: ak.motionValue(0.5), rotate: ak.motionValue(30) });
        b.revert();
        await ak.animate('#v', { x: 10 }, { duration: 0.05 });
        return document.getElementById('v').style.transform;
      });
      ok(!/scale|rotate/.test(t), `transform ${t}`);
    },
  },
  {
    name: 'magnetic().revert() restores the inline transform and a later animation starts clean',
    html: '<button id="b" style="margin:100px;width:80px;height:40px;transform:rotate(5deg)">b</button>',
    async run(page, h) {
      await page.evaluate(() => (window.m = window.ak.magnetic('#b', { radius: 200, strength: 0.5 })));
      await page.mouse.move(150, 120);
      await page.mouse.move(170, 130);
      await h.wait(200);
      const moved = await page.$eval('#b', (el) => el.style.transform);
      ok(/translate/.test(moved), `pulled ${moved}`);
      await page.evaluate(() => window.m.revert());
      await h.wait(200);
      const t = await page.$eval('#b', (el) => el.style.transform);
      ok(t === 'rotate(5deg)', `after revert ${t}`);
      // The pointer is still there: nothing pulls it any more.
      await page.mouse.move(175, 135);
      await h.wait(100);
      ok((await page.$eval('#b', (el) => el.style.transform)) === 'rotate(5deg)', 'pulled again after revert');
    },
  },
  {
    name: 'marquee: revert() twice and kill() then revert() are safe',
    html: '<div id="m" style="width:300px"><span style="display:inline-block;width:120px">one</span></div>',
    async run(page) {
      const r = await page.evaluate(() => {
        const el = document.getElementById('m');
        const before = el.outerHTML;
        const a = window.ak.marquee(el);
        a.revert();
        a.revert();
        const b = window.ak.marquee(el);
        b.kill();
        b.revert();
        return { before, after: el.outerHTML };
      });
      ok(r.after === r.before, `markup:\n${r.before}\n${r.after}`);
    },
  },
];
