// draggable() and gesture() with real layout and real pointer input (mouse,
// and touch through the DevTools protocol).
import { ok, near } from './assert.mjs';

const STAGE = `<style>
#area{position:absolute;left:100px;top:100px;width:400px;height:300px;background:#eee}
#box{position:absolute;left:0;top:0;width:50px;height:50px;background:#6ea8ff}
</style><div id="area"><div id="box"></div></div>`;

const pos = (page) => page.evaluate(() => {
  const r = document.getElementById('box').getBoundingClientRect();
  return { x: r.left, y: r.top, d: window.__d ? { x: window.__d.x, y: window.__d.y } : null };
});

async function mouseDrag(page, from, to, steps = 10) {
  await page.mouse.move(from[0], from[1]);
  await page.mouse.down();
  await page.mouse.move(to[0], to[1], { steps });
  await page.mouse.up();
}

async function touchDrag(page, from, to, steps = 10, ms = 8) {
  const cdp = await page.context().newCDPSession(page);
  const tp = (x, y, id = 0) => [{ x, y, id }];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: tp(from[0], from[1]) });
  for (let i = 1; i <= steps; i++) {
    const x = from[0] + ((to[0] - from[0]) * i) / steps;
    const y = from[1] + ((to[1] - from[1]) * i) / steps;
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: tp(x, y) });
    await page.waitForTimeout(ms);
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  return cdp;
}

