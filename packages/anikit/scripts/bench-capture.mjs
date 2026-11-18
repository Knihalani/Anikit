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
const buildCapture = async (globalName, source) => {
  const contents = `export * from ${JSON.stringify(source)}; export { animate, engine } from './src/index';`;
  return (
    await build({
      stdin: { contents, resolveDir: pkg, sourcefile: `${globalName}.ts` },
      bundle: true,
      format: 'iife',
      globalName,
      platform: 'browser',
      write: false,
      logLevel: 'error',
    })
  ).outputFiles[0].text;
};
const newBundle = await buildCapture('captureNew', join(pkg, 'src/capture.ts'));
const oldBundle = existsSync(oldSource) ? await buildCapture('captureOld', oldSource) : null;

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
  const setupContents = fixture.html.match(/<script>([\s\S]*?)<\/script>/)?.[1] ?? '';
  const setupScript = setupContents ? `<script>(() => {${setupContents}})();</script>` : '';
  return `<!doctype html><html><head>${headStyle}</head><body><script>
    const srcDescriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
    Object.defineProperty(HTMLImageElement.prototype, 'src', {
      configurable: true,
      get() { return srcDescriptor.get.call(this); },
      set(value) {
        const url = String(value);
        if (url.startsWith('data:image/svg+xml')) {
          window.__svgLengths.push(url.length);
          const lowered = url.toLowerCase();
          window.__lastSvgHasIframe = lowered.includes('%3ciframe') || lowered.includes('<iframe');
        }
        srcDescriptor.set.call(this, value);
      },
    });
    window.__svgLengths = [];
    window.__lastSvgHasIframe = false;
  </script><script>${oldBundle ?? ''}</script><script>${newBundle}</script>${body}${setupScript}</body></html>`;
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
      return {
        pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data),
        width: canvas.width,
        height: canvas.height,
        svgLength: window.__svgLengths.at(-1) ?? 0,
        svgHasIframe: window.__lastSvgHasIframe,
      };
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

