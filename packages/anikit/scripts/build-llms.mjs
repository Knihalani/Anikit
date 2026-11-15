// Generate llms.txt (index) and llms-full.txt (complete reference) for
// coding agents. Shipped in the npm package and at the repo root.
import { readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(pkg, '..', '..');
const api = readFileSync(join(root, 'docs/api.md'), 'utf8');
const agents = readFileSync(join(pkg, 'AGENTS.md'), 'utf8');
const sections = [...api.matchAll(/^## (.+)$/gm)].map((m) => m[1]);
const index = `# Anikit

> High-performance web animation engine: Rust/WebAssembly core with a TypeScript API (and Python bindings). Tweens, keyframes, timelines, springs, physics worlds, 3D, scroll triggers, SVG morphing, text effects, effects library.

Times are in seconds. Import from 'anikit'. Use \`testing\` + \`engine.inspect()\` to verify headlessly.

## Docs
- [Agent playbook](AGENTS.md): rules of thumb, recipes, verification, common mistakes
- [Full reference](llms-full.txt): every API in one file

## Reference sections
${sections.map((s) => `- ${s}`).join('\n')}
`;
const full = `# Anikit — full reference for LLMs\n\n${agents}\n\n---\n\n${api}`;
for (const dir of [pkg, root]) {
  writeFileSync(join(dir, 'llms.txt'), index);
  writeFileSync(join(dir, 'llms-full.txt'), full);
}
console.log(`llms.txt (${index.length} chars), llms-full.txt (${full.length} chars)`);
