// flip() with real layout: nested moving elements, scaled parents, scrolled
// containers and a scrolled window. At progress 0 every element must be
// drawn where it was before the change, at progress 1 where it is now.
import { near } from './assert.mjs';

// Records rects, runs the flip paused, and checks progress 0, 0.5 and 1.
async function check(page, sel, mutate, opts = {}, mid = true) {
  const res = await page.evaluate(([sel, mutate, opts]) => {
    const els = [...document.querySelectorAll(sel)];
    const rect = (e) => { const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; };
    const before = els.map(rect);
    const tl = window.ak.flip(sel, new Function(mutate), { ease: 'linear', duration: 1, autoplay: false, ...opts });
    tl.progress = 0;
    window.ak.engine.flush?.();
    const at0 = els.map(rect);
    tl.progress = 0.5;
    const half = els.map(rect);
    tl.progress = 1;
    const at1 = els.map(rect);
    // Final layout without any transform.
    for (const e of els) e.style.transform = 'none';
    const after = els.map(rect);
    return { before, at0, half, at1, after };
  }, [sel, mutate, opts]);
  res.before.forEach((b, i) => {
    for (let k = 0; k < 4; k++) near(res.at0[i][k], b[k], 1, `el ${i} at progress 0, component ${k}`);
    for (let k = 0; k < 4; k++) near(res.at1[i][k], res.after[i][k], 1, `el ${i} at progress 1, component ${k}`);
    // Translation-only flips are linear: halfway is halfway.
    if (mid) for (let k = 0; k < 2; k++) near(res.half[i][k], (b[k] + res.after[i][k]) / 2, 1.5, `el ${i} at progress 0.5, component ${k}`);
  });
  return res;
}

