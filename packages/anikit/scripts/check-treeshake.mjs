// Tree-shaking check: bundle single-import entries with esbuild (metafile per
// entry) and report which source modules each one pulls in, plus its size.
// Fails when a small entry drags in a heavy module it doesn't use.
//   node scripts/check-treeshake.mjs [--verbose]
import { build } from 'esbuild';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const HEAVY = ['src/blackhole.ts', 'src/video.ts', 'src/mesh.ts', 'src/cloud.ts', 'src/world.ts', 'src/capture.ts', 'src/audio.ts', 'src/react.ts'];
// entry: [import line, modules it must NOT include]
const ENTRIES = {
  animate: ["import { animate } from './src/index'; animate('.a', { x: 1 });", HEAVY],
  timeline: ["import { timeline } from './src/index'; timeline().add('.a', { x: 1 });", HEAVY],
  recipe: ["import { recipe } from './src/index'; recipe('hero-reveal', '.h');", HEAVY],
  spring: ["import { spring } from './src/index'; spring('.a', { x: 1 });", HEAVY],
  scrollTrigger: ["import { scrollTrigger } from './src/index'; scrollTrigger({ trigger: '.a' });", HEAVY],
  'react hooks': ["import { useAnimate } from './src/react'; console.log(useAnimate);", HEAVY.filter((m) => m !== 'src/react.ts')],
};
let bad = 0;
for (const [name, [code, forbidden]] of Object.entries(ENTRIES)) {
  const r = await build({
    stdin: { contents: code, resolveDir: root, loader: 'ts' }, bundle: true, write: false, minify: true, outfile: 'out.js', format: 'esm',
    metafile: true, external: ['react'], logLevel: 'silent',
    // The embedded wasm is shared by every entry; measure the JS around it.
    plugins: [{ name: 'no-wasm', setup(b) { b.onResolve({ filter: /generated\/wasm-bytes$/ }, () => ({ path: 'w', namespace: 'w' })); b.onLoad({ filter: /.*/, namespace: 'w' }, () => ({ contents: "export const WASM_BASE64 = '';", loader: 'ts' })); } }],
  });
  const out = Object.values(r.metafile.outputs)[0].inputs;
  // Modules that contribute code after tree-shaking (parsed-but-dropped ones have 0 bytes).
  const mods = Object.keys(out).filter((m) => m.startsWith('src/') && out[m].bytesInOutput > 0);
  const leaked = forbidden.filter((m) => mods.includes(m));
  const kb = (r.outputFiles[0].contents.length / 1024).toFixed(1);
  console.log(`${leaked.length ? '✗' : '✓'} ${name.padEnd(14)} ${kb.padStart(6)} KB min (without wasm), ${mods.length} modules${leaked.length ? ' — pulls in ' + leaked.join(', ') : ''}`);
  if (process.argv.includes('--verbose')) console.log('   ' + mods.join(' '));
  bad += leaked.length;
}
if (bad) process.exit(1);
