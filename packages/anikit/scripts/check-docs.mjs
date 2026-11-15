// Keep the docs honest:
//  1. every value export of src/index.ts is used in a type-checked example
//     (docs/api.md, docs/recipes.md or AGENTS.md), and every type export is
//     mentioned in docs/api.md;
//  2. those examples type-check against the real API, together with the
//     tests (tsconfig.test.json). api.md blocks are fragments: they get a
//     generated prelude (imports + placeholder declarations; see snippets.mjs).
// Run: node scripts/check-docs.mjs   (also part of `npm run typecheck`)
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { pkg, root, writeSnippets, checkedSnippets, exportNames, API_DOC } from './snippets.mjs';

/** Exports without an example (values) or a mention in api.md (types). */
export function undocumentedExports() {
  const api = readFileSync(API_DOC, 'utf8');
  const mentioned = new Set();
  for (const m of api.matchAll(/`([^`\n]+)`/g)) for (const id of m[1].matchAll(/[A-Za-z_$][\w$]*/g)) mentioned.add(id[0]);
  // Protected sections (3D, particles, video) are documented elsewhere; their
  // exports only need a mention.
  const used = new Set();
  for (const s of checkedSnippets()) {
    // Count real uses only: not in comments, imports, strings, `key:` or `.member` positions.
    const code = s.code
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/\/\/.*$/gm, '')
      .replace(/^\s*import[\s\S]*?from\s*['"][^'"]+['"];?/gm, '')
      .replace(/'(?:[^'\\\n]|\\.)*'|"(?:[^"\\\n]|\\.)*"/g, "''");
    for (const m of code.matchAll(/(?<![.\w$])([A-Za-z_$][\w$]*)(?!\s*:(?!:))/g)) used.add(m[1]);
  }
  const external = new Set([
    'scene3d', 'Scene3D', 'tilt', 'parseTransform', 'toMatrix3d', 'mixTransforms', 'pointCloud', 'PointCloud', 'textPoints', 'glow', 'Glow',
    'emitter', 'Emitter', 'mesh', 'Mesh3D', 'instances', 'Instances', 'renderer3d', 'Renderer3D', 'composition', 'Composition',
    'FrameBuffer', 'typeFx', 'exportWebM', 'muxWebM',
  ]);
  const values = exportNames('value').filter((n) => !used.has(n) && !(external.has(n) && mentioned.has(n)));
  const types = exportNames('type').filter((n) => !mentioned.has(n));
  return [...values.map((n) => `${n} (no type-checked example)`), ...types.map((n) => `${n} (type not mentioned)`)];
}

if (import.meta.url === `file://${process.argv[1]}`) {
  let ok = true;
  const missing = undocumentedExports();
  if (missing.length) {
    ok = false;
    console.error(`docs: ${missing.length} exports need docs:\n  ${missing.join('\n  ')}`);
  }
  const files = writeSnippets();
  try {
    execFileSync('npx', ['tsc', '-p', 'tsconfig.test.json'], { cwd: pkg, stdio: 'inherit' });
    console.log(`check-docs: ${exportNames().length} exports documented, ${files.length} doc examples + tests type-check`);
  } catch {
    ok = false;
    console.error('check-docs: type errors above (test/.snippets/<doc>-<line>.ts; line N of the file is line <line>+N-1 of the doc)');
  }
  void root;
  process.exit(ok ? 0 : 1);
}