export const tests = [
  {
    name: 'nested: parent and child both move (translation)',
    html: `<style>#p{position:absolute;left:50px;top:50px;width:300px;height:200px;background:#eee}
      #p.moved{left:400px;top:150px} #c{position:absolute;left:10px;top:10px;width:50px;height:50px;background:#69f}
      #c.moved{left:200px;top:100px}</style><div id="p" class="f"><div id="c" class="f"></div></div>`,
    async run(page) {
      await check(page, '.f', "document.getElementById('p').classList.add('moved'); document.getElementById('c').classList.add('moved');", { scale: false });
    },
  },
  {
    name: 'nested: only the parent moves, the child follows it',
    html: `<style>#p{position:absolute;left:50px;top:50px;width:300px;height:200px;background:#eee}
      #p.moved{left:400px;top:150px} #c{position:absolute;left:10px;top:10px;width:50px;height:50px;background:#69f}</style>
      <div id="p" class="f"><div id="c" class="f"></div></div>`,
    async run(page) {
      await check(page, '.f', "document.getElementById('p').classList.add('moved')", { scale: false });
    },
  },
  {
    name: 'nested with scale: parent resizes, child moves and resizes',
    html: `<style>#p{position:absolute;left:50px;top:50px;width:200px;height:100px;background:#eee}
      #p.big{left:300px;top:200px;width:400px;height:300px} #c{position:absolute;left:10px;top:10px;width:50px;height:50px;background:#69f}
      #c.big{left:100px;top:150px;width:120px;height:80px}</style><div id="p" class="f"><div id="c" class="f"></div></div>`,
    async run(page) {
      await check(page, '.f', "document.getElementById('p').classList.add('big'); document.getElementById('c').classList.add('big');", {}, false);
    },
  },
  {
    name: 'grandchild: three nested levels moving',
    html: `<style>.f{position:absolute;left:10px;top:10px;width:150px;height:150px;outline:1px solid #999}
      #a{width:500px;height:400px} .m#a{left:200px} .m#b{left:100px;top:60px} .m#c{left:40px;top:80px}</style>
      <div id="a" class="f"><div id="b" class="f"><div id="c" class="f"></div></div></div>`,
    async run(page) {
      await check(page, '.f', "for (const e of document.querySelectorAll('.f')) e.classList.add('m')", { scale: false });
    },
  },
  {
    name: 'items in a scrolled container reorder',
    html: `<style>#s{position:absolute;left:20px;top:20px;width:300px;height:200px;overflow:auto}
      .i{height:60px;margin:4px;background:#9cf}</style>
      <div id="s">${Array.from({ length: 12 }, (_, i) => `<div class="i">${i}</div>`).join('')}</div>`,
    async run(page) {
      await page.evaluate(() => { document.getElementById('s').scrollTop = 250; });
      await check(page, '.i', "const s = document.getElementById('s'); s.insertBefore(s.children[6], s.children[3]); s.insertBefore(s.children[8], s.children[0]);");
    },
  },
  {
    name: 'items on a scrolled page reorder',
    html: `<style>.i{height:60px;margin:4px;background:#9cf;width:300px} body{padding-bottom:2000px}</style>
      ${Array.from({ length: 30 }, (_, i) => `<div class="i">${i}</div>`).join('')}`,
    async run(page) {
      await page.evaluate(() => scrollTo(0, 700));
      await check(page, '.i', "const b = document.body; b.insertBefore(b.children[15], b.children[12]); b.insertBefore(b.children[14], b.children[16]);");
    },
  },
  {
    name: 'capture, scroll the page, then flip.from: starts where it was on screen',
    html: `<style>.i{height:60px;margin:4px;background:#9cf;width:300px} body{padding-bottom:2000px}</style>
      ${Array.from({ length: 30 }, (_, i) => `<div class="i">${i}</div>`).join('')}`,
    async run(page) {
      // Positions on the page are what matter: after a scroll, the old boxes
      // are where they were in the document, not in the viewport.
      const r = await page.evaluate(() => {
        const els = [...document.querySelectorAll('.i')];
        const doc = (e) => e.getBoundingClientRect().top + scrollY;
        const before = els.map(doc);
        const st = window.ak.flip.capture('.i');
        scrollTo(0, 300);
        const b = document.body;
        b.insertBefore(b.children[5], b.children[2]);
        const tl = window.ak.flip.from(st, { autoplay: false, duration: 1 });
        tl.progress = 0;
        return els.map((e, i) => doc(e) - before[i]);
      });
      r.forEach((d, i) => near(d, 0, 1, `el ${i} document position at progress 0`));
    },
  },
  {
    name: 'grandchild with scale at every level',
    html: `<style>.f{position:absolute;left:10px;top:10px;width:150px;height:150px;outline:1px solid #999}
      #a{width:500px;height:400px} .m#a{left:200px;width:300px;height:500px} .m#b{left:100px;top:60px;width:180px;height:100px}
      .m#c{left:40px;top:30px;width:60px;height:40px}</style>
      <div id="a" class="f"><div id="b" class="f"><div id="c" class="f"></div></div></div>`,
    async run(page) {
      await check(page, '.f', "for (const e of document.querySelectorAll('.f')) e.classList.add('m')", {}, false);
    },
  },
  {
    name: 'flip.from after a page scroll leaves fixed elements where they were on screen',
    html: `<style>body{height:3000px} .fx{position:fixed;top:20px;width:80px;height:40px;background:#f96} .fx.r{left:300px}</style>
      <div class="fx" style="left:20px"></div>`,
    async run(page) {
      const r = await page.evaluate(() => {
        const el = document.querySelector('.fx');
        const x0 = el.getBoundingClientRect().left;
        const st = window.ak.flip.capture('.fx');
        scrollTo(0, 500);
        el.style.left = '300px';
        const tl = window.ak.flip.from(st, { autoplay: false, duration: 1 });
        tl.progress = 0;
        const b = el.getBoundingClientRect();
        return [b.left - x0, b.top];
      });
      near(r[0], 0, 1, 'x on screen at progress 0');
      near(r[1], 20, 1, 'y on screen at progress 0');
    },
  },
];
