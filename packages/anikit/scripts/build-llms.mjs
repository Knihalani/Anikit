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
const videoAgents = readFileSync(join(root, 'docs/video/AGENTS.md'), 'utf8');
const videoSpec = readFileSync(join(root, 'docs/video/spec.md'), 'utf8');
const headings = (md) => [...md.matchAll(/^## (.+)$/gm)].map((m) => m[1]);

const index = (links) => `# Anikit

> High-performance web animation engine: Rust/WebAssembly core with a TypeScript API (and Python bindings). Tweens, keyframes, timelines, springs, reactive motion values, scroll triggers, marquees, SVG morphing, text effects, an effects library, physics worlds, 3D, particles, a software 3D renderer with a film grade, GIF/video capture and Node video rendering.

Import from 'anikit'. Times are in seconds (only \`engine.onFrame\`/\`engine.tick\` use ms). Mistakes print \`anikit:\` warnings that name the fix; \`testing\` + \`engine.inspect()\` verify animations headlessly.

## Docs
- [Agent playbook](${links.agents}): "want X → use Y" table, rules, snippets, verification, common mistakes and fixes
- [Recipes](${links.recipes}): task-sized, tested examples (${headings(recipes).filter((h) => h !== 'Python').length} tasks + Python)
- [Full reference](${links.full}): playbook + recipes + every API in one file
- [Options schema](${links.schema}): JSON Schema of every options object, with descriptions
${links.parity ? `- [Feature parity](${links.parity}): anikit vs GSAP, Motion, anime.js and Lottie\n` : ''}
## Quick start
\`\`\`ts
import { recipe } from 'anikit';
recipe('hero-reveal', '.hero');
recipe('magnetic-buttons', '.hero');
recipe('scroll-story', 'main', { children: 'section' });
recipe('parallax-gallery', '.gallery');
\`\`\`
Scaffold: \`npx anikit new vanilla|vite|react <dir>\`. React hooks: \`anikit/react\` (useAnikit, useAnimate, useRecipe, useSpring, useScroll, Presence).

## Video (anikit-video)
Native programmatic video for agents: a JSON spec (or a template plus a brand kit) in, an MP4 out, no browser. Builders: \`anikit/video\` (TS), \`anikit.video\` (Python); MCP server: \`anikit-video mcp\`.
${links.video ? `- [Video quick start for agents](${links.video.agents}): templates, the review loop, builders, MCP tools
- [Video spec reference](${links.video.spec}): scenes, layers, animation presets, transitions, theme
- [Prompt library](${links.video.prompts}): launch video generator, script writer, storyboarder, critic, copy guard, brand kit, motion rules
` : '- Full guide and spec: section 4 of llms-full.txt\n'}
## Recipes
${headings(recipes).map((s) => `- ${s}`).join('\n')}

## Reference sections
${headings(api).map((s) => `- ${s}`).join('\n')}
`;

const full = `# Anikit — full reference for LLMs

Contents: 1. Agent playbook · 2. Recipes · 3. API reference · 4. Video (anikit-video): quick start and spec

---

${agents}

---

${recipes}

---

${api}

---

${videoAgents}

---

${videoSpec}`;

const targets = [
  { dir: pkg, links: { agents: 'AGENTS.md', recipes: 'llms-full.txt', full: 'llms-full.txt', schema: 'api.schema.json' } },
  { dir: root, links: { agents: 'packages/anikit/AGENTS.md', recipes: 'docs/recipes.md', full: 'llms-full.txt', schema: 'docs/api.schema.json', parity: 'docs/parity.md', video: { agents: 'docs/video/AGENTS.md', spec: 'docs/video/spec.md', prompts: 'prompts/' } } },
];
for (const { dir, links } of targets) {
  writeFileSync(join(dir, 'llms.txt'), index(links));
  writeFileSync(join(dir, 'llms-full.txt'), full);
}
console.log(`llms.txt (${index(targets[0].links).length} chars), llms-full.txt (${full.length} chars)`);
