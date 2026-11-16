import { ok, near } from './assert.mjs';

const ROW = '<div id="m" style="width:300px;white-space:nowrap"><span style="display:inline-block;width:120px">one</span><span style="display:inline-block;width:120px">two</span></div>';
const tx = (page, sel) => page.$eval(sel, (el) => new DOMMatrix(getComputedStyle(el).transform).m41);

export const tests = [
  {
    name: 'reduce + "user": animate jumps to the end, callbacks and promises still fire',
    reducedMotion: 'reduce',
    html: '<div id="a" style="width:20px;height:20px"></div>',
    async run(page) {
      const r = await page.evaluate(async () => {
        window.ak.engine.reducedMotion = 'user';
        let done = 0;
        const a = window.ak.animate('#a', { x: 200, opacity: 0.5 }, { duration: 5, onComplete: () => done++ });
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        const t = getComputedStyle(document.getElementById('a')).transform;
        await a;
        return { t, done };
      });
      ok(r.t.includes('200'), `transform ${r.t}`);
      ok(r.done === 1, `onComplete ${r.done}`);
    },
  },
  {
    name: 'reduce + mode "fade": movement jumps, opacity still animates',
    reducedMotion: 'reduce',
    html: '<div id="a" style="width:20px;height:20px"></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.ak.engine.reducedMotion = 'user';
        window.ak.engine.reducedMotionMode = 'fade';
        window.ak.animate('#a', { x: 200, opacity: [1, 0] }, { duration: 1, ease: 'linear' });
      });
      await h.wait(300);
      const r = await page.$eval('#a', (el) => ({ x: new DOMMatrix(getComputedStyle(el).transform).m41, o: +getComputedStyle(el).opacity }));
      near(r.x, 200, 0.5, 'x jumped');
      ok(r.o > 0.2 && r.o < 0.95, `opacity mid-fade ${r.o}`);
    },
  },
  {
    name: 'reduce: spring() arrives at once',
    reducedMotion: 'reduce',
    html: '<div id="a" style="width:20px;height:20px"></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.ak.engine.reducedMotion = 'user';
        window.ak.spring('#a', { x: 150 }, { stiffness: 10, damping: 1 });
      });
      await h.frames(3);
      near(await tx(page, '#a'), 150, 0.5, 'spring x');
    },
  },
  {
    name: 'reduce: a marquee never moves (not even while easing to a stop)',
    reducedMotion: 'reduce',
    html: ROW,
    async run(page, h) {
      await page.evaluate(() => {
        window.ak.engine.reducedMotion = 'user';
        window.m = window.ak.marquee('#m', { speed: 400, smoothing: 1 });
      });
      await h.wait(400);
      const x = await tx(page, '#m > [data-anikit-marquee]');
      near(x, 0, 0.5, 'track offset');
      await page.evaluate(() => window.m.revert());
    },
  },
  {
    name: 'reduce: magnetic() and parallax() stay put',
    reducedMotion: 'reduce',
    html: '<div style="height:1500px"><button id="b" style="margin:200px;width:80px;height:40px">b</button><div id="p" style="height:100px">p</div></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.ak.engine.reducedMotion = 'user';
        window.ak.magnetic('#b');
        window.ak.parallax('#p', { speed: -0.5 });
      });
      await page.mouse.move(250, 225);
      await page.mouse.move(260, 230);
      await page.evaluate(() => scrollTo(0, 400));
      await h.wait(300);
      near(await tx(page, '#b'), 0, 0.01, 'magnetic x');
      const t = await page.$eval('#p', (el) => getComputedStyle(el).transform);
      ok(t === 'none', `parallax transform ${t}`);
    },
  },
  {
    name: 'reduce: presence exit still removes and enter still shows',
    reducedMotion: 'reduce',
    html: '<div id="l"><div id="a">a</div><div id="b">b</div></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.ak.engine.reducedMotion = 'user';
        const p = window.ak.presence('#l', { layout: false });
        p.remove(document.getElementById('a'));
        const c = document.createElement('div');
        c.id = 'c';
        p.add(c);
      });
      await h.wait(100);
      const r = await page.$$eval('#l > *', (els) => els.map((e) => e.id + ':' + getComputedStyle(e).opacity));
      ok(r.join() === 'b:1,c:1', `children ${r}`);
    },
  },
];
