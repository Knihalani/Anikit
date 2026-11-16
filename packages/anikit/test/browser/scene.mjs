import { ok, near } from './assert.mjs';

// WebGL2 scene(): render real frames (SwiftShader in headless Chromium) and read pixels back.
const html = '<canvas id="c" style="width:320px;height:240px;display:block"></canvas>';
const px = (page, x, y) => page.evaluate(([x, y]) => {
  const s = window.s; s.render();
  const h = s.gl.drawingBufferHeight;
  return Array.from(s.readPixels(x, h - 1 - y, 1, 1));
}, [x, y]);

export const tests = [
  {
    name: 'scene(): renders a lit sphere over the studio environment',
    html,
    async run(page) {
      await page.evaluate(() => {
        window.s = ak.scene('#c', { dpr: 1, adaptive: false, autoRender: false, camera: { position: [0, 0, 4] } });
        window.ball = s.add(ak.sphere(1, 48), { material: { color: '#ff0000', roughness: 0.4 } });
      });
      const centre = await px(page, 160, 120);
      ok(centre[0] > 60 && centre[0] > centre[1] * 1.5 && centre[0] > centre[2] * 1.5, `centre is red (${centre})`);
      const corner = await px(page, 4, 4);
      ok(Math.abs(corner[0] - corner[2]) < 40, `corner shows the grey studio backdrop (${corner})`);
      const st = await page.evaluate(() => s.stats);
      ok(st.drawCalls >= 3, `sky + shadow + main draws (${st.drawCalls})`);
      ok(st.triangles >= 2304, `triangles counted (${st.triangles})`);
    },
  },
  {
    name: 'scene(): animate(obj.position) moves the object; raycast and click events hit it',
    html,
    async run(page, h) {
      await page.evaluate(() => {
        window.s = ak.scene('#c', { dpr: 1, adaptive: false, autoRender: false, shadows: false, camera: { position: [0, 0, 5] } });
        window.cube = s.add('box', { material: 'plastic', position: [-3, 0, 0] });
        window.clicks = 0;
        cube.on('click', () => clicks++);
      });
      const miss = await page.evaluate(() => { const r = s.raycast(160, 120); return r && [r.distance, ...r.point]; });
      ok(miss === null, `nothing at the centre yet (${miss})`);
      await page.evaluate(() => { cube.position.x = 0; });
      const hit = await page.evaluate(() => { const r = s.raycast(160, 120); return r && { d: r.distance, z: r.point[2] }; });
      ok(hit, 'hit after moving');
      near(hit.z, 0.5, 0.01, 'front face of the unit box');
      const box = await page.evaluate(() => { const r = document.querySelector('#c').getBoundingClientRect(); return [r.left, r.top]; });
      await page.mouse.click(box[0] + 160, box[1] + 120);
      ok((await page.evaluate(() => clicks)) === 1, 'click fired on the object');
      await page.mouse.click(box[0] + 10, box[1] + 10);
      ok((await page.evaluate(() => clicks)) === 1, 'click on empty space ignored');
      // animate() drives plain Vector3 fields
      await page.evaluate(async () => { await ak.animate(cube.position, { y: 2 }, { duration: 0.05 }).finished; });
      near(await page.evaluate(() => cube.position.y), 2, 1e-6, 'animated position');
    },
  },
  {
    name: 'scene(): instanced draws, frustum culling, bloom + FXAA chain, revert',
    html,
    async run(page) {
      await page.evaluate(() => {
        window.s = ak.scene('#c', { dpr: 1, adaptive: false, autoRender: false, bloom: true, environment: 'night', camera: { position: [0, 0, 8] } });
        window.inst = s.instances(ak.box(0.2, 0.2, 0.2), 1000, (i, o) => { o.position.set((i % 10) - 4.5, ((i / 10 | 0) % 10) - 4.5, -(i / 100 | 0)); o.color[1] = 0.2; });
        s.add('sphere', { material: { emissive: '#ffffff', emissiveIntensity: 8, color: '#000000' }, position: [0, 0, 2] });
        s.add('sphere', { position: [0, 0, 50] }); // behind the camera: culled
      });
      await px(page, 160, 120);
      const st = await page.evaluate(() => s.stats);
      ok(st.culled >= 1, `culled (${st.culled})`);
      ok(st.triangles >= 12000, `1000 instances × 12 triangles (${st.triangles})`);
      const c = await px(page, 160, 120);
      ok(c[0] > 200 && c[1] > 200 && c[2] > 200, `emissive centre is bright through bloom/tonemap (${c})`);
      await page.evaluate(() => s.revert());
    },
  },
  {
    name: 'loadGLTF(): meshes, materials, hierarchy and a TRS animation from an embedded .gltf',
    html,
    async run(page) {
      const r = await page.evaluate(async () => {
        const pos = new Float32Array([-1, -1, 0, 1, -1, 0, 0, 1, 0]);
        const times = new Float32Array([0, 1]), trans = new Float32Array([0, 0, 0, 2, 0, 0]);
        const bytes = new Uint8Array(pos.byteLength + 8 + 24);
        bytes.set(new Uint8Array(pos.buffer), 0); bytes.set(new Uint8Array(times.buffer), 36); bytes.set(new Uint8Array(trans.buffer), 44);
        const b64 = btoa(String.fromCharCode(...bytes));
        const gltf = {
          asset: { version: '2.0' }, scene: 0, scenes: [{ nodes: [0] }],
          nodes: [{ children: [1], translation: [0, 0, 0] }, { mesh: 0, translation: [0, 0, -1] }],
          meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }],
          materials: [{ pbrMetallicRoughness: { baseColorFactor: [0, 1, 0, 1], metallicFactor: 0, roughnessFactor: 0.5 } }],
          buffers: [{ byteLength: bytes.length, uri: 'data:application/octet-stream;base64,' + b64 }],
          bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: 36 }, { buffer: 0, byteOffset: 36, byteLength: 8 }, { buffer: 0, byteOffset: 44, byteLength: 24 }],
          accessors: [
            { bufferView: 0, componentType: 5126, count: 3, type: 'VEC3' },
            { bufferView: 1, componentType: 5126, count: 2, type: 'SCALAR' },
            { bufferView: 2, componentType: 5126, count: 2, type: 'VEC3' },
          ],
          animations: [{ name: 'slide', samplers: [{ input: 1, output: 2 }], channels: [{ sampler: 0, target: { node: 0, path: 'translation' } }] }],
        };
        const url = 'data:model/gltf+json;base64,' + btoa(JSON.stringify(gltf));
        window.s = ak.scene('#c', { dpr: 1, adaptive: false, autoRender: false, shadows: false, camera: { position: [0, 0, 4] } });
        const m = await ak.loadGLTF(s, url);
        const hit = s.raycast(160, 120);
        const out = { names: m.animations, hit: hit && hit.point, green: hit && hit.object.material.color[1] };
        m.play('slide');
        await new Promise((res) => setTimeout(res, 1300));
        out.after = s.raycast(160, 120);
        return out;
      });
      ok(r.names[0] === 'slide', 'animation names');
      ok(r.hit && Math.abs(r.hit[2] + 1) < 1e-4, `triangle hit at z = -1 (${r.hit})`);
      ok(r.green === 1, 'material colour');
      ok(r.after === null, 'animation slid the triangle away from the centre');
    },
  },
];
