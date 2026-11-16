// scene() vs three.js in Chromium (Playwright): start-up time, draw calls and
// frame time for 10k instanced boxes (matrices rewritten every frame, shadows on).
// Usage: node scripts/build.mjs && node scripts/bench-scene.mjs [--gpu]
// Headless Chromium has no GPU here, so by default this runs WebGL on SwiftShader
// (CPU): absolute numbers are software-rendering numbers; compare the ratio.
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(join(pkg, 'package.json'));
const { chromium } = require('playwright');
const threePath = join(pkg, 'node_modules/three/build/three.module.js');
const hasThree = existsSync(threePath);
const N = 10000, FRAMES = 60, W = 960, H = 540;
const args = process.argv.includes('--gpu') ? [] : ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'];
const browser = await chromium.launch({ args });

const files = {
  '/anikit.js': readFileSync(join(pkg, 'dist/anikit.js'), 'utf8'),
  ...(hasThree ? { '/three.module.js': readFileSync(threePath, 'utf8'), '/three.core.js': readFileSync(join(pkg, 'node_modules/three/build/three.core.js'), 'utf8') } : {}),
};

const ANIKIT = `
import { scene, box, ready } from '/anikit.js';
await ready;
const t0 = performance.now();
const s = scene('#c', { dpr: 1, adaptive: false, autoRender: false, shadows: true, background: '#555555', camera: { position: [0, 40, 90], fov: 45 } });
s.add('plane', { scale: [200, 1, 200], castShadow: false, material: { color: '#444', roughness: 0.9 } });
const inst = s.instances(box(0.6, 0.6, 0.6), ${N}, (i, o) => o.position.set((i % 100) - 50, 0.3, ((i / 100) | 0) - 50), { material: 'plastic' });
s.render(); s.gl.readPixels(0, 0, 1, 1, s.gl.RGBA, s.gl.UNSIGNED_BYTE, new Uint8Array(4));
const startup = performance.now() - t0;
let k = 0;
const frame = () => { k++; inst.update((i, o) => { o.position.set((i % 100) - 50, 0.3 + Math.sin(k * 0.1 + i * 0.05), ((i / 100) | 0) - 50); o.rotation.y = k * 0.02 + i; }); s.render(); };
for (let i = 0; i < 10; i++) frame();
s.gl.finish();
const px = new Uint8Array(4);
const t1 = performance.now();
for (let i = 0; i < ${FRAMES}; i++) { frame(); s.gl.readPixels(0, 0, 1, 1, s.gl.RGBA, s.gl.UNSIGNED_BYTE, px); }
const frameMs = (performance.now() - t1) / ${FRAMES};
window.result = { startup, frameMs, drawCalls: s.stats.drawCalls };
`;
const THREE_SRC = `
import * as THREE from '/three.module.js';
const t0 = performance.now();
const canvas = document.querySelector('#c');
const r = new THREE.WebGLRenderer({ canvas, antialias: true });
r.setPixelRatio(1); r.setSize(${W}, ${H}, false);
r.shadowMap.enabled = true; r.shadowMap.type = THREE.PCFShadowMap; r.toneMapping = THREE.ACESFilmicToneMapping;
const sc = new THREE.Scene(); sc.background = new THREE.Color('#555');
const cam = new THREE.PerspectiveCamera(45, ${W} / ${H}, 0.05, 200); cam.position.set(0, 40, 90); cam.lookAt(0, 0, 0);
sc.add(new THREE.HemisphereLight(0xffffff, 0x333333, 1));
const sun = new THREE.DirectionalLight(0xffffff, 2.2); sun.position.set(35, 85, 40); sun.castShadow = true;
sun.shadow.mapSize.set(2048, 2048); Object.assign(sun.shadow.camera, { left: -60, right: 60, top: 60, bottom: -60, far: 300 }); sc.add(sun);
const floor = new THREE.Mesh(new THREE.PlaneGeometry(200, 200), new THREE.MeshStandardMaterial({ color: 0x444444, roughness: 0.9 }));
floor.rotation.x = -Math.PI / 2; floor.receiveShadow = true; sc.add(floor);
const mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(0.6, 0.6, 0.6), new THREE.MeshStandardMaterial({ color: 0xe8e8e8, roughness: 0.38 }), ${N});
mesh.castShadow = true; mesh.receiveShadow = true; sc.add(mesh);
const d = new THREE.Object3D();
const place = (k) => { for (let i = 0; i < ${N}; i++) { d.position.set((i % 100) - 50, 0.3 + (k ? Math.sin(k * 0.1 + i * 0.05) : 0), ((i / 100) | 0) - 50); d.rotation.set(0, k * 0.02 + i, 0); d.updateMatrix(); mesh.setMatrixAt(i, d.matrix); } mesh.instanceMatrix.needsUpdate = true; };
place(0);
const gl = r.getContext();
r.render(sc, cam); gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(4));
const startup = performance.now() - t0;
let k = 0;
const frame = () => { k++; place(k); r.render(sc, cam); };
for (let i = 0; i < 10; i++) frame();
gl.finish();
const px = new Uint8Array(4);
const t1 = performance.now();
for (let i = 0; i < ${FRAMES}; i++) { frame(); gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px); }
const frameMs = (performance.now() - t1) / ${FRAMES};
window.result = { startup, frameMs, drawCalls: r.info.render.calls };
`;

async function run(name, code) {
  const page = await browser.newPage({ viewport: { width: W, height: H } });
  page.on('pageerror', (e) => console.error(name, e.message));
  await page.route('http://bench.local/**', (route) => {
    const p = new URL(route.request().url()).pathname;
    if (files[p]) return route.fulfill({ status: 200, contentType: 'text/javascript', body: files[p] });
    return route.fulfill({ status: 200, contentType: 'text/html', body: `<!doctype html><style>body{margin:0}canvas{width:${W}px;height:${H}px;display:block}</style><canvas id="c" width="${W}" height="${H}"></canvas><script type="module">${code}</script>` });
  });
  await page.goto('http://bench.local/index.html', { waitUntil: 'commit' });
  await page.waitForFunction(() => window.result, null, { timeout: 300000, polling: 500 });
  const r = await page.evaluate(() => window.result);
  await page.close();
  return r;
}

const rows = [['anikit scene()', await run('anikit', ANIKIT)]];
if (hasThree) rows.push(['three.js ' + JSON.parse(readFileSync(join(pkg, 'node_modules/three/package.json'), 'utf8')).version, await run('three', THREE_SRC)]);
else console.log('(three not installed: npm i -D three to compare)');
console.log(`\n${N} instanced boxes, shadows on, ${W}×${H} @1x, ${args.length ? 'SwiftShader (CPU WebGL)' : 'GPU'}; ${FRAMES} frames, each synced with readPixels`);
console.log('renderer'.padEnd(18), 'start-up ms'.padStart(12), 'frame ms'.padStart(10), 'draw calls'.padStart(11));
for (const [n, r] of rows) console.log(n.padEnd(18), r.startup.toFixed(1).padStart(12), r.frameMs.toFixed(2).padStart(10), String(r.drawCalls).padStart(11));
await browser.close();