const checks = {};
if (oldBundle) {
  const pixelDiff = (older, newer) => {
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
    return { max, pixels };
  };
  const documentFixture = { html: '<main style="width:220px;height:120px;padding:8px;background:#eef"><p>document root</p></main>' };
  const documentPage = await browser.newPage({ viewport: { width: 320, height: 200 } });
  await documentPage.setContent(makePage(documentFixture));
  const documentNew = await documentPage.evaluate(async () => {
    const read = (canvas) => {
      const { width, height } = canvas;
      return { width, height, pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, width, height).data) };
    };
    const before = document.documentElement.outerHTML;
    const iframesBefore = document.querySelectorAll('iframe').length;
    window.__svgLengths = [];
    const newer = read(await window.captureNew.rasterize(undefined, { scale: 1, background: '#fff' }));
    const svgHasIframe = window.__lastSvgHasIframe;
    const afterNew = document.documentElement.outerHTML;
    const iframesAfterNew = document.querySelectorAll('iframe').length;
    return { newer, unchanged: before === afterNew, iframeCounts: `${iframesBefore}/${iframesAfterNew}`, svgHasIframe };
  });
  const documentOldPage = await browser.newPage({ viewport: { width: 320, height: 200 } });
  await documentOldPage.setContent(makePage(documentFixture));
  const documentOld = await documentOldPage.evaluate(async () => {
    const canvas = await window.captureOld.rasterize(undefined, { scale: 1, background: '#fff' });
    return { width: canvas.width, height: canvas.height, pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data) };
  });
  checks.documentElement = { ...pixelDiff(documentOld, documentNew.newer), unchanged: documentNew.unchanged, iframeCounts: documentNew.iframeCounts, svgHasIframe: documentNew.svgHasIframe };
  await documentPage.close();
  await documentOldPage.close();

  const lastChildFixture = { html: `<style>body{margin:0}body > :last-child{background:red}</style><div id="target" style="width:220px;height:100px;padding:8px">last child</div>` };
  const lastChildPage = await browser.newPage({ viewport: { width: 320, height: 200 } });
  await lastChildPage.setContent(makePage(lastChildFixture));
  const lastChildNew = await lastChildPage.evaluate(async () => {
    const read = (canvas) => {
      const { width, height } = canvas;
      return { width, height, pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, width, height).data) };
    };
    const target = document.querySelector('#target');
    const before = document.documentElement.outerHTML;
    const colorBefore = getComputedStyle(target).backgroundColor;
    window.__svgLengths = [];
    const newer = read(await window.captureNew.rasterize(target, { scale: 1, background: '#fff' }));
    const afterNew = document.documentElement.outerHTML;
    const colorAfter = getComputedStyle(target).backgroundColor;
    return { newer, unchanged: before === afterNew, colorBefore, colorAfter, iframeCount: document.querySelectorAll('iframe').length, isLastChild: target === document.body.lastElementChild };
  });
  const lastChildOldPage = await browser.newPage({ viewport: { width: 320, height: 200 } });
  await lastChildOldPage.setContent(makePage({
    html: `<style>body{margin:0}body > :last-child{background:red}</style><div id="prime" style="width:1px;height:1px"></div>`,
  }));
  await lastChildOldPage.evaluate(async () => {
    await window.captureOld.rasterize(document.querySelector('#prime'), { scale: 1, background: '#fff' });
    const target = document.createElement('div');
    target.id = 'target';
    target.setAttribute('style', 'width:220px;height:100px;padding:8px');
    target.textContent = 'last child';
    document.querySelector('#prime').remove();
    document.body.append(target);
  });
  const lastChildOld = await lastChildOldPage.evaluate(async () => {
    const target = document.querySelector('#target');
    const canvas = await window.captureOld.rasterize(target, { scale: 1, background: '#fff' });
    return {
      width: canvas.width,
      height: canvas.height,
      pixels: Array.from(canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data),
    };
  });
  checks.lastChild = {
    ...pixelDiff(lastChildOld, lastChildNew.newer),
    unchanged: lastChildNew.unchanged,
    colorBefore: lastChildNew.colorBefore,
    colorAfter: lastChildNew.colorAfter,
    iframeCount: lastChildNew.iframeCount,
    isLastChild: lastChildNew.isLastChild,
  };
  await lastChildPage.close();
  await lastChildOldPage.close();

  await page.setContent(makePage({
    html: `<style>
      body{margin:0;font:16px Arial,sans-serif}
      .record-root{box-sizing:border-box;width:260px;height:140px;padding:12px;color:#123;background:#f3f5f8}
      .middle{padding:8px;border:1px solid #567;background:#fff}
      .middle.active{font-size:22px;background:#f9c}
      .moving{display:inline-block;transform-origin:0 0}
    </style>
    <div id="old-root" class="record-root"><div class="middle"><span class="moving">animated inherited text</span></div></div>
    <div id="new-root" class="record-root"><div class="middle"><span class="moving">animated inherited text</span></div></div>`,
  }));
  checks.record = await page.evaluate(async () => {
    const captureFrames = async (api, selector) => {
      const root = document.querySelector(selector);
      const middle = root.querySelector('.middle');
      api.engine.manual(true);
      api.animate(root, { color: ['#123456', '#e02040'] }, { duration: 1, ease: 'linear', repeat: -1 });
      api.animate(root.querySelector('.moving'), { x: [0, 18], rotate: [0, 14] }, { duration: 1, ease: 'linear', repeat: -1 });
      let frame = 0;
      const result = await api.record(root, {
        duration: 1,
        fps: 10,
        format: 'frames',
        scale: 1,
        background: '#fff',
        onProgress: () => middle.classList.toggle('active', frame++ % 2 === 0),
      });
      const frames = [];
      for (const blob of result.images ?? []) {
        const bitmap = await createImageBitmap(blob);
        const canvas = document.createElement('canvas');
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const ctx = canvas.getContext('2d');
        ctx.drawImage(bitmap, 0, 0);
        frames.push({ width: canvas.width, height: canvas.height, pixels: ctx.getImageData(0, 0, canvas.width, canvas.height).data });
        bitmap.close();
      }
      return frames;
    };
    const oldFrames = await captureFrames(window.captureOld, '#old-root');
    const newFrames = await captureFrames(window.captureNew, '#new-root');
    let max = 0, pixels = 0;
    if (oldFrames.length !== newFrames.length) {
      max = 255;
      pixels++;
    }
    for (let f = 0; f < Math.min(oldFrames.length, newFrames.length); f++) {
      const a = oldFrames[f], b = newFrames[f];
      if (a.width !== b.width || a.height !== b.height) {
        max = 255;
        pixels++;
        continue;
      }
      for (let i = 0; i < a.pixels.length; i += 4) {
        let changed = false;
        for (let c = 0; c < 4; c++) {
          const d = Math.abs(a.pixels[i + c] - b.pixels[i + c]);
          max = Math.max(max, d);
          changed ||= d !== 0;
        }
        if (changed) pixels++;
      }
    }
    return { framesOld: oldFrames.length, framesNew: newFrames.length, max, pixels };
  });

  const median = (values) => {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted.length % 2 ? sorted[(sorted.length - 1) / 2] : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
  };
  const summarize = (values) => ({ mean: values.reduce((sum, v) => sum + v, 0) / values.length, median: median(values) });
  await page.setViewportSize({ width: 1000, height: 800 });
  await page.setContent(makePage(fixtures[0]));
  const rasterSamples = await page.evaluate(async () => {
    const target = document.querySelector('#target');
    const card = target.querySelector('.card');
    if (!card) throw new Error('capture benchmark grid fixture has no cards');
    const counter = document.createElement('span');
    counter.hidden = true;
    target.append(counter);
    const apis = { old: window.captureOld, newer: window.captureNew };
    const mutate = (i) => {
      counter.textContent = `run ${i}`;
      card.style.transform = `translateX(${i}px)`;
    };
    for (let i = 0; i < 5; i++) {
      mutate(`warm-${i}`);
      await apis.old.rasterize(target, { scale: 1, background: '#fff' });
      await apis.newer.rasterize(target, { scale: 1, background: '#fff' });
    }
    const samples = { old: [], newer: [] };
    const n = 40;
    for (let i = 0; i < n; i++) {
      mutate(i);
      const order = i % 2 ? ['old', 'newer'] : ['newer', 'old'];
      for (const name of order) {
        const start = performance.now();
        await apis[name].rasterize(target, { scale: 1, background: '#fff' });
        samples[name].push(performance.now() - start);
      }
    }
    return samples;
  });
  const recordSamples = { old: [], newer: [] };
  for (let repetition = 0; repetition < 2; repetition++) {
    const order = repetition % 2 ? ['old', 'newer'] : ['newer', 'old'];
    for (const name of order) {
      const recordPage = await browser.newPage({ viewport: { width: 1000, height: 800 } });
      await recordPage.setContent(makePage(fixtures[0]));
      const ms = await recordPage.evaluate(async (apiName) => {
        const api = window[apiName];
        const target = document.querySelector('#target');
        api.engine.manual(true);
        await api.record(target, { duration: 4, fps: 30, format: 'gif', scale: 1, background: '#fff' });
        api.animate(target, { color: ['#243044', '#8b2540'] }, { duration: 1, ease: 'linear', repeat: -1 });
        const cards = Array.from(target.querySelectorAll('.card')).slice(0, 5);
        cards.forEach((card, i) => api.animate(card, { x: [0, 10 + i], rotate: [0, i + 2] }, { duration: 1, ease: 'linear', repeat: -1 }));
        const start = performance.now();
        await api.record(target, { duration: 1, fps: 30, format: 'gif', scale: 1, background: '#fff' });
        return (performance.now() - start) / 30;
      }, name === 'old' ? 'captureOld' : 'captureNew');
      recordSamples[name].push(ms);
      await recordPage.close();
    }
  }
  checks.benchmark = {
    rasterize: { old: summarize(rasterSamples.old), newer: summarize(rasterSamples.newer), iterations: 40, warmups: 5 },
    record: { old: summarize(recordSamples.old), newer: summarize(recordSamples.newer), runs: 2, duration: 1, fps: 30, warmupFrames: 120 },
  };
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
if (checks.documentElement) {
  lines.push(`documentElement rasterize: max channel diff ${checks.documentElement.max}; differing pixels ${checks.documentElement.pixels}; iframes before/after-new ${checks.documentElement.iframeCounts}; DOM unchanged ${checks.documentElement.unchanged}; SVG contains iframe ${checks.documentElement.svgHasIframe}`);
  lines.push(`body last-child target: max channel diff ${checks.lastChild.max}; differing pixels ${checks.lastChild.pixels}; background ${checks.lastChild.colorBefore} → ${checks.lastChild.colorAfter}; target remains last child ${checks.lastChild.isLastChild}; iframes remaining ${checks.lastChild.iframeCount}; DOM unchanged ${checks.lastChild.unchanged}`);
  lines.push(`animated record() frames: ${checks.record.framesOld}/${checks.record.framesNew}; max channel diff ${checks.record.max}; differing pixel-frames ${checks.record.pixels}`);
  const b = checks.benchmark;
  lines.push('');
  lines.push(`Rasterize warm-up/samples: ${b.rasterize.warmups}/${b.rasterize.iterations}; mean/median ms per call old/new: ${b.rasterize.old.mean.toFixed(3)}/${b.rasterize.newer.mean.toFixed(3)} / ${b.rasterize.old.median.toFixed(3)}/${b.rasterize.newer.median.toFixed(3)}`);
  lines.push(`Animated GIF record runs: ${b.record.runs} each, ${b.record.duration}s at ${b.record.fps} fps; ${b.record.warmupFrames}-frame warm-up per page; mean/median ms per frame old/new: ${b.record.old.mean.toFixed(3)}/${b.record.newer.mean.toFixed(3)} / ${b.record.old.median.toFixed(3)}/${b.record.newer.median.toFixed(3)}`);
  const failed =
    results.some((r) => r.pixels !== 0 || r.max !== 0) ||
    checks.documentElement.max !== 0 ||
    checks.documentElement.pixels !== 0 ||
    !checks.documentElement.unchanged ||
    checks.documentElement.iframeCounts !== '0/0' ||
    checks.documentElement.svgHasIframe ||
    checks.lastChild.max !== 0 ||
    checks.lastChild.pixels !== 0 ||
    !checks.lastChild.unchanged ||
    checks.lastChild.colorBefore !== checks.lastChild.colorAfter ||
    checks.lastChild.iframeCount !== 0 ||
    !checks.lastChild.isLastChild ||
    checks.record.max !== 0 ||
    checks.record.pixels !== 0;
  if (failed) process.exitCode = 1;
}
const report = lines.join('\n') + '\n';
console.log(report);
if (outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, report);
}
