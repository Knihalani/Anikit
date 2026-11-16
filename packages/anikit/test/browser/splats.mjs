import { ok } from './assert.mjs';

const html = '<canvas id="c" style="width:200px;height:200px"></canvas>';

// Read the centre pixel of the layer's canvas (premultiplied RGBA bytes).
const centre = () => {
  const gl = window.layer.gl;
  const px = new Uint8Array(4);
  gl.readPixels(gl.drawingBufferWidth >> 1, gl.drawingBufferHeight >> 1, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
  return [...px];
};

const waitSorted = (page) => page.evaluate(async () => {
  const s0 = layer.stats.sorts;
  for (let i = 0; i < 200 && layer.stats.sorts === s0; i++) await new Promise((r) => setTimeout(r, 5));
  layer.render();
});

export const tests = [
  {
    name: 'splats(): two overlapping splats are blended back to front (worker sort), from either side',
    html,
    async run(page, h) {
      const worker = await page.evaluate(async () => {
        // A red splat at z = 0 and a blue one at z = 0.5, both opaque and big.
        const set = ak.splats.fromPoints([0, 0, 0, 0, 0, 0.5], {
          size: 0.4, opacity: 0.99, color: (i) => (i === 0 ? [1, 0, 0] : [0, 0, 1]),
        });
        window.layer = ak.splats('#c', set, { autoRender: false, camera: { x: 0, y: 0, z: 3, tx: 0, ty: 0, tz: 0, fov: 40 } });
        layer.render();
        await layer.sorted;
        layer.render();
        return layer.stats.worker;
      });
      ok(worker, 'sorting runs in a worker');
      let px = await page.evaluate(centre);
      ok(px[2] > 150 && px[0] < 60, `blue (near) splat is in front from +z: ${px}`);
      await page.evaluate(() => { layer.camera.z = -3; layer.render(); });
      await waitSorted(page);
      px = await page.evaluate(centre);
      ok(px[0] > 150 && px[2] < 60, `red (near) splat is in front from −z: ${px}`);
      ok(await page.evaluate(() => layer.stats.sorts >= 2), 're-sorted after the camera moved');
      await page.evaluate(() => layer.kill());
    },
  },
  {
    name: 'splats(): procedural SDF sphere renders; dissolve, explode and morph change the image',
    html,
    async run(page, h) {
      const coverage = () => page.evaluate(() => {
        const gl = layer.gl;
        const w = gl.drawingBufferWidth, hh = gl.drawingBufferHeight;
        const px = new Uint8Array(w * hh * 4);
        gl.readPixels(0, 0, w, hh, gl.RGBA, gl.UNSIGNED_BYTE, px);
        let n = 0;
        for (let i = 3; i < px.length; i += 4) if (px[i] > 20) n++;
        return n / (w * hh);
      });
      await page.evaluate(async () => {
        const sphere = ak.splats.fromSDF((x, y, z) => Math.hypot(x, y, z) - 1, { count: 20000, color: '#3fa' });
        window.layer = ak.splats('#c', sphere, { autoRender: false, background: null });
        layer.render();
        await layer.sorted;
        layer.render();
      });
      const full = await coverage();
      ok(full > 0.2 && full < 0.9, `sphere covers part of the canvas (${full})`);
      await page.evaluate(() => { layer.fx.dissolve = 1; layer.render(); });
      ok((await coverage()) < 0.01, 'fully dissolved draws nothing');
      await page.evaluate(() => { layer.fx.dissolve = 0; layer.fx.explode = 3; layer.render(); });
      const exploded = await coverage();
      ok(exploded !== full, `explode changes coverage (${exploded} vs ${full})`);
      await page.evaluate(() => { layer.fx.explode = 0; layer.fx.reveal = 0; layer.render(); });
      ok((await coverage()) < 0.01, 'reveal 0 hides everything');
      // Morph into a small cube through the timeline engine, with fake time.
      const done = await page.evaluate(async () => {
        layer.fx.reveal = 1;
        const cube = ak.splats.fromSDF((x, y, z) => Math.max(Math.abs(x), Math.abs(y), Math.abs(z)) - 0.3, { count: 5000, color: [1, 0.3, 0.2] });
        const a = layer.morph(cube, { duration: 0.2 });
        await a;
        layer.render();
        return { morph: layer.fx.morph, count: layer.set.count };
      });
      ok(done.morph === 0 && done.count === 20000, `morph adopts the target at the layer's count: ${JSON.stringify(done)}`);
      const small = await coverage();
      ok(small < full * 0.5, `the cube is smaller than the sphere (${small} vs ${full})`);
      ok(h.warnings().length === 0, 'no warnings');
      await page.evaluate(() => layer.kill());
    },
  },
  {
    name: 'splats(): draw(view, proj) renders inside a caller-owned WebGL2 context; unknown options warn',
    html,
    async run(page, h) {
      const px = await page.evaluate(async () => {
        const gl = document.getElementById('c').getContext('webgl2', { preserveDrawingBuffer: true });
        const set = ak.splats.fromPoints([0, 0, 0], { size: 0.5, color: [0, 1, 0], opacity: 1 });
        window.layer = ak.splats(gl, set, { orbitt: true });
        // Identity view looking down −z from the origin: move the cloud to z = −3.
        layer.transform.z = -3;
        const view = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
        const f = 1 / Math.tan(Math.PI / 8), n = 0.1, fa = 100;
        const proj = new Float32Array([f, 0, 0, 0, 0, f, 0, 0, 0, 0, (fa + n) / (n - fa), -1, 0, 0, (2 * fa * n) / (n - fa), 0]);
        gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
        gl.clearColor(0, 0, 0, 1);
        gl.clear(gl.COLOR_BUFFER_BIT);
        layer.draw(view, proj);
        const out = new Uint8Array(4);
        gl.readPixels(gl.drawingBufferWidth >> 1, gl.drawingBufferHeight >> 1, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, out);
        return [...out];
      });
      ok(px[1] > 150 && px[0] < 30, `green splat drawn through the hook: ${px}`);
      const w = h.warnings();
      ok(w.length === 1 && w[0].includes('orbitt') && w[0].includes('orbit'), `unknown option warns with a suggestion: ${w}`);
      await page.evaluate(() => layer.kill());
    },
  },
];
