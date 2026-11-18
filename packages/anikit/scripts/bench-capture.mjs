// Compare capture output and cost in Chromium.
// Usage: node scripts/bench-capture.mjs --old src/capture_old.ts --output bench/capture-results.txt
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const require = createRequire(join(pkg, 'package.json'));
let playwright;
try {
  playwright = require('playwright');
} catch {
  playwright = require(join(process.execPath, '../../lib/node_modules/playwright'));
}
const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i < 0 ? undefined : resolve(process.argv[i + 1]);
};
const oldSource = arg('--old') ?? join(pkg, 'src/capture_old.ts');
const outputPath = arg('--output');
const newBundle = (await build({
  entryPoints: [join(pkg, 'src/capture.ts')],
  bundle: true,
  format: 'iife',
  globalName: 'captureNew',
  platform: 'browser',
  write: false,
  logLevel: 'error',
})).outputFiles[0].text;
const oldBundle = existsSync(oldSource)
  ? (await build({
      entryPoints: [oldSource],
      bundle: true,
      format: 'iife',
      globalName: 'captureOld',
      platform: 'browser',
      write: false,
      logLevel: 'error',
    })).outputFiles[0].text
  : null;

const cardFixture = `
<style>
  body{margin:0;font:14px Arial,sans-serif;color:#243044}
  .capture{width:800px;height:600px;overflow:hidden;padding:12px;background:#f1f4f8}
  .grid{display:grid;grid-template-columns:repeat(4,1fr);gap:8px}
  .card{display:flex;flex-direction:column;min-height:100px;padding:8px;border:1px solid #c9d2df;border-radius:7px;background:linear-gradient(145deg,#fff,#e7edf5);box-shadow:0 2px 5px #18253822}
  .card h2{font-size:15px;margin:0 0 4px}.card p{margin:2px 0;line-height:1.3}
  .card img{width:24px;height:16px;object-fit:cover}.card button{margin-top:auto;border:1px solid #315b8a;border-radius:4px;background:#376fa9;color:white;padding:3px}
  .card ul{display:flex;gap:8px;margin:3px 0;padding-left:16px}
</style>
<div class="capture" id="target"><div class="grid"></div></div>
<script>
const grid=document.querySelector('.grid');
const svg='data:image/svg+xml,'+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><rect width="24" height="16" fill="#7aa"/></svg>');
for(let i=0;i<20;i++){
 const card=document.createElement('div'); card.className='card';
 const title=document.createElement('h2'); title.textContent='Card '+i; card.append(title);
 const image=document.createElement('img'); image.src=svg; image.alt=''; card.append(image);
 const p=document.createElement('p'); p.innerHTML='<span>Item '+i+'</span> <b>detail</b>'; card.append(p);
 const list=document.createElement('ul'); list.innerHTML='<li>A</li><li>B</li>'; card.append(list);
 const button=document.createElement('button'); button.textContent='Open'; card.append(button);
 grid.append(card);
}
</script>`;

const fixtures = [
  { name: '200-element card grid', html: cardFixture },
  {
    name: 'zero border and outline widths',
    html: `<style>.capture{width:240px;height:100px}.border{border-style:solid;border-width:0}.outline{outline-style:solid;outline-width:0}</style><div class="capture" id="target"><div class="border">Border</div><div class="outline">Outline</div></div>`,
  },
  {
    name: 'rtl list padding override',
    html: `<style>.capture{width:280px;height:120px}ul{padding-left:40px;padding-right:0}</style><div class="capture" id="target" dir="rtl"><ul><li>one</li><li>two</li></ul></div>`,
  },
  {
    name: 'link text decoration and color',
    html: `<style>.capture{width:240px;height:70px}a{color:#a12345;text-decoration:none}</style><div class="capture" id="target"><a href="#">Link</a></div>`,
  },
  {
    name: 'bold inherited weight',
    html: `<style>.capture{width:240px;height:70px;font-weight:700}b{font-weight:700}</style><div class="capture" id="target"><p>Text <b>bold</b></p></div>`,
  },
  {
    name: 'table border attribute and style',
    html: `<style>.capture{width:260px;height:120px}td{padding:5px}</style><div class="capture" id="target"><table border="1" style="border:none"><tbody><tr><td>A</td><td>B</td></tr></tbody></table></div>`,
  },
  {
    name: 'currentcolor borders and explicit black border',
    html: `<style>.capture{width:300px;height:100px;color:purple}.colored{color:green;border:2px solid currentcolor}.explicit{border:2px solid black}</style><div class="capture" id="target"><div class="colored">Current color</div><div class="explicit" style="color:red">Explicit</div></div>`,
  },
  {
    name: 'paragraph em margins and heading sizes',
    html: `<style>.capture{width:300px;height:180px}p{font-size:20px;margin:16px 0}h1{font-size:30px;line-height:1.1;margin:12px 0}</style><div class="capture" id="target"><p>Paragraph</p><h1>Heading</h1></div>`,
  },
  {
    name: 'transformed inline block origin',
    html: `<style>.capture{width:220px;height:100px}span{display:inline-block;transform-origin:0 0;transform:rotate(10deg);margin:20px}</style><div class="capture" id="target"><span>Rotate</span></div>`,
  },
  {
    name: 'inline svg and canvas',
    html: `<style>.capture{width:260px;height:140px}svg{width:70px;height:50px}canvas{width:80px;height:50px}</style><div class="capture" id="target"><svg viewBox="0 0 70 50"><circle cx="20" cy="20" r="14" fill="orange"/><path d="M2 45L68 4" stroke="blue" stroke-width="3"/></svg><canvas width="80" height="50"></canvas></div>`,
  },
  {
    name: 'style element within capture',
    html: `<style>.capture{width:260px;height:100px}.local{color:#248}</style><div class="capture" id="target"><style>.local{font-size:23px;text-decoration:underline}</style><p class="local">Local stylesheet</p></div>`,
  },
  {
    name: 'inherited color and nested font properties',
    html: `<style>.capture{width:300px;height:140px;color:#13579b;font-family:Arial}.one{font-family:Georgia;font-size:18px}.two{font-family:Verdana;font-size:16px}.three{font-family:Tahoma;font-size:14px}</style><div class="capture" id="target"><div class="one">one<div class="two">two<div class="three">three<div>four inherited color</div></div></div></div></div>`,
  },
];

