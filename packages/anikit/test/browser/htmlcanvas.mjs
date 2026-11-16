import { ok } from './assert.mjs';

const CARD = '<div id="card" style="position:absolute;left:40px;top:40px;width:200px;height:120px;background:rgb(255,0,0);color:#fff;font:20px sans-serif;padding:0">Hello <b>canvas</b><input id="in" value="typed" style="width:80px"></div>';

// Read one pixel (CSS coordinates) from a handle's canvas.
const pixel = (page, x, y) => page.evaluate(([x, y]) => {
  const c = window.hc.canvas, s = c.width / window.hc.width;
  return Array.from(c.getContext('2d').getImageData(Math.round(x * s), Math.round(y * s), 1, 1).data);
}, [x, y]);

export const tests = [
  {
    name: 'htmlCanvas fallback: rasterises the element, untainted, and re-rasterises live on change',
    html: CARD,
    async run(page, h) {
      await page.evaluate(() => { window.hc = ak.htmlCanvas('#card'); return window.hc.ready.then(() => 0); });
      const info = await page.evaluate(() => ({ native: hc.native, w: hc.width, h: hc.height, v: hc.version }));
      ok(!info.native && info.w === 200 && info.h === 120 && info.v >= 1, `first raster: ${JSON.stringify(info)}`);
      const red = await pixel(page, 190, 110);
      ok(red[0] > 240 && red[1] < 20 && red[3] === 255, `red background: ${red}`);
      // Under SwiftShader a font/resize observer can add a raster before the change: wait for one after it.
      const v0 = await page.evaluate(() => hc.version);
      await page.evaluate(() => { document.getElementById('card').style.background = 'rgb(0,0,255)'; });
      await page.waitForFunction((v0) => hc.version > v0, v0);
      const blue = await pixel(page, 190, 110);
      ok(blue[2] > 240 && blue[0] < 20, `live update to blue: ${blue}`);
      // No re-raster without a change.
      const v = await page.evaluate(() => hc.version);
      await h.wait(200);
      ok((await page.evaluate(() => hc.version)) === v, 'idle: no re-raster');
      await page.evaluate(() => hc.dispose());
    },
  },
  {
    name: 'htmlCanvas fallback: WebGL target gets a texture; cross-origin images are skipped with a warning',
    html: CARD.replace('</div>', '<img src="https://cross.example/x.png" width="10" height="10"></div>') + '<canvas id="gl" width="8" height="8"></canvas>',
    async run(page, h) {
      const r = await page.evaluate(async () => {
        const gl = document.getElementById('gl').getContext('webgl');
        window.hc = ak.htmlCanvas('#card', { target: gl, scale: 1 });
        await hc.ready;
        return { tex: !!hc.texture && gl.isTexture(hc.texture), err: gl.getError() };
      });
      ok(r.tex && r.err === 0, `texture: ${JSON.stringify(r)}`);
      const w = h.warnings();
      ok(w.some((s) => s.includes('cross-origin')), `warned: ${w}`);
    },
  },
  {
    name: "htmlEffect('shatter'): overlay plays, element hidden after, revert() restores it",
    html: CARD,
    async run(page, h) {
      await page.evaluate(() => { window.fx = ak.htmlEffect('#card', 'shatter', { duration: 0.4 }); });
      await page.waitForFunction(() => document.querySelectorAll('canvas').length === 1);
      const mid = await page.evaluate(() => getComputedStyle(document.getElementById('card')).opacity);
      ok(mid === '0', `element transparent while the overlay plays: ${mid}`);
      await page.evaluate(() => fx.finished);
      const after = await page.evaluate(() => ({ vis: document.getElementById('card').style.visibility, overlays: document.querySelectorAll('canvas').length }));
      ok(after.vis === 'hidden' && after.overlays === 0, `after: ${JSON.stringify(after)}`);
      await page.evaluate(() => fx.revert());
      ok((await page.evaluate(() => document.getElementById('card').style.visibility)) === '', 'restored');
      // mode 'in' reveals a hidden element and leaves it visible.
      await page.evaluate(() => { document.getElementById('card').style.visibility = 'hidden'; return ak.htmlEffect('#card', 'shatter', { mode: 'in', duration: 0.3 }).finished; });
      const vis = await page.evaluate(() => { const c = getComputedStyle(document.getElementById('card')); return c.visibility + ' ' + c.opacity; });
      ok(vis === 'visible 1', `revealed: ${vis}`);
    },
  },
  {
    name: 'htmlEffect: every effect renders without GL errors; pulse effects restore the element; clicks reach it',
    html: CARD,
    async run(page, h) {
      await page.evaluate(() => { window.clicks = 0; document.getElementById('card').addEventListener('click', () => clicks++); });
      for (const name of ['ripple', 'wave', 'glass', 'pixelate', 'dissolve', 'curl']) {
        await page.evaluate((name) => { window.fx = ak.htmlEffect('#card', name, { duration: 0.5 }); }, name);
        await page.waitForFunction(() => document.querySelector('canvas'));
        await h.wait(150);
        const px = await page.evaluate(() => {
          const c = document.querySelector('canvas');
          const gl = c.getContext('webgl');
          const out = new Uint8Array(4 * 16);
          gl.readPixels(Math.floor(c.width / 2), Math.floor(c.height / 2), 4, 4, gl.RGBA, gl.UNSIGNED_BYTE, out);
          return { err: gl.getError() };
        });
        ok(px.err === 0, `${name}: gl error ${px.err}`);
        if (name === 'ripple') await page.mouse.click(100, 80);
        await page.evaluate(() => fx.finished);
        const st = await page.evaluate(() => document.getElementById('card').style.cssText);
        if (['dissolve', 'curl'].includes(name)) {
          ok(/visibility: hidden/.test(st), `${name} leaves the element hidden: ${st}`);
          await page.evaluate(() => fx.revert());
        } else ok(!/opacity|visibility/.test(st), `${name} restored: ${st}`);
      }
      ok((await page.evaluate(() => clicks)) === 1, 'click during the effect reached the element');
    },
  },
  {
    name: 'htmlCanvas native path (--enable-experimental-web-platform-features): drawElementImage into a layoutsubtree canvas',
    args: ['--enable-experimental-web-platform-features'],
    html: '<canvas id="c" width="200" height="100" layoutsubtree><div id="d" style="width:200px;height:100px;background:rgb(0,255,0)"><button id="b">Hit me</button></div></canvas>',
    async run(page) {
      const sup = await page.evaluate(() => ak.htmlCanvasSupported());
      ok(sup, 'native API present behind the flag');
      await page.evaluate(() => { window.hc = ak.htmlCanvas('#d'); return hc.ready.then(() => 0); });
      const r = await page.evaluate(() => ({ native: hc.native, v: hc.version }));
      ok(r.native && r.v >= 1, `native: ${JSON.stringify(r)}`);
      const px = await page.evaluate(() => Array.from(hc.canvas.getContext('2d').getImageData(190, 90, 1, 1).data));
      ok(px[1] > 240 && px[0] < 20, `green from drawElementImage: ${px}`);
    },
  },
  {
    name: 'htmlCanvas native path: WebGL 2 texture via texElementImage2D (WebGL 1 falls back silently)',
    args: ['--enable-experimental-web-platform-features'],
    html: '<canvas id="c" width="64" height="64" layoutsubtree><div id="d" style="width:64px;height:64px;background:rgb(0,0,255)"></div></canvas>',
    async run(page) {
      const r = await page.evaluate(async () => {
        const gl = document.getElementById('c').getContext('webgl2');
        window.hc = ak.htmlCanvas('#d', { target: gl });
        await hc.ready;
        const out = { native: hc.native, err: gl.getError(), tex: gl.isTexture(hc.texture) };
        const c1 = document.createElement('canvas');
        c1.setAttribute('layoutsubtree', '');
        const d1 = document.createElement('div');
        d1.style.cssText = 'width:20px;height:20px;background:red';
        c1.append(d1);
        document.body.append(c1);
        const h1 = ak.htmlCanvas(d1, { target: c1.getContext('webgl') });
        await h1.ready;
        out.gl1native = h1.native;
        return out;
      });
      ok(r.native && r.err === 0 && r.tex && r.gl1native === false, `native gl: ${JSON.stringify(r)}`);
    },
  },
  {
    name: 'htmlCanvas without the flag: falls back even inside a layoutsubtree canvas',
    html: '<div id="d" style="width:50px;height:50px;background:rgb(0,0,255)"></div>',
    async run(page) {
      const r = await page.evaluate(async () => { window.hc = ak.htmlCanvas('#d'); await hc.ready; return { sup: ak.htmlCanvasSupported(), native: hc.native }; });
      ok(!r.sup && !r.native, JSON.stringify(r));
      const px = await pixel(page, 25, 25);
      ok(px[2] > 240, `blue: ${px}`);
    },
  },
  {
    name: 'htmlCanvas fallback limits (documented in docs/api.md): fonts embedded, form state kept, iframe/::before dropped; raster cost',
    html: `<style>@font-face{font-family:T;src:url("data:font/woff2;base64,AAAA")} #x::before{content:"BEFORE"} </style>
      <div id="x" style="width:300px;padding:10px;font-family:T,sans-serif"><input type="checkbox" checked><input value="v1"><select><option>a</option><option selected>b</option></select><textarea>txt</textarea><iframe srcdoc="hi"></iframe><p>${'para '.repeat(40)}</p></div>`,
    async run(page) {
      const r = await page.evaluate(async () => {
        document.querySelector('#x input[value]').value = 'edited';
        const { url } = await ak.htmlToSvg(document.getElementById('x'));
        const svg = decodeURIComponent(url.slice(url.indexOf(',') + 1));
        const hc = ak.htmlCanvas('#x', { live: false });
        await hc.ready;
        const t0 = performance.now();
        for (let i = 0; i < 10; i++) await hc.refresh();
        return { svg, ms: (performance.now() - t0) / 10 };
      });
      ok(r.svg.includes('@font-face') && r.svg.includes('data:font/woff2'), 'font-face embedded');
      ok(r.svg.includes('value="edited"') && /checked/.test(r.svg) && /selected/.test(r.svg) && r.svg.includes('txt'), 'form state');
      ok(!r.svg.includes('<iframe') && !r.svg.includes('BEFORE'), 'iframe and ::before dropped');
      console.log(`      fallback re-raster of a ~40-node card: ${r.ms.toFixed(1)} ms`);
      ok(r.ms < 100, `raster cost ${r.ms} ms`);
    },
  },
];

