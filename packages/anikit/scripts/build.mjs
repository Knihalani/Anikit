// Bundle the TypeScript layer (ESM + CJS + minified ESM) and emit typings.
import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import { statSync, writeFileSync, unlinkSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { readFileSync } from 'node:fs';

const common = {
  entryPoints: ['src/index.ts'], bundle: true, target: 'es2020', sourcemap: true, logLevel: 'warning',
  // The import.meta path is only reachable in the lean ESM build.
  logOverride: { 'empty-import-meta': 'silent' },
};
await build({ ...common, format: 'esm', outfile: 'dist/anikit.js' });
await build({ ...common, format: 'cjs', outfile: 'dist/anikit.cjs' });
await build({ ...common, format: 'esm', minify: true, outfile: 'dist/anikit.min.js' });
await build({ ...common, format: 'iife', globalName: 'anikit', minify: true, outfile: 'dist/anikit.iife.js' });
// Lean ESM build: no embedded module; fetches ./anikit.wasm (streaming compile).
const lean = {
  name: 'lean-wasm',
  setup(b) {
    b.onResolve({ filter: /generated\/wasm-bytes$/ }, () => ({ path: 'wasm-bytes', namespace: 'lean' }));
    b.onLoad({ filter: /.*/, namespace: 'lean' }, () => ({ contents: "export const WASM_BASE64 = '';", loader: 'ts' }));
  },
};
await build({ ...common, format: 'esm', minify: true, outfile: 'dist/anikit.lean.js', plugins: [lean] });
// Tween build: animate / timeline / stagger / spring only, with a core module
// keeping just the exports that bundle calls (wasm-metadce drops the others
// and, with them, everything only they reach), then wasm-opt -Oz.
await build({ ...common, entryPoints: ['src/tween.ts'], format: 'esm', minify: true, sourcemap: false, outfile: 'dist/anikit.tween.js', plugins: [lean] });
{
  const js = readFileSync('dist/anikit.tween.js', 'utf8').replace('./anikit.wasm', './anikit.tween.wasm');
  writeFileSync('dist/anikit.tween.js', js);
  const used = [...new Set(js.match(/\bak_[a-z0-9_]+/g) ?? [])].sort();
  const graph = [{ name: 'memory', root: true, export: 'memory' }, ...used.map((n) => ({ name: n, root: true, export: n }))];
  writeFileSync('dist/.tween-graph.json', JSON.stringify(graph));
  const bin = 'node_modules/binaryen/bin/';
  try {
    execFileSync(bin + 'wasm-metadce', ['dist/anikit.wasm', '--graph-file', 'dist/.tween-graph.json', '--enable-simd', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '-o', 'dist/anikit.tween.wasm'], { stdio: ['ignore', 'ignore', 'inherit'] });
    execFileSync(bin + 'wasm-opt', ['dist/anikit.tween.wasm', '-Oz', '--strip-debug', '--strip-producers', '--enable-simd', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '-o', 'dist/anikit.tween.wasm'], { stdio: 'inherit' });
  } catch (e) {
    console.warn('tween wasm: binaryen failed, shipping the full module as anikit.tween.wasm', e.message);
    writeFileSync('dist/anikit.tween.wasm', readFileSync('dist/anikit.wasm'));
  }
  unlinkSync('dist/.tween-graph.json');
  console.log(`tween build: ${used.length} core exports kept`);
  // Smoke run: every entry point of the tween bundle against the stripped core.
  execFileSync(process.execPath, ['--input-type=module', '-e', `
    const a = await import('./dist/anikit.tween.js'); await a.ready; a.engine.manual(true);
    const o = { x: 0, c: '#ff0000', w: '10px' }, os = [{ y: 0 }, { y: 0 }], p = { v: 0 }, q = { z: 0 };
    a.animate(o, { x: [0, 50, 100], c: '#0000ff', w: '20px' }, { duration: 1, ease: 'outCubic' });
    a.animate(os, { y: 100 }, { duration: 1, delay: a.stagger(0.1) });
    a.spring(p, { v: 10 }, { stiffness: 200, damping: 20 });
    a.inertia(q, { z: { velocity: 100 } });
    a.timeline().add(os[0], { y: 5 }, { duration: 0.5 }).add(o, { x: 1 }, { duration: 0.5 }).play();
    for (let i = 0; i < 200; i++) a.engine.tick(16);
    if (o.c !== 'rgb(0,0,255)' || Math.abs(p.v - 10) > 1e-3) throw new Error('tween bundle smoke run: ' + JSON.stringify([o, p]));
  `], { stdio: 'inherit' });
}
execFileSync('npx', ['tsc', '--emitDeclarationOnly', '--declaration', '--outDir', 'dist/types'], { stdio: 'inherit' });

const kb = (n) => (n / 1024).toFixed(1) + ' KB';
for (const f of ['dist/anikit.min.js', 'dist/anikit.lean.js', 'dist/anikit.wasm', 'dist/anikit.tween.js', 'dist/anikit.tween.wasm']) {
  const raw = readFileSync(f);
  console.log(`${f}: ${kb(statSync(f).size)} (gzip ${kb(gzipSync(raw, { level: 9 }).length)})`);
}
