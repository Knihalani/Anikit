// Bundle the TypeScript layer (ESM + CJS + minified ESM) and emit typings.
import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import { statSync } from 'node:fs';
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
// anikit/react: imports the main bundle (one engine) and React, both external.
const shareCore = {
  name: 'share-core',
  setup(b) {
    b.onResolve({ filter: /^\.\// }, (a) => (/(react|vue|svelte)\.ts$/.test(a.importer) ? { path: './anikit.js', external: true } : undefined));
  },
};
for (const fw of ['react', 'vue', 'svelte']) {
  await build({ ...common, entryPoints: [`src/${fw}.ts`], format: 'esm', outfile: `dist/${fw}.js`, external: ['react'], plugins: [shareCore] });
}
execFileSync('npx', ['tsc', '--emitDeclarationOnly', '--declaration', '--outDir', 'dist/types'], { stdio: 'inherit' });

const kb = (n) => (n / 1024).toFixed(1) + ' KB';
for (const f of ['dist/anikit.min.js', 'dist/anikit.lean.js', 'dist/anikit.wasm']) {
  const raw = readFileSync(f);
  console.log(`${f}: ${kb(statSync(f).size)} (gzip ${kb(gzipSync(raw, { level: 9 }).length)})`);
}