// scene() + htmlTexture: live HTML on a GPU mesh, with pointer forwarding through UV picking.
const SCENE_HTML = `<canvas id="c" style="width:320px;height:240px;display:block"></canvas>
  <div id="ui" style="position:fixed;left:-10000px;top:0;width:200px;height:200px;display:flex">
    <button id="l" style="all:unset;width:100px;height:200px;background:rgb(255,0,0)"></button>
    <button id="r" style="all:unset;width:100px;height:200px;background:rgb(0,0,255)"></button>
  </div>`;
const scenePx = (page, x, y) => page.evaluate(([x, y]) => {
  s.render();
  return Array.from(s.readPixels(x, s.gl.drawingBufferHeight - 1 - y, 1, 1));
}, [x, y]);

tests.push({
  name: 'htmlTexture on a scene() mesh: upright, live, and clicks reach the element under the UV',
  html: SCENE_HTML,
  async run(page) {
    await page.evaluate(async () => {
      window.s = ak.scene('#c', { dpr: 1, adaptive: false, autoRender: false, shadows: false, camera: { position: [0, 0, 3] } });
      window.ui = ak.htmlTexture('#ui');
      await ui.ready;
      window.card = s.add(ak.plane(2, 2), { rotation: [Math.PI / 2, 0, 0], material: { color: '#000000', emissive: '#ffffff', map: ui } });
      window.hits = [];
      document.getElementById('l').addEventListener('click', () => hits.push('l'));
      document.getElementById('r').addEventListener('click', () => hits.push('r'));
    });
    const left = await scenePx(page, 130, 120), right = await scenePx(page, 190, 120);
    ok(left[0] > 150 && left[2] < 80, `left half red (${left})`);
    ok(right[2] > 150 && right[0] < 80, `right half blue (${right})`);
    // Live: recolour the left button; the texture is re-uploaded once the new raster lands.
    const v = await page.evaluate(() => ui.version);
    await page.evaluate(() => { document.getElementById('l').style.background = 'rgb(0,255,0)'; });
    await page.waitForFunction((v) => ui.version > v, v);
    const green = await scenePx(page, 130, 120);
    ok(green[1] > 150 && green[0] < 80, `left half now green (${green})`);
    // Pointer forwarding through the mesh's UV.
    {
      const box = await page.$eval('#c', (c) => { const r = c.getBoundingClientRect(); return [r.left, r.top]; });
      await page.mouse.click(box[0] + 200, box[1] + 120);
      await page.mouse.click(box[0] + 120, box[1] + 120);
    }
    const hits = await page.evaluate(() => hits);
    ok(hits.join() === 'r,l', `clicks forwarded to the buttons under the UV: ${hits}`);
    const uv = await page.evaluate(() => s.raycast(160, 120)?.uv);
    ok(uv && Math.abs(uv[0] - 0.5) < 0.05 && Math.abs(uv[1] - 0.5) < 0.05, `centre UV ≈ (0.5, 0.5): ${uv}`);
  },
});
