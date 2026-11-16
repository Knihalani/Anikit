import { ok, near } from './assert.mjs';

const tall = (n, h = 600) => Array.from({ length: n }, (_, i) => `<section style="height:${h}px;border-top:1px solid #ccc">s${i}</section>`).join('');
const scrollWin = async (page, h, y, x = 0) => {
  await page.evaluate(([x, y]) => window.scrollTo(x, y), [x, y]);
  await h.frames(2);
};

export const tests = [
  {
    name: 'pin + scrub (window): pinned mid-range, survives resize and refresh, kill restores',
    html: `${tall(2)}<div id="pin" style="height:300px;background:#9cf;margin:10px 20px">pin</div>${tall(4)}`,
    async run(page, h) {
      const before = await page.$eval('#pin', (el) => ({ html: el.parentElement.innerHTML.length, style: el.getAttribute('style'), r: el.getBoundingClientRect().toJSON() }));
      await page.evaluate(() => {
        window.o = { x: 0 };
        const a = ak.animate(window.o, { x: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
        window.st = ak.scrollTrigger({ trigger: '#pin', start: 'top top', end: '+=600', pin: true, scrub: true, animation: a });
      });
      const start = await page.evaluate(() => st.start);
      near(start, before.r.top, 1, 'start is the pin top');
      await scrollWin(page, h, start + 300);
      let r = await page.$eval('#pin', (el) => el.getBoundingClientRect().toJSON());
      near(r.top, 0, 1, 'pinned at the top');
      near(r.left, 20, 1, 'keeps its left margin');
      near(r.width, before.r.width, 1, 'keeps its width');
      near(await page.evaluate(() => o.x), 50, 1, 'scrubbed half way');
      // Resize: narrower viewport, same scroll position.
      await page.setViewportSize({ width: 700, height: 600 });
      await h.wait(300);
      await h.frames(2);
      r = await page.$eval('#pin', (el) => el.getBoundingClientRect().toJSON());
      const st = await page.evaluate(() => ({ start: st.start, end: st.end, p: st.progress, y: scrollY }));
      near(st.end - st.start, 600, 1, 'range kept after resize');
      ok(st.y >= st.start && st.y <= st.end, `still inside the range after resize (${JSON.stringify(st)})`);
      near(r.top, 0, 1, 'still pinned at the top after resize');
      near(r.width, 700 - 40, 2, 'pinned width follows the new layout');
      near(await page.evaluate(() => o.x), st.p * 100, 1, 'animation matches progress after resize');
      // refresh() while pinned changes nothing visible.
      await page.evaluate(() => ak.refreshScrollTriggers());
      await h.frames(2);
      const r2 = await page.$eval('#pin', (el) => el.getBoundingClientRect().toJSON());
      near(r2.top, 0, 1, 'pinned after refresh');
      near(r2.left, r.left, 1, 'same left after refresh');
      // Past the end: scrolls away with the page, 600px further down than it started.
      await scrollWin(page, h, st.start + 900);
      const r3 = await page.$eval('#pin', (el) => el.getBoundingClientRect().top);
      near(r3, -300, 2, 'released after the end');
      // Back above the start.
      await scrollWin(page, h, 0);
      const r4 = await page.$eval('#pin', (el) => el.getBoundingClientRect().top);
      near(r4, st.start, 2, 'back in flow above the start');
      await page.evaluate(() => st.kill());
      const after = await page.$eval('#pin', (el) => ({ style: el.getAttribute('style'), spacer: !!el.closest('.anikit-pin-spacer') }));
      ok(after.style === before.style, `style restored: ${after.style} vs ${before.style}`);
      ok(!after.spacer, 'spacer removed');
    },
  },
  {
    name: 'pin: pinSpacing pushes later content by the range; later triggers measure after it',
    html: `${tall(1)}<div id="pin" style="height:200px;background:#9cf">pin</div><div id="after" style="height:100px">after</div>${tall(5)}`,
    async run(page, h) {
      const y0 = await page.$eval('#after', (el) => el.getBoundingClientRect().top);
      await page.evaluate(() => {
        window.a = ak.scrollTrigger({ trigger: '#pin', start: 'top top', end: '+=500', pin: true });
        window.b = ak.scrollTrigger({ trigger: '#after', start: 'top bottom' });
      });
      const y1 = await page.$eval('#after', (el) => el.getBoundingClientRect().top);
      near(y1 - y0, 500, 1, 'spacing added');
      const bs = await page.evaluate(() => b.start);
      near(bs, y1 - 700, 1, 'later trigger measured with the spacing');
      // Resize: start/end still consistent.
      await page.setViewportSize({ width: 800, height: 500 });
      await h.wait(300);
      const r = await page.evaluate(() => ({ a: [a.start, a.end], b: b.start, top: document.getElementById('after').getBoundingClientRect().top + scrollY }));
      near(r.b, r.top - 500, 1, 'later trigger re-measured after resize');
    },
  },
  {
    name: 'nested scroller inside a scrolled page: progress, pin and kill',
    html: `${tall(1, 400)}<div id="sc" style="height:400px;overflow:auto;margin-left:50px;border:1px solid">${tall(1, 300)}<div id="t" style="height:200px;background:#fc9">t</div>${tall(4, 300)}</div>${tall(3)}`,
    async run(page, h) {
      await scrollWin(page, h, 250);
      await page.evaluate(() => {
        window.o = { x: 0 };
        const a = ak.animate(window.o, { x: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
        window.st = ak.scrollTrigger({ scroller: '#sc', trigger: '#t', start: 'top top', end: '+=400', pin: true, scrub: true, animation: a });
      });
      const s = await page.evaluate(() => [st.start, st.end]);
      near(s[0], 301, 2, 'start relative to the scroller content');
      near(s[1] - s[0], 400, 1, 'range');
      await page.$eval('#sc', (el, y) => (el.scrollTop = y), s[0] + 200);
      await h.frames(2);
      const g = await page.evaluate(() => ({ p: st.progress, x: o.x, top: document.getElementById('t').getBoundingClientRect().top - document.getElementById('sc').getBoundingClientRect().top }));
      near(g.p, 0.5, 0.01, 'progress');
      near(g.x, 50, 1, 'scrubbed');
      near(g.top, 1, 2, 'pinned to the scroller top');
      // The window scrolling does not change the inner trigger.
      await scrollWin(page, h, 100);
      near(await page.evaluate(() => st.progress), 0.5, 0.01, 'window scroll leaves it alone');
      await page.evaluate(() => ak.refreshScrollTriggers());
      await h.frames(2);
      const s2 = await page.evaluate(() => [st.start, st.end, st.progress]);
      near(s2[0], s[0], 1, 'refresh with the page scrolled elsewhere: same start');
      near(s2[2], 0.5, 0.01, 'same progress after refresh');
      await page.evaluate(() => st.kill());
      const st2 = await page.$eval('#t', (el) => el.getAttribute('style'));
      ok(st2 === 'height:200px;background:#fc9', `style restored: ${st2}`);
    },
  },
  {
    name: 'horizontal scroller: start/end, progress, pin',
    html: `<div id="sc" style="width:600px;overflow-x:auto;white-space:nowrap;display:flex">${Array.from({ length: 8 }, (_, i) => `<div ${i === 2 ? 'id="t"' : ''} style="flex:0 0 400px;height:200px;border-left:1px solid">p${i}</div>`).join('')}</div>`,
    async run(page, h) {
      await page.evaluate(() => {
        window.o = { x: 0 };
        const a = ak.animate(window.o, { x: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
        window.st = ak.scrollTrigger({ scroller: '#sc', horizontal: true, trigger: '#t', start: 'left left', end: 'right left', scrub: true, animation: a });
      });
      const s = await page.evaluate(() => [st.start, st.end]);
      near(s[0], 802, 1, 'start');
      near(s[1], 1203, 1, 'end');
      await page.$eval('#sc', (el) => (el.scrollLeft = 1002.5));
      await h.frames(2);
      near(await page.evaluate(() => o.x), 50, 1, 'scrubbed half');
      near(await page.evaluate(() => st.progress), 0.5, 0.01, 'progress');
    },
  },
  {
    name: 'horizontal window scroll with a pin',
    viewport: { width: 800, height: 600 },
    html: `<div style="display:flex;width:max-content">${Array.from({ length: 6 }, (_, i) => `<div ${i === 1 ? 'id="t"' : ''} style="flex:0 0 700px;height:300px;border-left:1px solid">p${i}</div>`).join('')}</div>`,
    async run(page, h) {
      await page.evaluate(() => {
        window.st = ak.scrollTrigger({ horizontal: true, trigger: '#t', start: 'left left', end: '+=500', pin: true });
      });
      const s = await page.evaluate(() => [st.start, st.end]);
      near(s[0], 700, 1, 'start');
      await scrollWin(page, h, 0, 950);
      const r = await page.$eval('#t', (el) => el.getBoundingClientRect().toJSON());
      near(r.left, 0, 1, 'pinned at the left');
      near(r.top, 0, 1, 'top unchanged');
      await scrollWin(page, h, 0, 1400);
      const r2 = await page.$eval('#t', (el) => el.getBoundingClientRect().left);
      near(r2, -200, 2, 'released after the end');
    },
  },
  {
    name: 'smoothed scrub settles at the end after scrolling far past it',
    html: `${tall(2)}<div id="t" style="height:300px">t</div>${tall(6)}`,
    async run(page, h) {
      await page.evaluate(() => {
        window.o = { x: 0 };
        const a = ak.animate(window.o, { x: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
        window.st = ak.scrollTrigger({ trigger: '#t', start: 'top center', end: 'bottom center', scrub: 0.2, animation: a });
      });
      for (let y = 0; y < 3000; y += 150) await scrollWin(page, h, y);
      await h.wait(1500);
      near(await page.evaluate(() => o.x), 100, 0.5, 'at the end');
      await scrollWin(page, h, 0);
      await h.wait(1500);
      near(await page.evaluate(() => o.x), 0, 0.5, 'back at the start');
    },
  },
  {
    name: 'pin keeps the element\'s own inline transform, width % and position until pinned',
    html: `${tall(1)}<div id="w" style="width:800px"><div id="pin" style="width:50%;height:150px;position:relative;top:5px;transform:rotate(3deg);background:#9cf">pin</div></div>${tall(4)}`,
    async run(page, h) {
      const r0 = await page.$eval('#pin', (el) => el.getBoundingClientRect().toJSON());
      await page.evaluate(() => (window.st = ak.scrollTrigger({ trigger: '#pin', start: 'top top', end: '+=300', pin: true })));
      const s = await page.$eval('#pin', (el) => ({ t: el.style.transform, p: el.style.position, top: el.style.top, r: el.getBoundingClientRect().toJSON() }));
      ok(s.t === 'rotate(3deg)', `transform kept: ${s.t}`);
      ok(s.p === 'relative' && s.top === '5px', `position kept: ${s.p} ${s.top}`);
      near(s.r.top, r0.top, 1, 'same place');
      near(s.r.width, r0.width, 1, 'same width');
      await page.evaluate(() => ak.refreshScrollTriggers());
      ok(await page.$eval('#pin', (el) => el.style.transform) === 'rotate(3deg)', 'transform kept after refresh');
      await page.evaluate(() => st.kill());
      const a = await page.$eval('#pin', (el) => el.getAttribute('style'));
      ok(a === 'width:50%;height:150px;position:relative;top:5px;transform:rotate(3deg);background:#9cf', `restored: ${a}`);
    },
  },
  {
    name: 'scroll(): many links share one listener; cleanup inside a callback; x and y in one container',
    html: `<div id="sc" style="width:400px;height:300px;overflow:auto"><div style="width:2400px;height:1800px"></div></div>${tall(4)}`,
    async run(page, h) {
      await page.evaluate(() => {
        const sc = document.getElementById('sc');
        window.seen = { y: [], x: [], w: [], gone: 0 };
        window.offs = [];
        offs.push(ak.scroll((p) => seen.y.push(p), { container: sc }));
        offs.push(ak.scroll((p) => seen.x.push(p), { container: sc, axis: 'x' }));
        // Removes itself on its first call: the others still run.
        // (it is called once at creation, before `off` exists)
        let off = null;
        off = ak.scroll(() => { if (off) { seen.gone++; off(); } }, { container: sc });
        offs.push(ak.scroll((p) => seen.w.push(p)));
        window.o = { v: 0 };
        window.a = ak.animate(o, { v: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
        offs.push(ak.scroll(a, { container: sc }));
      });
      await page.$eval('#sc', (el) => { el.scrollTop = 750; el.scrollLeft = 1000; });
      await h.frames(3);
      await page.$eval('#sc', (el) => { el.scrollTop = 1500; });
      await h.frames(3);
      const r = await page.evaluate(() => ({ ...seen, v: o.v }));
      ok(r.gone === 1, `self-removing link ran once (${r.gone})`);
      near(r.y[r.y.length - 1], 1, 1e-6, 'y progress at the end');
      ok(r.x.some((p) => Math.abs(p - 0.5) < 1e-6), `x progress half way: ${r.x}`);
      near(r.v, 100, 0.01, 'animation at the end');
      ok(r.w.length <= 1, 'window link not called by the container scrolling');
      await scrollWin(page, h, 300);
      const w = await page.evaluate(() => seen.w[seen.w.length - 1]);
      ok(w > 0 && w < 1, `window link follows the window (${w})`);
      // Seeking the animation elsewhere, then scrolling inside the range: it follows again.
      await page.$eval('#sc', (el) => { el.scrollTop = 600; });
      await h.frames(3);
      near(await page.evaluate(() => o.v), 40, 0.5, 'back inside the range');
      await page.evaluate(() => offs.forEach((f) => f()));
      await page.$eval('#sc', (el) => { el.scrollTop = 0; });
      await h.frames(3);
      near(await page.evaluate(() => o.v), 40, 0.5, 'cleaned up: no more updates');
    },
  },
  {
    name: 'context().revert() stops scroll() links and inView(): the next scroll does not re-apply styles',
    html: `${tall(1)}<div id="a" style="height:100px">a</div><div id="b" style="height:100px">b</div>${tall(4)}`,
    async run(page, h) {
      await page.evaluate(() => {
        window.seen = { fn: 0, enter: 0 };
        window.ctx = ak.context(() => {
          ak.scroll(ak.animate('#a', { x: [0, 100] }, { ease: 'linear', autoplay: false }), { target: document.getElementById('a') });
          ak.scroll(ak.animate('#b', { opacity: [1, 0.2] }, { ease: 'linear', autoplay: false }), { smooth: 0.1 });
          ak.scroll(() => seen.fn++);
          ak.inView('#b', () => { seen.enter++; });
        });
      });
      await scrollWin(page, h, 400);
      await h.wait(300);
      const mid = await page.evaluate(() => [document.getElementById('a').style.transform, document.getElementById('b').style.opacity]);
      ok(mid[0] !== '' && mid[1] !== '', `linked before revert: ${mid}`);
      await page.evaluate(() => ctx.revert());
      const n = await page.evaluate(() => ({ ...seen }));
      await scrollWin(page, h, 0);
      await scrollWin(page, h, 900);
      await h.wait(300);
      const after = await page.evaluate(() => [document.getElementById('a').style.transform, document.getElementById('b').style.opacity, seen.fn, seen.enter]);
      ok(after[0] === '' && after[1] === '', `styles stay reverted: ${after}`);
      ok(after[2] === n.fn, `function link not called after revert (${n.fn} -> ${after[2]})`);
      ok(after[3] === n.enter, `inView not called after revert (${n.enter} -> ${after[3]})`);
    },
  },
];