const makePage = (fixture) => {
  const headStyle = fixture.html.match(/<style>[\s\S]*?<\/style>/)?.[0] ?? '';
  const body = fixture.html.replace(headStyle, '').replace(/<script>[\s\S]*?<\/script>/g, '');
  const setupScript = fixture.html.match(/<script>[\s\S]*?<\/script>/)?.[0] ?? '';
  return `<!doctype html><html><head>${headStyle}</head><body>${body}${setupScript}<script>
    const srcDescriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
    Object.defineProperty(HTMLImageElement.prototype, 'src', {
      configurable: true,
      get() { return srcDescriptor.get.call(this); },
      set(value) {
        if (String(value).startsWith('data:image/svg+xml')) window.__svgLengths.push(String(value).length);
        srcDescriptor.set.call(this, value);
      },
    });
    window.__svgLengths = [];
  </script><script>${oldBundle ?? ''}</script><script>${newBundle}</script></body></html>`;
};
const browser = await playwright.chromium.launch();
const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
const results = [];
for (const fixture of fixtures) {
  await page.setContent(makePage(fixture));
  await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (canvas) {
      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#e52';
      ctx.fillRect(0, 0, 80, 50);
      ctx.fillStyle = '#fff';
      ctx.fillRect(8, 8, 22, 22);
    }
  });
  const result = await page.evaluate(async ({ compare }) => {
    const target = document.querySelector('#target');
    const capture = async (api) => {
      window.__svgLengths = [];
      const canvas = await api.rasterize(target, { scale: 1, background: '#fff' });
      return { pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data), width: canvas.width, height: canvas.height, svgLength: window.__svgLengths.at(-1) ?? 0 };
    };
    const newer = await capture(window.captureNew);
    if (!compare) return { newer };
    const older = await capture(window.captureOld);
    let max = older.width === newer.width && older.height === newer.height ? 0 : 255;
    let pixels = older.width === newer.width && older.height === newer.height ? 0 : 1;
    for (let i = 0; i < Math.min(older.pixels.length, newer.pixels.length); i += 4) {
      let changed = false;
      for (let c = 0; c < 4; c++) {
        const d = Math.abs(older.pixels[i + c] - newer.pixels[i + c]);
        max = Math.max(max, d);
        changed ||= d !== 0;
      }
      if (changed) pixels++;
    }
    return { newer, max, pixels, oldSvgLength: older.svgLength };
  }, { compare: !!oldBundle });
  results.push({ name: fixture.name, ...result });
}

let bench = null;
if (oldBundle) {
  await page.setContent(makePage(fixtures[0]));
  bench = await page.evaluate(async () => {
    const target = document.querySelector('#target');
    const measure = async (api) => {
      for (let i = 0; i < 3; i++) await api.rasterize(target, { scale: 1, background: '#fff' });
      const n = 8, t = performance.now();
      for (let i = 0; i < n; i++) await api.rasterize(target, { scale: 1, background: '#fff' });
      const rasterMs = (performance.now() - t) / n;
      const recordStart = performance.now();
      await api.record(target, { duration: 1, fps: 30, format: 'gif', scale: 1, background: '#fff' });
      return { rasterMs, recordMs: (performance.now() - recordStart) / 30 };
    };
    return { old: await measure(window.captureOld), newer: await measure(window.captureNew) };
  });
}
await browser.close();

const lines = [];
lines.push('Capture pixel-equivalence and performance results');
lines.push(`old source: ${oldBundle ? oldSource : 'not supplied; new-only benchmark'}`);
lines.push('');
lines.push('Pixel comparison (scale 1, white background):');
for (const r of results) {
  lines.push(
    oldBundle
      ? `${r.name}: max channel diff ${r.max}; differing pixels ${r.pixels}; SVG data URL chars old/new ${r.oldSvgLength}/${r.newer.svgLength}`
      : `${r.name}: SVG data URL chars new ${r.newer.svgLength}`,
  );
}
if (bench) {
  lines.push('');
  lines.push('Warm rasterize mean (ms/call) and GIF record mean (ms/frame, 1 s at 30 fps):');
  lines.push(`old: ${bench.old.rasterMs.toFixed(3)} / ${bench.old.recordMs.toFixed(3)}`);
  lines.push(`new: ${bench.newer.rasterMs.toFixed(3)} / ${bench.newer.recordMs.toFixed(3)}`);
  const grid = results[0];
  lines.push(`card grid SVG data URL chars old/new: ${grid.oldSvgLength}/${grid.newer.svgLength}`);
  if (results.some((r) => r.pixels !== 0 || r.max !== 0)) process.exitCode = 1;
}
const report = lines.join('\n') + '\n';
console.log(report);
if (outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, report);
}
