import { ok, near } from './assert.mjs';

export const tests = [
  {
    name: 'world(): dragging() grabs a body, moves it and throws it on release',
    html: '<div id="c" style="position:absolute;left:50px;top:80px;width:600px;height:400px;background:#eee"></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.w = ak.world({ container: '#c', gravity: [0, 0], autoStep: false });
        w.add(100, 100, 20);
        w.add(400, 300, 20);
        w.dragging();
      });
      await page.mouse.move(150, 180); // body 0 at (100,100) local
      await page.mouse.down();
      for (let i = 1; i <= 10; i++) {
        await page.mouse.move(150 + i * 20, 180 + i * 5);
        await page.evaluate(() => w.step(1 / 60));
      }
      const held = await page.evaluate(() => [w.x(0), w.y(0)]);
      near(held[0], 300, 1, 'held x follows the pointer (local coordinates)');
      near(held[1], 150, 1, 'held y');
      await page.mouse.up();
      await page.evaluate(() => { for (let i = 0; i < 10; i++) w.step(1 / 60); });
      const v = await page.evaluate(() => [w.x(0), w.velocity(0)]);
      ok(v[0] > 300, `thrown to the right (${v[0]})`);
      ok(v[1][0] > 0, `velocity kept (${v[1]})`);
      near(await page.evaluate(() => w.x(1)), 400, 0.01, 'the other body untouched');
      // Grab again: a new press resets the throw velocity.
      await page.evaluate(() => w.setVelocity(0, 0, 0));
      const p0 = await page.evaluate(() => [w.x(0) + 50, w.y(0) + 80]);
      await page.mouse.move(p0[0], p0[1]);
      await page.mouse.down();
      await page.mouse.up();
      await page.evaluate(() => w.step(1 / 60));
      const v2 = await page.evaluate(() => w.velocity(0));
      ok(Math.abs(v2[0]) < 1 && Math.abs(v2[1]) < 1, `a click without moving does not throw (${v2})`);
      await page.evaluate(() => w.destroy());
    },
  },
  {
    name: 'world(): pointer() repels bodies near the pointer, stops on leave; attractors kept',
    html: '<div id="c" style="position:absolute;left:0;top:0;width:600px;height:400px"></div>',
    async run(page, h) {
      await page.evaluate(() => {
        window.w = ak.world({ container: '#c', gravity: [0, 0], autoStep: false, damping: 0 });
        w.add(300, 200, 10);
        w.add(100, 350, 10);
        w.pointer({ mode: 'repel', radius: 100, strength: 4000 });
      });
      await page.mouse.move(260, 200);
      await page.evaluate(() => { for (let i = 0; i < 20; i++) w.step(1 / 60); });
      const x = await page.evaluate(() => w.x(0));
      ok(x > 301, `pushed away from the pointer (${x})`);
      near(await page.evaluate(() => w.x(1)), 100, 0.01, 'far body untouched');
      await page.mouse.move(900, 600); // leaves the container
      await page.$eval('#c', (el) => el.dispatchEvent(new PointerEvent('pointerleave')));
      await page.evaluate(() => { w.setVelocity(0, 0, 0); w.step(1 / 60); });
      const x1 = await page.evaluate(() => w.x(0));
      await page.evaluate(() => { for (let i = 0; i < 10; i++) w.step(1 / 60); });
      near(await page.evaluate(() => w.x(0)), x1, 0.01, 'no force after leaving');
      await page.evaluate(() => w.destroy());
    },
  },
];