export const tests = [
  {
    name: 'mouse drag follows the pointer',
    html: STAGE,
    async run(page) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box')[0]; });
      await mouseDrag(page, [125, 125], [225, 175]);
      const p = await pos(page);
      near(p.x, 200, 1, 'x'); near(p.y, 150, 1, 'y');
      near(p.d.x, 100, 1, 'd.x'); near(p.d.y, 50, 1, 'd.y');
    },
  },
  {
    name: 'element bounds: clamped with edge resistance, springs back inside',
    html: STAGE,
    async run(page, h) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box', { bounds: '#area', edgeResistance: 0.5 })[0]; });
      await page.mouse.move(125, 125);
      await page.mouse.down();
      await page.mouse.move(725, 125, { steps: 10 }); // 600 right; max is 350
      const mid = await pos(page);
      near(mid.d.x, 350 + 250 * 0.5, 1, 'resisted past maxX');
      await page.mouse.up();
      await h.wait(1500);
      const end = await pos(page);
      near(end.d.x, 350, 1, 'back at maxX');
      near(end.x + 50, 500, 1, 'right edges aligned');
    },
  },
  {
    name: 'object bounds with snap (mouse): lands on a snap point inside the bounds',
    html: STAGE,
    async run(page, h) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box', { bounds: { minX: 0, maxX: 200, minY: 0, maxY: 0 }, snap: 100 })[0]; });
      await mouseDrag(page, [125, 125], [262, 190]);
      await h.wait(3500);
      const p = await pos(page);
      near(p.d.x, 100, 0.5, 'snapped x'); near(p.d.y, 0, 0.5, 'y clamped');
    },
  },
  {
    name: 'inertia (mouse fling) glides further, stays in bounds, snaps',
    html: STAGE,
    async run(page, h) {
      await page.evaluate(() => {
        window.__done = 0;
        window.__d = window.ak.draggable('#box', { type: 'x', bounds: '#area', inertia: true, snap: 50, onThrowComplete: () => window.__done++ })[0];
      });
      await page.mouse.move(125, 125);
      await page.mouse.down();
      for (let i = 1; i <= 6; i++) { await page.mouse.move(125 + i * 15, 125); await h.wait(8); }
      const rel = (await pos(page)).d.x;
      await page.mouse.up();
      await page.waitForFunction(() => window.__done > 0, null, { timeout: 8000 });
      const p = await pos(page);
      ok(p.d.x > rel, `glided past the release point (${rel} -> ${p.d.x})`);
      ok(p.d.x <= 350 + 0.5, `inside bounds (${p.d.x})`);
      near(p.d.x % 50, 0, 0.5, 'on a snap point');
      near(p.d.y, 0, 0.01, 'type x: y unchanged');
      ok(await page.evaluate(() => window.__done) === 1, 'onThrowComplete once');
    },
  },
  {
    name: 'touch drag with inertia and snap',
    html: STAGE,
    hasTouch: true,
    async run(page, h) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box', { bounds: '#area', inertia: true, snap: { x: 100, y: 50 } })[0]; });
      await touchDrag(page, [125, 125], [215, 165], 10);
      await h.wait(4000);
      const p = await pos(page);
      ok(p.d.x > 0, `moved by touch (${p.d.x})`);
      near(p.d.x % 100, 0, 0.5, 'x snapped to 100s');
      near(p.d.y % 50, 0, 0.5, 'y snapped to 50s');
      ok(p.d.x <= 350 && p.d.y <= 250, 'in bounds');
    },
  },
  {
    name: 'touch: a second finger does not hijack the drag',
    html: STAGE,
    hasTouch: true,
    async run(page, h) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box')[0]; });
      const cdp = await page.context().newCDPSession(page);
      const send = (type, pts) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: pts });
      await send('touchStart', [{ x: 110, y: 110, id: 0 }]);
      for (let i = 1; i <= 5; i++) await send('touchMove', [{ x: 110 + i * 10, y: 110, id: 0 }]);
      // Second finger lands on the box (now at x 50..100 inside the area) and moves far.
      await send('touchStart', [{ x: 160, y: 110, id: 0 }, { x: 180, y: 130, id: 1 }]);
      for (let i = 1; i <= 5; i++) await send('touchMove', [{ x: 160, y: 110, id: 0 }, { x: 180, y: 130 + i * 30, id: 1 }]);
      const p = await pos(page);
      near(p.d.x, 50, 1, 'x follows the first finger');
      near(p.d.y, 0, 1, 'y not moved by the second finger');
      await send('touchEnd', []);
      await h.wait(50);
    },
  },
  {
    name: 'lockAxis: locks to the dominant direction',
    html: STAGE,
    async run(page) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box', { lockAxis: true })[0]; });
      await mouseDrag(page, [125, 125], [245, 145], 12);
      const p = await pos(page);
      near(p.d.x, 120, 1, 'x follows');
      near(p.d.y, 0, 0.01, 'y locked');
    },
  },
  {
    name: 'kill() mid-drag stops following and leaves no listeners',
    html: STAGE,
    async run(page) {
      await page.evaluate(() => { window.__d = window.ak.draggable('#box')[0]; });
      await page.mouse.move(125, 125);
      await page.mouse.down();
      await page.mouse.move(175, 125, { steps: 5 });
      await page.evaluate(() => window.__d.kill());
      await page.mouse.move(300, 300, { steps: 5 });
      await page.mouse.up();
      const p = await pos(page);
      near(p.d.x, 50, 1, 'stayed where it was killed');
      ok(!(await page.evaluate(() => window.__d.isPressed || window.__d.isDragging)), 'not pressed/dragging after kill');
      // Pressing again does nothing.
      await mouseDrag(page, [160, 125], [260, 125]);
      near((await pos(page)).d.x, 50, 1, 'no drag after kill');
    },
  },
  {
    name: 'revert() mid-drag restores the original transform and handle styles',
    html: STAGE.replace('background:#6ea8ff', 'background:#6ea8ff;transform:translateX(20px)'),
    async run(page) {
      const before = await page.$eval('#box', (el) => [el.getAttribute('style'), getComputedStyle(el).transform]);
      await page.evaluate(() => { window.__d = window.ak.draggable('#box', { inertia: true })[0]; });
      await page.mouse.move(150, 125);
      await page.mouse.down();
      await page.mouse.move(250, 125, { steps: 5 });
      await page.evaluate(() => window.__d.revert());
      await page.mouse.move(300, 300, { steps: 3 });
      await page.mouse.up();
      await page.waitForTimeout(100);
      const after = await page.$eval('#box', (el) => [el.getAttribute('style'), getComputedStyle(el).transform]);
      ok(after[1] === before[1], `transform restored: ${before[1]} -> ${after[1]}`);
      ok((after[0] || null) === before[0], `style attribute restored: ${before[0]} -> ${after[0]}`);
    },
  },
  {
    name: 'context().revert() restores a dragged element',
    html: STAGE,
    async run(page) {
      await page.evaluate(() => { window.__c = window.ak.context(() => { window.__d = window.ak.draggable('#box')[0]; }); });
      await mouseDrag(page, [125, 125], [225, 125]);
      near((await pos(page)).d.x, 100, 1, 'dragged');
      await page.evaluate(() => window.__c.revert());
      const s = await page.$eval('#box', (el) => getComputedStyle(el).transform);
      ok(s === 'none' || s === 'matrix(1, 0, 0, 1, 0, 0)', `transform after revert: ${s}`);
      await mouseDrag(page, [125, 125], [225, 125]);
      const s2 = await page.$eval('#box', (el) => getComputedStyle(el).transform);
      ok(s2 === 'none' || s2 === 'matrix(1, 0, 0, 1, 0, 0)', `no drag after revert: ${s2}`);
    },
  },
  {
    name: 'gesture(): touch pinch scales about the fingers, settles into maxScale',
    html: `<div id="g" style="position:absolute;left:200px;top:200px;width:200px;height:200px;background:#cde"></div>`,
    hasTouch: true,
    async run(page, h) {
      await page.evaluate(() => { window.__g = window.ak.gesture('#g', { maxScale: 2 })[0]; });
      const cdp = await page.context().newCDPSession(page);
      const send = (type, pts) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: pts });
      await send('touchStart', [{ x: 280, y: 300, id: 0 }]);
      await send('touchStart', [{ x: 280, y: 300, id: 0 }, { x: 320, y: 300, id: 1 }]);
      for (let i = 1; i <= 10; i++) await send('touchMove', [{ x: 280 - i * 10, y: 300, id: 0 }, { x: 320 + i * 10, y: 300, id: 1 }]);
      const s = await page.evaluate(() => window.__g.scale);
      ok(s > 2 && s < 6, `pinched past max with resistance (${s})`);
      await send('touchEnd', []);
      await h.wait(1200);
      near(await page.evaluate(() => window.__g.scale), 2, 0.01, 'settled at maxScale');
      const r = await page.$eval('#g', (el) => { const b = el.getBoundingClientRect(); return b.left + b.width / 2; });
      near(r, 300, 1, 'centre stayed under the pinch centre');
    },
  },
  {
    name: 'gesture(): revert() mid-pan restores the transform and styles',
    html: `<div id="g" style="position:absolute;left:200px;top:200px;width:200px;height:200px;background:#cde;transform:rotate(10deg)"></div>`,
    async run(page) {
      const before = await page.$eval('#g', (el) => [el.style.cssText, getComputedStyle(el).transform]);
      await page.evaluate(() => { window.__g = window.ak.gesture('#g')[0]; });
      await page.mouse.move(300, 300);
      await page.mouse.down();
      await page.mouse.move(380, 340, { steps: 5 });
      ok(Math.abs(await page.evaluate(() => window.__g.x) - 80) < 1, 'panned');
      await page.evaluate(() => window.__g.revert());
      await page.mouse.move(400, 400, { steps: 3 });
      await page.mouse.up();
      const after = await page.$eval('#g', (el) => [el.style.cssText, getComputedStyle(el).transform]);
      ok(after[1] === before[1], `transform: ${before[1]} -> ${after[1]}`);
      ok(after[0] === before[0], `inline style: ${before[0]} -> ${after[0]}`);
    },
  },
];
