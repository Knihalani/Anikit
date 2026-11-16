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
// ESM: `anikit` and the separately importable `anikit/splats` share chunks
// (one core, one engine); importing `animate` never pulls in the splat renderer.
await build({
  ...common, entryPoints: { anikit: 'src/index.ts', splats: 'src/splats.ts' }, format: 'esm', splitting: true,
  outdir: 'dist', chunkNames: 'chunks/[name]-[hash]',
});
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
execFileSync('npx', ['tsc', '--emitDeclarationOnly', '--declaration', '--outDir', 'dist/types'], { stdio: 'inherit' });

const kb = (n) => (n / 1024).toFixed(1) + ' KB';
for (const f of ['dist/anikit.min.js', 'dist/anikit.lean.js', 'dist/anikit.wasm']) {
  const raw = readFileSync(f);
  console.log(`${f}: ${kb(statSync(f).size)} (gzip ${kb(gzipSync(raw, { level: 9 }).length)})`);
}

// `anikit/scene`: the real-time GPU 3D layer as its own entry. It re-exports
// from the main bundle so both share one engine (no duplicate rAF loop).
{
  const { writeFileSync } = await import('node:fs');
  const names = [];
  for (const f of ['src/gl.ts', 'src/geometry.ts']) {
    for (const m of readFileSync(f, 'utf8').matchAll(/^export (?:function|class|const) ([A-Za-z_$][\w$]*)/gm)) names.push(m[1]);
  }
  writeFileSync('dist/scene.js', `// Real-time WebGL2 3D (see docs/api.md, "Real-time GPU 3D").\nexport { ${names.join(', ')} } from './anikit.js';\n`);
  writeFileSync('dist/scene.d.ts', "export * from './types/gl';\nexport * from './types/geometry';\n");
}
