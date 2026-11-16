#!/usr/bin/env node
// anikit — project scaffolding. `npx anikit new <template> [dir]` writes a
// working starter that already uses recipes, scroll and springs.
import { mkdirSync, writeFileSync, existsSync, readdirSync, copyFileSync, realpathSync } from 'node:fs';
import { join, resolve, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = resolve(here, '..');
const TEMPLATES = ['vanilla', 'vite', 'react'];

const HELP = `anikit — scaffold an animated page

Usage
  anikit new <template> [dir]    write a starter (templates: ${TEMPLATES.join(', ')})
  anikit templates               list the templates
  anikit help                    this message

Templates
  vanilla   one index.html + anikit.js, no build step: open it (or serve the folder)
  vite      Vite project: npm install && npm run dev
  react     Vite + React project using hooks from anikit: npm install && npm run dev

Other commands: anikit-render (video in Node), anikit-record (record a page).
Docs for agents: node_modules/anikit/AGENTS.md and llms-full.txt.`;

const CSS = `* { box-sizing: border-box; margin: 0; }
body { font: 18px/1.5 system-ui, sans-serif; background: #0b0b10; color: #f3f3f7; }
.hero { min-height: 100vh; display: grid; place-content: center; text-align: center; gap: 1.25rem; padding: 2rem; }
.hero h1 { font-size: clamp(2.5rem, 8vw, 6rem); line-height: 1; letter-spacing: -0.03em; }
.hero p { opacity: .7; max-width: 36ch; margin: 0 auto; }
.cta { display: inline-block; padding: .9rem 1.6rem; border-radius: 999px; background: #7c5cff; color: #fff; text-decoration: none; font-weight: 600; }
main section { min-height: 70vh; display: grid; place-content: center; padding: 4rem 2rem; text-align: center; }
main h2 { font-size: clamp(2rem, 5vw, 3.5rem); margin-bottom: .5rem; }
.stats { display: flex; gap: 3rem; justify-content: center; font-size: 3rem; font-weight: 700; }
.gallery { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; padding: 4rem 2rem; }
.gallery figure { aspect-ratio: 3 / 4; border-radius: 1rem; background: linear-gradient(135deg, #7c5cff, #19d3c5); }
.gallery figure:nth-child(2n) { background: linear-gradient(135deg, #ff5c8a, #ffb35c); }
`;

const BODY = `  <header class="hero">
    <h1>Make it move</h1>
    <p>A starter built with anikit recipes: hero reveal, scroll story, magnetic buttons, parallax.</p>
    <div><a class="cta" href="#story">Scroll down</a></div>
  </header>
  <main id="story">
    <section><h2 class="scramble">DECODED</h2><p>Text scrambles in as it enters.</p></section>
    <section><div class="stats"><b class="count">1200</b><b class="count">98</b><b class="count">42</b></div></section>
    <div class="gallery"><figure></figure><figure></figure><figure></figure><figure></figure><figure></figure><figure></figure></div>
    <section><h2>Your turn</h2><p>Edit main.js. Every recipe returns a handle with revert().</p></section>
  </main>`;

const MAIN = (from) => `import { recipe } from '${from}';

recipe('hero-reveal', '.hero');
recipe('magnetic-buttons', '.hero');
recipe('scroll-story', 'main', { children: 'section' });
recipe('text-scramble', '.scramble', { onScroll: true });
recipe('count-up', '.count');
recipe('parallax-gallery', '.gallery', { children: 'figure' });
`;

const html = (script) => `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>anikit starter</title>
  <link rel="stylesheet" href="./style.css">
</head>
<body>
${BODY}
  <script type="module" src="${script}"></script>
</body>
</html>
`;

const REACT_APP = `import { useRef } from 'react';
import { useRecipe, useAnimate } from 'anikit/react';

export default function App() {
  const hero = useRef(null);
  const story = useRef(null);
  useRecipe('hero-reveal', hero);
  useRecipe('magnetic-buttons', hero);
  useRecipe('scroll-story', story, { children: 'section' });
  const [box, animate] = useAnimate();
  return (
    <>
      <header className="hero" ref={hero}>
        <h1>Make it move</h1>
        <p>anikit + React: recipes and hooks, cleaned up on unmount.</p>
        <div>
          <button className="cta" onClick={() => animate(box.current, { rotate: [0, 360], scale: [1, 1.2, 1] }, { duration: 0.8, ease: 'spring(bounce: 0.4)' })}>
            Spin the box
          </button>
        </div>
        <div ref={box} style={{ width: 64, height: 64, margin: '1rem auto', borderRadius: 16, background: '#19d3c5' }} />
      </header>
      <main ref={story}>
        <section><h2>Scroll story</h2><p>Each section rises in as it enters.</p></section>
        <section><h2>Springs everywhere</h2><p>Try useSpring and useScroll from anikit/react.</p></section>
      </main>
    </>
  );
}
`;

function vitePkg(name, react) {
  const deps = { anikit: '^0.1.0' };
  const dev = { vite: '^7.0.0' };
  if (react) {
    Object.assign(deps, { react: '^19.0.0', 'react-dom': '^19.0.0' });
    dev['@vitejs/plugin-react'] = '^5.0.0';
  }
  return JSON.stringify({ name, private: true, type: 'module', scripts: { dev: 'vite', build: 'vite build', preview: 'vite preview' }, dependencies: deps, devDependencies: dev }, null, 2) + '\n';
}

/** Files of a template, as { relativePath: contents | { copy: absPath } }. */
export function templateFiles(template, name = 'anikit-app') {
  switch (template) {
    case 'vanilla': {
      const dist = join(pkg, 'dist', 'anikit.js');
      return {
        'index.html': html('./main.js'),
        'style.css': CSS,
        'main.js': MAIN('./anikit.js'),
        'anikit.js': existsSync(dist) ? { copy: dist } : `export * from 'https://cdn.jsdelivr.net/npm/anikit/dist/anikit.js';\n`,
        'README.md': `# ${name}\n\nNo build step. Serve the folder (\`npx serve .\`) or open index.html, then edit main.js.\n`,
      };
    }
    case 'vite':
      return {
        'package.json': vitePkg(name, false),
        'index.html': html('/src/main.js'),
        'style.css': CSS,
        'src/main.js': `import '../style.css';\n` + MAIN('anikit'),
        'README.md': `# ${name}\n\n\`\`\`sh\nnpm install\nnpm run dev\n\`\`\`\n`,
      };
    case 'react':
      return {
        'package.json': vitePkg(name, true),
        'vite.config.js': `import { defineConfig } from 'vite';\nimport react from '@vitejs/plugin-react';\n\nexport default defineConfig({ plugins: [react()] });\n`,
        'index.html': `<!doctype html>\n<html lang="en">\n<head>\n  <meta charset="utf-8">\n  <meta name="viewport" content="width=device-width, initial-scale=1">\n  <title>anikit + React</title>\n</head>\n<body>\n  <div id="root"></div>\n  <script type="module" src="/src/main.jsx"></script>\n</body>\n</html>\n`,
        'style.css': CSS,
        'src/main.jsx': `import { createRoot } from 'react-dom/client';\nimport App from './App.jsx';\nimport '../style.css';\n\ncreateRoot(document.getElementById('root')).render(<App />);\n`,
        'src/App.jsx': REACT_APP,
        'README.md': `# ${name}\n\n\`\`\`sh\nnpm install\nnpm run dev\n\`\`\`\n\nHooks: useAnimate, useRecipe, useSpring, useScroll, useAnikit and <Presence> from \`anikit/react\`.\n`,
      };
    default:
      return null;
  }
}

/** Write `template` into `dir`. Returns the written paths. Throws on a bad template or a non-empty dir. */
export function scaffold(template, dir) {
  const target = resolve(dir);
  const files = templateFiles(template, basename(target).toLowerCase().replace(/[^a-z0-9-]/g, '-') || 'anikit-app');
  if (!files) throw new Error(`unknown template "${template}". Use one of: ${TEMPLATES.join(', ')}.`);
  if (existsSync(target) && readdirSync(target).length) throw new Error(`${target} is not empty; pick a new directory.`);
  const written = [];
  for (const [rel, body] of Object.entries(files)) {
    const out = join(target, rel);
    mkdirSync(dirname(out), { recursive: true });
    if (typeof body === 'string') writeFileSync(out, body);
    else copyFileSync(body.copy, out);
    written.push(rel);
  }
  return written;
}

function main(argv) {
  const [cmd, a, b] = argv;
  if (!cmd || cmd === 'help' || cmd === '--help' || cmd === '-h') return console.log(HELP);
  if (cmd === 'templates') return console.log(TEMPLATES.join('\n'));
  if (cmd !== 'new' && cmd !== 'create' && cmd !== 'init') {
    console.error(`anikit: unknown command "${cmd}".\n\n${HELP}`);
    process.exit(1);
  }
  if (a && b && !TEMPLATES.includes(a)) {
    console.error(`anikit: unknown template "${a}". Use one of: ${TEMPLATES.join(', ')}.`);
    process.exit(1);
  }
  const template = a && TEMPLATES.includes(a) ? a : 'vanilla';
  const dir = a && TEMPLATES.includes(a) ? b ?? `anikit-${template}` : a ?? 'anikit-vanilla';
  try {
    const files = scaffold(template, dir);
    console.log(`anikit: wrote ${template} starter to ${dir}/ (${files.join(', ')})`);
    console.log(template === 'vanilla' ? `  cd ${dir} && npx serve .` : `  cd ${dir} && npm install && npm run dev`);
  } catch (e) {
    console.error('anikit: ' + e.message);
    process.exit(1);
  }
}

if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url))) main(process.argv.slice(2));
