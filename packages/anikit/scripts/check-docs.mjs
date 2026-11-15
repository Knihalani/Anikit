// Keep the docs honest:
//  1. every export of src/index.ts is mentioned (in backticks) in docs/api.md;
//  2. every ```ts block of docs/recipes.md and AGENTS.md type-checks against
//     the real API (together with the tests), via tsconfig.test.json.
// Run: node scripts/check-docs.mjs   (also part of `npm run typecheck`)
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { pkg, root, writeSnippets } from './snippets.mjs';

export function exportNames() {
  const src = readFileSync(join(pkg, 'src/index.ts'), 'utf8');
  const names = [];
  for (const m of src.matchAll(/export\s+(?:type\s+)?\{([^}]+)\}/g)) {
    for (const part of m[1].split(',')) {
      const name = part.trim().split(/\s+as\s+/).pop();
      if (name) names.push(name);
    }
  }
  return names;
}

export function undocumentedExports() {
  const api = readFileSync(join(root, 'docs/api.md'), 'utf8');
  const mentioned = new Set();
  for (const m of api.matchAll(/`([^`\n]+)`/g)) for (const id of m[1].matchAll(/[A-Za-z_$][\w$]*/g)) mentioned.add(id[0]);
  return exportNames().filter((n) => !mentioned.has(n));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  let ok = true;
  const missing = undocumentedExports();
  if (missing.length) {
    ok = false;
    console.error(`docs/api.md does not mention these exports: ${missing.join(', ')}`);
  }
  const files = writeSnippets();
  try {
    execFileSync('npx', ['tsc', '-p', 'tsconfig.test.json'], { cwd: pkg, stdio: 'inherit' });
    console.log(`check-docs: ${exportNames().length} exports documented, ${files.length} snippets + tests type-check`);
  } catch {
    ok = false;
    console.error('check-docs: type errors above (snippet files are named <doc>-<line>.ts)');
  }
  process.exit(ok ? 0 : 1);
}
