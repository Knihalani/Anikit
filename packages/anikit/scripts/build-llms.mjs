// Generate llms.txt (index) and llms-full.txt (playbook + recipes + complete
// reference in one file) for coding agents. Shipped in the npm package and
// written at the repo root too (with links that resolve from there).
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(pkg, '..', '..');
const api = readFileSync(join(root, 'docs/api.md'), 'utf8');
const recipes = readFileSync(join(root, 'docs/recipes.md'), 'utf8');
const agents = readFileSync(join(pkg, 'AGENTS.md'), 'utf8');
const headings = (md) => [...md.matchAll(/^## (.+)$/gm)].map((m) => m[1]);

const index = (links) => `# Anikit

> High-performance web animation engine: Rust/WebAssembly core with a TypeScript API (and Python bindings). Tweens, keyframes, timelines, springs, scroll triggers, SVG morphing, text effects, an effects library, physics worlds, 3D, particles, GIF/video capture and Node video rendering.

Import from 'anikit'. Times are in seconds (only \`engine.onFrame\`/\`engine.tick\` use ms). Mistakes print \`anikit:\` warnings that name the fix; \`testing\` + \`engine.inspect()\` verify animations headlessly.

## Docs
- [Agent playbook](${links.agents}): "want X → use Y" table, rules, snippets, verification, common mistakes and fixes
- [Recipes](${links.recipes}): task-sized, tested examples (${headings(recipes).filter((h) => h !== 'Python').length} tasks + Python)
- [Full reference](${links.full}): playbook + recipes + every API in one file

## Recipes
${headings(recipes).map((s) => `- ${s}`).join('\n')}

## Reference sections
${headings(api).map((s) => `- ${s}`).join('\n')}
`;

const full = `# Anikit — full reference for LLMs

Contents: 1. Agent playbook · 2. Recipes · 3. API reference

---

${agents}

---

${recipes}

---

${api}`;

const targets = [
  { dir: pkg, links: { agents: 'AGENTS.md', recipes: 'llms-full.txt', full: 'llms-full.txt' } },
  { dir: root, links: { agents: 'packages/anikit/AGENTS.md', recipes: 'docs/recipes.md', full: 'llms-full.txt' } },
];
for (const { dir, links } of targets) {
  writeFileSync(join(dir, 'llms.txt'), index(links));
  writeFileSync(join(dir, 'llms-full.txt'), full);
}
console.log(`llms.txt (${index(targets[0].links).length} chars), llms-full.txt (${full.length} chars)`);
